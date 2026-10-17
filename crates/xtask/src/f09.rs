//! F09 native source-bound shared policy/scope qualification.
use crate::{Result, command};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{fs, path::Path};
const INACTIVE: &str = "52df4c8ae01abb2ef2f3cf3623dfae0ee0bd77a3884b0d4a4168b776d7b4d737";
const AUTHORITY: &str = "f6a9af265307b79741b13942852c725dddd70253479fc9a7131a216cd3b6a250";
const ORACLE: &str = "ad877c851232716b104af6bd71d11d689fb20ab7efcdb0767310a4a2f17b5a24";
const SOURCE_PINS: &str = "93bc13f946fb64d609b83b03d2ace7d2cceac85bb5f32fa10e6a2c61ef86c91d";
const TOPIC_ORDER: &str = "9cdded1fb6e6bc31970c3b887cf2e3f74621864b169a49cf44a286fc7811fe8e";
const PROFILES: &str = "5b7448123b7bbb8e812249a9c538281e798843ac1d678ced4ca70b32bf891fc8";
fn identity(raw: &[u8], expected: &str) -> Result<()> {
    if Sha256::digest(raw)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>()
        != expected
    {
        return Err("F09 reviewed source capture identity mismatch".into());
    }
    Ok(())
}
fn validate_inventory(output: &str, expected: &[&str], docs: bool) -> Result<()> {
    let mut names: Vec<_> = output
        .lines()
        .filter_map(|line| line.strip_suffix(": test"))
        .map(|name| {
            if docs {
                name.split_once(" (line ").map_or(name, |(n, _)| n)
            } else {
                name
            }
        })
        .collect();
    let mut expected = expected.to_vec();
    names.sort_unstable();
    expected.sort_unstable();
    if names != expected {
        return Err("F09 exact named test inventory changed".into());
    }
    Ok(())
}
fn completed(output: &str, count: usize) -> Result<()> {
    let marker = format!("test result: ok. {count} passed; 0 failed; 0 ignored; 0 measured;");
    if !output.lines().any(|line| line.starts_with(&marker)) {
        return Err("F09 regression execution incomplete, zero or ignored".into());
    }
    Ok(())
}
fn run_inventory(
    root: &Path,
    args: &[&str],
    expected: &[&str],
    docs: bool,
    records: &mut Vec<Value>,
) -> Result<()> {
    let mut list = args.to_vec();
    list.extend(["--", "--list"]);
    validate_inventory(&command(root, "cargo", &list, records)?, expected, docs)?;
    completed(&command(root, "cargo", args, records)?, expected.len())
}
pub fn check(root: &Path, records: &mut Vec<Value>) -> Result<()> {
    let oracle =
        fs::read(root.join("crates/xtask/fixtures/f09/oracle.json")).map_err(|e| e.to_string())?;
    identity(&oracle, ORACLE)?;
    identity(
        &fs::read(root.join("crates/xtask/fixtures/f09/authority.json"))
            .map_err(|e| e.to_string())?,
        AUTHORITY,
    )?;
    identity(
        &fs::read(root.join("crates/xtask/fixtures/f09/inactive.json"))
            .map_err(|e| e.to_string())?,
        INACTIVE,
    )?;
    let profiles =
        fs::read(root.join("docs/rust-rewrite/f09/profiles.json")).map_err(|e| e.to_string())?;
    identity(&profiles, PROFILES)?;
    identity(
        &fs::read(root.join("docs/rust-rewrite/f09/source-pins.json"))
            .map_err(|e| e.to_string())?,
        SOURCE_PINS,
    )?;
    identity(
        &fs::read(root.join("crates/xtask/fixtures/f09/topic-order.json"))
            .map_err(|e| e.to_string())?,
        TOPIC_ORDER,
    )?;
    let pins: Value = serde_json::from_slice(
        &fs::read(root.join("docs/rust-rewrite/f09/source-pins.json"))
            .map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    if pins["plan_id"] != "F09" || pins["baseline"] != "1f3f5bc85083f4fd24bd9a2b07f161626e6bfef5" {
        return Err("F09 source anchor mismatch".into());
    }
    let sources = pins["sources"]
        .as_array()
        .ok_or("F09 source pins missing")?;
    let expected = [
        "slopsearx/filters.py",
        "slopsearx/capabilities.py",
        "slopsearx/service.py",
        "slopsearx/mcp/tools.py",
        "slopsearx/router.py",
        "slopsearx/routing.py",
        "tests/test_mcp_enforcement.py",
        "tests/test_workflow_console_policy.py",
        "slopsearx/config.py",
        "slopsearx/adapter.py",
        "slopsearx/research_models.py",
        "slopsearx/research_planning.py",
    ];
    if sources.len() != expected.len() {
        return Err("F09 source inventory mismatch".into());
    }
    for (pin, path) in sources.iter().zip(expected) {
        if pin["path"] != path {
            return Err("F09 source path mismatch".into());
        }
        crate::date_amendment::source(
            root,
            path,
            "1f3f5bc85083f4fd24bd9a2b07f161626e6bfef5",
            pin["git_blob"].as_str().ok_or("F09 source blob")?,
            None,
            records,
        )?;
    }
    let app = [
        "explicit_inactive_members_reject_atomically_without_changing_http_or_preview_availability",
        "source_scope_replay",
        "source_filter_policy_date_replay",
        "denied_paths_never_dispatch_and_preview_has_no_authority",
        "http_preserves_unknown_and_explicit_sensitive_semantics",
        "strict_filters_invalid_bounds_and_availability_fail_closed",
        "revalidation_detects_enforcement_and_scope_changes",
        "specialist_grants_remain_independent_of_sensitive_grant_and_origin",
        "strict_positive_and_truthful_local_filter_layers",
        "research_intent_and_captured_evidence_are_revalidated",
        "validated_command_preserves_specialist_values_and_pagination_truth",
        "local_time_postfilter_requires_actual_publication_evidence",
        "source_public_denials_and_research_dispatch_policy",
        "default_template_and_explicit_research_intent_grants_remain_distinct",
        "recovered_research_rejects_unknown_and_media_intents_before_dispatch",
    ];
    run_inventory(
        root,
        &["test", "--locked", "-p", "ssx-application", "--test", "f09"],
        &app,
        false,
        records,
    )?;
    run_inventory(
        root,
        &["test", "--locked", "-p", "ssx-server", "--test", "f09"],
        &[
            "actual_f06_named_topic_order_aliases_merges_duplicates",
            "accepted_catalog_projection_keeps_distinct_eligibility_and_redacts",
        ],
        false,
        records,
    )?;
    run_inventory(
        root,
        &[
            "test",
            "--locked",
            "-p",
            "ssx-application",
            "--doc",
            "policy",
        ],
        &[
            "crates/ssx-application/src/policy.rs - policy::ValidatedScope",
            "crates/ssx-application/src/policy.rs - policy::ValidatedScope",
        ],
        true,
        records,
    )?;
    records.push(json!({"kind":"f09_policy_scope","oracle_sha256":ORACLE,"profiles_sha256":PROFILES,"source_observations":100,"provider_calls":0,"runtime":"native Rust"}));
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ignored_or_removed_regressions_fail_closed() {
        assert!(validate_inventory("one: test\n", &["one", "two"], false).is_err());
        assert!(
            completed(
                "test result: ok. 0 passed; 0 failed; 2 ignored; 0 measured;",
                2
            )
            .is_err()
        );
        assert!(
            completed(
                "test result: ok. 1 passed; 0 failed; 1 ignored; 0 measured;",
                2
            )
            .is_err()
        );
        completed(
            "test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured;",
            2,
        )
        .unwrap();
    }
    #[test]
    fn golden_answers_cannot_redeclare_identity() {
        let raw = include_bytes!("../fixtures/f09/oracle.json");
        identity(raw, ORACLE).unwrap();
        let mut data: Value = serde_json::from_slice(raw).unwrap();
        data["filters"][0]["expected"]["status"] = json!("enforced");
        assert!(identity(&serde_json::to_vec(&data).unwrap(), ORACLE).is_err());
        assert!(identity(b"", ORACLE).is_err());
    }
}
