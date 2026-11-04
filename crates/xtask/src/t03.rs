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
const SOURCES: [(&str, &[u8]); 12] = [
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
fn validate_budgets(scenario: &str, budgets: &Value) -> Result<()> {
    if scenario != "contract" {
        return exact_keys(budgets, &[], "error-scenario budgets");
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
            &format!("browser budget failed or changed: {name}"),
        )?;
    }
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
            "landing or results CLS exceeds portal budget",
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
    validate_http_error_evidence(&receipt, scenario, base_url)?;
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
        exact_keys(
            artifact,
            &["path", "sha256", "bytes", "kind", "width", "height"],
            "browser artifact",
        )?;
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
    validate_budgets(scenario, &receipt["budgets"])?;
    Ok(json!({"completion":completion,"receipt":receipt}))
}

fn validate_http_error_evidence(receipt: &Value, scenario: &str, base_url: &str) -> Result<()> {
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
    need(
        statuses == expected && console_statuses == expected,
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
        rows.len() == SCENARIOS.len(),
        "browser scenario inventory missing or extra",
    )?;
    let mut names = BTreeSet::new();
    for row in rows {
        let name = row["scenario"]
            .as_str()
            .ok_or("T03 scenario name missing")?;
        need(names.insert(name), "duplicate browser scenario")?;
        need(SCENARIOS.contains(&name), "unexpected browser scenario")?;
        need(
            row["source_sha256"] == source_sha,
            "stale browser suite input",
        )?;
        validate_cleanup(&row["terminal"])?;
    }
    need(
        names == SCENARIOS.into_iter().collect(),
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
    let rows:Vec<_>=SCENARIOS.into_iter().map(|scenario|json!({"scenario":scenario,"source_sha256":"synthetic-validator-input","terminal":{"stop_sent":true,"child_joined":true,"child_exit_success":true,"tree_retired":true,"stdout_eof":true,"stdout_tail_bytes":0,"stderr_bytes":0}})).collect();
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
    for scenario in SCENARIOS {
        let scenario_dir = run.join(format!("browser-{scenario}"));
        let mut fixture = Fixture::start(&binary, &binary_sha, scenario, scenario_dir)?;
        records.push(json!({"plan":"T03","role":"browser-fixture-start","scenario":scenario,"startup":fixture.startup}));
        let browser_dir = fixture.out.join("browser");
        fs::create_dir(&browser_dir).map_err(|e| e.to_string())?;
        let mut node = Command::new("node");
        node.arg(CLIENT)
            .env("T03_SCENARIO", scenario)
            .env("T03_BROWSER_BASE_URL", &fixture.base_url)
            .env("T03_EVIDENCE_DIR", &browser_dir);
        let browser = capture(root, node, Duration::from_secs(600), records);
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
        let browser = validate_browser(
            &browser_dir,
            &browser?,
            scenario,
            &fixture.base_url,
            &client_sha,
        )?;
        browser_rows.push(json!({"scenario":scenario,"source_sha256":source_sha,"startup":fixture.startup,"terminal":terminal,"browser":browser}));
    }
    let negative = controls(&browser_rows, &source_sha)?;
    need(
        sources(root)? == before && hash(&regular(&binary, 256 * 1024 * 1024)?) == binary_sha,
        "source or executed fixture image changed during gate",
    )?;
    records.push(json!({"plan":"T03","role":"native-browser-acceptance","source_inputs":before,"source_sha256":source_sha,"server_binary":{"path":binary,"sha256":binary_sha},"http_tests":HTTP_TESTS,"http_receipts":104,"http_processes":12,"browser_scenarios":browser_rows,"controls":negative,"evidence_directory":run,"scope":"real native HTTP plus Chromium deterministic browser journeys and measured synthetic budgets; actual Safari, manual visual/assistive technology and 200% zoom acceptance remain separate"}));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{validate_engine_call, validate_http_error_evidence};
    use serde_json::json;

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
            validate_http_error_evidence(&receipt, "contract", "http://127.0.0.1:4321").is_ok()
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
            validate_http_error_evidence(&unexpected, "contract", "http://127.0.0.1:4321").is_err()
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
                    "http://127.0.0.1:4321"
                )
                .is_err()
            );
        }
    }
}
