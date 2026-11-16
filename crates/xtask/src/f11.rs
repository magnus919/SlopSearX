//! Source-bound offline F11 acceptance. No oracle interpreter executes here.
use crate::{Result, command};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};
// Reviewed acceptance anchors live outside mutable fixture metadata. Refresh only
// after independent recapture review, never as an automatic manifest repair.
const CORPUS_SHA256: &str = "ea10849c58825e49efa74e2c44264c966daeba0d70c2ea8c3ffef01a0f243a7f";
const UNICODE_SHA256: &str = "d19fabfd497e3b5d5304928370813af3c6b0bb31d139c80c0dc35237d2109767";
const ORACLE_SHA256: &str = "6e86d02435e21a669c78bd4169c2f58797979765ba5117af4837cc7c82ae5714";
const PLATFORMS_SHA256: &str = "5a5d069b2a9c9418c18fa34b3709146453a8f8fd2fcb93eee37da24e4482454a";
fn anchored(actual: &[u8], declared: &Value, accepted: &str) -> Result<()> {
    if declared.as_str() != Some(accepted) || hash(actual) != accepted {
        return Err("F11 reviewed acceptance digest mismatch".into());
    }
    Ok(())
}
fn hash(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
fn normalization_dependency(dependencies: &[Value]) -> Result<()> {
    let mut selected = dependencies
        .iter()
        .filter(|dependency| dependency["name"] == "unicode-normalization");
    match (selected.next(), selected.next()) {
        (Some(dependency), None) if dependency["req"] == "=0.1.24" => Ok(()),
        _ => Err("F11 normalization dependency must be unique at exact version =0.1.24".into()),
    }
}
fn normalization_dependency_controls(dependencies: &[Value]) -> Result<()> {
    // Validate the actual dependency first, then duplicate that same record.
    normalization_dependency(dependencies)?;
    let actual = dependencies
        .iter()
        .find(|dependency| dependency["name"] == "unicode-normalization")
        .ok_or("F11 normalization positive dependency absent")?;
    for requirement in ["=0.1.24", "^0.1"] {
        let mut duplicate = actual.clone();
        duplicate["req"] = json!(requirement);
        let mut changed = dependencies.to_vec();
        changed.push(duplicate);
        if normalization_dependency(&changed).is_ok() {
            return Err("F11 duplicate normalization dependency escaped exact boundary".into());
        }
    }
    Ok(())
}
fn coverage(cases: &[Value], required: &serde_json::Map<String, Value>) -> Result<()> {
    let mut actual = BTreeMap::new();
    for case in cases {
        let id = case["id"]
            .as_str()
            .filter(|s| !s.is_empty())
            .ok_or("missing F11 case id")?;
        let kind = case["kind"].as_str().ok_or("missing F11 kind")?;
        if !["rank", "group", "retrieval", "url"].contains(&kind)
            || actual.insert(id, kind).is_some()
        {
            return Err("duplicate or unknown F11 case".into());
        }
    }
    let expected: BTreeMap<_, _> = required
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str().unwrap_or("")))
        .collect();
    if actual != expected
        || actual.values().copied().collect::<BTreeSet<_>>()
            != BTreeSet::from(["rank", "group", "retrieval", "url"])
    {
        return Err("F11 required coverage differs".into());
    }
    Ok(())
}
pub fn check(root: &Path, records: &mut Vec<Value>) -> Result<()> {
    let directory = root.join("docs/rust-rewrite/f11");
    let manifest: Value = serde_json::from_slice(
        &fs::read(directory.join("manifest.json")).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    let baseline = manifest["baseline"].as_str().ok_or("missing baseline")?;
    if baseline != "7cc420f7ffb61127750e77bf6130e225925f9186"
        || manifest["schema_version"] != 1
        || manifest["license"].as_str().is_none_or(str::is_empty)
    {
        return Err("unreviewed F11 provenance".into());
    }
    let mut bytes = Vec::new();
    let mut names = BTreeSet::new();
    for shard in manifest["shards"].as_array().ok_or("missing F11 shards")? {
        let name = shard.as_str().ok_or("invalid shard name")?;
        if !name.starts_with("goldens-")
            || !name.ends_with(".jsonl")
            || name.contains('/')
            || name.contains('\\')
            || !names.insert(name)
        {
            return Err("invalid duplicate shard".into());
        }
        let content = fs::read(directory.join(name)).map_err(|e| e.to_string())?;
        if content.len() > 500_000 {
            return Err("F11 shard exceeds repository fixture cap".into());
        }
        bytes.extend(content);
    }
    anchored(&bytes, &manifest["goldens_sha256"], CORPUS_SHA256)?;
    if manifest["goldens_sha256"] != hash(&bytes) {
        return Err("F11 corpus hash mismatch".into());
    }
    if manifest["unicode_sha256"]
        != hash(&fs::read(directory.join("unicode.json")).map_err(|e| e.to_string())?)
    {
        return Err("F11 Unicode table hash mismatch".into());
    }
    anchored(
        &fs::read(directory.join("unicode.json")).map_err(|e| e.to_string())?,
        &manifest["unicode_sha256"],
        UNICODE_SHA256,
    )?;
    let platform_bytes =
        fs::read(directory.join("numeric-platforms.json")).map_err(|e| e.to_string())?;
    anchored(&platform_bytes, &json!(PLATFORMS_SHA256), PLATFORMS_SHA256)?;
    let oracle = fs::read(directory.join("oracle/export.py")).map_err(|e| e.to_string())?;
    anchored(&oracle, &manifest["oracle"]["sha256"], ORACLE_SHA256)?;
    if manifest["oracle"]["path"] != "docs/rust-rewrite/f11/oracle/export.py"
        || manifest["oracle"]["sha256"] != hash(&oracle)
    {
        return Err("F11 oracle hash mismatch".into());
    }
    let cases = std::str::from_utf8(&bytes)
        .map_err(|e| e.to_string())?
        .lines()
        .map(serde_json::from_str)
        .collect::<std::result::Result<Vec<Value>, _>>()
        .map_err(|e| e.to_string())?;
    let required = manifest["case_manifest"]
        .as_object()
        .ok_or("missing F11 case manifest")?;
    coverage(&cases, required)?;
    for mutant in [
        Vec::new(),
        cases.get(1..).unwrap_or_default().to_vec(),
        vec![cases[0].clone(), cases[0].clone()],
    ] {
        if coverage(&mutant, required).is_ok() {
            return Err("F11 coverage negative check failed".into());
        }
    }
    // Mutating expected output and repairing fixture-local metadata cannot
    // self-authorize a new oracle observation.
    let mut mutant = cases.clone();
    mutant[0]["expected"] = json!("self-authorized wrong result");
    let altered = mutant
        .iter()
        .map(|c| serde_json::to_string(c).expect("JSON case") + "\n")
        .collect::<String>();
    if anchored(
        altered.as_bytes(),
        &json!(hash(altered.as_bytes())),
        CORPUS_SHA256,
    )
    .is_ok()
    {
        return Err("F11 accepted repaired-hash expected-output mutation".into());
    }
    let sources = manifest["sources"]
        .as_object()
        .ok_or("missing F11 sources")?;
    let required_sources = BTreeSet::from([
        "slopsearx/merger.py",
        "slopsearx/scholarly.py",
        "slopsearx/retrieval_url.py",
        "slopsearx/payload.py",
        "slopsearx/publication_metadata.py",
        "tests/test_scholarly.py",
        "tests/test_scholarly_arxiv_html_identity.py",
        "tests/test_rank_fusion.py",
        "tests/test_retrieval_handoff.py",
    ]);
    if sources.keys().map(String::as_str).collect::<BTreeSet<_>>() != required_sources {
        return Err("F11 source coverage differs".into());
    }
    for (path, expected) in sources {
        for args in [
            vec!["rev-parse".to_owned(), format!("{baseline}:{path}")],
            vec!["hash-object".into(), path.clone()],
        ] {
            let args: Vec<_> = args.iter().map(String::as_str).collect();
            if Some(command(root, "git", &args, records)?.trim()) != expected.as_str() {
                return Err(format!("F11 source drift: {path}"));
            }
        }
    }
    let metadata = command(
        root,
        "cargo",
        &["metadata", "--locked", "--no-deps", "--format-version", "1"],
        records,
    )?;
    let metadata: Value = serde_json::from_str(&metadata).map_err(|e| e.to_string())?;
    for (package, allowed) in [
        ("ssx-publication", BTreeSet::from(["ssx-json"])),
        (
            "ssx-application",
            // NFC entity keys use pure, deterministic in-memory normalization.
            BTreeSet::from([
                "ssx-domain",
                "ssx-json",
                "ssx-publication",
                "url",
                "sha2",
                "unicode-normalization",
            ]),
        ),
    ] {
        let p = metadata["packages"]
            .as_array()
            .ok_or("missing package graph")?
            .iter()
            .find(|p| p["name"] == package)
            .ok_or("missing F11 package")?;
        let dependencies = p["dependencies"].as_array().ok_or("missing dependencies")?;
        let actual = dependencies
            .iter()
            .map(|d| d["name"].as_str().unwrap_or(""))
            .collect::<BTreeSet<_>>();
        if actual != allowed {
            return Err(format!("{package} violates pure F11 dependency boundary"));
        }
        if package == "ssx-application" {
            normalization_dependency_controls(dependencies)?;
        }
    }
    command(
        root,
        "cargo",
        &["test", "--locked", "-p", "ssx-application"],
        records,
    )?;
    command(
        root,
        "cargo",
        &["test", "--locked", "-p", "ssx-publication"],
        records,
    )?;
    records.push(json!({"plan_id":"F11","case_count":cases.len(),"corpus_sha256":hash(&bytes),"source_baseline":baseline,"status":"passed","coverage_negative_checks":["empty","missing","duplicate","expected_output_plus_repaired_hash"],"runtime":"native only"}));
    Ok(())
}
