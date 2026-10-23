//! Permanent source-bound A13 gate. Executes Rust fixtures only, never originals/providers.
use crate::{Result, command};
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
#[cfg(test)]
pub(crate) use source_amendment::causal_successor;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};
#[path = "../../../docs/rust-rewrite/a13/source-amendments/validator.rs"]
pub(crate) mod source_amendment;
const QUALIFICATION_SHA: &str = "e2d9249fbadd0eca06a10215f796defc5845722c63fcf9f409ab9eac8a85f74e";
const PUBCHEM_OWNER_CLOCK_TEST: &str = "adapters::clinicaltrials::resource_tests::pubchem_second_request_renews_500ms_budget_on_the_same_manual_owner";
const QUALIFICATION: &str = "docs/rust-rewrite/a13/qualification-v1.json";
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Pin {
    path: String,
    sha256: String,
    bytes: usize,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Suite {
    package: String,
    target: String,
    kind: String,
    names: Vec<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Selector {
    id: String,
    name: String,
    package: String,
    target: String,
    kind: String,
    argv: Vec<String>,
    source: String,
    source_sha256: String,
    source_line: usize,
    inline_payloads: Vec<String>,
    markers: Vec<String>,
    child: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Qualification {
    schema_version: u32,
    plan_id: String,
    evidence_kind: String,
    source_provenance: Value,
    sources: Vec<Pin>,
    suites: Vec<Suite>,
    selectors: Vec<Selector>,
}
fn require(ok: bool, message: &str) -> Result<()> {
    if ok { Ok(()) } else { Err(message.into()) }
}
fn digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
pub(super) fn check_source_pin(
    root: &Path,
    container_path: &str,
    container_sha: &str,
    path: &str,
    expected_sha: &str,
    expected_bytes: Option<usize>,
) -> Result<()> {
    if let Some(result) = crate::w06_source_amendment::verify_shared(
        root,
        container_path,
        container_sha,
        path,
        expected_sha,
        expected_bytes,
    ) {
        return result;
    }
    source_amendment::verify_current(
        root,
        container_path,
        container_sha,
        path,
        expected_sha,
        expected_bytes,
    )
}
fn counter<'a>(items: impl IntoIterator<Item = &'a str>) -> BTreeMap<&'a str, usize> {
    let mut counts = BTreeMap::new();
    for s in items {
        *counts.entry(s).or_insert(0) += 1;
    }
    counts
}
fn same(actual: &[String], expected: &[String], label: &str) -> Result<()> {
    require(
        counter(actual.iter().map(String::as_str)) == counter(expected.iter().map(String::as_str)),
        label,
    )
}
fn strings(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| (*s).into()).collect()
}
fn listed(output: &str) -> Result<Vec<String>> {
    let mut names = Vec::new();
    let mut summaries = 0;
    for line in output.lines().filter(|s| !s.is_empty()) {
        if let Some(name) = line.strip_suffix(": test") {
            require(!name.is_empty(), "A13 empty compiled name")?;
            names.push(name.to_owned());
        } else {
            require(
                line == format!("{} tests, 0 benchmarks", names.len()),
                "A13 unrecognized compiled-list output",
            )?;
            summaries += 1;
        }
    }
    require(
        summaries == 1
            && !names.is_empty()
            && names.iter().collect::<BTreeSet<_>>().len() == names.len(),
        "A13 missing/duplicate compiled names/summary",
    )?;
    Ok(names)
}
// No dependency or lossy decoder: closed canonical RFC4648 alphabet/padding/unused bits.
fn base64(raw: &str) -> Result<Vec<u8>> {
    require(
        raw.len() <= 4 * (1024usize * 1024).div_ceil(3) && raw.len().is_multiple_of(4),
        "A13 raw pipe encoding cap/length",
    )?;
    fn digit(b: u8) -> Result<u8> {
        match b {
            b'A'..=b'Z' => Ok(b - b'A'),
            b'a'..=b'z' => Ok(b - b'a' + 26),
            b'0'..=b'9' => Ok(b - b'0' + 52),
            b'+' => Ok(62),
            b'/' => Ok(63),
            _ => Err("A13 noncanonical Base64 alphabet".into()),
        }
    }
    let mut bytes = Vec::with_capacity(raw.len() / 4 * 3);
    for (index, part) in raw.as_bytes().as_chunks::<4>().0.iter().enumerate() {
        let a = digit(part[0])?;
        let b = digit(part[1])?;
        bytes.push(a << 2 | b >> 4);
        if part[2] == b'=' {
            require(
                part[3] == b'=' && b & 15 == 0 && index + 1 == raw.len() / 4,
                "A13 noncanonical double padding",
            )?;
        } else {
            let c = digit(part[2])?;
            bytes.push(b << 4 | c >> 2);
            if part[3] == b'=' {
                require(
                    c & 3 == 0 && index + 1 == raw.len() / 4,
                    "A13 noncanonical single padding",
                )?;
            } else {
                bytes.push(c << 6 | digit(part[3])?);
            }
        }
    }
    require(bytes.len() <= 1024 * 1024, "A13 decoded pipe exceeds cap")?;
    Ok(bytes)
}
fn payload_allowed(payload: &str, allowed: &[String]) -> bool {
    allowed.iter().any(|p| {
        if let Some(prefix) = p.strip_suffix("<canonical-base64>") {
            payload
                .strip_prefix(prefix)
                .is_some_and(|s| base64(s).is_ok())
        } else {
            payload == p
        }
    })
}
// Exactly one named harness start/terminal; one source-owned split is permitted.
fn execution(output: &str, name: &str, allowed: &[String], filtered: usize) -> Result<()> {
    let lines: Vec<_> = output.lines().collect();
    let prefix = format!("test {name} ... ");
    let named: Vec<_> = lines
        .iter()
        .enumerate()
        .filter(|(_, s)| s.starts_with("test ") && !s.starts_with("test result:"))
        .collect();
    require(
        named.len() == 1,
        "A13 one exact named start/terminal required",
    )?;
    let (start, line) = named[0];
    let suffix = line
        .strip_prefix(&prefix)
        .ok_or("A13 wrong harness name/framing")?;
    let bare: Vec<_> = lines
        .iter()
        .enumerate()
        .filter(|(_, s)| **s == "ok")
        .collect();
    if suffix == "ok" {
        require(bare.is_empty(), "A13 unsolicited bare terminal")?;
    } else {
        require(
            payload_allowed(suffix, allowed) && bare.len() == 1 && bare[0].0 > start,
            "A13 invalid source-owned split terminal",
        )?;
    }
    require(
        lines.iter().filter(|s| **s == "running 1 test").count() == 1
            && !lines.iter().any(|s| matches!(*s, "FAILED" | "ignored")),
        "A13 running/failure/ignore framing",
    )?;
    let summaries: Vec<_> = lines
        .iter()
        .enumerate()
        .filter(|(_, s)| s.starts_with("test result:"))
        .collect();
    require(summaries.len() == 1, "A13 one harness summary required")?;
    let (summary_index, summary) = summaries[0];
    require(
        summary_index > start,
        "A13 summary must follow named terminal/start",
    )?;
    if let Some((index, _)) = bare.first() {
        require(*index < summary_index, "A13 terminal after summary")?;
    }
    let expected = format!(
        "test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; {filtered} filtered out; finished in "
    );
    let duration = summary
        .strip_prefix(&expected)
        .and_then(|s| s.strip_suffix('s'))
        .ok_or("A13 exact successful summary absent")?;
    let numeric = match duration.split_once('.') {
        Some((whole, fractional)) => {
            !whole.is_empty()
                && !fractional.is_empty()
                && whole
                    .bytes()
                    .chain(fractional.bytes())
                    .all(|b| b.is_ascii_digit())
        }
        None => !duration.is_empty() && duration.bytes().all(|b| b.is_ascii_digit()),
    };
    require(numeric, "A13 summary duration grammar")
}
fn markers(output: &str, name: &str, allowed: &[String]) -> Result<Vec<String>> {
    let prefix = format!("test {name} ... ");
    let mut found = Vec::new();
    for line in output.lines().filter(|s| s.contains("A13_")) {
        let marker = if line.starts_with("A13_") {
            line
        } else {
            let p = line
                .strip_prefix(&prefix)
                .ok_or("A13 marker has unrelated prefix")?;
            require(
                payload_allowed(p, allowed),
                "A13 unapproved named split marker",
            )?;
            p
        };
        require(
            marker.starts_with("A13_") && !marker.contains('\r'),
            "A13 invalid marker framing",
        )?;
        found.push(marker.into());
    }
    Ok(found)
}
fn one<'a>(all: &'a [String], prefix: &str) -> Result<&'a str> {
    let prefix = format!("{prefix} ");
    let values: Vec<_> = all.iter().filter_map(|s| s.strip_prefix(&prefix)).collect();
    require(
        values.len() == 1,
        "A13 one child raw/lifecycle record required",
    )?;
    Ok(values[0])
}
fn child(all: &[String], spec: &Selector, records: &mut Vec<Value>) -> Result<Vec<String>> {
    let engine = spec.child.as_deref().ok_or("A13 child engine absent")?;
    let prefix = if engine == "dehashed" {
        "A13_DEHASHED"
    } else {
        "A13_REMAINING"
    };
    let out = base64(one(all, &format!("{prefix}_CHILD_STDOUT_BASE64"))?)?;
    let err = base64(one(all, &format!("{prefix}_CHILD_STDERR_BASE64"))?)?;
    let lifecycle: Value = serde_json::from_str(one(all, &format!("{prefix}_CHILD_LIFECYCLE"))?)
        .map_err(|e| e.to_string())?;
    require(
        lifecycle["status_code"] == 0
            && lifecycle["wait_succeeded"] == true
            && lifecycle["success"] == true
            && lifecycle["forced_kill"] == false
            && lifecycle["stdout_eof"] == true
            && lifecycle["stderr_eof"] == true,
        "A13 child actual exit/EOF/forced-kill refusal",
    )?;
    require(
        lifecycle["stdout_bytes"].as_u64() == Some(out.len() as u64)
            && lifecycle["stderr_bytes"].as_u64() == Some(err.len() as u64)
            && out.len() + err.len() <= 1024 * 1024,
        "A13 child raw lengths/cap",
    )?;
    if engine == "dehashed" {
        for key in ["stdout_read", "stderr_read"] {
            let v = &lifecycle[key];
            require(
                v["eof"] == true
                    && v.get("read_error_kind").is_some_and(Value::is_null)
                    && [
                        "limit_hit",
                        "timed_out",
                        "task_cancelled",
                        "task_panicked",
                        "incomplete",
                    ]
                    .iter()
                    .all(|k| v[*k] == false),
                "A13 DeHashed reader not completely joined/EOF",
            )?;
        }
    } else {
        require(
            lifecycle["engine"] == engine
                && lifecycle["stdout_outcome"] == "eof"
                && lifecycle["stderr_outcome"] == "eof",
            "A13 remaining joined child reader/engine",
        )?;
    }
    let stdout = std::str::from_utf8(&out).map_err(|_| "A13 child stdout not lossless UTF8")?;
    let own = if engine == "dehashed" {
        "A13_DEHASHED_PRIVATE_CHILD_COMPLETE".into()
    } else {
        format!("A13_REMAINING_PRIVATE_CHILD_COMPLETE {engine}")
    };
    let filtered = match engine {
        "dehashed" => 20,
        "hibp" => 19,
        "intelx" => 21,
        _ => return Err("A13 unknown child engine".into()),
    };
    execution(stdout, &spec.name, std::slice::from_ref(&own), filtered)?;
    let nested = markers(stdout, &spec.name, std::slice::from_ref(&own))?;
    let mut expected = vec![own];
    if engine != "dehashed" {
        expected.push(format!("A13_REMAINING_CASE_COMPLETE {}", spec.name));
    }
    same(&nested, &expected, "A13 exact child-only marker Counter")?;
    // The enclosing command retains canonical Base64 bytes verbatim in evidence.
    records.push(json!({"A13_child":engine,"selector":spec.name,"lifecycle":lifecycle,"stdout_sha256":digest(&out),"stderr_sha256":digest(&err),"scope":"actual returned child status and joined pipe evidence; no OS process-family absence claim"}));
    Ok(vec![
        format!(
            "{prefix}_CHILD_STDOUT_BASE64 {}",
            one(all, &format!("{prefix}_CHILD_STDOUT_BASE64"))?
        ),
        format!(
            "{prefix}_CHILD_STDERR_BASE64 {}",
            one(all, &format!("{prefix}_CHILD_STDERR_BASE64"))?
        ),
        format!(
            "{prefix}_CHILD_LIFECYCLE {}",
            one(all, &format!("{prefix}_CHILD_LIFECYCLE"))?
        ),
    ])
}
fn validate(q: &Qualification) -> Result<()> {
    require(
        q.schema_version == 1
            && q.plan_id == "A13"
            && q.evidence_kind == "STATIC_SOURCE_BOUND_NATIVE_COMPONENT_GATE",
        "A13 qualification identity",
    )?;
    require(
        q.source_provenance["original_backend_executed"] == false
            && q.source_provenance["provider_calls"] == 0,
        "A13 original/provider evidence mislabeled",
    )?;
    require(
        q.selectors.len() == 75
            && q.selectors
                .iter()
                .map(|s| &s.id)
                .collect::<BTreeSet<_>>()
                .len()
                == 75
            && q.selectors
                .iter()
                .map(|s| &s.name)
                .collect::<BTreeSet<_>>()
                .len()
                == 75,
        "A13 exact75 unique selector identities",
    )?;
    require(
        q.suites.len() == 5,
        "A13 five complete compiled suites required",
    )?;
    let expected = [
        ("ssx-infra", "lib", "ssx_infra", 253),
        ("ssx-json", "lib", "ssx_json", 16),
        ("ssx-infra", "test", "a13_dehashed", 21),
        ("ssx-infra", "test", "a13_hibp", 20),
        ("ssx-infra", "test", "a13_intelx", 22),
    ];
    for (package, kind, target, count) in expected {
        let found: Vec<_> = q
            .suites
            .iter()
            .filter(|s| s.package == package && s.kind == kind && s.target == target)
            .collect();
        require(
            found.len() == 1
                && found[0].names.len() == count
                && found[0].names.iter().collect::<BTreeSet<_>>().len() == count,
            "A13 exact suite membership/cardinality",
        )?;
    }
    require(
        q.source_provenance["full_library_linux_only_names"]
            == json!([
                "process_pool::cleanup_tests::held_writer_refuses_direct_exec_but_script_interpreter_preserves_protocol_and_cleanup"
            ]),
        "A13 exact one source-declared Linux library extension",
    )?;
    let mut total = Vec::new();
    let mut children = Vec::new();
    for spec in &q.selectors {
        let suite = q
            .suites
            .iter()
            .find(|s| s.package == spec.package && s.target == spec.target && s.kind == spec.kind)
            .ok_or("A13 selector outside compiled suites")?;
        require(
            suite.names.contains(&spec.name),
            "A13 selector missing from closed compiled suite",
        )?;
        let mut argv = strings(&["test"]);
        // Preserve the reviewed order/capture flags, never arbitrary programs/options.
        if spec.id.starts_with("dehashed-") {
            argv.extend(strings(&["--locked", "--offline"]));
        } else {
            argv.extend(strings(&["--offline", "--locked"]));
        }
        argv.extend(strings(&["-p", &spec.package]));
        if spec.kind == "lib" {
            argv.push("--lib".into());
        } else {
            argv.extend(strings(&["--test", &spec.target]));
        }
        argv.extend(strings(&[&spec.name, "--", "--exact"]));
        if spec.id.starts_with("dehashed-") {
            argv.extend(strings(&["--show-output", "--test-threads=1"]));
        } else {
            argv.push("--nocapture".into());
        }
        require(argv == spec.argv, "A13 reviewed selector argv changed")?;
        require(
            !spec
                .markers
                .iter()
                .any(|s| !s.starts_with("A13_") || s.contains(['\r', '\n'])),
            "A13 invalid closed marker",
        )?;
        total.extend(spec.markers.iter().cloned());
        if let Some(engine) = &spec.child {
            children.push(format!("{engine}:{}", spec.name));
        }
    }
    require(
        total.len() == 128 && total.iter().collect::<BTreeSet<_>>().len() == 128,
        "A13 76+3 DeHashed /42+7 remaining parent marker inventory",
    )?;
    same(
        &children,
        &strings(&[
            "dehashed:private_literal_loopback_complete_http_contract",
            "hibp:hibp_private_literal_http_contract",
            "intelx:intelx_private_literal_two_phase_http",
        ]),
        "A13 three actual private child selectors",
    )?;
    Ok(())
}
// Preserve the full common214+33 inventory; this single source-gated Linux test is additional.
fn compiled_suite_names(suite: &Suite) -> Vec<String> {
    let mut names = suite.names.clone();
    if suite.package == "ssx-infra" && suite.kind == "lib" && suite.target == "ssx_infra" {
        names.push(
            "http::pypi_operation_timeout_tests::pypi_uses_fresh_per_operation_timeouts_on_the_caller_pool"
                .into(),
        );
        names.push(
            "adapters::clinicaltrials::deadline_tests::queue_time_consumes_absolute_context_instead_of_original_request_timeout"
                .into(),
        );
        names.push(
            "adapters::a07_matrix_replay::actual_source_matrix_preserves_requests_and_full_normalized_results"
                .into(),
        );
        names.push(PUBCHEM_OWNER_CLOCK_TEST.into());
        names.push("adapters::html_helper_tests::cold_bootstrap_expires_without_spending_search_deadline_or_leaking_cookies".into());
    }
    if suite.package == "ssx-infra" && suite.kind == "lib" && suite.target == "ssx_infra" {
        names.push("search::deadline_drain_tests::owner_manual_clock_preserves_completion_before_original_one_ms_deadline".into());
        names.push("search::deadline_drain_tests::owner_manual_clock_rejects_completion_at_original_one_ms_deadline".into());
        names.push("search::deadline_drain_tests::owner_manual_clock_rejects_completion_after_original_one_ms_deadline".into());
        names.push("search::deadline_drain_tests::renewed_context_mutant_does_not_match_original_absolute_deadline".into());
        names.push("search::deadline_drain_tests::manual_owner_clock_does_not_pause_unrelated_real_tokio_timeouts".into());
        names.push("http::f10_peer_timeout_tests::actual_peer_started_timeout_records_failure_with_null_latency".into());
        // Exact A04 integration-to-private-unit relocation; original capture inventory retained.
        names.push("http::a04_broad_api::replay_headers_accumulate_fragments_and_reject_early_eof_and_oversize".into());
        names.push("http::a04_broad_api::actual_source_edge_matrix_replays_full_output_and_ordered_requests".into());
        names.push("http::a04_broad_api::every_complete_source_record_replays_full_normalized_output_and_wire_contract".into());
        names.push("http::a04_broad_api::unrepresentable_reddit_subreddit_values_are_explicit_typed_boundaries".into());
        names.push(
            "http::a04_broad_api::batch_registration_alone_controls_runtime_availability".into(),
        );
        names.push(
            "http::a04_broad_api::brave_web_answers_and_images_are_parsed_from_loopback_http"
                .into(),
        );
        names.push("http::a04_broad_api::wikipedia_two_stages_preserve_request_order_corrections_and_infobox".into());
        names.push(
            "http::a04_broad_api::wikipedia_no_titles_and_missing_page_ids_match_captured_source"
                .into(),
        );
        names.push("http::a04_broad_api::reddit_token_precedence_auth_host_and_subreddit_routing_are_explicit".into());
        names.push(
            "http::a04_broad_api::reddit_default_oauth_rewrite_targets_source_host_without_network"
                .into(),
        );
        names.push(
            "http::a04_broad_api::wikipedia_deadline_is_per_operation_and_session_is_reused".into(),
        );
        names.push("http::a04_broad_api::reddit_listing_preserves_source_filter_content_dates_and_thumbnail_rules".into());
        names.push("http::a04_broad_api::reddit_empty_permalink_prefix_and_python_truthiness_follow_captured_source".into());
        names.push("http::a04_broad_api::reddit_subreddit_missing_empty_and_path_special_inputs_match_source_urls".into());
        names.push(
            "http::a04_broad_api::all_three_classify_http_errors_bad_json_and_timeout".into(),
        );
        names.push("http::a04_broad_api::dropped_search_cancels_outstanding_loopback_request_for_each_adapter".into());
        names.push(
            "http::a04_broad_api::empty_and_wrong_shape_responses_keep_engine_specific_outcomes"
                .into(),
        );
        names.push("http::a04_broad_api::missing_brave_credentials_fail_before_dispatch_and_reddit_401_is_unavailable".into());
        names.push("http::a04_broad_api::nonfinite_source_records_preserve_full_source_list_and_native_typed_refusal".into());
        names.push("http::a04_broad_api::representation_boundaries_preserve_source_outcomes_and_refuse_whole_native_response".into());
        names.push("http::a04_broad_api::source_json_byte_numeric_and_unicode_contracts_replay_every_record".into());
        names.push("http::a04_broad_api::subreddit_scopes_keep_shared_service_dispatch_cache_and_flights_separate".into());
        names.push("http::a04_broad_api::timeout_replay_aligns_accumulated_virtual_offset_before_native_request".into());
        names.push("http::loopback_timeout_replay_controls::manual_network_owner_rejects_remote_and_service_context".into());
        names.push("http::loopback_timeout_replay_controls::manual_network_session_rejects_different_endpoint_before_wire".into());
        // Add the exact #1032 controls without changing the historical suite.
        names.push("http::expired_startup_tests::system_expired_startup_rejects_even_while_its_timer_is_pending".into());
        names.push("http::expired_startup_tests::manual_expired_startup_includes_exact_deadline_and_past_deadline".into());
        names.push("http::expired_startup_tests::expired_startup_preserves_stop_then_cancel_then_deadline_precedence".into());
        names.push("http::expired_startup_tests::unexpired_startup_dispatches_with_system_and_manual_owner_clocks".into());
    }
    if cfg!(target_os = "linux")
        && suite.package == "ssx-infra"
        && suite.kind == "lib"
        && suite.target == "ssx_infra"
    {
        names.push("process_pool::cleanup_tests::held_writer_refuses_direct_exec_but_script_interpreter_preserves_protocol_and_cleanup".into());
    }
    names
}
pub fn check(root: &Path, records: &mut Vec<Value>) -> Result<()> {
    #[cfg(test)]
    causal_successor::control_bridge(root)?;
    let bytes = fs::read(root.join(QUALIFICATION)).map_err(|e| e.to_string())?;
    require(
        digest(&bytes) == QUALIFICATION_SHA,
        "A13 permanent qualification changed; review before repin",
    )?;
    let q: Qualification = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    validate(&q)?;
    let mut source_paths = BTreeSet::new();
    for pin in &q.sources {
        require(
            !Path::new(&pin.path).is_absolute()
                && !pin.path.split('/').any(|s| s == "..")
                && source_paths.insert(&pin.path),
            "A13 duplicate/unsafe source pin",
        )?;
        check_source_pin(
            root,
            QUALIFICATION,
            &digest(&bytes),
            &pin.path,
            &pin.sha256,
            Some(pin.bytes),
        )
        .map_err(|_| "A13 source/provenance drift".to_owned())?;
    }
    require(
        source_paths.len() >= 40,
        "A13 source closure unexpectedly small",
    )?;
    for spec in &q.selectors {
        require(
            q.sources
                .iter()
                .any(|p| p.path == spec.source && p.sha256 == spec.source_sha256),
            "A13 selector source not pinned",
        )?;
        let source = fs::read_to_string(root.join(&spec.source)).map_err(|e| e.to_string())?;
        let name = spec
            .name
            .rsplit("::")
            .next()
            .ok_or("A13 source function name")?;
        require(
            spec.source_line > 0
                && source
                    .lines()
                    .nth(spec.source_line - 1)
                    .is_some_and(|line| line.contains(&format!("fn {name}("))),
            "A13 exact source function location drift",
        )?;
    }
    // Standalone gate must produce real helper images before any fixture can use them.
    command(
        root,
        "cargo",
        &[
            "build",
            "--offline",
            "--locked",
            "--workspace",
            "--all-targets",
        ],
        records,
    )?;
    for suite in &q.suites {
        let mut args = strings(&["test", "--offline", "--locked", "-p", &suite.package]);
        if suite.kind == "lib" {
            args.push("--lib".into());
        } else {
            args.extend(strings(&["--test", &suite.target]));
        }
        args.extend(strings(&["--", "--list"]));
        let output = command(
            root,
            "cargo",
            &args.iter().map(String::as_str).collect::<Vec<_>>(),
            records,
        )?;
        same(
            &listed(&output)?,
            &compiled_suite_names(suite),
            "A13 exact compiled inventory missing/extra/duplicate",
        )?;
    }
    for spec in &q.selectors {
        let output = command(
            root,
            "cargo",
            &spec.argv.iter().map(String::as_str).collect::<Vec<_>>(),
            records,
        )?;
        let suite = q
            .suites
            .iter()
            .find(|s| s.package == spec.package && s.target == spec.target && s.kind == spec.kind)
            .ok_or("A13 suite disappeared")?;
        execution(
            &output,
            &spec.name,
            &spec.inline_payloads,
            compiled_suite_names(suite).len() - 1,
        )?;
        let actual = markers(&output, &spec.name, &spec.inline_payloads)?;
        let mut expected = spec.markers.clone();
        if spec.child.is_some() {
            expected.extend(child(&actual, spec, records)?);
        }
        same(
            &actual,
            &expected,
            "A13 complete parent markers/child raw fields missing/extra/duplicate",
        )?;
        records.push(json!({"A13_native_completion":spec.name,"parent_markers":spec.markers,"original_execution":false,"provider_calls":0}));
    }
    let output = command(
        root,
        "cargo",
        &[
            "test",
            "--offline",
            "--locked",
            "-p",
            "xtask",
            "a13::tests::",
            "--",
            "--test-threads=1",
        ],
        records,
    )?;
    validate_controls(&output)?;
    Ok(())
}
fn validate_controls(output: &str) -> Result<()> {
    let expected = [
        "a13::tests::strict_harness_framing",
        "a13::tests::canonical_base64",
        "a13::tests::whole_source_marker_framing",
        "a13::tests::child_status_pipes_and_nested_identity",
        "a13::tests::compiled_inventory_is_closed",
    ];
    let names: Vec<String> = output
        .lines()
        .filter_map(|s| {
            s.strip_prefix("test ")
                .and_then(|s| s.strip_suffix(" ... ok"))
                .map(str::to_owned)
        })
        .collect();
    same(
        &names,
        &strings(&expected),
        "A13 five actual validator controls",
    )?;
    require(
        output
            .lines()
            .filter(|s| s.starts_with("test result:"))
            .count()
            == 1
            && output.lines().any(|s| {
                s.starts_with("test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; ")
            }),
        "A13 validator summary",
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    fn output(name: &str, payload: &str, split: bool) -> String {
        let terminal = if split {
            format!("test {name} ... {payload}\nok\n")
        } else {
            format!("test {name} ... ok\n{payload}\n")
        };
        format!(
            "running 1 test\n{terminal}test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s\n"
        )
    }
    #[test]
    fn strict_harness_framing() {
        let good = output("selected", "", false);
        assert!(execution(&good, "selected", &[], 0).is_ok());
        for bad in [
            good.replace("selected", "other"),
            good.replace("... ok", "... arbitrary"),
            good.replace("1 passed", "0 passed"),
            good.replace("0 ignored", "1 ignored"),
            good.replace(" ... ok", " ... FAILED"),
            format!("{good}{good}"),
            format!("{good}ok\n"),
            good.replace("0 filtered", "1 filtered"),
        ] {
            assert!(execution(&bad, "selected", &[], 0).is_err(), "{bad}");
        }
        let summary = good
            .lines()
            .find(|line| line.starts_with("test result:"))
            .unwrap();
        let premature_summary = format!("running 1 test\n{summary}\ntest selected ... ok\n");
        assert!(execution(&premature_summary, "selected", &[], 0).is_err());
        for invalid_duration in [".", ".00", "0.", "0..0", "", "nan", "inf", "1e2"] {
            assert!(
                execution(
                    &good.replace("0.00s", &format!("{invalid_duration}s")),
                    "selected",
                    &[],
                    0
                )
                .is_err()
            );
        }
        let allowed = strings(&["A13_REMAINING_CASE_COMPLETE selected"]);
        let split = output("selected", &allowed[0], true);
        assert!(execution(&split, "selected", &allowed, 0).is_ok());
        assert!(
            execution(
                &split.replace("A13_REMAINING", "junk A13_REMAINING"),
                "selected",
                &allowed,
                0
            )
            .is_err()
        );
    }
    #[test]
    fn canonical_base64() {
        for (raw, bytes) in [
            ("", b"".as_slice()),
            ("AA==", &[0]),
            ("AQI=", &[1, 2]),
            ("YWJj", b"abc"),
        ] {
            assert_eq!(base64(raw).unwrap(), bytes);
        }
        for raw in [
            "AB==", "AAF=", "A===", "====", "AA==AAAA", "AA\n=", "YWJj ", "____",
        ] {
            assert!(base64(raw).is_err(), "{raw}");
        }
    }
    #[test]
    fn whole_source_marker_framing() {
        let allowed = strings(&["A13_REMAINING_CASE_COMPLETE selected"]);
        assert_eq!(
            markers(&output("selected", &allowed[0], true), "selected", &allowed).unwrap(),
            allowed
        );
        assert!(
            markers(
                "junk A13_REMAINING_CASE_COMPLETE selected",
                "selected",
                &allowed
            )
            .is_err()
        );
        assert!(
            markers(
                "test other ... A13_REMAINING_CASE_COMPLETE selected",
                "selected",
                &allowed
            )
            .is_err()
        );
        assert!(same(&strings(&["x", "x"]), &strings(&["x"]), "duplicate").is_err());
        assert!(same(&strings(&["unknown"]), &strings(&["owned"]), "unknown").is_err());
    }
    #[test]
    fn child_status_pipes_and_nested_identity() {
        // Use the exact reader seam with a minimal real harness encoding, not a fake process claim.
        let spec = Selector {
            id: "child".into(),
            name: "selected".into(),
            package: "ssx-infra".into(),
            target: "a13_hibp".into(),
            kind: "test".into(),
            argv: vec![],
            source: String::new(),
            source_sha256: String::new(),
            source_line: 1,
            inline_payloads: vec![],
            markers: vec![],
            child: Some("hibp".into()),
        };
        let text = output(
            "selected",
            "A13_REMAINING_PRIVATE_CHILD_COMPLETE hibp\nA13_REMAINING_CASE_COMPLETE selected",
            false,
        )
        .replace("0 filtered out", "19 filtered out");
        fn encode(bytes: &[u8]) -> String {
            const A: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
            let mut out = String::new();
            for p in bytes.chunks(3) {
                out.push(A[(p[0] >> 2) as usize] as char);
                out.push(
                    A[(((p[0] & 3) << 4) | (p.get(1).copied().unwrap_or(0) >> 4)) as usize] as char,
                );
                out.push(if p.len() > 1 {
                    A[(((p[1] & 15) << 2) | (p.get(2).copied().unwrap_or(0) >> 6)) as usize] as char
                } else {
                    '='
                });
                out.push(if p.len() > 2 {
                    A[(p[2] & 63) as usize] as char
                } else {
                    '='
                });
            }
            out
        }
        let life = json!({"engine":"hibp","status_code":0,"wait_succeeded":true,"success":true,"forced_kill":false,"stdout_outcome":"eof","stderr_outcome":"eof","stdout_eof":true,"stderr_eof":true,"stdout_bytes":text.len(),"stderr_bytes":0});
        let base = vec![
            format!(
                "A13_REMAINING_CHILD_STDOUT_BASE64 {}",
                encode(text.as_bytes())
            ),
            "A13_REMAINING_CHILD_STDERR_BASE64 ".into(),
            format!("A13_REMAINING_CHILD_LIFECYCLE {life}"),
        ];
        assert!(child(&base, &spec, &mut vec![]).is_ok());
        for (key, value) in [
            ("status_code", json!(1)),
            ("forced_kill", json!(true)),
            ("stdout_eof", json!(false)),
            ("stdout_bytes", json!(0)),
            ("stdout_outcome", json!("incomplete")),
            ("engine", json!("intelx")),
        ] {
            let mut changed = life.clone();
            changed[key] = value;
            let mut raw = base.clone();
            raw[2] = format!("A13_REMAINING_CHILD_LIFECYCLE {changed}");
            assert!(child(&raw, &spec, &mut vec![]).is_err(), "{key}");
        }
        let mut duplicate = base.clone();
        duplicate.push(base[0].clone());
        assert!(child(&duplicate, &spec, &mut vec![]).is_err());
        let mut wrong = base.clone();
        wrong[0] = format!(
            "A13_REMAINING_CHILD_STDOUT_BASE64 {}",
            encode(text.replace("test selected", "test other").as_bytes())
        );
        assert!(child(&wrong, &spec, &mut vec![]).is_err());
    }
    #[test]
    fn compiled_inventory_is_closed() {
        assert_eq!(
            listed("alpha: test\nbeta: test\n2 tests, 0 benchmarks\n").unwrap(),
            strings(&["alpha", "beta"])
        );
        for bad in [
            "",
            "alpha: test\nalpha: test\n2 tests, 0 benchmarks\n",
            "alpha: test\n0 tests, 0 benchmarks\n",
            "alpha: test\n1 tests, 0 benchmarks\nunknown",
        ] {
            assert!(listed(bad).is_err());
        }
        let suite_for = |package: &str, kind: &str, target: &str| Suite {
            package: package.into(),
            target: target.into(),
            kind: kind.into(),
            names: strings(&["sentinel"]),
        };
        let exact = compiled_suite_names(&suite_for("ssx-infra", "lib", "ssx_infra"));
        let count = |needle: &str| exact.iter().filter(|name| name.as_str() == needle).count();
        assert_eq!(count("sentinel"), 1);
        assert_eq!(count(PUBCHEM_OWNER_CLOCK_TEST), 1);
        let cold = "adapters::html_helper_tests::cold_bootstrap_expires_without_spending_search_deadline_or_leaking_cookies";
        assert_eq!(count(cold), 1);
        for (package, kind, target) in [
            ("other", "lib", "ssx_infra"),
            ("ssx-infra", "test", "ssx_infra"),
            ("ssx-infra", "lib", "other"),
        ] {
            assert_eq!(
                compiled_suite_names(&suite_for(package, kind, target)),
                strings(&["sentinel"])
            );
        }
        let mut missing = exact.clone();
        missing.retain(|name| name.as_str() != PUBCHEM_OWNER_CLOCK_TEST);
        let mut extra = exact.clone();
        extra.push("unrelated::unexpected_test".into());
        let mut duplicate = exact.clone();
        duplicate.push(PUBCHEM_OWNER_CLOCK_TEST.into());
        let mut cold_missing = exact.clone();
        cold_missing.retain(|name| name != cold);
        let mut cold_duplicate = exact.clone();
        cold_duplicate.push(cold.into());
        for actual in [missing, extra, duplicate, cold_missing, cold_duplicate] {
            assert!(
                same(
                    &actual,
                    &exact,
                    "A13 exact compiled inventory missing/extra/duplicate"
                )
                .is_err()
            );
        }
    }
}
