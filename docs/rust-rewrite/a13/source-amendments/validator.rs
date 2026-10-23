//! Narrow shared validator for exact A13 historical-source amendments.
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{fs, path::Path};

#[path = "../../storage-document-amendment/validator.rs"]
pub(crate) mod storage_document;
const MANIFEST_PATH: &str =
    "docs/rust-rewrite/a13/source-amendments/a13-current-source-amendment-v6.json";
const MANIFEST_SHA256: &str = "69abef686c432ca207a5a3905bc5d3829763d879e227dbebeaf9f01fc27b79e5";
const PREDECESSOR_PATH: &str =
    "docs/rust-rewrite/a13/source-amendments/a13-current-source-amendment-v5.json";
const PREDECESSOR_SHA256: &str = "4b64a708e58858425e4f7758333caef9ad4a4037e48c083702c3c3da0c71f3a8";
const AMENDED_PATHS: [&str; 9] = [
    "crates/ssx-infra/src/http.rs",
    "crates/ssx-infra/tests/a13_dehashed.rs",
    "crates/ssx-infra/tests/a13_remaining_fixture/mod.rs",
    "crates/xtask/src/a11.rs",
    "crates/ssx-infra/src/adapters/clinicaltrials.rs",
    "crates/ssx-infra/src/adapters/mod.rs",
    "crates/ssx-infra/src/search.rs",
    "crates/ssx-infra/src/search_deadline_drain_tests.rs",
    "crates/xtask/src/a09.rs",
];

const LOCK_MANIFEST_PATH: &str =
    "docs/rust-rewrite/a13/source-amendments/a13-security-lock-amendment-v1.json";
const LOCK_MANIFEST_SHA256: &str =
    "e0186db3ff2ef6db560b4e8b75c70b094987ff68b72d780844a5ba81588e9bb3";
const LOCK_AMENDED_PATHS: [&str; 10] = [
    "crates/ssx-infra/src/http.rs",
    "crates/ssx-infra/tests/a13_dehashed.rs",
    "crates/ssx-infra/tests/a13_remaining_fixture/mod.rs",
    "crates/xtask/src/a11.rs",
    "crates/ssx-infra/src/adapters/clinicaltrials.rs",
    "crates/ssx-infra/src/adapters/mod.rs",
    "crates/ssx-infra/src/search.rs",
    "crates/ssx-infra/src/search_deadline_drain_tests.rs",
    "crates/xtask/src/a09.rs",
    "Cargo.lock",
];

const LOCK_BASELINE_PATH: &str =
    "docs/rust-rewrite/a13/source-amendments/a13-current-source-amendment-v5.json";
const LOCK_BASELINE_SHA256: &str =
    "4b64a708e58858425e4f7758333caef9ad4a4037e48c083702c3c3da0c71f3a8";

fn digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn require(ok: bool, message: &str) -> Result<(), String> {
    if ok { Ok(()) } else { Err(message.into()) }
}

#[cfg(unix)]
fn mode(path: &Path) -> Result<String, String> {
    use std::os::unix::fs::PermissionsExt;
    Ok(format!(
        "{:04o}",
        fs::metadata(path)
            .map_err(|error| error.to_string())?
            .permissions()
            .mode()
            & 0o777
    ))
}

#[cfg(not(unix))]
fn mode(_path: &Path) -> Result<String, String> {
    Ok("0644".into())
}

fn validate_scope(manifest: &Value) -> Result<(), String> {
    require(
        manifest["kind"] == "A13_CURRENT_SOURCE_AMENDMENT_V6"
            && manifest["amended_source_paths"] == serde_json::json!(AMENDED_PATHS),
        "A13 amendment path scope changed",
    )?;
    let amendments = manifest["amendments"]
        .as_array()
        .ok_or("A13 amendment entries absent")?;
    require(
        amendments.len() == AMENDED_PATHS.len()
            && amendments
                .iter()
                .map(|entry| entry["source_path"].as_str().unwrap_or(""))
                .eq(AMENDED_PATHS),
        "A13 amendment membership changed",
    )
}

fn verify_additional_source(
    manifest: &Value,
    bytes: &[u8],
    source_mode: &str,
) -> Result<(), String> {
    let bindings = manifest["additional_sources"]
        .as_array()
        .ok_or("A13 additional source bindings absent")?;
    require(
        bindings.len() == 1
            && bindings[0]["path"] == "crates/ssx-infra/tests/support/http_expired_startup.rs"
            && bindings[0]["bytes"].as_u64() == Some(bytes.len() as u64)
            && bindings[0]["sha256"].as_str() == Some(digest(bytes).as_str())
            && bindings[0]["mode"] == source_mode,
        "A13 exact HTTP startup control source differs",
    )
}

fn verify_additional_source_file(root: &Path, manifest: &Value) -> Result<(), String> {
    let path = root.join("crates/ssx-infra/tests/support/http_expired_startup.rs");
    let bytes = fs::read(&path).map_err(|error| error.to_string())?;
    let source_mode = mode(&path)?;
    let historical = causal_successor::a06_source_successor::real_clock_additional_source(
        root,
        manifest,
        &bytes,
        &source_mode,
    )?;
    verify_additional_source(manifest, &historical, "0644")
}

struct PinCheck<'a> {
    amendment: &'a Value,
    container_path: &'a str,
    container_sha: &'a str,
    container_bytes: &'a [u8],
    source_path: &'a str,
    historical_sha: &'a str,
    historical_bytes: Option<usize>,
    current_bytes: &'a [u8],
    archive_bytes: &'a [u8],
    current_mode: &'a str,
    archive_mode: &'a str,
}

fn verify_payload(check: PinCheck<'_>) -> Result<(), String> {
    let PinCheck {
        amendment,
        container_path,
        container_sha,
        container_bytes,
        source_path,
        historical_sha,
        historical_bytes,
        current_bytes,
        archive_bytes,
        current_mode,
        archive_mode,
    } = check;
    require(
        amendment["source_path"] == source_path,
        "A13 amendment source path differs",
    )?;
    let pins = amendment["historical_pins"]
        .as_array()
        .ok_or("A13 historical pins absent")?;
    let matching = pins
        .iter()
        .filter(|pin| pin["container_path"] == container_path)
        .collect::<Vec<_>>();
    require(
        matching.len() == 1,
        "A13 historical container binding differs",
    )?;
    let pin = matching[0];
    let archived_len = amendment["archive"]["bytes"]
        .as_u64()
        .ok_or("A13 archive byte count absent")? as usize;
    require(
        pin["container_sha256"] == container_sha
            && digest(container_bytes) == container_sha
            && pin["source_sha256"] == historical_sha
            && pin["source_bytes"].as_u64() == Some(archived_len as u64)
            && historical_bytes.is_none_or(|expected| expected == archived_len)
            && pin["source_mode"] == amendment["archive"]["mode"],
        "A13 historical source binding differs",
    )?;
    let archived_sha = amendment["archive"]["sha256"]
        .as_str()
        .ok_or("A13 archive digest absent")?;
    let archived_len = amendment["archive"]["bytes"]
        .as_u64()
        .ok_or("A13 archive byte count absent")? as usize;
    let successor_sha = amendment["successor"]["sha256"]
        .as_str()
        .ok_or("A13 successor digest absent")?;
    let successor_len = amendment["successor"]["bytes"]
        .as_u64()
        .ok_or("A13 successor byte count absent")? as usize;
    require(
        amendment["archive"]["mode"] == archive_mode
            && amendment["successor"]["mode"] == current_mode
            && amendment["successor"]["path"] == source_path
            && historical_sha == archived_sha
            && archive_bytes.len() == archived_len
            && digest(archive_bytes) == archived_sha
            && current_bytes.len() == successor_len
            && digest(current_bytes) == successor_sha,
        "A13 archive or reviewed successor differs",
    )
}

fn replace_lock_package(
    lock: &str,
    name: &str,
    old_version: &str,
    new_version: &str,
    old_checksum: &str,
    new_checksum: &str,
) -> Result<String, String> {
    let blocks: Vec<&str> = lock.split("[[package]]\n").collect();
    let mut found = 0;
    let mut result = String::new();
    for (index, block) in blocks.iter().enumerate() {
        if index > 0 {
            result.push_str("[[package]]\n");
        }
        let name_line = format!(r#"name = "{name}""#);
        if block.lines().any(|line| line == name_line) {
            found += 1;
            let old_ver = format!(r#"version = "{old_version}""#);
            let new_ver = format!(r#"version = "{new_version}""#);
            let old_sum = format!(r#"checksum = "{old_checksum}""#);
            let new_sum = format!(r#"checksum = "{new_checksum}""#);
            let registry = r#"source = "registry+https://github.com/rust-lang/crates.io-index""#;
            require(
                block.matches(&name_line).count() == 1
                    && block.matches(&old_ver).count() == 1
                    && block.matches(&old_sum).count() == 1
                    && block.matches(registry).count() == 1,
                "A13 lock archived package identity differs",
            )?;
            let changed = block
                .replace(&old_ver, &new_ver)
                .replace(&old_sum, &new_sum);
            require(
                changed.matches(&new_ver).count() == 1 && changed.matches(&new_sum).count() == 1,
                "A13 lock package replacement ambiguous",
            )?;
            result.push_str(&changed);
        } else {
            result.push_str(block);
        }
    }
    require(found == 1, "A13 lock package missing/duplicate")?;
    Ok(result)
}

fn verify_lock_reconstruction(root: &Path, archive: &[u8], current: &[u8]) -> Result<(), String> {
    let old = std::str::from_utf8(archive).map_err(|error| error.to_string())?;
    let current = std::str::from_utf8(current).map_err(|error| error.to_string())?;
    let first = replace_lock_package(
        old,
        "rustls-webpki",
        "0.103.15",
        "0.103.16",
        "f3c3cf1d8b1e7d4927e2d154c3fcb02979afb9939629c62cd9048d4f07b60ac2",
        "4443b1b6d39d7c663139942cc4558262562d2c84123bb6282ef177009ca8b281",
    )?;
    let expected = replace_lock_package(
        &first,
        "webpki-root-certs",
        "1.0.9",
        "1.0.10",
        "b96554aa2acc8ccdb7e1c9a58a7a68dd5d13bccc69cd124cb09406db612a1c9b",
        "58ec80e73fd8baf57bf0d9615554c75348bda990d6d5bbeab964dd068319fecb",
    )?;
    if expected.as_bytes() == current.as_bytes() {
        return Ok(());
    }
    // Both exact TLS replacements above are still mandatory. The resulting
    // expected bytes must be the sealed accepted-main baseline, and the supplied
    // current bytes must be the actual fully reconstructed A06 successor.
    causal_successor::a06_source_successor::security_lock_successor(
        root,
        expected.as_bytes(),
        current.as_bytes(),
    )
    .map_err(|error| format!("A13 Cargo.lock contains an unapproved delta: {error}"))
}

fn read_archive(root: &Path, rel: &str) -> Result<(std::path::PathBuf, Vec<u8>), String> {
    let rel_path = Path::new(rel);
    require(
        !rel_path.is_absolute() && !rel.split('/').any(|part| part == ".."),
        "A13 archive path escapes repository",
    )?;
    let path = root.join(rel_path);
    let bytes = fs::read(&path).map_err(|error| format!("{}: {error}", path.display()))?;
    Ok((path, bytes))
}

fn validate_lock_scope(sidecar: &Value, v5: &Value) -> Result<(), String> {
    require(
        sidecar["kind"] == "A13_SECURITY_LOCK_AMENDMENT_V1"
            && sidecar["predecessor_manifest"]["path"] == LOCK_BASELINE_PATH
            && sidecar["predecessor_manifest"]["sha256"] == LOCK_BASELINE_SHA256
            && sidecar["amended_source_paths"] == serde_json::json!(LOCK_AMENDED_PATHS),
        "A13 security lock sidecar identity/scope changed",
    )?;
    let amendments = sidecar["amendments"]
        .as_array()
        .ok_or("A13 lock amendments absent")?;
    let prior = v5["amendments"].as_array().ok_or("A13 V5 prefix absent")?;
    require(
        prior.len() == 9
            && amendments.len() == 10
            && amendments[..9] == prior[..]
            && amendments[9]["source_path"] == "Cargo.lock",
        "A13 security lock sidecar changed V5 prefix",
    )
}

fn load_lock_baseline(root: &Path) -> Result<Value, String> {
    let bytes = fs::read(root.join(LOCK_BASELINE_PATH)).map_err(|error| error.to_string())?;
    require(
        digest(&bytes) == LOCK_BASELINE_SHA256,
        "A13 lock V5 baseline differs",
    )?;
    serde_json::from_slice(&bytes).map_err(|error| error.to_string())
}

fn verify_previous_successor(
    root: &Path,
    amendment: &Value,
    override_bytes: Option<&[u8]>,
) -> Result<(), String> {
    let source_path = amendment["source_path"]
        .as_str()
        .ok_or("A13 amendment source path absent")?;
    let has_predecessor = amendment.get("previous_successor_archive").is_some();
    let previous = amendment.get("previous_successor_archive");
    if !has_predecessor {
        return Ok(());
    }
    let previous = previous.ok_or("A13 accepted predecessor archive absent")?;
    let rel = previous["path"]
        .as_str()
        .ok_or("A13 predecessor archive path absent")?;
    let rel_path = Path::new(rel);
    require(
        !rel_path.is_absolute()
            && !rel.split('/').any(|part| part == "..")
            && previous["source_path"] == source_path
            && amendment["successor"]["predecessor_sha256"] == previous["sha256"],
        "A13 predecessor archive identity differs",
    )?;
    let archive_path = root.join(rel_path);
    let disk_bytes = fs::read(&archive_path).map_err(|error| error.to_string())?;
    let bytes = override_bytes.unwrap_or(&disk_bytes);
    require(
        previous["mode"] == mode(&archive_path)?
            && previous["bytes"].as_u64() == Some(bytes.len() as u64)
            && previous["sha256"].as_str() == Some(digest(bytes).as_str()),
        "A13 previous-successor archive bytes/mode differ",
    )
}

#[cfg(test)]
fn rejects_clinicaltrials_mutants(root: &Path) -> Result<(), String> {
    let manifest: Value = serde_json::from_slice(
        &fs::read(root.join(MANIFEST_PATH)).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    let path = "crates/ssx-infra/src/adapters/clinicaltrials.rs";
    let amendment = manifest["amendments"]
        .as_array()
        .and_then(|rows| rows.iter().find(|row| row["source_path"] == path))
        .ok_or("A13 ClinicalTrials amendment absent")?
        .clone();
    let pin = amendment["historical_pins"]
        .as_array()
        .and_then(|rows| rows.first())
        .ok_or("A13 ClinicalTrials historical pin absent")?;
    let container_path = pin["container_path"]
        .as_str()
        .ok_or("A13 ClinicalTrials container absent")?;
    let container = fs::read(root.join(container_path)).map_err(|error| error.to_string())?;
    let container_sha = pin["container_sha256"]
        .as_str()
        .ok_or("A13 ClinicalTrials container SHA absent")?;
    let actual_current = fs::read(root.join(path)).map_err(|error| error.to_string())?;
    let current = causal_successor::a06_source_successor::legacy_control_operand(
        root,
        path,
        &amendment["successor"],
        &actual_current,
    )?;
    let archive_path = amendment["archive"]["path"]
        .as_str()
        .ok_or("A13 ClinicalTrials archive absent")?;
    let archive_file = root.join(archive_path);
    let archive = fs::read(&archive_file).map_err(|error| error.to_string())?;
    let current_path = root.join(path);
    let check = |entry: &Value, current_bytes: &[u8], archive_bytes: &[u8]| {
        verify_payload(PinCheck {
            amendment: entry,
            container_path,
            container_sha,
            container_bytes: &container,
            source_path: path,
            historical_sha: pin["source_sha256"].as_str().unwrap(),
            historical_bytes: pin["source_bytes"].as_u64().map(|value| value as usize),
            current_bytes,
            archive_bytes,
            current_mode: &mode(&current_path)?,
            archive_mode: &mode(&archive_file)?,
        })
    };
    require(
        check(&amendment, &current, &archive).is_ok(),
        "A13 ClinicalTrials amendment positive control failed",
    )?;
    let mut bad_successor = amendment.clone();
    bad_successor["successor"]["sha256"] = serde_json::json!("mutated-successor");
    require(
        check(&bad_successor, &current, &archive).is_err(),
        "A13 ClinicalTrials successor mutation was accepted",
    )?;
    let mut bad_archive = archive.clone();
    bad_archive.push(0);
    require(
        check(&amendment, &current, &bad_archive).is_err(),
        "A13 ClinicalTrials archive mutation was accepted",
    )?;
    let mut bad_current = current.clone();
    bad_current.push(0);
    require(
        check(&amendment, &bad_current, &archive).is_err(),
        "A13 ClinicalTrials current-source mutation was accepted",
    )?;
    require(
        verify_previous_successor(root, &amendment, None).is_ok(),
        "A13 ClinicalTrials predecessor positive control failed",
    )?;
    let prior = &amendment["previous_successor_archive"];
    let prior_path = root.join(
        prior["path"]
            .as_str()
            .ok_or("A13 ClinicalTrials predecessor path absent")?,
    );
    let prior_bytes = fs::read(&prior_path).map_err(|error| error.to_string())?;
    let mut changed_prior = prior_bytes.clone();
    changed_prior.push(0);
    require(
        verify_previous_successor(root, &amendment, Some(&changed_prior)).is_err(),
        "A13 ClinicalTrials predecessor mutation was accepted",
    )?;
    let mut changed_prior_pin = amendment.clone();
    changed_prior_pin["previous_successor_archive"]["sha256"] =
        serde_json::json!("mutated-clinicaltrials-predecessor");
    require(
        verify_previous_successor(root, &changed_prior_pin, None).is_err(),
        "A13 ClinicalTrials predecessor pin mutation was accepted",
    )
}

struct SecurityLockContext<'a> {
    v5: &'a Value,
    container_path: &'a str,
    container_sha: &'a str,
    container: &'a [u8],
    historical_sha: &'a str,
    historical_bytes: Option<usize>,
    current: &'a [u8],
    current_path: &'a Path,
}

fn verify_security_lock(root: &Path, context: SecurityLockContext<'_>) -> Result<(), String> {
    let SecurityLockContext {
        v5,
        container_path,
        container_sha,
        container,
        historical_sha,
        historical_bytes,
        current,
        current_path,
    } = context;
    let sidecar_bytes = fs::read(root.join(LOCK_MANIFEST_PATH)).map_err(|e| e.to_string())?;
    require(
        digest(&sidecar_bytes) == LOCK_MANIFEST_SHA256,
        "A13 security lock sidecar changed",
    )?;
    let sidecar: Value = serde_json::from_slice(&sidecar_bytes).map_err(|e| e.to_string())?;
    validate_lock_scope(&sidecar, v5)?;
    let amendment = &sidecar["amendments"][9];
    let rel = amendment["archive"]["path"]
        .as_str()
        .ok_or("A13 security archive path absent")?;
    let (archive_path, archive) = read_archive(root, rel)?;
    let accepted_main_lock =
        causal_successor::a06_source_successor::security_lock_baseline(root, current)?;
    verify_payload(PinCheck {
        amendment,
        container_path,
        container_sha,
        container_bytes: container,
        source_path: "Cargo.lock",
        historical_sha,
        historical_bytes,
        current_bytes: &accepted_main_lock,
        archive_bytes: &archive,
        current_mode: &mode(current_path)?,
        archive_mode: &mode(&archive_path)?,
    })?;
    verify_lock_reconstruction(root, &archive, current)
}

pub fn verify(
    root: &Path,
    container_path: &str,
    container_sha: &str,
    source_path: &str,
    historical_sha: &str,
    historical_bytes: Option<usize>,
) -> Result<(), String> {
    let container = fs::read(root.join(container_path))
        .map_err(|error| format!("{container_path}: {error}"))?;
    let manifest_bytes = fs::read(root.join(MANIFEST_PATH)).map_err(|error| error.to_string())?;
    require(
        digest(&manifest_bytes) == MANIFEST_SHA256,
        "A13 source amendment manifest changed",
    )?;
    let manifest: Value =
        serde_json::from_slice(&manifest_bytes).map_err(|error| error.to_string())?;
    let predecessor = fs::read(root.join(PREDECESSOR_PATH)).map_err(|error| error.to_string())?;
    require(
        digest(&predecessor) == PREDECESSOR_SHA256
            && manifest["predecessor_manifest"]["path"] == PREDECESSOR_PATH
            && manifest["predecessor_manifest"]["sha256"] == PREDECESSOR_SHA256,
        "A13 amendment predecessor manifest differs",
    )?;
    // Preserve V5's exact V4 binding and archive obligations when V6 advances
    // only HTTP. Prior accepted manifests are verified, not merely retained.
    let v5: Value = serde_json::from_slice(&predecessor).map_err(|error| error.to_string())?;
    let v4_path = "docs/rust-rewrite/a13/source-amendments/a13-current-source-amendment-v4.json";
    let v4_sha = "0126da6005a6cbab910d07779a8c2a3d9076d4823d49c08f8cca64b7e791207a";
    let v4_bytes = fs::read(root.join(v4_path)).map_err(|error| error.to_string())?;
    require(
        digest(&v4_bytes) == v4_sha
            && v5["predecessor_manifest"]["path"] == v4_path
            && v5["predecessor_manifest"]["sha256"] == v4_sha,
        "A13 V5 predecessor binding differs",
    )?;
    if let Some(prior) = v5["amendments"].as_array().and_then(|entries| {
        entries
            .iter()
            .find(|entry| entry["source_path"] == source_path)
    }) {
        verify_previous_successor(root, prior, None)?;
    }
    let v4: Value = serde_json::from_slice(&v4_bytes).map_err(|error| error.to_string())?;
    let v3_path = "docs/rust-rewrite/a13/source-amendments/a13-current-source-amendment-v3.json";
    let v3_sha = "bb24fceef429f6e4084dcfa242936b3993b9da5e9bae66c9dcc84580fd7d8212";
    let v3 = fs::read(root.join(v3_path)).map_err(|error| error.to_string())?;
    require(
        digest(&v3) == v3_sha
            && v4["predecessor_manifest"]["path"] == v3_path
            && v4["predecessor_manifest"]["sha256"] == v3_sha,
        "A13 V4 predecessor binding differs",
    )?;
    let previous: Value = serde_json::from_slice(&v3).map_err(|error| error.to_string())?;
    let v2_path = "docs/rust-rewrite/a13/source-amendments/a13-current-source-amendment-v2.json";
    let v2_sha = "da84e17cb8e96491e3c9f353f540aa9b17770fe6bd95b5081a3895f830a50dd8";
    let v2 = fs::read(root.join(v2_path)).map_err(|error| error.to_string())?;
    require(
        digest(&v2) == v2_sha
            && previous["predecessor_manifest"]["path"] == v2_path
            && previous["predecessor_manifest"]["sha256"] == v2_sha,
        "A13 V3 predecessor binding differs",
    )?;
    // Keep V4's previous-successor archive obligation live after the HTTP
    // current pin advances again. For HTTP this is the exact 48e9 archive;
    // V5 above verifies 7d5d; V6 below verifies the exact 24ee HTTP predecessor.
    let prior_amendment = v4["amendments"].as_array().and_then(|entries| {
        entries
            .iter()
            .find(|entry| entry["source_path"] == source_path)
    });
    if let Some(prior) = prior_amendment {
        verify_previous_successor(root, prior, None)?;
    }
    validate_scope(&manifest)?;
    verify_additional_source_file(root, &manifest)?;
    let current_path = root.join(source_path);
    let current = fs::read(&current_path).map_err(|error| format!("{source_path}: {error}"))?;
    if source_path == "Cargo.lock" {
        let v5 = load_lock_baseline(root)?;
        return verify_security_lock(
            root,
            SecurityLockContext {
                v5: &v5,
                container_path,
                container_sha,
                container: &container,
                historical_sha,
                historical_bytes,
                current: &current,
                current_path: &current_path,
            },
        );
    }
    if storage_document::is_path(source_path) {
        require(
            digest(&container) == container_sha,
            "A13 storage document container changed",
        )?;
        let archive = fs::read(root.join(storage_document::archive_path(source_path)?))
            .map_err(|error| error.to_string())?;
        return storage_document::verify(
            source_path,
            historical_sha,
            historical_bytes,
            &current,
            &archive,
        );
    }
    let amendments = manifest["amendments"].as_array().unwrap();
    if let Some(amendment) = amendments
        .iter()
        .find(|entry| entry["source_path"] == source_path)
    {
        let current = if source_path == "crates/ssx-infra/src/search.rs" {
            use causal_successor::a06_source_successor::w07_evidence_successor::{
                self, SearchOperand,
            };
            w07_evidence_successor::search_operand(
                root,
                SearchOperand::RuntimePin,
                &amendment["successor"],
                &current,
            )?
        } else {
            current
        };
        let archive_path = amendment["archive"]["path"]
            .as_str()
            .ok_or("A13 archive path absent")?;
        let archive_file = root.join(archive_path);
        let archive = fs::read(&archive_file).map_err(|error| error.to_string())?;
        verify_payload(PinCheck {
            amendment,
            container_path,
            container_sha,
            container_bytes: &container,
            source_path,
            historical_sha,
            historical_bytes,
            current_bytes: &current,
            archive_bytes: &archive,
            current_mode: &mode(&current_path)?,
            archive_mode: &mode(&archive_file)?,
        })?;
        return verify_previous_successor(root, amendment, None);
    }
    require(
        !AMENDED_PATHS.contains(&source_path)
            && digest(&container) == container_sha
            && historical_bytes.is_none_or(|length| current.len() == length)
            && digest(&current) == historical_sha,
        "A13 unamended source pin differs",
    )
}

/// Exercise the verifier against in-memory manifest, container, archive, and
/// successor mutations without changing any source or evidence file.
#[cfg(test)]
pub fn rejects_mutants(root: &Path) -> Result<(), String> {
    let manifest_bytes = fs::read(root.join(MANIFEST_PATH)).map_err(|error| error.to_string())?;
    let mut manifest: Value =
        serde_json::from_slice(&manifest_bytes).map_err(|error| error.to_string())?;
    validate_scope(&manifest)?;
    verify_additional_source_file(root, &manifest)?;
    let added_path = root.join("crates/ssx-infra/tests/support/http_expired_startup.rs");
    let actual_added_bytes = fs::read(&added_path).map_err(|error| error.to_string())?;
    // Original negative mutates proven09ee only after mandatory actual-child proof.
    let mut added_bytes = causal_successor::a06_source_successor::real_clock_additional_source(
        root,
        &manifest,
        &actual_added_bytes,
        &mode(&added_path)?,
    )?;
    verify_additional_source(&manifest, &added_bytes, "0644")?;
    added_bytes.push(0);
    require(
        verify_additional_source(&manifest, &added_bytes, &mode(&added_path)?).is_err(),
        "A13 altered HTTP startup control source was accepted",
    )?;
    let mut changed_addition = manifest.clone();
    changed_addition["additional_sources"][0]["path"] = serde_json::json!("unreviewed.rs");
    require(
        verify_additional_source_file(root, &changed_addition).is_err(),
        "A13 substituted HTTP startup control binding was accepted",
    )?;
    let path = AMENDED_PATHS[0];
    let amendment = manifest["amendments"]
        .as_array()
        .unwrap()
        .first()
        .unwrap()
        .clone();
    let pin = &amendment["historical_pins"][0];
    let container_path = pin["container_path"].as_str().unwrap();
    let container_sha = pin["container_sha256"].as_str().unwrap();
    let container = fs::read(root.join(container_path)).map_err(|error| error.to_string())?;
    let current = fs::read(root.join(path)).map_err(|error| error.to_string())?;
    let archive_path = amendment["archive"]["path"].as_str().unwrap();
    let archive_file = root.join(archive_path);
    let archive = fs::read(&archive_file).map_err(|error| error.to_string())?;
    let old_sha = pin["source_sha256"].as_str().unwrap();
    let old_bytes = pin["source_bytes"].as_u64().unwrap() as usize;
    let current_mode = mode(&root.join(path))?;
    let archive_mode = mode(&archive_file)?;
    let accepts = |entry: &Value,
                   path: &str,
                   container_sha: &str,
                   container: &[u8],
                   current: &[u8],
                   archive: &[u8]| {
        verify_payload(PinCheck {
            amendment: entry,
            container_path,
            container_sha,
            container_bytes: container,
            source_path: path,
            historical_sha: old_sha,
            historical_bytes: Some(old_bytes),
            current_bytes: current,
            archive_bytes: archive,
            current_mode: &current_mode,
            archive_mode: &archive_mode,
        })
        .is_ok()
    };
    require(
        accepts(
            &amendment,
            path,
            container_sha,
            &container,
            &current,
            &archive,
        ),
        "A13 amendment positive control failed",
    )?;
    let mut changed_successor = amendment.clone();
    changed_successor["successor"]["sha256"] = serde_json::json!("changed-successor");
    require(
        !accepts(
            &changed_successor,
            path,
            container_sha,
            &container,
            &current,
            &archive,
        ),
        "A13 successor mutation was accepted",
    )?;
    let mut changed_archive = amendment.clone();
    changed_archive["archive"]["sha256"] = serde_json::json!("changed-archive");
    require(
        !accepts(
            &changed_archive,
            path,
            container_sha,
            &container,
            &current,
            &archive,
        ),
        "A13 archive mutation was accepted",
    )?;
    let mut changed_archive_bytes = archive.clone();
    changed_archive_bytes.push(0);
    require(
        !accepts(
            &amendment,
            path,
            container_sha,
            &container,
            &current,
            &changed_archive_bytes,
        ),
        "A13 archive byte mutation was accepted",
    )?;
    let mut changed_container = container.clone();
    changed_container.push(0);
    require(
        !accepts(
            &amendment,
            path,
            container_sha,
            &changed_container,
            &current,
            &archive,
        ),
        "A13 container mutation was accepted",
    )?;
    let mut changed_current = current.clone();
    changed_current.push(0);
    require(
        !accepts(
            &amendment,
            path,
            container_sha,
            &container,
            &changed_current,
            &archive,
        ),
        "A13 source mutation was accepted",
    )?;
    require(
        !accepts(
            &amendment,
            "crates/ssx-infra/src/search.rs",
            container_sha,
            &container,
            &current,
            &archive,
        ),
        "A13 path substitution was accepted",
    )?;
    manifest["amended_source_paths"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!("crates/ssx-infra/src/search.rs"));
    require(
        verify_previous_successor(root, &amendment, None).is_ok(),
        "A13 prior HTTP successor positive control failed",
    )?;
    let prior = &amendment["previous_successor_archive"];
    let prior_path = root.join(
        prior["path"]
            .as_str()
            .ok_or("A13 prior HTTP archive path absent")?,
    );
    let prior_bytes = fs::read(&prior_path).map_err(|error| error.to_string())?;
    let mut mutated_prior_bytes = prior_bytes.clone();
    mutated_prior_bytes.push(0);
    require(
        verify_previous_successor(root, &amendment, Some(&mutated_prior_bytes)).is_err(),
        "A13 prior HTTP archive byte mutation was accepted",
    )?;
    let mut mutated_prior_pin = amendment.clone();
    mutated_prior_pin["previous_successor_archive"]["sha256"] = serde_json::json!("mutated-prior");
    require(
        verify_previous_successor(root, &mutated_prior_pin, None).is_err(),
        "A13 prior HTTP archive digest mutation was accepted",
    )?;
    rejects_lock_mutants(root, &load_lock_baseline(root)?)?;
    rejects_additive_mutants(root)?;
    rejects_clinicaltrials_mutants(root)?;
    require(
        validate_scope(&manifest).is_err(),
        "A13 unlisted amendment was accepted",
    )
}

#[cfg(test)]
fn rejects_lock_mutants(root: &Path, v5: &Value) -> Result<(), String> {
    let bytes = fs::read(root.join(LOCK_MANIFEST_PATH)).map_err(|e| e.to_string())?;
    require(
        digest(&bytes) == LOCK_MANIFEST_SHA256,
        "A13 lock test sidecar hash changed",
    )?;
    let sidecar: Value = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    validate_lock_scope(&sidecar, v5)?;
    let lock = &sidecar["amendments"][9];
    let rel = lock["archive"]["path"]
        .as_str()
        .ok_or("A13 lock archive path absent")?;
    let (archive_path, archive) = read_archive(root, rel)?;
    let current_path = root.join("Cargo.lock");
    let actual_current = fs::read(&current_path).map_err(|e| e.to_string())?;
    verify_lock_reconstruction(root, &archive, &actual_current)?;
    let current =
        causal_successor::a06_source_successor::security_lock_baseline(root, &actual_current)?;
    let absent = format!("{rel}.a13-test-absent");
    require(
        !root.join(&absent).exists(),
        "A13 test missing-archive path unexpectedly exists",
    )?;
    require(
        read_archive(root, &absent).is_err(),
        "A13 production archive reader accepted missing archive",
    )?;
    for pin in lock["historical_pins"]
        .as_array()
        .ok_or("A13 lock pins absent")?
    {
        let cp = pin["container_path"]
            .as_str()
            .ok_or("A13 container path absent")?;
        let container_path = root.join(cp);
        let container = fs::read(&container_path).map_err(|e| e.to_string())?;
        let check = |entry: &Value, path: &str, bytes: &[u8], archive_bytes: &[u8]| {
            verify_payload(PinCheck {
                amendment: entry,
                container_path: path,
                container_sha: pin["container_sha256"].as_str().unwrap(),
                container_bytes: bytes,
                source_path: "Cargo.lock",
                historical_sha: pin["source_sha256"].as_str().unwrap(),
                historical_bytes: pin["source_bytes"].as_u64().map(|v| v as usize),
                current_bytes: &current,
                archive_bytes,
                current_mode: &mode(&current_path)?,
                archive_mode: &mode(&archive_path)?,
            })
        };
        require(
            check(lock, cp, &container, &archive).is_ok(),
            "A13 lock positive container control failed",
        )?;
        let wrong = if cp == "docs/rust-rewrite/a09/coverage.json" {
            "docs/rust-rewrite/a10/qualification.json"
        } else {
            "docs/rust-rewrite/a09/coverage.json"
        };
        require(
            check(lock, wrong, &container, &archive).is_err(),
            "A13 wrong lock container path accepted",
        )?;
        let mut bad_container = container.clone();
        bad_container.push(0);
        require(
            check(lock, cp, &bad_container, &archive).is_err(),
            "A13 changed lock container accepted",
        )?;
        let mut bad_archive = archive.clone();
        bad_archive.push(0);
        require(
            check(lock, cp, &container, &bad_archive).is_err(),
            "A13 changed lock archive accepted",
        )?;
    }
    let current_text = std::str::from_utf8(&current).map_err(|e| e.to_string())?;
    let rollback_first = replace_lock_package(
        current_text,
        "rustls-webpki",
        "0.103.16",
        "0.103.15",
        "4443b1b6d39d7c663139942cc4558262562d2c84123bb6282ef177009ca8b281",
        "f3c3cf1d8b1e7d4927e2d154c3fcb02979afb9939629c62cd9048d4f07b60ac2",
    )?;
    require(
        verify_lock_reconstruction(root, &archive, rollback_first.as_bytes()).is_err(),
        "A13 first lock package rollback accepted",
    )?;
    let rollback_second = replace_lock_package(
        current_text,
        "webpki-root-certs",
        "1.0.10",
        "1.0.9",
        "58ec80e73fd8baf57bf0d9615554c75348bda990d6d5bbeab964dd068319fecb",
        "b96554aa2acc8ccdb7e1c9a58a7a68dd5d13bccc69cd124cb09406db612a1c9b",
    )?;
    require(
        verify_lock_reconstruction(root, &archive, rollback_second.as_bytes()).is_err(),
        "A13 second lock package rollback accepted",
    )?;
    let third = replace_lock_package(
        current_text,
        "adler2",
        "2.0.1",
        "2.0.2",
        "320119579fcad9c21884f5c4861d16174d0e06250625266f50fe6898340abefa",
        "1111111111111111111111111111111111111111111111111111111111111111",
    )?;
    let third_bytes = third.as_bytes();
    let mut adjusted = lock.clone();
    adjusted["successor"]["sha256"] = serde_json::json!(digest(third_bytes));
    adjusted["successor"]["bytes"] = serde_json::json!(third_bytes.len());
    let pin = &lock["historical_pins"][0];
    let cp = pin["container_path"].as_str().unwrap();
    let container = fs::read(root.join(cp)).map_err(|e| e.to_string())?;
    require(
        verify_payload(PinCheck {
            amendment: &adjusted,
            container_path: cp,
            container_sha: pin["container_sha256"].as_str().unwrap(),
            container_bytes: &container,
            source_path: "Cargo.lock",
            historical_sha: pin["source_sha256"].as_str().unwrap(),
            historical_bytes: pin["source_bytes"].as_u64().map(|v| v as usize),
            current_bytes: third_bytes,
            archive_bytes: &archive,
            current_mode: &mode(&current_path)?,
            archive_mode: &mode(&archive_path)?,
        })
        .is_ok(),
        "A13 adjusted third-package generic pins failed to demonstrate the gap",
    )?;
    require(
        verify_lock_reconstruction(root, &archive, third_bytes).is_err(),
        "A13 third-package mutation accepted by exact reconstruction",
    )?;
    let mut changed = sidecar.clone();
    changed["amendments"][0]["successor"]["sha256"] = serde_json::json!("mutated");
    require(
        validate_lock_scope(&changed, v5).is_err(),
        "A13 changed V5 prefix accepted",
    )?;
    let mut removed = sidecar.clone();
    removed["amendments"].as_array_mut().unwrap().remove(0);
    removed["amended_source_paths"]
        .as_array_mut()
        .unwrap()
        .remove(0);
    require(
        validate_lock_scope(&removed, v5).is_err(),
        "A13 removed prior V5 entry accepted",
    )?;
    let mut missing = sidecar.clone();
    missing["amendments"].as_array_mut().unwrap().pop();
    missing["amended_source_paths"]
        .as_array_mut()
        .unwrap()
        .pop();
    require(
        validate_lock_scope(&missing, v5).is_err(),
        "A13 missing security lock amendment accepted",
    )?;
    Ok(())
}

#[cfg(test)]
fn rejects_additive_mutants(root: &Path) -> Result<(), String> {
    let manifest: Value =
        serde_json::from_slice(&fs::read(root.join(MANIFEST_PATH)).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
    for path in [
        "crates/ssx-infra/src/search.rs",
        "crates/ssx-infra/src/search_deadline_drain_tests.rs",
        "crates/ssx-infra/src/adapters/mod.rs",
        "crates/xtask/src/a09.rs",
    ] {
        let entry = manifest["amendments"]
            .as_array()
            .unwrap()
            .iter()
            .find(|v| v["source_path"] == path)
            .ok_or("A13 additive amendment missing")?;
        for pin in entry["historical_pins"].as_array().unwrap() {
            let container_path = pin["container_path"].as_str().unwrap();
            let container = fs::read(root.join(container_path)).map_err(|e| e.to_string())?;
            let archive_path = root.join(entry["archive"]["path"].as_str().unwrap());
            let archive = fs::read(&archive_path).map_err(|e| e.to_string())?;
            let current_path = root.join(path);
            let actual_current = fs::read(&current_path).map_err(|e| e.to_string())?;
            let current = if path == "crates/ssx-infra/src/adapters/mod.rs" {
                causal_successor::a06_source_successor::legacy_control_operand(
                    root,
                    path,
                    &entry["successor"],
                    &actual_current,
                )?
            } else if path == "crates/ssx-infra/src/search.rs" {
                use causal_successor::a06_source_successor::w07_evidence_successor::{
                    self, SearchOperand,
                };
                w07_evidence_successor::search_operand(
                    root,
                    SearchOperand::AdditiveControl,
                    &entry["successor"],
                    &actual_current,
                )?
            } else {
                actual_current
            };
            let check = |payload: &Value, archive: &[u8], current: &[u8], container: &[u8]| {
                verify_payload(PinCheck {
                    amendment: payload,
                    container_path,
                    container_sha: pin["container_sha256"].as_str().unwrap(),
                    container_bytes: container,
                    source_path: path,
                    historical_sha: pin["source_sha256"].as_str().unwrap(),
                    historical_bytes: pin["source_bytes"].as_u64().map(|v| v as usize),
                    current_bytes: current,
                    archive_bytes: archive,
                    current_mode: &mode(&current_path)?,
                    archive_mode: &mode(&archive_path)?,
                })
            };
            require(
                check(entry, &archive, &current, &container).is_ok(),
                "A13 additive amendment positive control failed",
            )?;
            let mut bad = current.clone();
            bad.push(0);
            require(
                check(entry, &archive, &bad, &container).is_err(),
                "A13 additive successor mutation accepted",
            )?;
            let mut bad_archive = archive.clone();
            bad_archive.push(0);
            require(
                check(entry, &bad_archive, &current, &container).is_err(),
                "A13 additive historical archive mutation accepted",
            )?;
            let mut bad_container = container.clone();
            bad_container.push(0);
            require(
                check(entry, &archive, &current, &bad_container).is_err(),
                "A13 additive container mutation accepted",
            )?;
            let mut bad_pin = entry.clone();
            bad_pin["successor"]["sha256"] = serde_json::json!("changed");
            require(
                check(&bad_pin, &archive, &current, &container).is_err(),
                "A13 additive successor pin mutation accepted",
            )?;
        }
    }
    Ok(())
}

// Appended PR1034 draft only: each direct include owns this same nested module.
#[path = "../../a04/source-amendments/scoped-successor.rs"]
pub(crate) mod causal_successor;
pub(crate) use causal_successor::verify_current;

/// Historical-only legacy controls; default current-read entrypoint remains unchanged.
#[cfg(test)]
pub(crate) fn rejects_mutants_on_historical_evidence(
    root: &Path,
    historical: &causal_successor::HistoricalControlEvidence,
) -> Result<(), String> {
    let manifest_bytes = fs::read(root.join(MANIFEST_PATH)).map_err(|error| error.to_string())?;
    let mut manifest: Value =
        serde_json::from_slice(&manifest_bytes).map_err(|error| error.to_string())?;
    validate_scope(&manifest)?;
    verify_additional_source_file(root, &manifest)?;
    let added_path = root.join("crates/ssx-infra/tests/support/http_expired_startup.rs");
    let actual_added_bytes = fs::read(&added_path).map_err(|error| error.to_string())?;
    // Original negative mutates proven09ee only after mandatory actual-child proof.
    let mut added_bytes = causal_successor::a06_source_successor::real_clock_additional_source(
        root,
        &manifest,
        &actual_added_bytes,
        &mode(&added_path)?,
    )?;
    verify_additional_source(&manifest, &added_bytes, "0644")?;
    added_bytes.push(0);
    require(
        verify_additional_source(&manifest, &added_bytes, &mode(&added_path)?).is_err(),
        "A13 altered HTTP startup control source was accepted",
    )?;
    let mut changed_addition = manifest.clone();
    changed_addition["additional_sources"][0]["path"] = serde_json::json!("unreviewed.rs");
    require(
        verify_additional_source_file(root, &changed_addition).is_err(),
        "A13 substituted HTTP startup control binding was accepted",
    )?;
    let path = AMENDED_PATHS[0];
    let amendment = manifest["amendments"]
        .as_array()
        .unwrap()
        .first()
        .unwrap()
        .clone();
    let pin = &amendment["historical_pins"][0];
    let container_path = pin["container_path"].as_str().unwrap();
    let container_sha = pin["container_sha256"].as_str().unwrap();
    let container = fs::read(root.join(container_path)).map_err(|error| error.to_string())?;
    // Explicit accepted-V6 historical operand for unchanged pure legacy assertions.
    let current = historical.accepted_v6_successor_bytes().to_vec();
    let archive_path = amendment["archive"]["path"].as_str().unwrap();
    let archive_file = root.join(archive_path);
    let archive = fs::read(&archive_file).map_err(|error| error.to_string())?;
    let old_sha = pin["source_sha256"].as_str().unwrap();
    let old_bytes = pin["source_bytes"].as_u64().unwrap() as usize;
    let current_mode = mode(&root.join(path))?;
    let archive_mode = mode(&archive_file)?;
    let accepts = |entry: &Value,
                   path: &str,
                   container_sha: &str,
                   container: &[u8],
                   current: &[u8],
                   archive: &[u8]| {
        verify_payload(PinCheck {
            amendment: entry,
            container_path,
            container_sha,
            container_bytes: container,
            source_path: path,
            historical_sha: old_sha,
            historical_bytes: Some(old_bytes),
            current_bytes: current,
            archive_bytes: archive,
            current_mode: &current_mode,
            archive_mode: &archive_mode,
        })
        .is_ok()
    };
    require(
        accepts(
            &amendment,
            path,
            container_sha,
            &container,
            &current,
            &archive,
        ),
        "A13 amendment positive control failed",
    )?;
    let mut changed_successor = amendment.clone();
    changed_successor["successor"]["sha256"] = serde_json::json!("changed-successor");
    require(
        !accepts(
            &changed_successor,
            path,
            container_sha,
            &container,
            &current,
            &archive,
        ),
        "A13 successor mutation was accepted",
    )?;
    let mut changed_archive = amendment.clone();
    changed_archive["archive"]["sha256"] = serde_json::json!("changed-archive");
    require(
        !accepts(
            &changed_archive,
            path,
            container_sha,
            &container,
            &current,
            &archive,
        ),
        "A13 archive mutation was accepted",
    )?;
    let mut changed_archive_bytes = archive.clone();
    changed_archive_bytes.push(0);
    require(
        !accepts(
            &amendment,
            path,
            container_sha,
            &container,
            &current,
            &changed_archive_bytes,
        ),
        "A13 archive byte mutation was accepted",
    )?;
    let mut changed_container = container.clone();
    changed_container.push(0);
    require(
        !accepts(
            &amendment,
            path,
            container_sha,
            &changed_container,
            &current,
            &archive,
        ),
        "A13 container mutation was accepted",
    )?;
    let mut changed_current = current.clone();
    changed_current.push(0);
    require(
        !accepts(
            &amendment,
            path,
            container_sha,
            &container,
            &changed_current,
            &archive,
        ),
        "A13 source mutation was accepted",
    )?;
    require(
        !accepts(
            &amendment,
            "crates/ssx-infra/src/search.rs",
            container_sha,
            &container,
            &current,
            &archive,
        ),
        "A13 path substitution was accepted",
    )?;
    manifest["amended_source_paths"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!("crates/ssx-infra/src/search.rs"));
    require(
        verify_previous_successor(root, &amendment, None).is_ok(),
        "A13 prior HTTP successor positive control failed",
    )?;
    let prior = &amendment["previous_successor_archive"];
    let prior_path = root.join(
        prior["path"]
            .as_str()
            .ok_or("A13 prior HTTP archive path absent")?,
    );
    let prior_bytes = fs::read(&prior_path).map_err(|error| error.to_string())?;
    let mut mutated_prior_bytes = prior_bytes.clone();
    mutated_prior_bytes.push(0);
    require(
        verify_previous_successor(root, &amendment, Some(&mutated_prior_bytes)).is_err(),
        "A13 prior HTTP archive byte mutation was accepted",
    )?;
    let mut mutated_prior_pin = amendment.clone();
    mutated_prior_pin["previous_successor_archive"]["sha256"] = serde_json::json!("mutated-prior");
    require(
        verify_previous_successor(root, &mutated_prior_pin, None).is_err(),
        "A13 prior HTTP archive digest mutation was accepted",
    )?;
    rejects_lock_mutants(root, &load_lock_baseline(root)?)?;
    rejects_additive_mutants(root)?;
    rejects_clinicaltrials_mutants(root)?;
    require(
        validate_scope(&manifest).is_err(),
        "A13 unlisted amendment was accepted",
    )
}
