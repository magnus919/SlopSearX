//! S01: source-bound advisory identity view for local third-party forks.
//!
//! cargo-deny 0.20.2 turns a local path package's absent Cargo metadata
//! `source` into no source and skips it before advisory matching. This module
//! produces a clearly labelled, derived metadata view in which only the
//! reviewed local forks carry their upstream registry identity. The physical
//! lock graph and every existing cargo-deny check stay untouched; the derived
//! view is evidence for upstream advisory identity matching only and must never
//! be used as resolver, feature, TLS or shipping evidence.
use crate::{Result, command};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, fs, path::Path};

pub const MAPPINGS: &str = "docs/rust-rewrite/t08a/local-fork-advisory-mappings.json";
pub const DERIVED: &str = "target/rust-evidence/local-fork-advisory-metadata.json";

fn digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn need(ok: bool, why: &str) -> Result<()> {
    if ok {
        Ok(())
    } else {
        Err(format!("S01 local-fork advisory: {why}"))
    }
}

/// One reviewed local package classification.
pub struct Entry {
    pub kind: String,
    pub local_name: String,
    pub local_version: String,
    pub upstream_name: Option<String>,
    pub upstream_version: Option<String>,
    pub upstream_source: Option<String>,
}

fn mappings(root: &Path) -> Result<(Vec<Entry>, String)> {
    let raw = fs::read(root.join(MAPPINGS)).map_err(|e| e.to_string())?;
    let parsed: Value = serde_json::from_slice(&raw).map_err(|e| e.to_string())?;
    let rows = parsed["mappings"]
        .as_array()
        .ok_or("S01 local-fork advisory: mappings array missing")?;
    let mut entries = Vec::new();
    for row in rows {
        let kind = row["kind"].as_str().ok_or("S01 mapping kind missing")?;
        let local_name = row["local_name"]
            .as_str()
            .ok_or("S01 mapping local_name missing")?;
        let local_version = row["local_version"]
            .as_str()
            .ok_or("S01 mapping local_version missing")?;
        let (upstream_name, upstream_version, upstream_source) = if kind == "third_party_fork" {
            (
                Some(
                    row["upstream_name"]
                        .as_str()
                        .ok_or("S01 upstream_name")?
                        .to_owned(),
                ),
                Some(
                    row["upstream_version"]
                        .as_str()
                        .ok_or("S01 upstream_version")?
                        .to_owned(),
                ),
                Some(
                    row["upstream_source"]
                        .as_str()
                        .ok_or("S01 upstream_source")?
                        .to_owned(),
                ),
            )
        } else {
            (None, None, None)
        };
        entries.push(Entry {
            kind: kind.to_owned(),
            local_name: local_name.to_owned(),
            local_version: local_version.to_owned(),
            upstream_name,
            upstream_version,
            upstream_source,
        });
    }
    Ok((entries, digest(&raw)))
}

fn workspace_members(metadata: &Value) -> Result<BTreeMap<String, bool>> {
    let mut members = BTreeMap::new();
    for id in metadata["workspace_members"]
        .as_array()
        .ok_or("S01 workspace_members missing")?
    {
        members.insert(
            id.as_str().ok_or("S01 workspace member id")?.to_owned(),
            true,
        );
    }
    Ok(members)
}

/// Rewrites only the reviewed local forks to their upstream registry identity.
/// Package IDs and resolve edges stay intact so the derived document keeps the
/// same graph shape; only advisory-relevant identity fields change.
pub fn transform_metadata(metadata: &Value, entries: &[Entry]) -> Result<(Value, Vec<Value>)> {
    let members = workspace_members(metadata)?;
    let mut derived = metadata.clone();
    let mut evidence = Vec::new();
    let packages = derived["packages"]
        .as_array_mut()
        .ok_or("S01 packages missing")?;
    for package in packages.iter_mut() {
        if package["source"] != Value::Null {
            continue;
        }
        let id = package["id"]
            .as_str()
            .ok_or("S01 package id missing")?
            .to_owned();
        let is_member = members.contains_key(&id);
        let name = package["name"]
            .as_str()
            .ok_or("S01 package name")?
            .to_owned();
        let version = package["version"]
            .as_str()
            .ok_or("S01 package version")?
            .to_owned();
        let entry = entries
            .iter()
            .find(|entry| entry.local_name == name && entry.local_version == version);
        match entry {
            Some(entry) if entry.kind == "project_owned" => {
                evidence.push(json!({
                    "id": id,
                    "name": name,
                    "version": version,
                    "kind": "project_owned",
                    "workspace_member": is_member,
                }));
            }
            Some(entry) => {
                let upstream_name = entry.upstream_name.as_deref().ok_or("S01 upstream_name")?;
                let upstream_version = entry
                    .upstream_version
                    .as_deref()
                    .ok_or("S01 upstream_version")?;
                let upstream_source = entry
                    .upstream_source
                    .as_deref()
                    .ok_or("S01 upstream_source")?;
                package["name"] = json!(upstream_name);
                package["version"] = json!(upstream_version);
                package["source"] = json!(upstream_source);
                evidence.push(json!({
                    "id": id,
                    "local_name": name,
                    "local_version": version,
                    "kind": "third_party_fork",
                    "upstream_name": upstream_name,
                    "upstream_version": upstream_version,
                    "upstream_source": upstream_source,
                    "workspace_member": is_member,
                }));
            }
            None if is_member => {
                // Unlisted workspace members are project-owned by default.
            }
            None => {
                return Err(format!(
                    "S01 local-fork advisory: unknown local package {name} {version} is not reviewed"
                ));
            }
        }
    }
    Ok((derived, evidence))
}

/// Runs the bounded inventory, writes the labelled derived advisory view and
/// returns the evidence record for the caller to print or retain.
pub fn run(root: &Path) -> Result<Value> {
    let (entries, mapping_sha) = mappings(root)?;
    let mut records = Vec::new();
    let metadata = command(
        root,
        "cargo",
        &["metadata", "--locked", "--format-version", "1"],
        &mut records,
    )?;
    let raw_metadata_sha = digest(metadata.as_bytes());
    let metadata: Value = serde_json::from_str(&metadata).map_err(|e| e.to_string())?;
    let (derived, evidence) = transform_metadata(&metadata, &entries)?;
    need(!evidence.is_empty(), "no reviewed local fork was exercised")?;
    let derived_bytes = serde_json::to_vec_pretty(&derived).map_err(|e| e.to_string())?;
    let output = root.join(DERIVED);
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    fs::write(&output, &derived_bytes).map_err(|e| e.to_string())?;
    Ok(json!({
        "plan": "S01",
        "role": "local-fork-advisory-derived-metadata",
        "mappings": MAPPINGS,
        "mappings_sha256": mapping_sha,
        "physical_metadata_sha256": raw_metadata_sha,
        "derived_metadata_path": DERIVED,
        "derived_metadata_sha256": digest(&derived_bytes),
        "derived_metadata_bytes": derived_bytes.len(),
        "entries": evidence,
        "commands": records,
        "scope": "upstream advisory identity view only; not resolver, feature, TLS or shipping evidence",
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry() -> Entry {
        Entry {
            kind: "third_party_fork".to_owned(),
            local_name: "ssx-cli-hyper".to_owned(),
            local_version: "1.12.0".to_owned(),
            upstream_name: Some("hyper".to_owned()),
            upstream_version: Some("1.12.0".to_owned()),
            upstream_source: Some(
                "registry+https://github.com/rust-lang/crates.io-index".to_owned(),
            ),
        }
    }

    fn metadata() -> Value {
        json!({
            "workspace_members": ["path+file:///repo#1.12.0"],
            "packages": [
                {
                    "id": "path+file:///repo#1.12.0",
                    "name": "ssx-cli-hyper",
                    "version": "1.12.0",
                    "source": null,
                },
                {
                    "id": "registry+https://github.com/rust-lang/crates.io-index#hyper@1.12.0",
                    "name": "hyper",
                    "version": "1.12.0",
                    "source": "registry+https://github.com/rust-lang/crates.io-index",
                }
            ]
        })
    }

    #[test]
    fn reviewed_fork_gains_upstream_identity_while_id_and_edges_stay() {
        let (derived, evidence) = transform_metadata(&metadata(), &[entry()]).expect("transform");
        assert_eq!(derived["packages"][0]["name"], "hyper");
        assert_eq!(
            derived["packages"][0]["source"],
            "registry+https://github.com/rust-lang/crates.io-index"
        );
        assert_eq!(derived["packages"][0]["id"], "path+file:///repo#1.12.0");
        assert_eq!(evidence[0]["kind"], "third_party_fork");
    }

    #[test]
    fn unmapped_non_workspace_local_package_fails_closed() {
        let mut value = metadata();
        value["workspace_members"] = json!([]);
        value["packages"][0]["name"] = json!("unknown-fork");
        let error = transform_metadata(&value, &[entry()]).expect_err("must reject");
        assert!(error.contains("unknown local package"));
    }

    #[test]
    fn project_owned_entry_is_untouched() {
        let owned = Entry {
            kind: "project_owned".to_owned(),
            local_name: "ssx-cli-hyper".to_owned(),
            local_version: "1.12.0".to_owned(),
            upstream_name: None,
            upstream_version: None,
            upstream_source: None,
        };
        let (derived, evidence) = transform_metadata(&metadata(), &[owned]).expect("transform");
        assert_eq!(derived["packages"][0]["name"], "ssx-cli-hyper");
        assert_eq!(derived["packages"][0]["source"], Value::Null);
        assert_eq!(evidence[0]["kind"], "project_owned");
    }
}
