//! T03 real-process HTTP and browser acceptance. Synthetic inputs never contact providers.
use crate::{Result, t08a_child};
use process_wrap::tokio::{ChildWrapper, CommandWrap, KillOnDrop};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    net::SocketAddr,
    path::{Component, Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

const CLIENT: &str = "crates/ssx-http/tests/t03-browser/run.mjs";
const SCENARIOS: [&str; 5] = [
    "contract",
    "cached-error",
    "rate-limited",
    "html-disabled",
    "unavailable",
];
const ENGINES: [&str; 3] = ["chromium", "firefox", "webkit"];

/// Engines that report blocked-document and subresource HTTP statuses on the
/// console. Measured, not assumed: Chromium and Playwright WebKit both report
/// them, Gecko does not.
fn reports_console_status(engine: &str) -> bool {
    matches!(engine, "chromium" | "webkit")
}
const HTTP_TESTS: [&str; 11] = [
    "static_assets_and_highlight_boundary_preserve_safe_progressive_rendering",
    "inherited_scope_and_date_journeys_cover_all_16_literal_assertions",
    "browser_enforcement_uses_capture_without_changing_machine_contract",
    "malicious_optional_empty_partial_and_unavailable_states_are_truthful",
    "unavailable_machine_format_keeps_the_legacy_unknown_scope_summary",
    "invalid_and_sensitive_submissions_keep_only_escaped_public_fields_without_dispatch",
    "cached_rate_limited_and_disabled_format_errors_retain_query_and_join",
    "exact_page_limit_keeps_current_previous_and_disables_only_unadmitted_next",
    "public_routes_ignore_opaque_cookie_bytes",
    "blank_optional_html_bounds_are_absent_and_machine_blanks_stay_rejected",
    "landing_native_form_submits_query_and_scope_together_from_both_buttons",
];
const SOURCES: [(&str, &[u8]); 13] = [
    ("crates/xtask/src/t03.rs", include_bytes!("t03.rs")),
    (
        "crates/ssx-http/src/portal.rs",
        include_bytes!("../../ssx-http/src/portal.rs"),
    ),
    (
        "crates/ssx-http/src/portal/style.css",
        include_bytes!("../../ssx-http/src/portal/style.css"),
    ),
    (
        "crates/ssx-http/src/portal/behavior.js",
        include_bytes!("../../ssx-http/src/portal/behavior.js"),
    ),
    (
        "crates/ssx-http/examples/t03_fixture.rs",
        include_bytes!("../../ssx-http/examples/t03_fixture.rs"),
    ),
    (
        "crates/ssx-http/tests/t03.rs",
        include_bytes!("../../ssx-http/tests/t03.rs"),
    ),
    (
        "crates/ssx-http/tests/support/t03_peer.rs",
        include_bytes!("../../ssx-http/tests/support/t03_peer.rs"),
    ),
    (
        "crates/ssx-http/tests/portal_keyboard.test.mjs",
        include_bytes!("../../ssx-http/tests/portal_keyboard.test.mjs"),
    ),
    (
        CLIENT,
        include_bytes!("../../ssx-http/tests/t03-browser/run.mjs"),
    ),
    (
        "crates/ssx-http/tests/t03-browser/package.json",
        include_bytes!("../../ssx-http/tests/t03-browser/package.json"),
    ),
    (
        "crates/ssx-http/tests/t03-browser/package-lock.json",
        include_bytes!("../../ssx-http/tests/t03-browser/package-lock.json"),
    ),
    (
        "crates/ssx-http/tests/t03-browser/interaction.mjs",
        include_bytes!("../../ssx-http/tests/t03-browser/interaction.mjs"),
    ),
    (
        "docs/rust-rewrite/t02/original-bindings.json",
        include_bytes!("../../../docs/rust-rewrite/t02/original-bindings.json"),
    ),
];
fn need(ok: bool, message: &str) -> Result<()> {
    if ok {
        Ok(())
    } else {
        Err(format!("T03 {message}"))
    }
}
fn hash(raw: &[u8]) -> String {
    Sha256::digest(raw)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
fn regular(path: &Path, limit: u64) -> Result<Vec<u8>> {
    let metadata = fs::symlink_metadata(path).map_err(|e| format!("{}: {e}", path.display()))?;
    need(
        metadata.is_file() && !metadata.file_type().is_symlink() && metadata.len() <= limit,
        "regular bounded artifact required",
    )?;
    fs::read(path).map_err(|e| format!("{}: {e}", path.display()))
}
fn sources(root: &Path) -> Result<Value> {
    let mut rows = Vec::new();
    for (path, embedded) in SOURCES {
        let physical = regular(&root.join(path), 2 * 1024 * 1024)?;
        need(
            physical == embedded,
            &format!("stale compiled source input {path}"),
        )?;
        rows.push(json!({"path":path,"bytes":physical.len(),"sha256":hash(&physical)}));
    }
    Ok(Value::Array(rows))
}
fn owned_file(base: &Path, path: &str, limit: u64) -> Result<(PathBuf, Vec<u8>)> {
    let candidate = Path::new(path);
    need(candidate.is_absolute(), "artifact path must be absolute")?;
    need(
        !candidate
            .components()
            .any(|part| matches!(part, Component::ParentDir)),
        "artifact path traverses parent",
    )?;
    let relative = candidate
        .strip_prefix(base)
        .map_err(|_| "T03 artifact path is not lexically owned")?;
    let mut checked = base.to_path_buf();
    for component in relative.components() {
        checked.push(component);
        need(
            !fs::symlink_metadata(&checked)
                .map_err(|e| e.to_string())?
                .file_type()
                .is_symlink(),
            "artifact path contains a symlink",
        )?;
    }
    let resolved = candidate.canonicalize().map_err(|e| e.to_string())?;
    need(
        resolved.starts_with(base.canonicalize().map_err(|e| e.to_string())?),
        "artifact escaped owned evidence directory",
    )?;
    let raw = regular(candidate, limit)?;
    Ok((resolved, raw))
}
fn read_completion(
    base: &Path,
    row: &Value,
    path_key: &str,
    hash_key: &str,
) -> Result<(PathBuf, Value)> {
    let (path, raw) = owned_file(
        base,
        row[path_key]
            .as_str()
            .ok_or("T03 completion path missing")?,
        8 * 1024 * 1024,
    )?;
    need(
        row[hash_key] == hash(&raw),
        "artifact digest differs from completion",
    )?;
    let value = serde_json::from_slice(&raw).map_err(|e| format!("T03 receipt JSON: {e}"))?;
    Ok((path, value))
}
fn completion_lines(stdout: &str, marker: &str) -> Result<Vec<Value>> {
    stdout
        .lines()
        .filter_map(|line| line.split_once(marker).map(|(_, payload)| payload))
        .map(|payload| {
            serde_json::from_str(payload).map_err(|e| format!("T03 malformed {marker}: {e}"))
        })
        .collect()
}
fn capture(
    root: &Path,
    mut command: Command,
    budget: Duration,
    records: &mut Vec<Value>,
) -> Result<String> {
    command.current_dir(root);
    let program = command.get_program().to_string_lossy().into_owned();
    let args: Vec<_> = command
        .get_args()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect();
    let output = t08a_child::capture(command, budget, 32 * 1024 * 1024, 8 * 1024 * 1024)?;
    let stdout = String::from_utf8(output.stdout).map_err(|e| e.to_string())?;
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    records.push(json!({"program":program,"args":args,"pid":output.pid,"exit_code":output.status,"stdout":stdout,"stderr":stderr,"error":output.error,"cleanup":output.cleanup}));
    need(
        output.error.is_none() && output.status == Some(0),
        "owned command failed or exceeded deadline",
    )?;
    need(
        output.cleanup["waited"] == true && output.cleanup["tree_retired"] == true,
        "owned command process cleanup incomplete",
    )?;
    Ok(stdout)
}
fn cargo(root: &Path, args: &[&str], budget: Duration, records: &mut Vec<Value>) -> Result<String> {
    let mut command = Command::new("cargo");
    command.args(args);
    capture(root, command, budget, records)
}
fn required_test_inventory(stdout: &str) -> Result<()> {
    let actual: Vec<_> = stdout
        .lines()
        .filter_map(|line| line.strip_suffix(": test"))
        .collect();
    let set: BTreeSet<_> = actual.iter().copied().collect();
    need(
        actual.len() == HTTP_TESTS.len() && set == HTTP_TESTS.into_iter().collect(),
        "HTTP test inventory missing, duplicated or unexpected",
    )
}
fn completed_tests(stdout: &str) -> Result<()> {
    for name in HTTP_TESTS {
        need(
            stdout
                .lines()
                .filter(|line| line.starts_with(&format!("test {name} ... ")))
                .count()
                == 1,
            "HTTP test did not execute exactly once",
        )?;
    }
    need(
        stdout.contains(
            "test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out;",
        ),
        "HTTP suite failed, skipped or filtered",
    )
}

#[derive(Clone)]
struct HttpExpected {
    group: &'static str,
    id: String,
    status: u16,
    calls: usize,
}
fn expected_http() -> Vec<HttpExpected> {
    let mut rows = Vec::new();
    let mut add = |group, id: &str, status, calls| {
        rows.push(HttpExpected {
            group,
            id: id.into(),
            status,
            calls,
        })
    };
    for (group, id, status, calls) in [
        ("inherited", "inherited-scope", 200, 2),
        ("inherited", "inherited-date", 200, 1),
        ("metadata", "metadata-html", 200, 1),
        ("metadata", "metadata-json", 200, 1),
        ("metadata", "metadata-yaml", 200, 1),
        ("unavailable-format", "unavailable-html", 503, 0),
        ("unavailable-format", "unavailable-yaml", 503, 0),
        ("denials", "deny-strict", 400, 0),
        ("denials", "deny-date", 400, 0),
        ("denials", "deny-restricted", 403, 0),
        ("denials", "deny-page", 400, 0),
        ("cached-error", "error-recovery", 503, 0),
        ("rate-limited", "error-recovery", 429, 0),
        ("html-disabled", "error-recovery", 403, 0),
        ("page-limit", "page-limit", 200, 2),
        ("cookies", "cookie-landing", 200, 0),
        ("landing-form", "landing-form-source", 200, 0),
        ("landing-form", "landing-submit-0", 200, 1),
        ("landing-form", "landing-submit-1", 200, 1),
    ] {
        add(group, id, status, calls);
    }
    for query in [
        "malicious",
        "missing",
        "empty",
        "partial",
        "all-failed",
        "long",
        "groups",
    ] {
        add(
            "states",
            &format!("state-{query}"),
            if query == "all-failed" { 503 } else { 200 },
            2,
        );
    }
    for path in ["root", "search"] {
        for method in ["GET", "POST"] {
            for format in ["html", "json", "yaml", "csv", "rss"] {
                for index in 0..3 {
                    add(
                        "cookies",
                        &format!("cookie-{path}-{method}-{format}-{index}"),
                        200,
                        2,
                    );
                }
            }
        }
    }
    for method in ["GET", "POST"] {
        for format in ["html", "json", "yaml"] {
            for case in ["both-empty", "from-only", "to-only"] {
                add(
                    "blank-bounds",
                    &format!("blank-{method}-{format}-{case}"),
                    if format == "html" { 200 } else { 400 },
                    if format != "html" {
                        0
                    } else if case == "both-empty" {
                        2
                    } else {
                        1
                    },
                );
            }
        }
    }
    rows
}
fn group_for(path: &Path, groups: &BTreeSet<&str>) -> Result<String> {
    let name = path
        .parent()
        .and_then(Path::file_name)
        .and_then(|name| name.to_str())
        .ok_or("T03 receipt parent missing")?;
    let selected: Vec<_> = groups
        .iter()
        .filter(|group| name.starts_with(&format!("t03-{group}-")))
        .collect();
    need(selected.len() == 1, "unexpected HTTP process directory")?;
    Ok((*selected[0]).to_owned())
}
fn validate_http(base: &Path, stdout: &str, binary: &Path, binary_sha: &str) -> Result<Value> {
    let expected = expected_http();
    need(
        expected.len() == 104,
        "compiled HTTP receipt inventory is incomplete",
    )?;
    let groups: BTreeSet<_> = expected.iter().map(|row| row.group).collect();
    need(
        groups.len() == 12,
        "compiled HTTP process inventory is incomplete",
    )?;
    let wanted: BTreeMap<_, _> = expected
        .iter()
        .map(|row| ((row.group.to_owned(), row.id.clone()), row))
        .collect();
    let actual = completion_lines(stdout, "T03_BINDING ")?;
    need(
        actual.len() == wanted.len(),
        "HTTP completion inventory missing or extra",
    )?;
    let mut seen = BTreeSet::new();
    let mut process_dirs = BTreeMap::new();
    let mut wire = Vec::new();
    let mut total_calls = BTreeMap::<String, usize>::new();
    for row in actual {
        exact_keys(
            &row,
            &["id", "receipt", "sha256", "status", "engine_call_delta"],
            "HTTP completion",
        )?;
        let (path, receipt) = read_completion(base, &row, "receipt", "sha256")?;
        exact_keys(
            &receipt,
            &[
                "id",
                "method",
                "target",
                "status",
                "headers",
                "body",
                "engine_call_delta",
            ],
            "HTTP receipt",
        )?;
        let group = group_for(&path, &groups)?;
        let id = row["id"]
            .as_str()
            .ok_or("T03 HTTP completion ID missing")?
            .to_owned();
        need(
            seen.insert((group.clone(), id.clone())),
            "duplicate HTTP completion",
        )?;
        let wanted = wanted
            .get(&(group.clone(), id.clone()))
            .ok_or("T03 unexpected HTTP completion")?;
        need(
            receipt["id"] == id
                && path.file_name().and_then(|name| name.to_str())
                    == Some(format!("{id}.json").as_str()),
            "HTTP receipt identity differs",
        )?;
        for observed in [&row, &receipt] {
            need(
                observed["status"] == wanted.status
                    && observed["engine_call_delta"] == wanted.calls,
                "HTTP status or dispatch count differs",
            )?;
        }
        need(
            receipt["headers"].is_object()
                && receipt["body"].is_string()
                && matches!(receipt["method"].as_str(), Some("GET" | "POST"))
                && receipt["target"]
                    .as_str()
                    .is_some_and(|target| target.starts_with('/')),
            "whole HTTP observation missing",
        )?;
        *total_calls.entry(group.clone()).or_default() += wanted.calls;
        let directory = path
            .parent()
            .ok_or("T03 HTTP receipt directory")?
            .to_path_buf();
        if let Some(previous) = process_dirs.insert(group.clone(), directory.clone()) {
            need(
                previous == directory,
                "multiple HTTP processes for one named scenario",
            )?;
        }
        wire.push(json!({"group":group,"binding":row,"observation":receipt}));
    }
    need(
        seen == wanted.keys().cloned().collect(),
        "HTTP receipt inventory differs",
    )?;
    let terminals = completion_lines(stdout, "T03_TERMINAL ")?;
    need(
        terminals.len() == groups.len(),
        "HTTP joined process inventory missing or extra",
    )?;
    let mut joined = BTreeSet::new();
    let mut processes = Vec::new();
    for row in terminals {
        exact_keys(&row, &["path", "sha256"], "HTTP terminal completion")?;
        let (path, terminal) = read_completion(base, &row, "path", "sha256")?;
        exact_keys(
            &terminal,
            &["child_joined", "child_exit_success", "engine_calls"],
            "HTTP terminal",
        )?;
        let group = group_for(&path, &groups)?;
        need(
            joined.insert(group.clone()),
            "duplicate HTTP terminal process",
        )?;
        need(
            path.parent() == process_dirs.get(&group).map(PathBuf::as_path),
            "HTTP terminal belongs to another process",
        )?;
        need(
            terminal["child_joined"] == true
                && terminal["child_exit_success"] == true
                && terminal["engine_calls"] == total_calls[&group],
            "HTTP process cleanup or total dispatch differs",
        )?;
        let start_path = path
            .parent()
            .ok_or("T03 terminal parent")?
            .join("process-start.json");
        let start: Value =
            serde_json::from_slice(&regular(&start_path, 65536)?).map_err(|e| e.to_string())?;
        exact_keys(
            &start,
            &["binary", "binary_sha256", "pid", "startup"],
            "HTTP startup",
        )?;
        exact_keys(
            &start["startup"],
            &["address", "mode", "renderer", "scenario"],
            "HTTP fixture startup",
        )?;
        need(
            start["binary"] == binary.to_string_lossy().as_ref()
                && start["binary_sha256"] == binary_sha
                && start["pid"].as_u64().is_some_and(|pid| pid > 0)
                && start["startup"]["mode"] == "offline-t03-fixture"
                && start["startup"]["renderer"] == "PublicPortal",
            "HTTP server binary or startup identity differs",
        )?;
        processes.push(
            json!({"group":group,"startup":start,"terminal":terminal,"terminal_binding":row}),
        );
    }
    need(
        joined == groups.into_iter().map(str::to_owned).collect(),
        "HTTP process inventory differs",
    )?;
    let inherited = process_dirs
        .get("inherited")
        .ok_or("T03 inherited process absent")?;
    let source_nodes: Value =
        serde_json::from_slice(&regular(&inherited.join("source-assertions.json"), 65536)?)
            .map_err(|e| e.to_string())?;
    let frozen: Value =
        serde_json::from_slice(SOURCES[SOURCES.len() - 1].1).map_err(|e| e.to_string())?;
    let nodes: Vec<_> = frozen["bindings"]
        .as_array()
        .ok_or("T03 original bindings missing")?
        .iter()
        .filter(|row| row["owner"] == "T03")
        .cloned()
        .collect();
    need(
        nodes.len() == 2
            && nodes
                .iter()
                .map(|node| node["assertions"].as_array().map_or(0, Vec::len))
                .sum::<usize>()
                == 16
            && source_nodes.as_array() == Some(&nodes),
        "original two-node/16-assertion handoff differs",
    )?;
    let assertions = assertion_evidence(&nodes, &wire)?;
    for (group, directory) in &process_dirs {
        let mut wanted_files: BTreeSet<_> = expected
            .iter()
            .filter(|row| row.group == group.as_str())
            .map(|row| format!("{}.json", row.id))
            .collect();
        for name in [
            "process-start.json",
            "terminal.json",
            "calls.jsonl",
            "server-stderr.log",
        ] {
            wanted_files.insert(name.into());
        }
        if group == "inherited" {
            wanted_files.insert("source-assertions.json".into());
        }
        let mut actual_files = BTreeSet::new();
        for entry in fs::read_dir(directory).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            need(
                entry.file_type().map_err(|e| e.to_string())?.is_file(),
                "HTTP evidence contains nonregular or nested artifact",
            )?;
            actual_files.insert(
                entry
                    .file_name()
                    .into_string()
                    .map_err(|_| "T03 non-UTF8 HTTP artifact name")?,
            );
        }
        need(
            actual_files == wanted_files,
            "HTTP physical artifact inventory differs",
        )?;
        need(
            regular(&directory.join("server-stderr.log"), 1024 * 1024)?.is_empty(),
            "HTTP fixture wrote unexpected stderr",
        )?;
        let calls = String::from_utf8(regular(&directory.join("calls.jsonl"), 8 * 1024 * 1024)?)
            .map_err(|e| e.to_string())?;
        need(
            calls.lines().count() == total_calls[group],
            "HTTP raw engine-call count differs",
        )?;
        for line in calls.lines() {
            validate_engine_call(line, "HTTP fixture reached an unexpected engine")?;
        }
    }
    Ok(
        json!({"http_receipts":wire,"processes":processes,"source_assertions":assertions,"child_joined":true,"child_exit_success":true}),
    )
}
fn assertion_evidence(nodes: &[Value], wire: &[Value]) -> Result<Value> {
    let mut assertions = Vec::new();
    for (index, (id, literals)) in [
        (
            "inherited-scope",
            &[
                "Scope and filters",
                "name=\"categories\"",
                "value=\"general\"",
                "Past month",
                "← Previous",
                "Try next page →",
                "Why this result appeared",
                "Cross-source presence",
                "it is not confidence",
                "unsupported for this result",
                "structurally eligible",
            ][..],
        ),
        (
            "inherited-date",
            &[
                "name=\"date_from\" value=\"2024-02-29\"",
                "date_from=2024-02-29",
                "published from (2024-02-29): enforced",
            ][..],
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let actual = wire
            .iter()
            .find(|row| row["group"] == "inherited" && row["binding"]["id"] == id)
            .ok_or("T03 inherited assertion receipt missing")?;
        let source = nodes[index]["assertions"]
            .as_array()
            .ok_or("T03 source assertions missing")?;
        need(
            source.len() == literals.len() + 1 && actual["observation"]["status"] == 200,
            "inherited status assertion differs",
        )?;
        assertions.push(json!({"source_node":nodes[index]["source_node"],"source":source[0],"receipt":actual["binding"],"comparison":"HTTP status 200"}));
        for (original, literal) in source.iter().skip(1).zip(literals) {
            need(
                actual["observation"]["body"]
                    .as_str()
                    .is_some_and(|body| body.contains(*literal)),
                "inherited literal assertion differs",
            )?;
            assertions.push(json!({"source_node":nodes[index]["source_node"],"source":original,"receipt":actual["binding"],"comparison":{"body_contains":literal}}));
        }
    }
    need(
        assertions.len() == 16,
        "source assertion evidence is incomplete",
    )?;
    Ok(Value::Array(assertions))
}

fn exact_keys(value: &Value, expected: &[&str], label: &str) -> Result<()> {
    let actual = value
        .as_object()
        .ok_or_else(|| format!("T03 {label} must be an object"))?;
    need(
        actual.keys().map(String::as_str).collect::<BTreeSet<_>>()
            == expected.iter().copied().collect(),
        &format!("{label} fields differ"),
    )
}

fn validate_engine_call(line: &str, error: &str) -> Result<()> {
    // Engine-call query parameters are raw request evidence. The T03 page-limit
    // case intentionally includes a 4,300-digit page token, so parse through the
    // lossless JSON profile instead of serde_json's bounded integer representation.
    let call: ssx_json::Value = ssx_json::from_str(line).map_err(|e| e.to_string())?;
    need(
        matches!(call["engine"].as_str(), Some("alpha" | "beta" | "openalex")),
        error,
    )
}

fn journey_ids(scenario: &str) -> Result<&'static [&'static str]> {
    match scenario {
        "contract" => Ok(&[
            "results-normal",
            "optional-metadata",
            "date-controls-roundtrip",
            "empty-suggestions",
            "partial-results",
            "all-source-failure",
            "filter-rejections",
            "malicious-content",
            "page-two-history",
            "dark-darker-storage",
            "progressive-explanations",
            "keyboard-only",
            "viewport-desktop",
            "viewport-narrow",
            "viewport-320css",
            "reduced-motion",
        ]),
        "cached-error" => Ok(&["cached-error"]),
        "rate-limited" => Ok(&["rate-limited"]),
        "html-disabled" => Ok(&["disabled-format"]),
        "unavailable" => Ok(&["unavailable-source"]),
        _ => Err("T03 unknown browser scenario".into()),
    }
}
fn validate_budgets(scenario: &str, engine: &str, budgets: &Value) -> Result<()> {
    if scenario != "contract" || engine != "chromium" {
        // Only the Chromium contract run carries the CDP-throttled performance
        // profile. Other engines run the same journeys without claiming it.
        return exact_keys(budgets, &[], "non-profiled browser budgets");
    }
    exact_keys(
        budgets,
        &[
            "profile",
            "html_uncompressed_bytes",
            "html_gzip_bytes",
            "css_js_uncompressed_bytes",
            "fcp_ms",
            "cls",
            "cls_samples",
            "interaction_p95_ms",
            "interaction_evidence",
            "handoff_p95_ms",
        ],
        "budget",
    )?;
    let profile = budgets["profile"]
        .as_str()
        .ok_or("T03 browser budget profile missing")?;
    need(
        profile.contains("synthetic") && profile.to_ascii_lowercase().contains("fast 3g"),
        "budget profile must name synthetic fixture and Fast 3G",
    )?;
    for (name, limit) in [
        ("html_uncompressed_bytes", 51200.0),
        ("html_gzip_bytes", 15360.0),
        ("css_js_uncompressed_bytes", 30720.0),
        ("fcp_ms", 1500.0),
        ("cls", 0.1),
        ("interaction_p95_ms", 100.0),
        ("handoff_p95_ms", 150.0),
    ] {
        let metric = &budgets[name];
        exact_keys(metric, &["observed", "limit", "pass"], "budget metric")?;
        let observed = metric["observed"]
            .as_f64()
            .ok_or("T03 measured budget value missing")?;
        need(
            observed.is_finite()
                && (0.0..=limit).contains(&observed)
                && metric["limit"].as_f64() == Some(limit)
                && metric["pass"] == true,
            &format!(
                "browser budget failed or changed: {name} observed={observed} limit={limit} declared_limit={}",
                metric["limit"]
            ),
        )?;
    }
    let evidence = &budgets["interaction_evidence"];
    exact_keys(
        evidence,
        &["definition", "samples", "controls"],
        "interaction evidence",
    )?;
    need(
        evidence["definition"] == "trusted-event-timestamp-to-state-and-next-frame-v1",
        "interaction measurement definition",
    )?;
    let samples = evidence["samples"]
        .as_array()
        .ok_or("interaction samples missing")?;
    need(samples.len() == 4, "all four interaction samples required")?;
    let mut maximum = 0.0_f64;
    for (sample, (id, event)) in samples.iter().zip([
        ("theme", "click"),
        ("theme-storage-blocked", "click"),
        ("disclosure", "keydown"),
        ("focus", "keydown"),
    ]) {
        exact_keys(
            sample,
            &[
                "id",
                "event",
                "trusted",
                "state_observed",
                "duration_ms",
                "driver_ms",
            ],
            "interaction sample",
        )?;
        let duration = sample["duration_ms"]
            .as_f64()
            .ok_or("interaction duration missing")?;
        let driver = sample["driver_ms"]
            .as_f64()
            .ok_or("interaction driver duration missing")?;
        need(
            sample["id"] == id
                && sample["event"] == event
                && sample["trusted"] == true
                && sample["state_observed"] == true
                && duration.is_finite()
                && duration >= 0.0
                && driver.is_finite()
                && driver >= 0.0,
            "trusted interaction identity/state/timing",
        )?;
        maximum = maximum.max(duration);
    }
    need(
        budgets["interaction_p95_ms"]["observed"].as_f64() == Some(maximum),
        "interaction p95 must derive from all four samples",
    )?;
    let controls = &evidence["controls"];
    exact_keys(
        controls,
        &[
            "fast_ms",
            "slow_ms",
            "absent_input_rejected",
            "absent_state_rejected",
            "untrusted_input_rejected",
        ],
        "interaction controls",
    )?;
    need(
        controls["fast_ms"]
            .as_f64()
            .is_some_and(|value| value.is_finite() && (0.0..=100.0).contains(&value))
            && controls["slow_ms"]
                .as_f64()
                .is_some_and(|value| value.is_finite() && value > 100.0)
            && controls["absent_input_rejected"] == true
            && controls["absent_state_rejected"] == true
            && controls["untrusted_input_rejected"] == true,
        "interaction measurement controls must detect slow/missing/untrusted input",
    )?;
    exact_keys(
        &budgets["cls_samples"],
        &["landing", "results", "result_settle_ms"],
        "CLS samples",
    )?;
    for sample in ["landing", "results"] {
        let value = budgets["cls_samples"][sample]
            .as_f64()
            .ok_or("CLS sample missing")?;
        need(
            value.is_finite() && (0.0..=0.1).contains(&value),
            &format!("{sample} CLS exceeds the 0.1 portal budget: observed={value}"),
        )?;
    }
    need(
        budgets["cls_samples"]["result_settle_ms"].as_u64() == Some(250),
        "results CLS must be measured after the documented settle interval",
    )?;
    Ok(())
}

fn validate_browser(
    base: &Path,
    stdout: &str,
    scenario: &str,
    engine: &str,
    base_url: &str,
    client_sha: &str,
) -> Result<Value> {
    let lines: Vec<_> = stdout
        .lines()
        .filter(|line| !line.trim().is_empty())
        .collect();
    need(
        lines.len() == 1 && lines[0].starts_with("T03_BROWSER_COMPLETION "),
        "browser stdout completion inventory differs",
    )?;
    let completion: Value = serde_json::from_str(
        lines[0]
            .strip_prefix("T03_BROWSER_COMPLETION ")
            .ok_or("T03 browser completion missing")?,
    )
    .map_err(|e| e.to_string())?;
    exact_keys(
        &completion,
        &["scenario", "receipt", "sha256"],
        "browser completion",
    )?;
    need(
        completion["scenario"] == scenario,
        "browser completion scenario differs",
    )?;
    let (receipt_path, receipt) = read_completion(base, &completion, "receipt", "sha256")?;
    exact_keys(
        &receipt,
        &[
            "schema",
            "scenario",
            "base_url",
            "client_sha256",
            "browser",
            "completed",
            "skipped",
            "failures",
            "unexpected_requests",
            "console_errors",
            "http_error_responses",
            "http_error_console_statuses",
            "engine_console_parity",
            "page_errors",
            "browser_closed",
            "artifacts",
            "budgets",
        ],
        "browser receipt",
    )?;
    need(
        receipt["schema"] == "T03_BROWSER_V1"
            && receipt["scenario"] == scenario
            && receipt["base_url"] == base_url
            && receipt["client_sha256"] == client_sha,
        "stale browser source or fixture input",
    )?;
    exact_keys(
        &receipt["browser"],
        &["name", "version", "playwright_version"],
        "browser identity",
    )?;
    for field in ["name", "version", "playwright_version"] {
        need(
            receipt["browser"][field]
                .as_str()
                .is_some_and(|value| !value.is_empty()),
            "browser runtime identity missing",
        )?;
    }
    need(
        receipt["browser"]["name"] == engine,
        "browser receipt does not name the requested engine",
    )?;
    let expected_console_parity = if reports_console_status(engine) {
        "chromium"
    } else {
        "not-captured"
    };
    need(
        receipt["engine_console_parity"] == expected_console_parity,
        "browser console parity claim differs from the engine",
    )?;
    let lock: Value = serde_json::from_slice(SOURCES[10].1).map_err(|e| e.to_string())?;
    let playwright = lock["packages"]["node_modules/playwright-core"]["version"]
        .as_str()
        .ok_or("T03 locked Playwright version missing")?;
    need(
        receipt["browser"]["playwright_version"] == playwright,
        "browser dependency differs from lockfile",
    )?;
    for field in [
        "skipped",
        "failures",
        "unexpected_requests",
        "console_errors",
        "page_errors",
    ] {
        need(
            receipt[field].as_array().is_some_and(Vec::is_empty),
            &format!("browser {field} must be empty"),
        )?;
    }
    validate_http_error_evidence(&receipt, scenario, engine, base_url)?;
    need(
        receipt["browser_closed"] == true,
        "browser process cleanup not completed",
    )?;
    let completed = receipt["completed"]
        .as_array()
        .ok_or("T03 browser journey inventory missing")?;
    let completed_ids = completed
        .iter()
        .map(|id| {
            id.as_str()
                .ok_or_else(|| "T03 browser journey ID is not text".to_string())
        })
        .collect::<Result<Vec<_>>>()?;
    let expected: BTreeSet<_> = journey_ids(scenario)?.iter().copied().collect();
    need(
        completed_ids.len() == expected.len()
            && completed_ids.into_iter().collect::<BTreeSet<_>>() == expected,
        "browser journey inventory missing, duplicated or unexpected",
    )?;
    let artifacts = receipt["artifacts"]
        .as_array()
        .ok_or("T03 browser artifact inventory missing")?;
    need(!artifacts.is_empty(), "browser artifacts missing")?;
    let mut paths = BTreeSet::new();
    let mut screenshot = false;
    for artifact in artifacts {
        let artifact_kind = artifact["kind"]
            .as_str()
            .ok_or("T03 browser artifact kind missing")?;
        if artifact_kind == "date-roundtrip-machine-response" {
            exact_keys(
                artifact,
                &[
                    "path",
                    "sha256",
                    "bytes",
                    "kind",
                    "content_type",
                    "width",
                    "height",
                ],
                "browser machine response artifact",
            )?;
            need(
                artifact["content_type"].as_str().is_some_and(|value| {
                    !value.is_empty()
                        && value.len() <= 256
                        && !value.contains('\r')
                        && !value.contains('\n')
                }),
                "browser machine response content type missing",
            )?;
        } else {
            exact_keys(
                artifact,
                &["path", "sha256", "bytes", "kind", "width", "height"],
                "browser artifact",
            )?;
        }
        let (path, raw) = owned_file(
            base,
            artifact["path"]
                .as_str()
                .ok_or("T03 browser artifact path missing")?,
            32 * 1024 * 1024,
        )?;
        need(
            path != receipt_path && paths.insert(path),
            "browser artifact duplicate or self-reference",
        )?;
        need(
            artifact["sha256"] == hash(&raw)
                && artifact["bytes"].as_u64() == Some(raw.len() as u64),
            "browser artifact size or digest differs",
        )?;
        let kind = artifact["kind"]
            .as_str()
            .ok_or("T03 browser artifact kind missing")?;
        need(!kind.is_empty(), "browser artifact kind is empty")?;
        if kind == "screenshot" {
            screenshot = true;
            need(
                artifact["width"].as_u64().is_some_and(|n| n > 0)
                    && artifact["height"].as_u64().is_some_and(|n| n > 0),
                "screenshot dimensions missing",
            )?;
        }
    }
    need(screenshot, "browser screenshot evidence missing")?;
    validate_budgets(scenario, engine, &receipt["budgets"])?;
    Ok(json!({"completion":completion,"receipt":receipt}))
}

type DateRoundtripCallKey = (String, u64, Option<String>, Option<String>);

fn roundtrip_query_value(url: &url::Url, name: &str) -> Result<Option<String>> {
    let values: Vec<_> = url
        .query_pairs()
        .filter(|(key, _)| key == name)
        .map(|(_, value)| value.into_owned())
        .collect();
    need(
        values.len() <= 1,
        &format!("T03 duplicate date-roundtrip URL field {name}"),
    )?;
    Ok(values.into_iter().next().filter(|value| !value.is_empty()))
}

fn roundtrip_request_key(raw: &str, base_url: &str) -> Result<DateRoundtripCallKey> {
    let url = url::Url::parse(raw).map_err(|e| e.to_string())?;
    let base = url::Url::parse(base_url).map_err(|e| e.to_string())?;
    need(
        url.origin() == base.origin()
            && url.path() == "/search"
            && url.username().is_empty()
            && url.password().is_none(),
        "T03 date-roundtrip navigation escaped its loopback search route",
    )?;
    let query = roundtrip_query_value(&url, "q")?.ok_or("T03 date-roundtrip query missing")?;
    need(
        query.starts_with("t03-roundtrip-")
            && roundtrip_query_value(&url, "engines")?.as_deref() == Some("openalex"),
        "T03 date-roundtrip request lost its unique query or explicit OpenAlex scope",
    )?;
    let page = roundtrip_query_value(&url, "pageno")?
        .unwrap_or_else(|| "1".into())
        .parse::<u64>()
        .map_err(|e| e.to_string())?;
    need(page > 0, "T03 date-roundtrip page must be positive")?;
    Ok((
        query,
        page,
        roundtrip_query_value(&url, "date_from")?,
        roundtrip_query_value(&url, "date_to")?,
    ))
}

fn roundtrip_string<'a>(value: &'a Value, key: &str, label: &str) -> Result<&'a str> {
    value[key]
        .as_str()
        .ok_or_else(|| format!("T03 date-roundtrip {label}.{key} must be text"))
}

fn roundtrip_form_value(rows: &Value, name: &str, expected: &str) -> Result<()> {
    let rows = rows
        .as_array()
        .ok_or("T03 date-roundtrip FormData must be an array")?;
    let mut actual = Vec::new();
    for row in rows {
        exact_keys(row, &["name", "value"], "date-roundtrip FormData entry")?;
        if row["name"].as_str() == Some(name) {
            actual.push(
                row["value"]
                    .as_str()
                    .ok_or("T03 date-roundtrip FormData value must be text")?,
            );
        }
    }
    need(
        actual.as_slice() == [expected],
        &format!("T03 date-roundtrip FormData field {name} differs"),
    )
}

fn roundtrip_controls(
    value: &Value,
    query: &str,
    date_from: Option<&str>,
    date_to: Option<&str>,
) -> Result<()> {
    exact_keys(
        value,
        &[
            "query",
            "date_from",
            "date_to",
            "date_from_type",
            "date_to_type",
            "date_from_valid",
            "date_to_valid",
        ],
        "date-roundtrip rendered controls",
    )?;
    need(
        roundtrip_string(value, "query", "controls")? == query
            && roundtrip_string(value, "date_from", "controls")? == date_from.unwrap_or("")
            && roundtrip_string(value, "date_to", "controls")? == date_to.unwrap_or("")
            && roundtrip_string(value, "date_from_type", "controls")? == "date"
            && roundtrip_string(value, "date_to_type", "controls")? == "date"
            && value["date_from_valid"] == true
            && value["date_to_valid"] == true,
        "T03 date-roundtrip visible date controls differ from the request",
    )
}

fn roundtrip_html_proof(
    value: &Value,
    date_from: Option<&str>,
    date_to: Option<&str>,
) -> Result<()> {
    exact_keys(
        value,
        &["date_from", "date_to"],
        "date-roundtrip HTML proof",
    )?;
    for (field, expected) in [("date_from", date_from), ("date_to", date_to)] {
        let row = &value[field];
        if let Some(expected) = expected {
            exact_keys(row, &["status", "text"], "date-roundtrip HTML filter row")?;
            let status = roundtrip_string(row, "status", "HTML proof")?;
            let text = roundtrip_string(row, "text", "HTML proof")?;
            need(
                status == "enforced" && text.contains(expected),
                "T03 date-roundtrip HTML proof does not show the requested enforced bound",
            )?;
        } else {
            need(
                row.is_null(),
                "T03 date-roundtrip HTML proof retained a row for an absent bound",
            )?;
        }
    }
    Ok(())
}

fn roundtrip_machine_document(
    observation: &Value,
    format: &str,
    evidence_dir: &Path,
    artifacts: &[Value],
) -> Result<Value> {
    let content_type = roundtrip_string(observation, "content_type", "machine link")?;
    let media_type = content_type
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    let expected_media_type = match format {
        "json" => "application/json",
        "yaml" => "text/vnd.yaml+markdown",
        _ => return Err("T03 date-roundtrip machine format is invalid".into()),
    };
    need(
        media_type == expected_media_type,
        "T03 machine response content type differs from its format",
    )?;
    let path = roundtrip_string(observation, "body_path", "machine link")?;
    let matching: Vec<_> = artifacts
        .iter()
        .filter(|artifact| {
            artifact["kind"] == "date-roundtrip-machine-response"
                && artifact["path"].as_str() == Some(path)
        })
        .collect();
    need(
        matching.len() == 1,
        "T03 machine response is missing from the owned artifact inventory",
    )?;
    let (_, raw) = owned_file(evidence_dir, path, 8 * 1024 * 1024)?;
    let digest = hash(&raw);
    need(
        observation["body_sha256"] == digest
            && observation["body_bytes"].as_u64() == Some(raw.len() as u64)
            && matching[0]["sha256"] == digest
            && matching[0]["bytes"].as_u64() == Some(raw.len() as u64)
            && matching[0]["content_type"] == content_type,
        "T03 machine response bytes differ from retained evidence",
    )?;
    if format == "json" {
        return serde_json::from_slice(&raw).map_err(|error| error.to_string());
    }
    let text = std::str::from_utf8(&raw).map_err(|error| error.to_string())?;
    let (front_matter, summary) = text
        .split_once("\n---\n")
        .ok_or("T03 YAML response summary delimiter is missing")?;
    need(
        summary.starts_with("## Results Summary\n"),
        "T03 YAML response is missing the expected Markdown summary",
    )?;
    let options = serde_saphyr::options! {
        duplicate_keys: serde_saphyr::DuplicateKeyPolicy::Error,
        budget: serde_saphyr::budget! {
            max_events: 100_000,
            max_aliases: 0,
            max_anchors: 0,
            flow_nesting_limit: 64,
        },
    };
    serde_saphyr::from_str_with_options(front_matter, options)
        .map_err(|error| format!("T03 YAML front matter: {error}"))
}

fn validate_date_roundtrip_machine_proof(proof: &Value) -> Result<()> {
    need(
        proof["status"] == "enforced"
            && proof["requested"]["date_from"] == "2024-02-29"
            && proof["requested"]["date_to"] == "2024-02-29"
            && proof["applied"]["date_from"] == "2024-02-29"
            && proof["applied"]["date_to"] == "2024-02-29"
            && proof["enforced_by"]
                .as_array()
                .is_some_and(|items| items.iter().any(|item| item == "upstream:openalex")),
        "T03 machine response did not decode to the required date proof",
    )
}

fn validate_date_roundtrip_evidence(
    value: &Value,
    base_url: &str,
    evidence_dir: &Path,
    artifacts: &[Value],
) -> Result<()> {
    exact_keys(
        value,
        &["schema", "seed", "navigation_requests", "observations"],
        "date-roundtrip artifact",
    )?;
    need(
        value["schema"] == "T03_DATE_ROUNDTRIP_V1",
        "T03 date-roundtrip artifact schema differs",
    )?;
    let seed = &value["seed"];
    exact_keys(seed, &["status", "url", "controls"], "date-roundtrip seed")?;
    need(
        seed["status"].as_u64() == Some(200),
        "T03 date-roundtrip seed did not return HTTP 200",
    )?;
    roundtrip_controls(&seed["controls"], "t03-roundtrip-browser", None, None)?;
    let seed_url = roundtrip_string(seed, "url", "seed")?;
    need(
        roundtrip_request_key(seed_url, base_url)?
            == ("t03-roundtrip-browser".into(), 1, None, None),
        "T03 date-roundtrip seed request differs",
    )?;

    let observations = value["observations"]
        .as_array()
        .ok_or("T03 date-roundtrip observations missing")?;
    let expected_ids = [
        "blank-submit",
        "lower-only",
        "upper-only",
        "equal-leap-day",
        "next-page-before-query-refinement",
        "query-refinement",
        "next-page",
        "scope-refinement",
        "scope-restoration",
        "next-page-after-scope",
        "previous-page",
        "json-link",
        "yaml-link",
        "clear-bounds",
        "cleared-machine-links",
    ];
    need(
        observations.len() == expected_ids.len(),
        "T03 date-roundtrip observation count differs",
    )?;
    let mut observation_requests = Vec::new();
    for (observation, expected_id) in observations.iter().zip(expected_ids) {
        need(
            observation["id"].as_str() == Some(expected_id),
            "T03 date-roundtrip observation order or identity differs",
        )?;
        match expected_id {
            "blank-submit" | "lower-only" | "upper-only" | "equal-leap-day"
            | "query-refinement" | "scope-refinement" | "scope-restoration" | "clear-bounds" => {
                exact_keys(
                    observation,
                    &[
                        "id",
                        "kind",
                        "source_page",
                        "form_data",
                        "submitted_url",
                        "final_url",
                        "status",
                        "controls",
                        "html_proof",
                    ],
                    "date-roundtrip form observation",
                )?;
                need(
                    observation["kind"] == "form" && observation["status"].as_u64() == Some(200),
                    "T03 date-roundtrip form submission did not succeed",
                )?;
                let (query, from, to) = match expected_id {
                    "blank-submit" => ("t03-roundtrip-browser", None, None),
                    "lower-only" => ("t03-roundtrip-browser", Some("2024-02-29"), None),
                    "upper-only" => ("t03-roundtrip-browser", None, Some("2024-02-29")),
                    "equal-leap-day" => (
                        "t03-roundtrip-browser",
                        Some("2024-02-29"),
                        Some("2024-02-29"),
                    ),
                    "query-refinement" => (
                        "t03-roundtrip-refined",
                        Some("2024-02-29"),
                        Some("2024-02-29"),
                    ),
                    "scope-refinement" => (
                        "t03-roundtrip-refined",
                        Some("2024-02-29"),
                        Some("2024-03-01"),
                    ),
                    "scope-restoration" => (
                        "t03-roundtrip-refined",
                        Some("2024-02-29"),
                        Some("2024-02-29"),
                    ),
                    _ => ("t03-roundtrip-refined", None, None),
                };
                let source_page = if matches!(expected_id, "query-refinement" | "scope-refinement")
                {
                    2
                } else {
                    1
                };
                need(
                    observation["source_page"].as_u64() == Some(source_page),
                    "T03 query/scope form was not submitted from its required source page",
                )?;
                roundtrip_form_value(&observation["form_data"], "q", query)?;
                roundtrip_form_value(&observation["form_data"], "engines", "openalex")?;
                roundtrip_form_value(&observation["form_data"], "date_from", from.unwrap_or(""))?;
                roundtrip_form_value(&observation["form_data"], "date_to", to.unwrap_or(""))?;
                roundtrip_controls(&observation["controls"], query, from, to)?;
                roundtrip_html_proof(&observation["html_proof"], from, to)?;
                let submitted_url = roundtrip_string(observation, "submitted_url", "form")?;
                need(
                    roundtrip_request_key(submitted_url, base_url)?
                        == (
                            query.into(),
                            1,
                            from.map(str::to_owned),
                            to.map(str::to_owned),
                        ),
                    "T03 date-roundtrip form URL differs from its controls",
                )?;
                need(
                    roundtrip_string(observation, "final_url", "form")? == submitted_url,
                    "T03 date-roundtrip form redirected unexpectedly",
                )?;
                observation_requests.push(submitted_url.to_owned());
            }
            "next-page-before-query-refinement"
            | "next-page"
            | "next-page-after-scope"
            | "previous-page"
            | "json-link"
            | "yaml-link" => {
                exact_keys(
                    observation,
                    &[
                        "id",
                        "kind",
                        "href",
                        "request_url",
                        "final_url",
                        "status",
                        "format",
                        "content_type",
                        "body_path",
                        "controls",
                        "proof",
                        "body_sha256",
                        "body_bytes",
                    ],
                    "date-roundtrip link observation",
                )?;
                need(
                    observation["kind"] == "link" && observation["status"].as_u64() == Some(200),
                    "T03 date-roundtrip link did not return HTTP 200",
                )?;
                let (format, page, query) = match expected_id {
                    "next-page-before-query-refinement" => ("html", 2, "t03-roundtrip-browser"),
                    "next-page" | "next-page-after-scope" => ("html", 2, "t03-roundtrip-refined"),
                    "previous-page" => ("html", 1, "t03-roundtrip-refined"),
                    "json-link" => ("json", 1, "t03-roundtrip-refined"),
                    _ => ("yaml", 1, "t03-roundtrip-refined"),
                };
                need(
                    observation["format"].as_str() == Some(format),
                    "T03 date-roundtrip link format differs",
                )?;
                let request_url = roundtrip_string(observation, "request_url", "link")?;
                let wanted_format = if format == "html" { None } else { Some(format) };
                need(
                    roundtrip_request_key(request_url, base_url)?
                        == (
                            query.into(),
                            page,
                            Some("2024-02-29".into()),
                            Some("2024-02-29".into()),
                        ),
                    "T03 date-roundtrip link dropped query, page or absolute bounds",
                )?;
                let parsed = url::Url::parse(request_url).map_err(|e| e.to_string())?;
                need(
                    roundtrip_query_value(&parsed, "format")?.as_deref() == wanted_format,
                    "T03 date-roundtrip link format query differs",
                )?;
                need(
                    url::Url::parse(roundtrip_string(observation, "href", "link")?)
                        .map_err(|e| e.to_string())?
                        == parsed,
                    "T03 date-roundtrip activated a different URL from its rendered link",
                )?;
                need(
                    roundtrip_string(observation, "final_url", "link")? == request_url,
                    "T03 date-roundtrip link redirected unexpectedly",
                )?;
                let body_hash = roundtrip_string(observation, "body_sha256", "link")?;
                need(
                    body_hash.len() == 64
                        && body_hash.bytes().all(|byte| byte.is_ascii_hexdigit())
                        && observation["body_bytes"]
                            .as_u64()
                            .is_some_and(|bytes| bytes > 0),
                    "T03 date-roundtrip response body identity is incomplete",
                )?;
                if format == "html" {
                    need(
                        observation["body_path"].is_null(),
                        "T03 HTML response unexpectedly references a machine artifact",
                    )?;
                    let content_type = roundtrip_string(observation, "content_type", "HTML link")?;
                    need(
                        content_type
                            .split(';')
                            .next()
                            .unwrap_or("")
                            .trim()
                            .eq_ignore_ascii_case("text/html"),
                        "T03 HTML link content type differs",
                    )?;
                    roundtrip_controls(
                        &observation["controls"],
                        query,
                        Some("2024-02-29"),
                        Some("2024-02-29"),
                    )?;
                    roundtrip_html_proof(
                        &observation["proof"],
                        Some("2024-02-29"),
                        Some("2024-02-29"),
                    )?;
                } else {
                    let document =
                        roundtrip_machine_document(observation, format, evidence_dir, artifacts)?;
                    let proof = &document["meta"]["date_filter"];
                    validate_date_roundtrip_machine_proof(proof)?;
                    if format == "json" {
                        need(
                            observation["proof"] == *proof,
                            "T03 JavaScript JSON proof differs from retained response bytes",
                        )?;
                    } else {
                        need(
                            observation["proof"].is_null(),
                            "T03 YAML proof must be derived from retained response bytes",
                        )?;
                    }
                }
                observation_requests.push(request_url.to_owned());
            }
            "cleared-machine-links" => {
                exact_keys(
                    observation,
                    &["id", "kind", "links"],
                    "cleared date-roundtrip machine-link inventory",
                )?;
                need(
                    observation["kind"] == "link-inventory",
                    "T03 cleared machine-link observation kind differs",
                )?;
                let links = observation["links"]
                    .as_array()
                    .ok_or("T03 cleared machine links missing")?;
                need(
                    links.len() == 2,
                    "T03 cleared JSON/YAML links are incomplete",
                )?;
                let mut formats = BTreeSet::new();
                for link in links {
                    exact_keys(
                        link,
                        &["format", "href", "date_from_present", "date_to_present"],
                        "cleared date-roundtrip machine link",
                    )?;
                    let format = roundtrip_string(link, "format", "cleared link")?;
                    need(
                        matches!(format, "json" | "yaml")
                            && formats.insert(format.to_owned())
                            && link["date_from_present"] == false
                            && link["date_to_present"] == false,
                        "T03 cleared machine link retained an absolute date bound",
                    )?;
                    let href = url::Url::parse(roundtrip_string(link, "href", "cleared link")?)
                        .map_err(|e| e.to_string())?;
                    need(
                        roundtrip_request_key(href.as_str(), base_url)?
                            == ("t03-roundtrip-refined".into(), 1, None, None)
                            && roundtrip_query_value(&href, "format")?.as_deref() == Some(format),
                        "T03 cleared machine link changed query or retained date state",
                    )?;
                }
                need(
                    formats == ["json".to_owned(), "yaml".to_owned()].into(),
                    "T03 cleared machine link formats differ",
                )?;
            }
            _ => return Err("T03 unexpected date-roundtrip observation ID".into()),
        }
    }

    let navigation_requests = value["navigation_requests"]
        .as_array()
        .ok_or("T03 date-roundtrip navigation request list missing")?;
    need(
        navigation_requests.len() >= 15,
        "T03 browser did not retain every date-roundtrip navigation request",
    )?;
    let mut observed = BTreeMap::<String, usize>::new();
    for request in navigation_requests {
        let request = request
            .as_str()
            .ok_or("T03 date-roundtrip navigation URL must be text")?;
        let key = roundtrip_request_key(request, base_url)?;
        *observed.entry(request.to_owned()).or_default() += 1;
        need(
            key.0.starts_with("t03-roundtrip-"),
            "T03 date-roundtrip captured a non-journey navigation",
        )?;
    }
    for request in std::iter::once(seed_url).chain(observation_requests.iter().map(String::as_str))
    {
        need(
            observed.contains_key(request),
            "T03 date-roundtrip submitted navigation was not retained",
        )?;
    }
    Ok(())
}

fn validate_date_roundtrip_call_log(
    calls_jsonl: &str,
    evidence: &Value,
    base_url: &str,
) -> Result<Value> {
    let mut expected = BTreeMap::<DateRoundtripCallKey, usize>::new();
    for request in evidence["navigation_requests"]
        .as_array()
        .ok_or("T03 date-roundtrip navigation request list missing")?
    {
        let request = request
            .as_str()
            .ok_or("T03 date-roundtrip navigation URL must be text")?;
        *expected
            .entry(roundtrip_request_key(request, base_url)?)
            .or_default() += 1;
    }

    let mut actual = BTreeMap::<DateRoundtripCallKey, usize>::new();
    for line in calls_jsonl.lines() {
        let call: ssx_json::Value = ssx_json::from_str(line).map_err(|e| e.to_string())?;
        let Some(query) = call["query"].as_str() else {
            continue;
        };
        if !query.starts_with("t03-roundtrip-") {
            continue;
        }
        need(
            call["engine"].as_str() == Some("openalex"),
            "T03 date-roundtrip reached an engine outside explicit OpenAlex scope",
        )?;
        let page_wire = ssx_json::to_string(&call["page"]).map_err(|e| e.to_string())?;
        let page = page_wire
            .trim_matches('"')
            .parse::<u64>()
            .map_err(|e| e.to_string())?;
        let date = |key: &str| -> Result<Option<String>> {
            if call[key].is_null() {
                Ok(None)
            } else {
                call[key]
                    .as_str()
                    .map(|value| Some(value.to_owned()))
                    .ok_or_else(|| {
                        format!("T03 date-roundtrip engine call {key} is not text or null")
                    })
            }
        };
        *actual
            .entry((query.to_owned(), page, date("date_from")?, date("date_to")?))
            .or_default() += 1;
    }
    need(
        actual == expected,
        "T03 browser navigation requests differ from the native SearchService call log",
    )?;
    for (key, minimum) in [
        (("t03-roundtrip-browser".into(), 1, None, None), 2),
        (
            (
                "t03-roundtrip-browser".into(),
                2,
                Some("2024-02-29".into()),
                Some("2024-02-29".into()),
            ),
            1,
        ),
        (
            (
                "t03-roundtrip-browser".into(),
                1,
                Some("2024-02-29".into()),
                None,
            ),
            1,
        ),
        (
            (
                "t03-roundtrip-browser".into(),
                1,
                None,
                Some("2024-02-29".into()),
            ),
            1,
        ),
        (
            (
                "t03-roundtrip-browser".into(),
                1,
                Some("2024-02-29".into()),
                Some("2024-02-29".into()),
            ),
            1,
        ),
        (
            (
                "t03-roundtrip-refined".into(),
                1,
                Some("2024-02-29".into()),
                Some("2024-02-29".into()),
            ),
            4,
        ),
        (
            (
                "t03-roundtrip-refined".into(),
                2,
                Some("2024-02-29".into()),
                Some("2024-02-29".into()),
            ),
            2,
        ),
        (
            (
                "t03-roundtrip-refined".into(),
                1,
                Some("2024-02-29".into()),
                Some("2024-03-01".into()),
            ),
            1,
        ),
        (("t03-roundtrip-refined".into(), 1, None, None), 1),
    ] {
        need(
            actual.get(&key).copied().unwrap_or_default() >= minimum,
            "T03 date-roundtrip call log is missing a required browser state",
        )?;
    }
    Ok(json!({
        "navigation_requests":expected.values().sum::<usize>(),
        "service_calls":actual.values().sum::<usize>(),
        "query_states":actual.len(),
        "matched":true,
        "engine":"openalex",
        "source":"synthetic T03 fixture SearchService; provider bytes remain covered by science_openalex integration"
    }))
}

fn validate_http_error_evidence(
    receipt: &Value,
    scenario: &str,
    engine: &str,
    base_url: &str,
) -> Result<()> {
    let expected: &[u64] = match scenario {
        "contract" => &[400, 403, 503],
        "cached-error" => &[503],
        "rate-limited" => &[429],
        "html-disabled" => &[403],
        "unavailable" => &[503],
        _ => return Err("T03 unknown browser scenario for HTTP statuses".into()),
    };
    let responses = receipt["http_error_responses"]
        .as_array()
        .ok_or("T03 HTTP error response evidence missing")?;
    let mut statuses = Vec::new();
    for response in responses {
        exact_keys(response, &["status", "url"], "HTTP error response")?;
        let status = response["status"]
            .as_u64()
            .ok_or("T03 HTTP error response status missing")?;
        let url = response["url"]
            .as_str()
            .ok_or("T03 HTTP error response URL missing")?;
        let expected_origin = url::Url::parse(base_url).map_err(|e| e.to_string())?;
        let observed_url = url::Url::parse(url).map_err(|e| e.to_string())?;
        need(
            status >= 400
                && expected_origin.origin() == observed_url.origin()
                && observed_url.username().is_empty()
                && observed_url.password().is_none(),
            "T03 unexpected or non-loopback HTTP error response",
        )?;
        statuses.push(status);
    }
    statuses.sort_unstable();
    let mut console_statuses = receipt["http_error_console_statuses"]
        .as_array()
        .ok_or("T03 expected HTTP console status evidence missing")?
        .iter()
        .map(|status| {
            status
                .as_u64()
                .ok_or_else(|| "T03 console HTTP status is not an integer".to_owned())
        })
        .collect::<Result<Vec<_>>>()?;
    console_statuses.sort_unstable();
    // Chromium and Playwright WebKit report the blocked document/subresource
    // status on the console; Gecko does not, so only the response inventory is
    // cross-engine for Firefox.
    let console_expectation = if reports_console_status(engine) {
        statuses == expected && console_statuses == expected
    } else {
        statuses == expected && console_statuses.is_empty()
    };
    need(
        console_expectation,
        "T03 HTTP error responses differ from explicitly asserted scenario statuses",
    )
}

struct Fixture {
    runtime: Option<tokio::runtime::Runtime>,
    child: Option<Box<dyn ChildWrapper>>,
    stdout: Option<tokio::process::ChildStdout>,
    out: PathBuf,
    pid: u32,
    startup: Value,
    base_url: String,
    finished: bool,
}
async fn startup_line(stdout: &mut tokio::process::ChildStdout) -> Result<Vec<u8>> {
    let mut line = Vec::new();
    loop {
        need(line.len() < 65536, "fixture startup exceeds bound")?;
        let mut byte = [0];
        stdout
            .read_exact(&mut byte)
            .await
            .map_err(|e| e.to_string())?;
        line.push(byte[0]);
        if byte[0] == b'\n' {
            return Ok(line);
        }
    }
}
#[cfg(unix)]
async fn tree_absent(pid: u32) -> bool {
    use nix::{errno::Errno, sys::signal::killpg, unistd::Pid};
    let Ok(pid) = i32::try_from(pid) else {
        return false;
    };
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    let mut absent = 0;
    while tokio::time::Instant::now() < deadline {
        if matches!(killpg(Pid::from_raw(pid), None), Err(Errno::ESRCH)) {
            absent += 1;
            if absent == 2 {
                return true;
            }
        } else {
            absent = 0;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    false
}
#[cfg(windows)]
fn job_absent(child: &dyn ChildWrapper) -> bool {
    let any: &dyn std::any::Any = child;
    any.downcast_ref::<process_wrap::tokio::JobObjectChild>()
        .is_some_and(|job| job.active_processes().ok() == Some(0))
}
impl Fixture {
    fn start(binary: &Path, binary_sha: &str, scenario: &str, out: PathBuf) -> Result<Self> {
        fs::create_dir(&out).map_err(|e| e.to_string())?;
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| e.to_string())?;
        let stderr = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(out.join("fixture-stderr.log"))
            .map_err(|e| e.to_string())?;
        let child = runtime.block_on(async {
            let mut command = tokio::process::Command::new(binary);
            command
                .arg(out.join("calls.jsonl"))
                .arg("dark")
                .arg(scenario)
                .env_clear()
                .env("TZ", "UTC")
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(stderr);
            let mut command = CommandWrap::from(command);
            command.wrap(KillOnDrop);
            #[cfg(unix)]
            {
                command.wrap(process_wrap::tokio::ProcessGroup::leader());
            }
            #[cfg(windows)]
            {
                command.wrap(process_wrap::tokio::JobObject);
            }
            command.spawn().map_err(|e| e.to_string())
        })?;
        let mut fixture = Self {
            runtime: Some(runtime),
            child: Some(child),
            stdout: None,
            out,
            pid: 0,
            startup: Value::Null,
            base_url: String::new(),
            finished: false,
        };
        let child = fixture.child.as_mut().ok_or("T03 fixture child absent")?;
        fixture.pid = child.id().ok_or("T03 fixture PID absent")?;
        fixture.stdout = child.stdout().take();
        let stdout = fixture.stdout.as_mut().ok_or("T03 fixture stdout absent")?;
        let raw = fixture
            .runtime
            .as_ref()
            .ok_or("T03 fixture runtime absent")?
            .block_on(async {
                tokio::time::timeout(Duration::from_secs(10), startup_line(stdout))
                    .await
                    .map_err(|_| "T03 fixture startup deadline".to_owned())?
            })?;
        fs::write(fixture.out.join("fixture-startup.jsonl"), &raw).map_err(|e| e.to_string())?;
        let startup: Value = serde_json::from_slice(&raw).map_err(|e| e.to_string())?;
        exact_keys(
            &startup,
            &["address", "mode", "renderer", "scenario"],
            "fixture startup",
        )?;
        need(
            startup["mode"] == "offline-t03-fixture"
                && startup["renderer"] == "PublicPortal"
                && startup["scenario"] == scenario,
            "fixture startup scenario or renderer differs",
        )?;
        let address: SocketAddr = startup["address"]
            .as_str()
            .ok_or("T03 fixture listener missing")?
            .parse()
            .map_err(|e| format!("T03 fixture listener: {e}"))?;
        need(
            address.ip().is_loopback() && address.port() != 0,
            "fixture listener must be bound loopback",
        )?;
        fixture.base_url = format!("http://{address}");
        fixture.startup = json!({"pid":fixture.pid,"binary":binary.to_string_lossy().as_ref(),"binary_sha256":binary_sha,"startup":startup,"base_url":fixture.base_url});
        fs::write(
            fixture.out.join("process-start.json"),
            serde_json::to_vec_pretty(&fixture.startup).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        Ok(fixture)
    }
    fn finish(&mut self) -> Result<Value> {
        if self.finished {
            return Err("T03 fixture retirement attempted twice".into());
        }
        self.finished = true;
        let child = self.child.as_mut().ok_or("T03 fixture child absent")?;
        let stdout = &mut self.stdout;
        let pid = self.pid;
        let terminal = self.runtime.as_ref().ok_or("T03 fixture runtime absent")?.block_on(async {
            let mut stop_sent = false;
            if let Some(mut stdin) = child.stdin().take() {
                stop_sent = matches!(tokio::time::timeout(Duration::from_secs(1), stdin.write_all(b"STOP\n")).await, Ok(Ok(())));
                drop(stdin);
            }
            let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
            let mut exit = None;
            while tokio::time::Instant::now() < deadline {
                match child.try_wait() { Ok(Some(status)) => { exit=Some(status); break; }, Ok(None)=>tokio::time::sleep(Duration::from_millis(10)).await, Err(_)=>break }
            }
            let graceful = exit.as_ref().is_some_and(std::process::ExitStatus::success);
            let kill = child.start_kill();
            let waited = matches!(tokio::time::timeout(Duration::from_secs(5), child.wait()).await, Ok(Ok(_)));
            #[cfg(unix)] let retired = tree_absent(pid).await;
            #[cfg(windows)] let retired = job_absent(child.as_ref());
            let mut tail = Vec::new();
            let stdout_eof = if let Some(stdout)=stdout.as_mut() { matches!(tokio::time::timeout(Duration::from_secs(1), stdout.take(65537).read_to_end(&mut tail)).await, Ok(Ok(_))) } else {false};
            json!({"pid":pid,"stop_sent":stop_sent,"child_joined":waited,"child_exit_success":graceful,"tree_retired":retired,"stdout_eof":stdout_eof,"stdout_tail_bytes":tail.len(),"stdout_tail_sha256":hash(&tail),"forced_cleanup_error":kill.err().map(|e|e.to_string()),"deadline_seconds":{"stop":1,"graceful":5,"join":5,"tree_absence":5,"stdout":1}})
        });
        let calls = regular(&self.out.join("calls.jsonl"), 8 * 1024 * 1024)?;
        let stderr = regular(&self.out.join("fixture-stderr.log"), 1024 * 1024)?;
        let mut terminal = terminal;
        terminal["engine_calls"] = json!(
            String::from_utf8(calls.clone())
                .map_err(|e| e.to_string())?
                .lines()
                .count()
        );
        terminal["calls_sha256"] = json!(hash(&calls));
        terminal["calls_jsonl"] = json!(String::from_utf8(calls).map_err(|e| e.to_string())?);
        terminal["stderr_bytes"] = json!(stderr.len());
        terminal["stderr_sha256"] = json!(hash(&stderr));
        fs::write(
            self.out.join("terminal.json"),
            serde_json::to_vec_pretty(&terminal).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        self.child.take();
        if let Some(runtime) = self.runtime.take() {
            runtime.shutdown_timeout(Duration::from_secs(1));
        }
        Ok(terminal)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        if !self.finished {
            let _ = self.finish();
        }
        if let Some(child) = self.child.as_mut() {
            let _ = child.start_kill();
        }
        self.child.take();
        if let Some(runtime) = self.runtime.take() {
            runtime.shutdown_timeout(Duration::from_secs(1));
        }
    }
}
fn validate_cleanup(terminal: &Value) -> Result<()> {
    need(
        terminal["stop_sent"] == true
            && terminal["child_joined"] == true
            && terminal["child_exit_success"] == true
            && terminal["tree_retired"] == true
            && terminal["stdout_eof"] == true
            && terminal["stdout_tail_bytes"] == 0
            && terminal["stderr_bytes"] == 0,
        "fixture process cleanup incomplete",
    )
}
fn suite_inventory(rows: &[Value], source_sha: &str) -> Result<()> {
    need(
        rows.len() == SCENARIOS.len() * ENGINES.len(),
        "browser scenario inventory missing or extra",
    )?;
    let mut names = BTreeSet::new();
    for row in rows {
        let name = row["scenario"]
            .as_str()
            .ok_or("T03 scenario name missing")?;
        let engine = row["engine"].as_str().ok_or("T03 engine name missing")?;
        need(names.insert((name, engine)), "duplicate browser scenario")?;
        need(SCENARIOS.contains(&name), "unexpected browser scenario")?;
        need(ENGINES.contains(&engine), "unexpected browser engine")?;
        need(
            row["source_sha256"] == source_sha,
            "stale browser suite input",
        )?;
        validate_cleanup(&row["terminal"])?;
    }
    need(
        names
            == SCENARIOS
                .into_iter()
                .flat_map(|scenario| ENGINES.into_iter().map(move |engine| (scenario, engine)))
                .collect(),
        "browser scenario inventory differs",
    )
}
fn controls(rows: &[Value], source_sha: &str) -> Result<Value> {
    suite_inventory(rows, source_sha)?;
    let mut outcomes = Vec::new();
    for (id, reason) in [
        (
            "omission",
            "T03 browser scenario inventory missing or extra",
        ),
        ("duplicate", "T03 duplicate browser scenario"),
        ("stale-input", "T03 stale browser suite input"),
        ("process-cleanup", "T03 fixture process cleanup incomplete"),
        ("engine-substitution", "T03 unexpected browser engine"),
    ] {
        let mut mutant = rows.to_vec();
        match id {
            "omission" => {
                mutant.pop();
            }
            "duplicate" => {
                mutant[1] = mutant[0].clone();
            }
            "stale-input" => {
                mutant[0]["source_sha256"] = json!("stale");
            }
            "engine-substitution" => {
                mutant[0]["engine"] = json!("safari");
            }
            _ => {
                mutant[0]["terminal"]["tree_retired"] = json!(false);
            }
        }
        need(
            suite_inventory(&mutant, source_sha).err().as_deref() == Some(reason),
            "negative control did not reject its named defect",
        )?;
        outcomes.push(json!({"id":id,"expected_rejection":reason,"mutant_sha256":hash(&serde_json::to_vec(&mutant).map_err(|e|e.to_string())?)}));
    }
    Ok(
        json!({"positive_validated_first":true,"positive_sha256":hash(&serde_json::to_vec(rows).map_err(|e|e.to_string())?),"negative_controls":outcomes}),
    )
}
#[cfg(test)]
pub(crate) fn validator_controls() -> Result<()> {
    let rows:Vec<_>=SCENARIOS.into_iter().flat_map(|scenario|ENGINES.into_iter().map(move |engine|json!({"scenario":scenario,"engine":engine,"source_sha256":"synthetic-validator-input","terminal":{"stop_sent":true,"child_joined":true,"child_exit_success":true,"tree_retired":true,"stdout_eof":true,"stdout_tail_bytes":0,"stderr_bytes":0}}))).collect();
    controls(&rows, "synthetic-validator-input")?;
    need(
        expected_http().len() == 104 && HTTP_TESTS.len() == 11,
        "closed HTTP inventory",
    )
}
pub fn check(root: &Path, records: &mut Vec<Value>) -> Result<()> {
    let before = sources(root)?;
    let source_sha = hash(&serde_json::to_vec(&before).map_err(|e| e.to_string())?);
    let base = PathBuf::from(
        std::env::var_os("T03_EVIDENCE_DIR")
            .ok_or("T03_EVIDENCE_DIR must name the registered absolute evidence root")?,
    );
    need(base.is_absolute(), "evidence root must be absolute")?;
    if let Ok(metadata) = fs::symlink_metadata(&base) {
        need(
            !metadata.file_type().is_symlink(),
            "evidence root is a symlink",
        )?;
    }
    let base = crate::validate_output(root, &base)?;
    fs::create_dir_all(&base).map_err(|e| e.to_string())?;
    need(
        !fs::symlink_metadata(&base)
            .map_err(|e| e.to_string())?
            .file_type()
            .is_symlink(),
        "evidence root is a symlink",
    )?;
    let run = base.join(format!(
        "t03-run-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_nanos()
    ));
    fs::create_dir(&run).map_err(|e| e.to_string())?;
    let mut node_identity = Command::new("node");
    node_identity.arg("--version");
    let node_version = capture(root, node_identity, Duration::from_secs(30), records)?;
    need(
        node_version.trim().starts_with('v'),
        "Node runtime identity missing",
    )?;
    let build = cargo(
        root,
        &[
            "build",
            "--locked",
            "-p",
            "ssx-http",
            "--example",
            "t03_fixture",
            "--message-format=json",
        ],
        Duration::from_secs(1200),
        records,
    )?;
    let images = build
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .filter(|row| {
            row["reason"] == "compiler-artifact"
                && row["target"]["name"] == "t03_fixture"
                && row["target"]["kind"]
                    .as_array()
                    .is_some_and(|kinds| kinds.iter().any(|kind| kind == "example"))
        })
        .filter_map(|row| row["executable"].as_str().map(str::to_owned))
        .collect::<Vec<_>>();
    need(
        images.len() == 1,
        "fixture compiler artifact identity missing or duplicate",
    )?;
    let binary = PathBuf::from(&images[0])
        .canonicalize()
        .map_err(|e| e.to_string())?;
    let binary_sha = hash(&regular(&binary, 256 * 1024 * 1024)?);
    let list = cargo(
        root,
        &[
            "test", "--locked", "-p", "ssx-http", "--test", "t03", "--", "--list",
        ],
        Duration::from_secs(1200),
        records,
    )?;
    required_test_inventory(&list)?;
    let http_dir = run.join("http");
    fs::create_dir(&http_dir).map_err(|e| e.to_string())?;
    let mut command = Command::new("cargo");
    command
        .args([
            "test",
            "--locked",
            "-p",
            "ssx-http",
            "--test",
            "t03",
            "--",
            "--nocapture",
            "--test-threads=1",
        ])
        .env("T03_EVIDENCE_DIR", &http_dir)
        .env("T03_NATIVE_SERVER_BINARY", &binary);
    let output = capture(root, command, Duration::from_secs(600), records)?;
    completed_tests(&output)?;
    let http = validate_http(&http_dir, &output, &binary, &binary_sha)?;
    records.push(json!({"plan":"T03","role":"actual-http-inventory","source_sha256":source_sha,"evidence":http}));
    let client_sha = hash(SOURCES[8].1);
    let mut browser_rows = Vec::new();
    for engine in ENGINES {
        for scenario in SCENARIOS {
            let scenario_dir = run.join(format!("browser-{scenario}-{engine}"));
            let mut fixture = Fixture::start(&binary, &binary_sha, scenario, scenario_dir)?;
            records.push(json!({"plan":"T03","role":"browser-fixture-start","scenario":scenario,"engine":engine,"startup":fixture.startup}));
            let browser_dir = fixture.out.join("browser");
            fs::create_dir(&browser_dir).map_err(|e| e.to_string())?;
            let mut node = Command::new("node");
            node.arg(CLIENT)
                .env("T03_SCENARIO", scenario)
                .env("T03_BROWSER_ENGINE", engine)
                .env("T03_BROWSER_BASE_URL", &fixture.base_url)
                .env("T03_EVIDENCE_DIR", &browser_dir);
            let browser = capture(root, node, Duration::from_secs(600), records);
            if let Err(error) = &browser {
                let receipt = browser_dir.join(format!("{scenario}.browser-receipt.json"));
                if let Ok(bytes) = fs::read(&receipt)
                    && let Ok(receipt) = serde_json::from_slice::<Value>(&bytes)
                {
                    return Err(format!("{error}: browser failures {}", receipt["failures"]));
                }
                return Err(error.clone());
            }
            let terminal = fixture.finish();
            records.push(json!({"plan":"T03","role":"browser-fixture-terminal","scenario":scenario,"terminal":terminal.as_ref().ok(),"error":terminal.as_ref().err()}));
            let terminal = terminal?;
            validate_cleanup(&terminal)?;
            if scenario != "contract" {
                need(
                    terminal["engine_calls"] == 0,
                    "browser error scenario dispatched an engine",
                )?;
            }
            let calls = terminal["calls_jsonl"]
                .as_str()
                .ok_or("T03 browser engine-call observations missing")?;
            for line in calls.lines() {
                validate_engine_call(line, "browser fixture reached an unexpected engine")?;
            }
            let mut browser = validate_browser(
                &browser_dir,
                &browser?,
                scenario,
                engine,
                &fixture.base_url,
                &client_sha,
            )?;
            if scenario == "contract" {
                let artifacts = browser["receipt"]["artifacts"]
                    .as_array()
                    .ok_or("T03 browser date-roundtrip artifact inventory missing")?;
                let date_artifacts: Vec<_> = artifacts
                    .iter()
                    .filter(|artifact| artifact["kind"] == "date-roundtrip")
                    .collect();
                need(
                    date_artifacts.len() == 1,
                    "contract browser must retain one date-roundtrip artifact",
                )?;
                let (path, raw) = owned_file(
                    &browser_dir,
                    date_artifacts[0]["path"]
                        .as_str()
                        .ok_or("T03 date-roundtrip artifact path missing")?,
                    2 * 1024 * 1024,
                )?;
                let date_roundtrip: Value =
                    serde_json::from_slice(&raw).map_err(|e| e.to_string())?;
                validate_date_roundtrip_evidence(
                    &date_roundtrip,
                    &fixture.base_url,
                    &browser_dir,
                    artifacts,
                )?;
                let call_validation =
                    validate_date_roundtrip_call_log(calls, &date_roundtrip, &fixture.base_url)?;
                browser["date_roundtrip_artifact"] = json!({
                    "path":path,
                    "sha256":hash(&raw),
                    "bytes":raw.len(),
                    "evidence":date_roundtrip,
                    "call_log":call_validation
                });
            }
            browser_rows.push(json!({"scenario":scenario,"engine":engine,"source_sha256":source_sha,"startup":fixture.startup,"terminal":terminal,"browser":browser}));
        }
    }
    let negative = controls(&browser_rows, &source_sha)?;
    need(
        sources(root)? == before && hash(&regular(&binary, 256 * 1024 * 1024)?) == binary_sha,
        "source or executed fixture image changed during gate",
    )?;
    records.push(json!({"plan":"T03","role":"native-browser-acceptance","source_inputs":before,"source_sha256":source_sha,"server_binary":{"path":binary,"sha256":binary_sha},"http_tests":HTTP_TESTS,"http_receipts":104,"http_processes":12,"browser_scenarios":browser_rows,"controls":negative,"evidence_directory":run,"scope":"real native HTTP plus deterministic Chromium and Firefox browser journeys and measured synthetic budgets; actual Safari, manual visual/assistive technology and 200% zoom acceptance remain separate"}));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        hash, roundtrip_controls, roundtrip_html_proof, roundtrip_machine_document,
        roundtrip_request_key, validate_budgets, validate_date_roundtrip_call_log,
        validate_date_roundtrip_machine_proof, validate_engine_call, validate_http_error_evidence,
    };
    use serde_json::{Value, json};
    use std::{
        fs,
        path::{Path, PathBuf},
        time::{SystemTime, UNIX_EPOCH},
    };
    use url::Url;

    struct TestEvidenceDir(PathBuf);

    impl TestEvidenceDir {
        fn new() -> Self {
            let nonce = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock must be after the Unix epoch")
                .as_nanos();
            let path = std::env::temp_dir()
                .join(format!("ssx-t03-evidence-{}-{nonce}", std::process::id()));
            fs::create_dir(&path).expect("create owned T03 unit-test directory");
            Self(path)
        }
    }

    impl Drop for TestEvidenceDir {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).expect("remove owned T03 unit-test directory");
        }
    }

    fn roundtrip_url(
        query: &str,
        page: u64,
        date_from: Option<&str>,
        date_to: Option<&str>,
    ) -> String {
        let mut url = Url::parse("http://127.0.0.1:4321/search").expect("fixture URL");
        {
            let mut pairs = url.query_pairs_mut();
            pairs.append_pair("q", query);
            pairs.append_pair("engines", "openalex");
            pairs.append_pair("pageno", &page.to_string());
            if let Some(value) = date_from {
                pairs.append_pair("date_from", value);
            }
            if let Some(value) = date_to {
                pairs.append_pair("date_to", value);
            }
        }
        url.to_string()
    }

    fn rendered_controls(query: &str, date_from: Option<&str>, date_to: Option<&str>) -> Value {
        json!({
            "query": query,
            "date_from": date_from.unwrap_or(""),
            "date_to": date_to.unwrap_or(""),
            "date_from_type": "date",
            "date_to_type": "date",
            "date_from_valid": true,
            "date_to_valid": true
        })
    }

    fn rendered_proof(date_from: Option<&str>, date_to: Option<&str>) -> Value {
        let row = |name: &str, value: Option<&str>| {
            value.map_or(
                Value::Null,
                |value| json!({"status":"enforced", "text":format!("{name}: {value}")}),
            )
        };
        json!({
            "date_from": row("date_from", date_from),
            "date_to": row("date_to", date_to)
        })
    }

    fn machine_response_fixture(
        directory: &Path,
        relative_path: &str,
        content_type: &str,
        bytes: &[u8],
    ) -> (Value, Vec<Value>) {
        let absolute_path = directory.join(relative_path).to_string_lossy().into_owned();
        fs::write(&absolute_path, bytes).expect("write owned machine response");
        let digest = hash(bytes);
        let observation = json!({
            "content_type": content_type,
            "body_path": absolute_path.clone(),
            "body_sha256": digest,
            "body_bytes": bytes.len()
        });
        let artifacts = vec![json!({
            "kind": "date-roundtrip-machine-response",
            "path": absolute_path,
            "sha256": digest,
            "bytes": bytes.len(),
            "content_type": content_type
        })];
        (observation, artifacts)
    }

    fn call_line(
        query: &str,
        engine: &str,
        page: u64,
        date_from: Option<&str>,
        date_to: Option<&str>,
    ) -> String {
        json!({
            "query": query,
            "engine": engine,
            "page": page,
            "date_from": date_from,
            "date_to": date_to
        })
        .to_string()
    }

    pub(super) fn passing_budgets() -> Value {
        json!({
            "profile": "synthetic offline T03 fixture; Chromium; Fast 3G emulation",
            "html_uncompressed_bytes": {"observed": 1024.0, "limit": 51200.0, "pass": true},
            "html_gzip_bytes": {"observed": 256.0, "limit": 15360.0, "pass": true},
            "css_js_uncompressed_bytes": {"observed": 512.0, "limit": 30720.0, "pass": true},
            "fcp_ms": {"observed": 900.0, "limit": 1500.0, "pass": true},
            "cls": {"observed": 0.01, "limit": 0.1, "pass": true},
            "cls_samples": {"landing": 0.01, "results": 0.02, "result_settle_ms": 250},
            "interaction_p95_ms": {"observed": 40.0, "limit": 100.0, "pass": true},
            "interaction_evidence": {
                "definition": "trusted-event-timestamp-to-state-and-next-frame-v1",
                "samples": [
                    {"id":"theme", "event":"click", "trusted":true, "state_observed":true, "duration_ms":40.0, "driver_ms":100.0},
                    {"id":"theme-storage-blocked", "event":"click", "trusted":true, "state_observed":true, "duration_ms":30.0, "driver_ms":100.0},
                    {"id":"disclosure", "event":"keydown", "trusted":true, "state_observed":true, "duration_ms":20.0, "driver_ms":100.0},
                    {"id":"focus", "event":"keydown", "trusted":true, "state_observed":true, "duration_ms":10.0, "driver_ms":100.0}
                ],
                "controls": {"fast_ms":20.0,"slow_ms":150.0,"absent_input_rejected":true,"absent_state_rejected":true,"untrusted_input_rejected":true}
            },
            "handoff_p95_ms": {"observed": 60.0, "limit": 150.0, "pass": true}
        })
    }

    #[test]
    fn budget_failure_names_the_measured_value_and_limit() {
        let mut budgets = passing_budgets();
        budgets["fcp_ms"]["observed"] = json!(2000.0);
        budgets["fcp_ms"]["pass"] = json!(false);
        let error = validate_budgets("contract", "chromium", &budgets).expect_err("must fail");
        assert!(error.contains("fcp_ms"), "{error}");
        assert!(error.contains("observed=2000"), "{error}");
        assert!(error.contains("limit=1500"), "{error}");
    }

    #[test]
    fn cls_sample_failure_names_the_sample_and_value() {
        let mut budgets = passing_budgets();
        budgets["cls_samples"]["results"] = json!(0.5);
        let error = validate_budgets("contract", "chromium", &budgets).expect_err("must fail");
        assert!(error.contains("results CLS"), "{error}");
        assert!(error.contains("observed=0.5"), "{error}");
    }

    #[test]
    fn passing_budgets_validate() {
        validate_budgets("contract", "chromium", &passing_budgets()).expect("budgets pass");
        validate_budgets("contract", "firefox", &json!({})).expect("non-profiled has no budgets");
    }

    #[test]
    fn date_roundtrip_request_requires_unique_loopback_query_scope_and_bounds() {
        let base = "http://127.0.0.1:4321";
        let valid = roundtrip_url(
            "t03-roundtrip-browser",
            2,
            Some("2024-02-29"),
            Some("2024-03-01"),
        );
        assert_eq!(
            roundtrip_request_key(&valid, base).expect("valid scoped request"),
            (
                "t03-roundtrip-browser".into(),
                2,
                Some("2024-02-29".into()),
                Some("2024-03-01".into())
            )
        );
        let blank = format!(
            "{}&date_from=&date_to=",
            roundtrip_url("t03-roundtrip-browser", 1, None, None)
        );
        assert_eq!(
            roundtrip_request_key(&blank, base).expect("blank bounds normalize to absent"),
            ("t03-roundtrip-browser".into(), 1, None, None)
        );
        for invalid in [
            format!("{valid}&date_from=2024-02-29"),
            format!("{valid}&engines=brave"),
            format!("{valid}&pageno=0"),
            roundtrip_url("t03-roundtrip-browser", 0, None, None),
            valid.replace("127.0.0.1:4321", "127.0.0.1:4322"),
            valid.replace("t03-roundtrip-browser", "ordinary-query"),
        ] {
            assert!(
                roundtrip_request_key(&invalid, base).is_err(),
                "accepted {invalid}"
            );
        }
    }

    #[test]
    fn date_roundtrip_controls_and_optional_html_proof_are_truthful() {
        for (date_from, date_to) in [
            (None, None),
            (Some("2024-02-29"), None),
            (None, Some("2024-02-29")),
            (Some("2024-02-29"), Some("2024-02-29")),
        ] {
            roundtrip_controls(
                &rendered_controls("t03-roundtrip-browser", date_from, date_to),
                "t03-roundtrip-browser",
                date_from,
                date_to,
            )
            .expect("visible valid controls match the request");
            roundtrip_html_proof(&rendered_proof(date_from, date_to), date_from, date_to)
                .expect("proof rows match present bounds and omit absent bounds");
        }

        let mut wrong_type = rendered_controls("t03-roundtrip-browser", None, None);
        wrong_type["date_from_type"] = json!("text");
        assert!(roundtrip_controls(&wrong_type, "t03-roundtrip-browser", None, None).is_err());
        let mut invalid_control = rendered_controls("t03-roundtrip-browser", None, None);
        invalid_control["date_to_valid"] = json!(false);
        assert!(roundtrip_controls(&invalid_control, "t03-roundtrip-browser", None, None).is_err());
        assert!(
            roundtrip_html_proof(&rendered_proof(Some("2024-02-29"), None), None, None).is_err()
        );
        assert!(
            roundtrip_html_proof(
                &rendered_proof(Some("2024-02-28"), None),
                Some("2024-02-29"),
                None
            )
            .is_err()
        );
    }

    #[test]
    fn date_roundtrip_machine_documents_parse_owned_json_and_yaml_bytes() {
        let directory = TestEvidenceDir::new();
        let proof = json!({
            "status":"enforced",
            "requested":{"date_from":"2024-02-29","date_to":"2024-02-29"},
            "applied":{"date_from":"2024-02-29","date_to":"2024-02-29"},
            "enforced_by":["upstream:openalex"]
        });
        let json_body = json!({"meta":{"date_filter":proof}}).to_string();
        let (json_observation, json_artifacts) = machine_response_fixture(
            &directory.0,
            "date-roundtrip.json",
            "application/json; charset=utf-8",
            json_body.as_bytes(),
        );
        let decoded_json =
            roundtrip_machine_document(&json_observation, "json", &directory.0, &json_artifacts)
                .expect("decode the retained JSON response");
        let decoded_proof = &decoded_json["meta"]["date_filter"];
        validate_date_roundtrip_machine_proof(decoded_proof)
            .expect("decoded JSON contains the required bounded-date proof");
        for (pointer, invalid) in [
            ("/status", json!("unsupported")),
            ("/requested/date_from", json!("2024-02-28")),
            ("/applied/date_to", json!("2024-03-01")),
            ("/enforced_by", json!(["local:openalex"])),
        ] {
            let mut altered = decoded_proof.clone();
            *altered.pointer_mut(pointer).expect("proof field exists") = invalid;
            assert!(
                validate_date_roundtrip_machine_proof(&altered).is_err(),
                "accepted {pointer}"
            );
        }

        let yaml_body = concat!(
            "---\n",
            "meta:\n",
            "  date_filter:\n",
            "    status: enforced\n",
            "    requested:\n",
            "      date_from: \"2024-02-29\"\n",
            "      date_to: \"2024-02-29\"\n",
            "    applied:\n",
            "      date_from: \"2024-02-29\"\n",
            "      date_to: \"2024-02-29\"\n",
            "    enforced_by:\n",
            "      - upstream:openalex\n",
            "---\n",
            "## Results Summary\n",
            "\n",
            "Two bounded results.\n",
        )
        .as_bytes();
        let (yaml_observation, yaml_artifacts) = machine_response_fixture(
            &directory.0,
            "date-roundtrip.yaml.md",
            "text/vnd.yaml+markdown; charset=utf-8",
            yaml_body,
        );
        let decoded_yaml =
            roundtrip_machine_document(&yaml_observation, "yaml", &directory.0, &yaml_artifacts)
                .expect("decode the retained YAML front matter");
        validate_date_roundtrip_machine_proof(&decoded_yaml["meta"]["date_filter"])
            .expect("decoded YAML contains the required bounded-date proof");

        let mut mismatched_artifact = json_artifacts.clone();
        mismatched_artifact[0]["content_type"] = json!("text/plain");
        assert!(
            roundtrip_machine_document(
                &json_observation,
                "json",
                &directory.0,
                &mismatched_artifact
            )
            .is_err()
        );
        let mut altered_digest = json_observation;
        altered_digest["body_sha256"] = json!("0".repeat(64));
        assert!(
            roundtrip_machine_document(&altered_digest, "json", &directory.0, &json_artifacts)
                .is_err()
        );
    }

    #[test]
    fn date_roundtrip_call_log_matches_navigation_multiset_and_rejects_drift() {
        let base = "http://127.0.0.1:4321";
        let states = [
            ("t03-roundtrip-browser", 1, None, None, 2),
            (
                "t03-roundtrip-browser",
                2,
                Some("2024-02-29"),
                Some("2024-02-29"),
                1,
            ),
            ("t03-roundtrip-browser", 1, Some("2024-02-29"), None, 1),
            ("t03-roundtrip-browser", 1, None, Some("2024-02-29"), 1),
            (
                "t03-roundtrip-browser",
                1,
                Some("2024-02-29"),
                Some("2024-02-29"),
                1,
            ),
            (
                "t03-roundtrip-refined",
                1,
                Some("2024-02-29"),
                Some("2024-02-29"),
                4,
            ),
            (
                "t03-roundtrip-refined",
                2,
                Some("2024-02-29"),
                Some("2024-02-29"),
                2,
            ),
            (
                "t03-roundtrip-refined",
                1,
                Some("2024-02-29"),
                Some("2024-03-01"),
                1,
            ),
            ("t03-roundtrip-refined", 1, None, None, 1),
        ];
        let mut requests = Vec::new();
        let mut calls = Vec::new();
        for (query, page, date_from, date_to, count) in states {
            let url = roundtrip_url(query, page, date_from, date_to);
            for _ in 0..count {
                requests.push(url.clone());
                calls.push(call_line(query, "openalex", page, date_from, date_to));
            }
        }
        calls.push(call_line("unrelated-fixture-query", "brave", 1, None, None));
        let evidence = json!({"navigation_requests":requests});
        let log = calls.join("\n");
        let summary = validate_date_roundtrip_call_log(&log, &evidence, base)
            .expect("all navigation requests match actual OpenAlex service calls");
        assert_eq!(summary["matched"], true);
        assert_eq!(summary["navigation_requests"], 14);

        let mut missing_call = calls.clone();
        missing_call.remove(0);
        assert!(
            validate_date_roundtrip_call_log(&missing_call.join("\n"), &evidence, base).is_err()
        );
        let mut wrong_engine = calls.clone();
        wrong_engine[0] = call_line("t03-roundtrip-browser", "brave", 1, None, None);
        assert!(
            validate_date_roundtrip_call_log(&wrong_engine.join("\n"), &evidence, base).is_err()
        );
    }

    #[test]
    fn engine_call_keeps_arbitrary_precision_page_evidence() {
        let page = "9".repeat(4300);
        let line = format!(r#"{{"engine":"beta","page":{page}}}"#);
        assert!(validate_engine_call(&line, "unexpected engine").is_ok());
    }

    #[test]
    fn engine_call_still_rejects_unexpected_engine() {
        assert!(
            validate_engine_call(r#"{"engine":"untrusted","page":1}"#, "unexpected engine")
                .is_err()
        );
    }

    #[test]
    fn expected_http_error_statuses_are_explicit_and_loopback_bound() {
        let receipt = json!({
            "http_error_responses": [
                {"status":503,"url":"http://127.0.0.1:4321/search?q=failed"},
                {"status":400,"url":"http://127.0.0.1:4321/search?q=invalid"},
                {"status":403,"url":"http://127.0.0.1:4321/search?q=denied"}
            ],
            "http_error_console_statuses": [503,400,403]
        });
        assert!(
            validate_http_error_evidence(&receipt, "contract", "chromium", "http://127.0.0.1:4321")
                .is_ok()
        );

        let unexpected = json!({
            "http_error_responses": [
                {"status":400,"url":"http://127.0.0.1:4321/search?q=invalid"},
                {"status":403,"url":"http://127.0.0.1:4321/search?q=denied"},
                {"status":500,"url":"http://127.0.0.1:4321/search?q=unexpected"}
            ],
            "http_error_console_statuses": [400,403,500]
        });
        assert!(
            validate_http_error_evidence(
                &unexpected,
                "contract",
                "chromium",
                "http://127.0.0.1:4321"
            )
            .is_err()
        );

        // Measured: Playwright WebKit reports console statuses exactly like
        // Chromium, so the Chromium expectation applies to it as well.
        assert!(
            validate_http_error_evidence(&receipt, "contract", "webkit", "http://127.0.0.1:4321")
                .is_ok()
        );
        let gecko_shaped = json!({
            "http_error_responses": [
                {"status":503,"url":"http://127.0.0.1:4321/search?q=failed"},
                {"status":400,"url":"http://127.0.0.1:4321/search?q=invalid"},
                {"status":403,"url":"http://127.0.0.1:4321/search?q=denied"}
            ],
            "http_error_console_statuses": []
        });
        assert!(
            validate_http_error_evidence(
                &gecko_shaped,
                "contract",
                "webkit",
                "http://127.0.0.1:4321"
            )
            .is_err()
        );
        assert!(
            validate_http_error_evidence(
                &gecko_shaped,
                "contract",
                "firefox",
                "http://127.0.0.1:4321"
            )
            .is_ok()
        );

        for url in [
            "http://127.0.0.1:43210/search?q=wrong-port",
            "http://127.0.0.1:4321@evil.example/search?q=userinfo-host",
            "http://user@127.0.0.1:4321/search?q=userinfo-loopback",
        ] {
            let lookalike_origin = json!({
                "http_error_responses": [
                    {"status":400,"url":"http://127.0.0.1:4321/search?q=invalid"},
                    {"status":403,"url":"http://127.0.0.1:4321/search?q=denied"},
                    {"status":503,"url":url}
                ],
                "http_error_console_statuses": [400,403,503]
            });
            assert!(
                validate_http_error_evidence(
                    &lookalike_origin,
                    "contract",
                    "chromium",
                    "http://127.0.0.1:4321"
                )
                .is_err()
            );
        }
    }
}

#[cfg(test)]
#[test]
fn interaction_evidence_rejects_omitted_samples_forged_percentile_and_weak_controls() {
    let valid = tests::passing_budgets();
    for pointer in [
        "/interaction_evidence/samples/0/trusted",
        "/interaction_evidence/samples/0/state_observed",
        "/interaction_evidence/controls/absent_input_rejected",
        "/interaction_evidence/controls/absent_state_rejected",
        "/interaction_evidence/controls/untrusted_input_rejected",
    ] {
        let mut changed = valid.clone();
        *changed.pointer_mut(pointer).unwrap() = json!(false);
        assert!(validate_budgets("contract", "chromium", &changed).is_err());
    }
    let mut changed = valid.clone();
    changed["interaction_evidence"]["samples"]
        .as_array_mut()
        .unwrap()
        .pop();
    assert!(validate_budgets("contract", "chromium", &changed).is_err());
    let mut changed = valid.clone();
    changed["interaction_p95_ms"]["observed"] = json!(1.0);
    assert!(validate_budgets("contract", "chromium", &changed).is_err());
    let mut changed = valid;
    changed["interaction_evidence"]["controls"]["slow_ms"] = json!(99.0);
    assert!(validate_budgets("contract", "chromium", &changed).is_err());
}
