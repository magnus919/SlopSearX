//! One finite lock successor; historical captures retain their original profile.
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, fs, path::Path};

type Result<T> = std::result::Result<T, String>;
pub const SOURCE_PATH: &str = "uv.lock";
pub const HISTORICAL_SHA256: &str =
    "8ecfcd029d2461631d44a2910ef5cfaa23a436da9cbc9a96ed0302e1d96861d0";
const HISTORICAL_BLOB: &str = "24e92a80e8ecc761880fe603a01da41312dea180";
const CURRENT_BLOB: &str = "ffd869ca0dab8d8f749bdbcb1ffba9899a15518c";
const MANIFEST_PATH: &str =
    "docs/rust-rewrite/t06/source-amendments/current-lock-amendment-v1.json";
const MANIFEST_SHA256: &str = "852f1e2971f5ba3fc5b8a71e5ed4ef7b50d27d14bdf1078406fb83f57cbc5662";
const ALLOWED: [&str; 3] = ["filelock", "packaging", "virtualenv"];

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Amendment {
    schema_version: u32,
    kind: String,
    source_path: String,
    historical: Historical,
    current: Current,
    inverse_entries: Vec<Entry>,
    normal_runtime: serde_json::Value,
    protocol_comparison: serde_json::Value,
    scope: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Historical {
    revision: String,
    sha256: String,
    git_blob: String,
    bytes: usize,
    mode: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Current {
    revision_introduced: String,
    sha256: String,
    git_blob: String,
    bytes: usize,
    mode: String,
}
#[derive(Deserialize, Clone)]
#[serde(deny_unknown_fields)]
struct Entry {
    name: String,
    scope: String,
    before_utf8: String,
    after_utf8: String,
}
fn digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
fn require(ok: bool, message: &str) -> Result<()> {
    if ok {
        Ok(())
    } else {
        Err(format!("A06 exact security lock amendment: {message}"))
    }
}
fn packages(source: &str) -> Result<BTreeMap<&str, &str>> {
    let mut result = BTreeMap::new();
    for block in source.split("[[package]]\n").skip(1) {
        let name = block
            .lines()
            .next()
            .and_then(|line| line.strip_prefix("name = \""))
            .and_then(|line| line.strip_suffix('"'))
            .ok_or("A06 missing exact lock package name")?;
        require(result.insert(name, block).is_none(), "duplicate package")?;
    }
    Ok(result)
}
fn reconstruct(entries: &[Entry], current: &str) -> Result<String> {
    require(
        entries.iter().map(|entry| entry.name.as_str()).eq(ALLOWED),
        "only the three named package entries may change, exactly once",
    )?;
    let mut historical = current.to_owned();
    for (entry, (before, after)) in entries.iter().zip([
        ("4.1.0", "4.2.1"),
        ("26.2", "26.3"),
        ("21.14.5", "21.14.10"),
    ]) {
        let prefix = format!("[[package]]\nname = \"{}\"\nversion = ", entry.name);
        require(
            entry
                .before_utf8
                .starts_with(&format!("{prefix}\"{before}\"\n"))
                && entry
                    .after_utf8
                    .starts_with(&format!("{prefix}\"{after}\"\n"))
                && entry.scope
                    == if entry.name == "packaging" {
                        "runtime-shared"
                    } else {
                        "development-only"
                    },
            "entry versions or runtime/development classification differ",
        )?;
        require(
            current.matches(&entry.after_utf8).count() == 1
                && current.matches(&entry.before_utf8).count() == 0
                && entry.after_utf8.matches("[[package]]\n").count() == 1
                && entry.before_utf8.matches("[[package]]\n").count() == 1,
            "inverse span is missing, ambiguous, historical or outside one package",
        )?;
        historical = historical.replacen(&entry.after_utf8, &entry.before_utf8, 1);
    }
    let old_packages = packages(&historical)?;
    let new_packages = packages(current)?;
    require(
        old_packages.len() == 138
            && old_packages.keys().eq(new_packages.keys())
            && old_packages
                .iter()
                .filter(|(name, block)| new_packages.get(**name) != Some(*block))
                .map(|(name, _)| *name)
                .eq(ALLOWED),
        "package inventory or exact three-entry change set differs",
    )?;
    require(
        historical.len() == 470753 && digest(historical.as_bytes()) == HISTORICAL_SHA256,
        "inverse does not recover the complete Git-archived historical lock",
    )?;
    Ok(historical)
}
fn validate_materials(
    raw: &[u8],
    path: &str,
    historical_pin: &str,
    current: &[u8],
    mode: u32,
) -> Result<()> {
    require(
        digest(raw) == MANIFEST_SHA256,
        "manifest byte identity differs",
    )?;
    let manifest: Amendment = serde_json::from_slice(raw).map_err(|e| e.to_string())?;
    require(
        manifest.schema_version == 1
            && manifest.kind == "A06_EXACT_SECURITY_LOCK_AMENDMENT_V1"
            && manifest.source_path == SOURCE_PATH
            && path == SOURCE_PATH
            && historical_pin == HISTORICAL_SHA256
            && !manifest.scope.is_empty(),
        "caller path, original row hash or amendment scope differs",
    )?;
    require(
        manifest.historical.revision == "6c44976444340cc8f343fef61e2e8d723b189c19"
            && manifest.historical.sha256 == HISTORICAL_SHA256
            && manifest.historical.git_blob == HISTORICAL_BLOB
            && manifest.historical.bytes == 470753
            && manifest.historical.mode == "0644"
            && manifest.current.revision_introduced == "19fa47f414930ad416488457ce8406e55554f4e4"
            && manifest.current.git_blob == CURRENT_BLOB
            && manifest.current.bytes == 470758
            && manifest.current.mode == "0644"
            && mode == 0o644
            && current.len() == manifest.current.bytes
            && digest(current) == manifest.current.sha256,
        "historical/current full-file hash, bytes, Git provenance or mode differs",
    )?;
    let current = std::str::from_utf8(current).map_err(|e| e.to_string())?;
    let historical = reconstruct(&manifest.inverse_entries, current)?;
    let old = packages(&historical)?;
    let new = packages(current)?;
    let runtime = &manifest.normal_runtime;
    let closure = runtime["closure"]
        .as_array()
        .ok_or("A06 runtime closure absent")?;
    require(
        closure.len() == 85
            && closure.iter().any(|name| name == "packaging")
            && !closure
                .iter()
                .any(|name| name == "filelock" || name == "virtualenv")
            && runtime["changed_entries"] == serde_json::json!(["packaging"])
            && runtime["unchanged_closure_entries"] == 84,
        "shared runtime packaging amendment must be disclosed",
    )?;
    for name in closure {
        let name = name.as_str().ok_or("A06 runtime package name absent")?;
        require(
            old.contains_key(name)
                && new.contains_key(name)
                && (name == "packaging" || old[name] == new[name]),
            "another normal runtime package changed",
        )?;
    }
    let sdk = runtime["sdk_package_entries"]
        .as_array()
        .ok_or("A06 SDK entries absent")?;
    require(
        sdk.iter().filter_map(|row| row["name"].as_str()).eq([
            "fastmcp",
            "fastmcp-slim",
            "mcp",
            "mcp-types",
            "pydantic",
            "pyyaml",
        ]),
        "exact SDK entry inventory differs",
    )?;
    for row in sdk {
        let name = row["name"].as_str().ok_or("A06 SDK entry name absent")?;
        let block = format!("[[package]]\n{}", new[name]);
        require(
            old[name] == new[name]
                && row["bytes"].as_u64() == Some(block.len() as u64)
                && row["sha256"] == digest(block.as_bytes()),
            "SDK package entry changed",
        )?;
    }
    Ok(())
}

fn compare_packets(old: &serde_json::Value, new: &serde_json::Value) -> Result<()> {
    for key in [
        "baseline",
        "sources",
        "producer_sha256",
        "bindings_sha256",
        "interpreter",
        "tools",
        "full_catalog",
        "cases",
        "provider_calls",
        "normalization",
    ] {
        require(
            old[key] == new[key] && !old[key].is_null(),
            "packaging wire comparison differs or is incomplete",
        )?;
    }
    require(
        old["versions"]
            == serde_json::json!({"fastmcp":"4.0.11","mcp":"2.3.0","pydantic":"2.13.4","packaging":"26.2"})
            && new["versions"]
                == serde_json::json!({"fastmcp":"4.0.11","mcp":"2.3.0","pydantic":"2.13.4","packaging":"26.3"})
            && old["interpreter"]["version"]
                .as_str()
                .is_some_and(|s| s.starts_with("3.14.7 "))
            && old["full_catalog"].as_array().map(Vec::len) == Some(38)
            && old["cases"].as_array().map(Vec::len) == Some(22)
            && old["provider_calls"] == 0
            && old["normalization"] == "none",
        "finite measured packaging profile or coverage differs",
    )?;
    for name in ["fastmcp", "mcp", "pydantic"] {
        require(
            !old["sdk_files"][name].is_null() && old["sdk_files"][name] == new["sdk_files"][name],
            "measured SDK source identity differs",
        )?;
    }
    // The overlay observes the changed runtime dependency while retaining the
    // historical source guard. It does not rewrite the old T06 observations.
    let frozen: serde_json::Value =
        serde_json::from_slice(include_bytes!("../source_bindings_v2.json"))
            .map_err(|e| e.to_string())?;
    for row in frozen["sources"]
        .as_array()
        .ok_or("A06 frozen source rows absent")?
    {
        let path = row["file"]
            .as_str()
            .ok_or("A06 frozen source path absent")?;
        require(
            old["sources"][path] == row["sha256"],
            "measured Python source differs from historical T06 source",
        )?;
    }
    Ok(())
}
fn validate_protocol(manifest: &Amendment, packets: [&[u8]; 2], producer: &[u8]) -> Result<()> {
    let proof = &manifest.protocol_comparison;
    let rows = proof["observations"]
        .as_array()
        .ok_or("A06 packaging observations absent")?;
    require(
        rows.len() == 2
            && proof["normalization"] == "none"
            && proof["catalog_tools"] == 38
            && proof["negative_calls"] == 22
            && proof["provider_calls"] == 0,
        "packaging comparison scope differs",
    )?;
    for (row, bytes) in rows.iter().zip(packets) {
        require(
            row["sha256"] == digest(bytes) && row["bytes"].as_u64() == Some(bytes.len() as u64),
            "measured packaging packet byte identity differs",
        )?;
    }
    require(
        proof["producer"]["sha256"] == digest(producer)
            && proof["producer"]["bytes"].as_u64() == Some(producer.len() as u64),
        "measured packaging producer identity differs",
    )?;
    let old: serde_json::Value = serde_json::from_slice(packets[0]).map_err(|e| e.to_string())?;
    let new: serde_json::Value = serde_json::from_slice(packets[1]).map_err(|e| e.to_string())?;
    require(
        old["producer_sha256"] == proof["producer"]["sha256"],
        "measured producer binding differs",
    )?;
    compare_packets(&old, &new)
}

pub fn verify(root: &Path, path: &str, historical_pin: &str) -> Result<()> {
    require(path == SOURCE_PATH, "wrong source path")?;
    let raw = fs::read(root.join(MANIFEST_PATH)).map_err(|e| e.to_string())?;
    let source = root.join(SOURCE_PATH);
    let metadata = fs::symlink_metadata(&source).map_err(|e| e.to_string())?;
    require(
        metadata.file_type().is_file(),
        "current lock must be a regular file",
    )?;
    #[cfg(unix)]
    let mode = {
        use std::os::unix::fs::PermissionsExt;
        metadata.permissions().mode() & 0o7777
    };
    #[cfg(not(unix))]
    let mode = 0; // This qualification currently supports Linux and macOS only.
    let current = fs::read(source).map_err(|e| e.to_string())?;
    validate_materials(&raw, path, historical_pin, &current, mode)?;
    let manifest: Amendment = serde_json::from_slice(&raw).map_err(|e| e.to_string())?;
    let proof = &manifest.protocol_comparison;
    let read = |row: &serde_json::Value| -> Result<Vec<u8>> {
        let path = row["path"]
            .as_str()
            .ok_or("A06 measured packet/producer path absent")?;
        fs::read(root.join(path)).map_err(|e| e.to_string())
    };
    let old = read(&proof["observations"][0])?;
    let new = read(&proof["observations"][1])?;
    let producer = read(&proof["producer"])?;
    let provenance = read(&proof["provenance"])?;
    require(
        proof["provenance"]["sha256"] == digest(&provenance)
            && proof["provenance"]["bytes"].as_u64() == Some(provenance.len() as u64),
        "manual capture command provenance differs",
    )?;
    validate_protocol(&manifest, [&old, &new], &producer)
}

pub fn verify_git_pins(path: &str, row: &str, archived: &str, current: &str) -> Result<()> {
    require(
        path == SOURCE_PATH
            && row == HISTORICAL_BLOB
            && archived == HISTORICAL_BLOB
            && current == CURRENT_BLOB,
        "original row/archive or actual current Git blob differs",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    const RAW: &[u8] = include_bytes!("current-lock-amendment-v1.json");
    const LIVE: &[u8] = include_bytes!("../../../../uv.lock");
    const OLD_WIRE: &[u8] = include_bytes!("packaging262-wire.json");
    const NEW_WIRE: &[u8] = include_bytes!("packaging263-wire.json");
    const PRODUCER: &[u8] = include_bytes!("packaging-wire-producer.py.txt");

    #[test]
    fn exact_security_lock_successor_retains_historical_profile_and_shared_runtime_disclosure() {
        validate_materials(RAW, SOURCE_PATH, HISTORICAL_SHA256, LIVE, 0o644).unwrap();
        verify_git_pins(SOURCE_PATH, HISTORICAL_BLOB, HISTORICAL_BLOB, CURRENT_BLOB).unwrap();
        let manifest: Amendment = serde_json::from_slice(RAW).unwrap();
        validate_protocol(&manifest, [OLD_WIRE, NEW_WIRE], PRODUCER).unwrap();
        assert!(validate_protocol(&manifest, [OLD_WIRE, OLD_WIRE], PRODUCER).is_err());
        assert!(validate_protocol(&manifest, [OLD_WIRE, NEW_WIRE], b"changed producer").is_err());
        let old = reconstruct(
            &manifest.inverse_entries,
            std::str::from_utf8(LIVE).unwrap(),
        )
        .unwrap();
        assert_eq!(digest(old.as_bytes()), HISTORICAL_SHA256);
        assert!(
            validate_materials(RAW, SOURCE_PATH, HISTORICAL_SHA256, old.as_bytes(), 0o644).is_err()
        );
    }

    #[test]
    fn changed_entry_sdk_and_unrelated_lock_mutants_cannot_use_the_amendment() {
        let live = std::str::from_utf8(LIVE).unwrap();
        for (before, after) in [
            (
                "name = \"filelock\"\nversion = \"4.2.1\"",
                "name = \"filelock\"\nversion = \"4.2.2\"",
            ),
            (
                "name = \"packaging\"\nversion = \"26.3\"",
                "name = \"packaging\"\nversion = \"26.4\"",
            ),
            (
                "name = \"virtualenv\"\nversion = \"21.14.10\"",
                "name = \"virtualenv\"\nversion = \"21.14.11\"",
            ),
            (
                "name = \"fastmcp\"\nversion = \"4.0.11\"",
                "name = \"fastmcp\"\nversion = \"4.0.12\"",
            ),
            (
                "name = \"mcp\"\nversion = \"2.3.0\"",
                "name = \"mcp\"\nversion = \"2.3.1\"",
            ),
            (
                "name = \"pydantic\"\nversion = \"2.13.4\"",
                "name = \"pydantic\"\nversion = \"2.13.5\"",
            ),
            ("resolution-markers = [", "resolution-markers = [ # changed"),
        ] {
            assert_eq!(
                live.matches(before).count(),
                1,
                "negative target absent: {before}"
            );
            let wrong = live.replacen(before, after, 1);
            assert!(
                validate_materials(RAW, SOURCE_PATH, HISTORICAL_SHA256, wrong.as_bytes(), 0o644)
                    .is_err()
            );
        }
    }

    #[test]
    fn inverse_scope_rejects_missing_duplicate_extra_and_tampered_package_spans() {
        let manifest: Amendment = serde_json::from_slice(RAW).unwrap();
        let live = std::str::from_utf8(LIVE).unwrap();
        let entries = manifest.inverse_entries;
        let mut wrong = entries.clone();
        wrong.pop();
        assert!(reconstruct(&wrong, live).is_err());
        let mut wrong = entries.clone();
        wrong.push(entries[0].clone());
        assert!(reconstruct(&wrong, live).is_err());
        let mut wrong = entries.clone();
        wrong[0].name = "fastmcp".into();
        assert!(reconstruct(&wrong, live).is_err());
        let mut wrong = entries.clone();
        wrong[0].before_utf8.push(' ');
        assert!(reconstruct(&wrong, live).is_err());
        let mut wrong = entries.clone();
        wrong[1].scope = "development-only".into();
        assert!(reconstruct(&wrong, live).is_err());
        let old_packet: serde_json::Value = serde_json::from_slice(OLD_WIRE).unwrap();
        let new_packet: serde_json::Value = serde_json::from_slice(NEW_WIRE).unwrap();
        for key in ["full_catalog", "cases", "sources"] {
            let mut wrong = new_packet.clone();
            wrong[key] = serde_json::Value::Null;
            assert!(compare_packets(&old_packet, &wrong).is_err());
        }
        // Exercise reconstruction without the outer current hash: a runtime
        // mutation outside the inverse spans must still miss the frozen preimage.
        let wrong = live.replacen(
            "name = \"mcp\"\nversion = \"2.3.0\"",
            "name = \"mcp\"\nversion = \"2.3.1\"",
            1,
        );
        assert!(reconstruct(&entries, &wrong).is_err());
    }

    #[test]
    fn source_path_row_hash_manifest_git_mode_and_missing_material_controls_fail_closed() {
        assert!(validate_materials(RAW, "other.lock", HISTORICAL_SHA256, LIVE, 0o644).is_err());
        assert!(validate_materials(RAW, SOURCE_PATH, &"0".repeat(64), LIVE, 0o644).is_err());
        assert!(validate_materials(RAW, SOURCE_PATH, HISTORICAL_SHA256, LIVE, 0o755).is_err());
        assert!(validate_materials(RAW, SOURCE_PATH, HISTORICAL_SHA256, b"", 0o644).is_err());
        let mut tampered: serde_json::Value = serde_json::from_slice(RAW).unwrap();
        tampered["current"]["sha256"] = serde_json::json!(digest(b"tampered"));
        assert!(
            validate_materials(
                &serde_json::to_vec(&tampered).unwrap(),
                SOURCE_PATH,
                HISTORICAL_SHA256,
                b"tampered",
                0o644
            )
            .is_err()
        );
        for (path, row, archive, current) in [
            ("other.lock", HISTORICAL_BLOB, HISTORICAL_BLOB, CURRENT_BLOB),
            (SOURCE_PATH, "wrong", HISTORICAL_BLOB, CURRENT_BLOB),
            (SOURCE_PATH, HISTORICAL_BLOB, "wrong", CURRENT_BLOB),
            (SOURCE_PATH, HISTORICAL_BLOB, HISTORICAL_BLOB, "wrong"),
            (
                SOURCE_PATH,
                HISTORICAL_BLOB,
                HISTORICAL_BLOB,
                HISTORICAL_BLOB,
            ),
        ] {
            assert!(verify_git_pins(path, row, archive, current).is_err());
        }
        assert!(
            verify(
                Path::new("/nonexistent-a06-lock-proof-root"),
                SOURCE_PATH,
                HISTORICAL_SHA256
            )
            .is_err()
        );
    }
}
