//! F12 exact-source offline qualification. No Python/provider execution.
use crate::{Result, command};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{fs, path::Path};
const SOURCE: &str = "e0be6fc6246992ca59914fb02e656dc23426b8654b96901dee4b84ea93c0ec91";
const OVERFLOW: &str = "87019ce71862f98460a3457b2945a35cc3f4e4a0b08097c144a7df541af0202f";
const HAZARDS: &str = "641e35375e8d289fa5067da182df509b057e91626210761b67ccb93e6640a295";
const PINS: &str = "ac88040dc41f2bd461922f978f2fc49f24bf410ea1194a276c11dfff0dfb7971";
const BASELINE: &str = "6ae5e34fed19e8e9da7c19b95eeea894512f0e6d";
const TESTS: &[&str] = &[
    "source_overflow_per_field_corrections_and_signed_zero_order",
    "source_configuration_identity_and_keyless_replay",
    "source_provider_replay_exact_body_boolean_threshold_order_and_failure",
    "malformed_nonfinite_transport_and_total_deadline_fallback",
    "eligibility_raw_scope_and_preview_reuse_never_double_call",
    "automatic_policy_rechecks_advice_budget_and_current_provenance",
    "real_valkey_cache_exact_bytes_ttl_malformed_miss_and_outage",
    "invalid_ttl_skips_write_cache_failure_keeps_decision_and_empty_candidates_no_call",
    "final_explanations_recompute_coverage_without_duplicate_tradeoffs",
    "source_scope_choices_explanations_and_keyless_fallback_replay",
    "revoked_grants_sensitive_policy_and_strict_filters_cannot_admit_advice",
];
fn identity(raw: &[u8], accepted: &str) -> Result<()> {
    let actual: String = Sha256::digest(raw)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    if actual != accepted {
        return Err("F12 reviewed capture identity mismatch".into());
    }
    Ok(())
}
fn inventory(raw: &str) -> Result<()> {
    let mut names: Vec<_> = raw
        .lines()
        .filter_map(|l| l.strip_suffix(": test"))
        .collect();
    names.sort();
    let mut expected = TESTS.to_vec();
    expected.sort();
    if names != expected {
        return Err("F12 exact named regression inventory changed".into());
    }
    Ok(())
}
fn completed(raw: &str) -> Result<()> {
    if !raw.lines().any(|l| {
        l.starts_with(&format!(
            "test result: ok. {} passed; 0 failed; 0 ignored; 0 measured; 0 filtered out;",
            TESTS.len()
        ))
    }) {
        return Err("F12 tests missing, ignored or failed".into());
    }
    Ok(())
}
pub fn check(root: &Path, records: &mut Vec<Value>) -> Result<()> {
    for (path, anchor) in [
        ("crates/xtask/fixtures/f12/source.json", SOURCE),
        ("crates/xtask/fixtures/f12/overflow.json", OVERFLOW),
        ("crates/xtask/fixtures/f12/hazards.json", HAZARDS),
        ("docs/rust-rewrite/f12/source-pins.json", PINS),
    ] {
        identity(
            &fs::read(root.join(path)).map_err(|e| e.to_string())?,
            anchor,
        )?;
    }
    let source: Value = serde_json::from_slice(
        &fs::read(root.join("crates/xtask/fixtures/f12/source.json")).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    if source["baseline"] != BASELINE
        || source["plan_id"] != "F12"
        || source["python"] != "3.14.7"
        || source["mcp"] != "2.3.0"
        || source["fastmcp"] != "4.0.11"
    {
        return Err("F12 source identity differs".into());
    }
    let paths = [
        "slopsearx/jev.py",
        "slopsearx/service.py",
        "slopsearx/routing.py",
        "slopsearx/router.py",
        "tests/test_jev.py",
        "tests/test_routing_eval.py",
    ];
    let sources = source["sources"]
        .as_array()
        .ok_or("F12 missing source pins")?;
    if sources.len() != paths.len() {
        return Err("F12 source inventory differs".into());
    }
    for (pin, path) in sources.iter().zip(paths) {
        if pin["path"] != path {
            return Err("F12 source path mismatch".into());
        }
        crate::date_amendment::source(
            root,
            path,
            BASELINE,
            pin["git_blob"].as_str().ok_or("F12 source blob")?,
            None,
            records,
        )?;
    }
    for args in [
        vec![
            "hash-object".to_string(),
            "--".into(),
            "slopsearx/cache.py".into(),
        ],
        vec!["rev-parse".into(), format!("{BASELINE}:slopsearx/cache.py")],
    ] {
        let args: Vec<_> = args.iter().map(String::as_str).collect();
        if command(root, "git", &args, records)?.trim()
            != "354dbc179c4ca6ebc4ac9f85dd66a40d07ba3c09"
        {
            return Err("F12 overflow cache source drift".into());
        }
    }
    inventory(&command(
        root,
        "cargo",
        &[
            "test",
            "--locked",
            "-p",
            "ssx-infra",
            "--test",
            "f12",
            "--",
            "--list",
        ],
        records,
    )?)?;
    completed(&command(
        root,
        "cargo",
        &["test", "--locked", "-p", "ssx-infra", "--test", "f12"],
        records,
    )?)?;
    let helper_names = [
        "f12::tests::expected_output_mutation_cannot_repair_accepted_anchor",
        "f12::tests::removed_renamed_and_ignored_regressions_fail_closed",
    ];
    let listed = command(
        root,
        "cargo",
        &[
            "test",
            "--locked",
            "-p",
            "xtask",
            "f12::tests",
            "--",
            "--list",
        ],
        records,
    )?;
    let actual: Vec<_> = listed
        .lines()
        .filter_map(|l| l.strip_suffix(": test"))
        .collect();
    if actual != helper_names {
        return Err("F12 qualification negative controls missing".into());
    }
    let ran = command(
        root,
        "cargo",
        &["test", "--locked", "-p", "xtask", "f12::tests"],
        records,
    )?;
    if !ran
        .lines()
        .any(|l| l.starts_with("test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured;"))
    {
        return Err("F12 negative controls incomplete or ignored".into());
    }
    // F09 regression checks prove the additive path has not replaced baseline policy.
    crate::f09::check(root, records)?;
    records.push(json!({"plan_id":"F12","status":"passed","source_baseline":BASELINE,"corpus_sha256":SOURCE,"hazards_sha256":HAZARDS,"source_pin_sha256":PINS,"source_provider_cases":11,"source_config_cases":8,"source_scope_cases":5,"overflow_cases":9,"overflow_sha256":OVERFLOW,"named_regressions":TESTS,"runtime":"native only; actual loopback provider and disposable Valkey; no live provider"}));
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn expected_output_mutation_cannot_repair_accepted_anchor() {
        let raw = include_bytes!("../fixtures/f12/source.json");
        identity(raw, SOURCE).unwrap();
        let mut value: Value = serde_json::from_slice(raw).unwrap();
        value["cases"][0]["decision"]["engines"] = json!([]);
        let mutated = serde_json::to_vec(&value).unwrap();
        assert!(identity(&mutated, SOURCE).is_err());
    }
    #[test]
    fn removed_renamed_and_ignored_regressions_fail_closed() {
        let list = TESTS
            .iter()
            .map(|n| format!("{n}: test\n"))
            .collect::<String>();
        inventory(&list).unwrap();
        assert!(inventory("").is_err());
        assert!(inventory(&list.replacen(TESTS[0], "renamed", 1)).is_err());
        assert!(
            completed(
                "test result: ok. 9 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out;"
            )
            .is_err()
        );
        assert!(
            completed(
                "test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 10 filtered out;"
            )
            .is_err()
        );
    }
}
