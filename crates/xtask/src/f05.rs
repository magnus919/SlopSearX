//! Native source-bound F05 serialization and identity acceptance gate.
use crate::{Result, command};
use sha2::{Digest, Sha256};
use ssx_domain::{
    canonical::{self, JsonProfile},
    storage,
};
use ssx_json::{Value, json};
use std::{collections::BTreeSet, fs, path::Path};
fn encode(value: impl serde::Serialize) -> Result<Value> {
    ssx_json::to_value(value).map_err(|e| e.to_string())
}
fn source_result(input: &Value) -> Result<ssx_domain::SearchResult> {
    let mut result = storage::decode_result(input.clone()).map_err(|e| e.to_string())?;
    if let Value::Number(number) = &input["score"] {
        result.score =
            ssx_domain::FiniteF64::from_number(number.clone()).map_err(|e| e.to_string())?;
    }
    if let Some(media) = result.media.as_mut()
        && let Value::Number(number) = &input["media"]["duration"]
    {
        media.duration =
            Some(ssx_domain::FiniteF64::from_number(number.clone()).map_err(|e| e.to_string())?);
    }
    Ok(result)
}
fn source_response(input: &Value) -> Result<ssx_domain::SearchResponse> {
    let mut response = storage::decode_response(input.clone()).map_err(|e| e.to_string())?;
    if let Some(results) = input["results"].as_array() {
        response.results = results.iter().map(source_result).collect::<Result<_>>()?;
    }
    if let Some(scores) = input["scope"]["jev_scores"].as_object() {
        for (key, value) in scores {
            if let Value::Number(number) = value {
                response.scope.jev_scores.insert(
                    key.clone(),
                    ssx_domain::FiniteF64::from_number(number.clone())
                        .map_err(|e| e.to_string())?,
                );
            }
        }
    }
    if let Some(outcomes) = input["engine_outcomes"].as_array() {
        for (outcome, raw) in response.engine_outcomes.iter_mut().zip(outcomes) {
            if let Value::Number(number) = &raw["latency_ms"] {
                outcome.latency_ms = Some(
                    ssx_domain::FiniteF64::from_number(number.clone())
                        .map_err(|e| e.to_string())?,
                );
            }
        }
    }
    Ok(response)
}
fn observe(kind: &str, input: &Value) -> Result<Value> {
    match kind {
        "storage_result_write" => {
            storage::encode_result(&source_result(input)?).map_err(|e| e.to_string())
        }
        "storage_result_read" => storage::encode_result(
            &storage::decode_result(input.clone()).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string()),
        "storage_response_write" => {
            storage::encode_response(&source_response(input)?).map_err(|e| e.to_string())
        }
        "storage_response_read" => storage::encode_response(
            &storage::decode_response(input.clone()).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string()),
        "http_json_result" => {
            let result = source_result(input.get("result").ok_or("missing HTTP result")?)?;
            let metadata: ssx_wire::HttpResultMetadata = ssx_json::from_value(
                input
                    .get("metadata")
                    .cloned()
                    .ok_or("missing precomputed formatter metadata")?,
            )
            .map_err(|e| e.to_string())?;
            encode(ssx_http::projection::result_json(&result, &metadata))
        }
        "http_yaml_result" => encode(ssx_http::projection::result_yaml(&source_result(input)?)),
        "paper_fields" => encode(ssx_wire::paper_fields(Some(&json!({"paper": input})))),
        "media_build" => {
            let media = storage::build_media(
                input["media_type"].as_str().ok_or("missing media type")?,
                input["url"].as_str(),
                input["thumbnail"].as_str(),
                input["source"].as_str(),
                input["width"].as_i64(),
                input["height"].as_i64(),
                match &input["duration"] {
                    Value::Number(number) => Some(
                        ssx_domain::FiniteF64::from_number(number.clone())
                            .map_err(|e| e.to_string())?,
                    ),
                    _ => None,
                },
            );
            let result = ssx_domain::SearchResult {
                media,
                ..Default::default()
            };
            let encoded = storage::encode_result(&result).map_err(|e| e.to_string())?;
            Ok(encoded["media"].clone())
        }
        "mcp_card" => {
            let result = source_result(input.get("result").ok_or("missing card result")?)?;
            let id = if let Some(snapshot) = input.get("snapshot_id").and_then(Value::as_str) {
                Some(ssx_domain::ResultId::new(
                    ssx_domain::SnapshotId::new(snapshot).map_err(|e| e.to_string())?,
                    input
                        .get("index")
                        .and_then(Value::as_u64)
                        .ok_or("missing card index")?,
                ))
            } else {
                None
            };
            let retrieval: ssx_wire::McpRetrievalDto = ssx_json::from_value(
                input
                    .get("retrieval")
                    .cloned()
                    .ok_or("missing preclassified retrieval input")?,
            )
            .map_err(|e| e.to_string())?;
            encode(ssx_mcp::projection::result_card(
                &result,
                id.as_ref(),
                retrieval,
            ))
        }
        "canonical_json" => {
            let profile = match input["profile"].as_str() {
                Some("ascii_compact") => JsonProfile::AsciiCompact,
                Some("utf8_compact") => JsonProfile::Utf8Compact,
                Some("ascii_spaced") => JsonProfile::AsciiSpaced,
                _ => return Err("unknown canonical profile".into()),
            };
            Ok(Value::String(
                String::from_utf8(
                    canonical::canonical_json_text(
                        input["value_json"]
                            .as_str()
                            .ok_or("missing lossless JSON input")?,
                        profile,
                    )
                    .map_err(|e| e.to_string())?,
                )
                .map_err(|e| e.to_string())?,
            ))
        }
        "unicode_casefold" => Ok(json!(canonical::casefold(
            input
                .as_str()
                .or_else(|| input["value"].as_str())
                .ok_or("missing Unicode casefold input")?
        ))),
        "composite_id" => {
            let parts = input
                .as_array()
                .ok_or("composite input must be array")?
                .iter()
                .map(|p| p.as_str().ok_or("composite part must be string"))
                .collect::<std::result::Result<Vec<_>, _>>()?;
            Ok(json!(
                canonical::composite_artifact_id(&parts).map_err(|e| e.to_string())?
            ))
        }
        "manifest_id" => Ok(json!(
            canonical::manifest_artifact_id(input).map_err(|e| e.to_string())?
        )),
        "plan_digest" => Ok(json!(
            canonical::plan_digest(input).map_err(|e| e.to_string())?
        )),
        "saved_identity" => Ok(json!(
            canonical::saved_identity(
                input
                    .as_array()
                    .ok_or("saved identity input must be parts array")?
            )
            .map_err(|e| e.to_string())?
        )),
        _ => Err(format!("unsupported F05 case kind: {kind}")),
    }
}
fn compare(expected: &Value, actual: &Value, path: &str) -> Result<()> {
    match (expected, actual) {
        (Value::Object(a), Value::Object(b)) => {
            if a.keys().collect::<BTreeSet<_>>() != b.keys().collect::<BTreeSet<_>>() {
                return Err(format!("{path}: field set differs"));
            }
            for (key, value) in a {
                compare(value, &b[key], &format!("{path}/{key}"))?;
            }
        }
        (Value::Array(a), Value::Array(b)) => {
            if a.len() != b.len() {
                return Err(format!("{path}: length differs"));
            }
            for (i, (a, b)) in a.iter().zip(b).enumerate() {
                compare(a, b, &format!("{path}/{i}"))?;
            }
        }
        _ => {
            if expected != actual {
                return Err(format!("{path}: expected {expected}, observed {actual}"));
            }
        }
    }
    Ok(())
}
fn validate_case_manifest(cases: &[Value], required: &ssx_json::Map<String, Value>) -> Result<()> {
    let mut actual = ssx_json::Map::new();
    for case in cases {
        let id = case["id"].as_str().ok_or("missing F05 case id")?;
        let kind = case["kind"].as_str().ok_or("missing F05 case kind")?;
        if id.is_empty()
            || actual
                .insert(id.into(), Value::String(kind.into()))
                .is_some()
        {
            return Err("empty/duplicate F05 case id".into());
        }
    }
    if &actual != required {
        return Err("F05 cases differ from reviewed coverage manifest".into());
    }
    Ok(())
}
fn negative_checks() -> Result<()> {
    let required = json!({"one":"canonical_json","two":"manifest_id"});
    let valid = vec![
        json!({"id":"one","kind":"canonical_json"}),
        json!({"id":"two","kind":"manifest_id"}),
    ];
    let required = required.as_object().ok_or("invalid negative manifest")?;
    validate_case_manifest(&valid, required)?;
    for mutant in [
        vec![valid[0].clone()],
        vec![valid[0].clone(), valid[0].clone()],
        vec![
            valid[0].clone(),
            json!({"id":"unknown","kind":"manifest_id"}),
        ],
        vec![
            valid[0].clone(),
            json!({"id":"two","kind":"canonical_json"}),
        ],
    ] {
        if validate_case_manifest(&mutant, required).is_ok() {
            return Err("F05 accepted missing/duplicate/unknown case coverage".into());
        }
    }

    if compare(&json!(1), &json!(1.0), "negative").is_ok() {
        return Err("F05 comparator erased integer/float representation".into());
    }
    if compare(&json!("-0.0"), &json!("0.0"), "canonical-negative").is_ok() {
        return Err("F05 canonical comparator erased signed zero".into());
    }

    for mutant in [
        json!({}),
        json!({"value":null}),
        json!({"value":2}),
        json!({"value":1,"extra":true}),
    ] {
        if compare(&json!({"value":1}), &mutant, "negative").is_ok() {
            return Err("F05 comparator accepted schema/value mutation".into());
        }
    }
    if compare(&json!([1, 2]), &json!([2, 1]), "negative").is_ok() {
        return Err("F05 comparator accepted reordered array".into());
    }
    let first: Value = ssx_json::from_str("18446744073709551617").map_err(|e| e.to_string())?;
    let second: Value = ssx_json::from_str("18446744073709551618").map_err(|e| e.to_string())?;
    if compare(&first, &second, "negative").is_ok() {
        return Err("F05 comparator rounded large integers".into());
    }
    if observe("unknown", &Value::Null).is_ok()
        || observe(
            "canonical_json",
            &json!({"profile":"unknown","value_json":"null"}),
        )
        .is_ok()
    {
        return Err("F05 accepted unknown codec/profile".into());
    }
    Ok(())
}
pub fn check(root: &Path, records: &mut Vec<serde_json::Value>) -> Result<()> {
    negative_checks()?;
    let corpus: Value = ssx_json::from_slice(
        &fs::read(root.join("crates/xtask/fixtures/f05/corpus.json")).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    if corpus["schema_version"] != 1 {
        return Err("unsupported F05 corpus schema".into());
    }
    let baseline = corpus["baseline_sha"]
        .as_str()
        .ok_or("missing F05 baseline")?;
    if baseline != "16f03be656f34d90edd8285f9c81dfb1c4626b0a" {
        return Err("unreviewed F05 source baseline".into());
    }
    if corpus["provenance"]["license"] != "MIT"
        || corpus["provenance"]["redaction"]
            .as_str()
            .is_none_or(str::is_empty)
    {
        return Err("missing F05 license/redaction provenance".into());
    }
    let oracle = &corpus["provenance"]["oracle"];
    if oracle["path"] != "docs/rust-rewrite/f05/oracle/export.py" {
        return Err("unreviewed F05 oracle path".into());
    }
    let oracle_bytes =
        fs::read(root.join("docs/rust-rewrite/f05/oracle/export.py")).map_err(|e| e.to_string())?;
    let oracle_hash = Sha256::digest(oracle_bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    if oracle["sha256"].as_str() != Some(oracle_hash.as_str()) {
        return Err("F05 oracle provenance hash mismatch".into());
    }
    let sources = corpus["provenance"]["sources"]
        .as_array()
        .ok_or("missing F05 provenance sources")?;
    let mut paths = BTreeSet::new();
    for source in sources {
        let path = source["path"].as_str().ok_or("missing source path")?;
        if path.starts_with('/') || path.split('/').any(|part| part == "..") || !paths.insert(path)
        {
            return Err("invalid/duplicate F05 source".into());
        }
        let actual = command(
            root,
            "git",
            &["rev-parse", &format!("{baseline}:{path}")],
            records,
        )?;
        if Some(actual.trim()) != source["git_blob"].as_str() {
            return Err(format!("F05 source provenance mismatch: {path}"));
        }
    }
    for path in [
        "slopsearx/adapter.py",
        "slopsearx/service.py",
        "slopsearx/payload.py",
        "slopsearx/publication_metadata.py",
        "slopsearx/artifacts.py",
        "slopsearx/staged.py",
        "slopsearx/saved_models.py",
        "slopsearx/formatter.py",
        "slopsearx/mcp/result_serialization.py",
    ] {
        if !paths.contains(path) {
            return Err(format!("missing F05 required source: {path}"));
        }
    }
    let metadata = command(
        root,
        "cargo",
        &["metadata", "--locked", "--no-deps", "--format-version", "1"],
        records,
    )?;
    let metadata: serde_json::Value = serde_json::from_str(&metadata).map_err(|e| e.to_string())?;
    let packages = metadata["packages"]
        .as_array()
        .ok_or("missing cargo packages")?;
    let pure = packages
        .iter()
        .find(|p| p["name"] == "ssx-json")
        .ok_or("missing pure JSON crate")?;
    let names: BTreeSet<_> = pure["dependencies"]
        .as_array()
        .ok_or("missing pure JSON dependencies")?
        .iter()
        .map(|d| d["name"].as_str().unwrap_or(""))
        .collect();
    if names != BTreeSet::from(["serde", "serde_json", "ryu", "indexmap"]) {
        return Err("ssx-json must have only pure serialization dependencies".into());
    }
    let wire = packages
        .iter()
        .find(|p| p["name"] == "ssx-wire")
        .ok_or("missing public wire crate")?;
    for dependency in wire["dependencies"]
        .as_array()
        .ok_or("missing wire dependencies")?
    {
        if dependency["name"].as_str().is_some_and(|name| {
            name.starts_with("ssx-") && name != "ssx-json" && name != "ssx-publication"
        }) {
            return Err("wire crate depends on backend internals".into());
        }
    }
    let features = command(
        root,
        "cargo",
        &["tree", "--locked", "-e", "features", "-i", "serde_json"],
        records,
    )?;
    if features.contains("arbitrary_precision") || features.contains("preserve_order") {
        return Err("unreviewed global serde_json number/order feature".into());
    }
    let table =
        fs::read(root.join("contracts/unicode/casefold-16.0.0.json")).map_err(|e| e.to_string())?;
    if Sha256::digest(table)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>()
        != "53e8bd24021fff4b07206c8ac27e8bbf9e801dc4335656e4e97f4169d506981b"
    {
        return Err("Unicode16 table digest mismatch".into());
    }
    let text_tables = fs::read(root.join("contracts/unicode/python-text-16.0.0.json"))
        .map_err(|e| e.to_string())?;
    let text_hash = Sha256::digest(text_tables)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    if text_hash != "9f6d9cb6e81031954d91cc03084237592e9d7bb2224f22cab5a5fba8cebefabd" {
        return Err("Python Unicode16 text table digest mismatch".into());
    }
    let cases = corpus["cases"].as_array().ok_or("missing F05 cases")?;
    let manifest: Value = ssx_json::from_slice(
        &fs::read(root.join("crates/xtask/fixtures/f05/manifest.json"))
            .map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    if manifest["schema_version"] != 1 {
        return Err("unsupported F05 coverage manifest".into());
    }
    validate_case_manifest(
        cases,
        manifest["required_cases"]
            .as_object()
            .ok_or("missing reviewed F05 cases")?,
    )?;
    let required_sources = manifest["required_sources"]
        .as_array()
        .ok_or("missing reviewed F05 sources")?
        .iter()
        .map(|v| v.as_str().ok_or("invalid reviewed source"))
        .collect::<std::result::Result<BTreeSet<_>, _>>()?;
    if paths != required_sources {
        return Err("F05 sources differ from reviewed coverage manifest".into());
    }

    if corpus
        .as_object()
        .map(|o| o.keys().map(String::as_str).collect::<BTreeSet<_>>())
        != Some(BTreeSet::from([
            "schema_version",
            "baseline_sha",
            "provenance",
            "cases",
        ]))
    {
        return Err("F05 corpus field set differs".into());
    }
    let mut ids = BTreeSet::new();
    let mut kinds = BTreeSet::new();
    for case in cases {
        if case
            .as_object()
            .map(|o| o.keys().map(String::as_str).collect::<BTreeSet<_>>())
            != Some(BTreeSet::from(["id", "kind", "input", "expected"]))
        {
            return Err("F05 case field set differs".into());
        }
        let id = case["id"].as_str().ok_or("missing F05 case id")?;
        if id.is_empty() || !ids.insert(id) {
            return Err("empty/duplicate F05 case id".into());
        }
        let kind = case["kind"].as_str().ok_or("missing F05 case kind")?;
        kinds.insert(kind);
        let actual = observe(kind, case.get("input").ok_or("missing F05 case input")?)
            .map_err(|e| format!("{id}: {e}"))?;
        compare(
            case.get("expected").ok_or("missing F05 case expected")?,
            &actual,
            id,
        )?;
        records.push(serde_json::json!({"program":"native-f05","args":[id,kind],"exit_code":0,"stdout":"source golden matched","stderr":""}));
    }
    for kind in [
        "storage_result_write",
        "storage_result_read",
        "storage_response_write",
        "storage_response_read",
        "http_json_result",
        "http_yaml_result",
        "canonical_json",
        "composite_id",
        "manifest_id",
        "plan_digest",
        "saved_identity",
        "mcp_card",
        "media_build",
        "paper_fields",
        "unicode_casefold",
    ] {
        if !kinds.contains(kind) {
            return Err(format!("missing F05 coverage kind: {kind}"));
        }
    }
    command(
        root,
        "cargo",
        &[
            "test",
            "--locked",
            "-p",
            "ssx-json",
            "-p",
            "ssx-domain",
            "-p",
            "ssx-wire",
            "-p",
            "ssx-http",
            "-p",
            "ssx-mcp",
        ],
        records,
    )?;
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn comparison_rejects_contract_mutants() {
        for mutant in [
            json!({}),
            json!({"score":null}),
            json!({"score":2}),
            json!({"score":1,"extra":true}),
        ] {
            assert!(compare(&json!({"score":1}), &mutant, "case").is_err());
        }
        assert!(observe("unknown", &Value::Null).is_err());
        assert!(observe("canonical_json", &json!({"profile":"unknown","value":null})).is_err());
    }
}
