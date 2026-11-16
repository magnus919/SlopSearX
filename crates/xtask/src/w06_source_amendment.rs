//! Exact, additive reconciliation for the one W07-owned source added after W06.
use crate::{Result, command};
use serde::Deserialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{fs, path::Path};

const MANIFEST_PATH: &str =
    "docs/rust-rewrite/w06/source-amendments/current-source-amendment-v1.json";
const SOURCE_PATH: &str = "crates/ssx-application/src/lib.rs";
const PARENT_ARCHIVE_PATH: &str =
    "docs/rust-rewrite/w06/source-amendments/de04-pre-w07/crates/ssx-application/src/lib.rs";
const MANIFEST_SHA256: &str = "efc0c4f1a767e4850800f48d632ddb2c6827d2b8c6c9ddb96003785f31200d30";
const HISTORICAL_SHA256: &str = "a7102f8c75244e74680cf8773c527d21fb2dcd177a29f0bd27abeef2beeb880e";
const PARENT_REVISION: &str = "de04ab2e4263616dfd22ad6c9dc45aeb94f9bea4";
const PARENT_GIT_BLOB: &str = "14df2ec4ba0f1f03e18bb7071c5646536a45a773";
const CURRENT_GIT_BLOB: &str = "45dbed9d4d1f3ee84709c1f73026e19ac9dcb64d";
const ALLOWED_APPEND: &[u8] = b"\npub mod lineage;\npub mod receipts;\n";

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Amendment {
    schema_version: u32,
    plan_id: String,
    purpose: String,
    source_path: String,
    historical_pin: HistoricalPin,
    successor: Successor,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct HistoricalPin {
    sha256: String,
    bytes: usize,
    git_revision: String,
    git_blob: String,
    archive_path: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Successor {
    parent_revision: String,
    parent_archive_path: String,
    parent_sha256: String,
    parent_bytes: usize,
    parent_git_blob: String,
    allowed_append_utf8: String,
    allowed_append_bytes: usize,
    allowed_append_sha256: String,
    sha256: String,
    bytes: usize,
    git_blob: String,
}

fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
fn reject(message: &str) -> Result<()> {
    Err(format!("W06 source amendment: {message}"))
}

fn validate_materials(
    manifest_bytes: &[u8],
    historical_expected: &str,
    historical_archive: &[u8],
    parent_archive: &[u8],
    current: &[u8],
) -> Result<()> {
    if sha256(manifest_bytes) != MANIFEST_SHA256 {
        return reject("manifest identity changed");
    }
    let amendment: Amendment = serde_json::from_slice(manifest_bytes)
        .map_err(|_| "W06 source amendment: malformed manifest")?;
    let historical = amendment.historical_pin;
    let successor = amendment.successor;
    if amendment.schema_version != 1
        || amendment.plan_id != "W06"
        || amendment.purpose.is_empty()
        || amendment.source_path != SOURCE_PATH
        || historical_expected != HISTORICAL_SHA256
        || historical.sha256 != HISTORICAL_SHA256
        || historical.bytes != 5525
        || historical.git_revision != PARENT_REVISION
        || historical.git_blob != PARENT_GIT_BLOB
        || historical.archive_path != PARENT_ARCHIVE_PATH
    {
        return reject("historical pin or amendment scope changed");
    }
    if sha256(historical_archive) != historical.sha256
        || historical_archive.len() != historical.bytes
    {
        return reject("archived pre-W07 source does not match frozen pin");
    }
    if successor.parent_revision != PARENT_REVISION
        || successor.parent_archive_path != PARENT_ARCHIVE_PATH
        || successor.parent_sha256 != HISTORICAL_SHA256
        || successor.parent_bytes != 5525
        || successor.parent_git_blob != PARENT_GIT_BLOB
        || successor.allowed_append_utf8.as_bytes() != ALLOWED_APPEND
        || successor.allowed_append_bytes != ALLOWED_APPEND.len()
        || successor.allowed_append_sha256 != sha256(ALLOWED_APPEND)
    {
        return reject("parent or allowed delta changed");
    }
    if sha256(parent_archive) != successor.parent_sha256
        || parent_archive.len() != successor.parent_bytes
        || historical_archive != parent_archive
    {
        return reject("pre-W07 archive does not match the frozen pin");
    }
    if successor.sha256 != "1ff0647013d66e6711587bbd103659959ab860bd3bd2d2b42d10b7a640a5544e"
        || successor.bytes != 5561
        || successor.git_blob != CURRENT_GIT_BLOB
    {
        return reject("successor identity changed");
    }
    let mut expected = Vec::with_capacity(parent_archive.len() + ALLOWED_APPEND.len());
    expected.extend_from_slice(parent_archive);
    expected.extend_from_slice(ALLOWED_APPEND);
    if current != expected
        || sha256(current) != successor.sha256
        || current.len() != successor.bytes
    {
        return reject("live source is not the exact reviewed successor");
    }
    Ok(())
}

pub(super) fn verify(
    root: &Path,
    historical_expected: &str,
    records: &mut Vec<Value>,
) -> Result<()> {
    let manifest = fs::read(root.join(MANIFEST_PATH))
        .map_err(|error| format!("W06 source amendment manifest: {error}"))?;
    let archive = fs::read(root.join(PARENT_ARCHIVE_PATH))
        .map_err(|error| format!("W06 source amendment archive: {error}"))?;
    let current =
        fs::read(root.join(SOURCE_PATH)).map_err(|error| format!("W06 amended source: {error}"))?;
    validate_materials(&manifest, historical_expected, &archive, &archive, &current)?;
    let parent_spec = format!("{PARENT_REVISION}:{SOURCE_PATH}");
    let current_spec = format!("HEAD:{SOURCE_PATH}");
    let parent_blob = command(root, "git", &["rev-parse", &parent_spec], records)?;
    let current_blob = command(root, "git", &["rev-parse", &current_spec], records)?;
    if parent_blob.trim() != PARENT_GIT_BLOB || current_blob.trim() != CURRENT_GIT_BLOB {
        return reject("historical or current Git blob identity changed");
    }
    negative_controls(&manifest, historical_expected, &archive, &current)
}

fn negative_controls(
    manifest: &[u8],
    historical_expected: &str,
    archive: &[u8],
    current: &[u8],
) -> Result<()> {
    let must_reject = |label: &str,
                       manifest: &[u8],
                       expected: &str,
                       frozen: &[u8],
                       parent: &[u8],
                       live: &[u8]| {
        if validate_materials(manifest, expected, frozen, parent, live).is_ok() {
            return reject(&format!("negative control was accepted: {label}"));
        }
        Ok(())
    };
    let mut altered_manifest = manifest.to_vec();
    altered_manifest[0] = b' ';
    must_reject(
        "manifest byte",
        &altered_manifest,
        historical_expected,
        archive,
        archive,
        current,
    )?;
    must_reject(
        "historical pin",
        manifest,
        "0".repeat(64).as_str(),
        archive,
        archive,
        current,
    )?;
    let mut altered_archive = archive.to_vec();
    altered_archive[0] ^= 1;
    must_reject(
        "archived source byte",
        manifest,
        historical_expected,
        &altered_archive,
        archive,
        current,
    )?;
    must_reject(
        "parent archive byte",
        manifest,
        historical_expected,
        archive,
        &altered_archive,
        current,
    )?;
    let mut extra_source = current.to_vec();
    extra_source.extend_from_slice(b"\npub mod unreviewed;\n");
    must_reject(
        "unreviewed source addition",
        manifest,
        historical_expected,
        archive,
        archive,
        &extra_source,
    )
}

// A separate exact proof keeps the original lib.rs amendment/manifest immutable.
const MAIN_SOURCE_PATH: &str = "crates/xtask/src/main.rs";
const MAIN_MANIFEST_PATH: &str =
    "docs/rust-rewrite/w06/source-amendments/xtask-main-source-amendment-v1.json";
const MAIN_ARCHIVE_PATH: &str =
    "docs/rust-rewrite/w06/source-amendments/de04-pre-w07/crates/xtask/src/main.rs";
const MAIN_MANIFEST_SHA256: &str =
    "06d56eaf4df8f5ac7f6dc10048001523fe0561e6bf0f834e6e80cff80c2ccc9e";
const MAIN_HISTORICAL_SHA256: &str =
    "0c60e728c3e5783f4ff01c98a3dd3b8232699b3bf0e7b35620f8fa4643ea53c1";
const MAIN_CURRENT_SHA256: &str =
    "1023f5d3eb5903da3b85bc45fe4c1a7a21211bceeb64125c53528a8f50a49e4e";
const MAIN_PARENT_BLOB: &str = "a38b67ce097ec941455666d6c73aa506846337c3";
const MAIN_CURRENT_BLOB: &str = "fb209dfb01af9a807fe42dc2d2f990fe2c54ee01";
const MAIN_ANCHOR: &[u8] = b"mod w06;\n";
const MAIN_INSERT: &[u8] = b"mod w06_source_amendment;\n";

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MainAmendment {
    schema_version: u32,
    plan_id: String,
    purpose: String,
    source_path: String,
    historical_pin: HistoricalPin,
    successor: MainSuccessor,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MainSuccessor {
    parent_revision: String,
    parent_archive_path: String,
    parent_sha256: String,
    parent_bytes: usize,
    parent_git_blob: String,
    insertion_anchor_utf8: String,
    allowed_insert_utf8: String,
    allowed_insert_bytes: usize,
    allowed_insert_sha256: String,
    sha256: String,
    bytes: usize,
    git_blob: String,
}

fn validate_main_materials(
    manifest_bytes: &[u8],
    historical_expected: &str,
    archive: &[u8],
    current: &[u8],
) -> Result<()> {
    if sha256(manifest_bytes) != MAIN_MANIFEST_SHA256 {
        return reject("main manifest identity changed");
    }
    let amendment: MainAmendment = serde_json::from_slice(manifest_bytes)
        .map_err(|_| "W06 source amendment: malformed main manifest")?;
    let historical = amendment.historical_pin;
    let successor = amendment.successor;
    if amendment.schema_version != 1
        || amendment.plan_id != "W07"
        || amendment.purpose.is_empty()
        || amendment.source_path != MAIN_SOURCE_PATH
        || historical_expected != MAIN_HISTORICAL_SHA256
        || historical.sha256 != MAIN_HISTORICAL_SHA256
        || historical.bytes != 38817
        || historical.git_revision != PARENT_REVISION
        || historical.git_blob != MAIN_PARENT_BLOB
        || historical.archive_path != MAIN_ARCHIVE_PATH
    {
        return reject("main historical pin or amendment scope changed");
    }
    if archive.len() != historical.bytes || sha256(archive) != historical.sha256 {
        return reject("main archive differs from frozen pin");
    }
    if successor.parent_revision != PARENT_REVISION
        || successor.parent_archive_path != MAIN_ARCHIVE_PATH
        || successor.parent_sha256 != MAIN_HISTORICAL_SHA256
        || successor.parent_bytes != 38817
        || successor.parent_git_blob != MAIN_PARENT_BLOB
        || successor.insertion_anchor_utf8.as_bytes() != MAIN_ANCHOR
        || successor.allowed_insert_utf8.as_bytes() != MAIN_INSERT
        || successor.allowed_insert_bytes != MAIN_INSERT.len()
        || successor.allowed_insert_sha256 != sha256(MAIN_INSERT)
        || successor.sha256 != MAIN_CURRENT_SHA256
        || successor.bytes != 38843
        || successor.git_blob != MAIN_CURRENT_BLOB
    {
        return reject("main parent, insertion or successor identity changed");
    }
    let anchors = archive
        .windows(MAIN_ANCHOR.len())
        .enumerate()
        .filter_map(|(index, bytes)| (bytes == MAIN_ANCHOR).then_some(index))
        .collect::<Vec<_>>();
    if anchors.len() != 1
        || archive
            .windows(MAIN_INSERT.len())
            .any(|bytes| bytes == MAIN_INSERT)
    {
        return reject("main insertion anchor is not unique or declaration already exists");
    }
    let offset = anchors[0] + MAIN_ANCHOR.len();
    let mut expected = Vec::with_capacity(archive.len() + MAIN_INSERT.len());
    expected.extend_from_slice(&archive[..offset]);
    expected.extend_from_slice(MAIN_INSERT);
    expected.extend_from_slice(&archive[offset..]);
    if current != expected
        || current.len() != successor.bytes
        || sha256(current) != successor.sha256
    {
        return reject("live main is not the exact reviewed single insertion");
    }
    Ok(())
}

fn validate_main_git(parent_blob: &str, current_blob: &str) -> Result<()> {
    if parent_blob != MAIN_PARENT_BLOB || current_blob != MAIN_CURRENT_BLOB {
        return reject("main historical or HEAD Git blob changed");
    }
    Ok(())
}

pub(super) fn verify_main(
    root: &Path,
    historical_expected: &str,
    records: &mut Vec<Value>,
) -> Result<()> {
    let manifest = fs::read(root.join(MAIN_MANIFEST_PATH))
        .map_err(|error| format!("W06 main amendment manifest: {error}"))?;
    let archive = fs::read(root.join(MAIN_ARCHIVE_PATH))
        .map_err(|error| format!("W06 main amendment archive: {error}"))?;
    let current = fs::read(root.join(MAIN_SOURCE_PATH))
        .map_err(|error| format!("W06 amended main: {error}"))?;
    validate_main_materials(&manifest, historical_expected, &archive, &current)?;
    let parent_spec = format!("{PARENT_REVISION}:{MAIN_SOURCE_PATH}");
    let current_spec = format!("HEAD:{MAIN_SOURCE_PATH}");
    let parent_blob = command(root, "git", &["rev-parse", &parent_spec], records)?;
    let current_blob = command(root, "git", &["rev-parse", &current_spec], records)?;
    validate_main_git(parent_blob.trim(), current_blob.trim())?;
    let parent_source = command(root, "git", &["show", &parent_spec], records)?;
    let current_source = command(root, "git", &["show", &current_spec], records)?;
    if parent_source.as_bytes() != archive || current_source.as_bytes() != current {
        return reject("main Git source bytes differ from archive or live source");
    }
    main_negative_controls(&manifest, historical_expected, &archive, &current)
}

fn main_negative_controls(
    manifest: &[u8],
    historical_expected: &str,
    archive: &[u8],
    current: &[u8],
) -> Result<()> {
    let must_reject = |label: &str, manifest: &[u8], expected: &str, frozen: &[u8], live: &[u8]| {
        if validate_main_materials(manifest, expected, frozen, live).is_ok() {
            return reject(&format!("main negative control was accepted: {label}"));
        }
        Ok(())
    };
    let mut altered_manifest = manifest.to_vec();
    altered_manifest[0] ^= 1;
    must_reject(
        "manifest byte",
        &altered_manifest,
        historical_expected,
        archive,
        current,
    )?;
    must_reject(
        "wrong historical pin",
        manifest,
        &"0".repeat(64),
        archive,
        current,
    )?;
    let mut altered_archive = archive.to_vec();
    altered_archive[0] ^= 1;
    must_reject(
        "archive byte",
        manifest,
        historical_expected,
        &altered_archive,
        current,
    )?;
    must_reject(
        "missing insertion",
        manifest,
        historical_expected,
        archive,
        archive,
    )?;
    let mut duplicate = current.to_vec();
    duplicate.extend_from_slice(MAIN_INSERT);
    must_reject(
        "duplicate insertion",
        manifest,
        historical_expected,
        archive,
        &duplicate,
    )?;
    let mut changed = current.to_vec();
    changed[0] ^= 1;
    must_reject(
        "unrelated live byte",
        manifest,
        historical_expected,
        archive,
        &changed,
    )?;
    let mut extra = current.to_vec();
    extra.extend_from_slice(b"\nmod unreviewed;\n");
    must_reject(
        "extra declaration",
        manifest,
        historical_expected,
        archive,
        &extra,
    )?;
    if validate_main_git(&"0".repeat(40), MAIN_CURRENT_BLOB).is_ok()
        || validate_main_git(MAIN_PARENT_BLOB, &"0".repeat(40)).is_ok()
    {
        return reject("main wrong Git blob negative control was accepted");
    }
    Ok(())
}

const SHARED_BINDINGS: [(&str, &str, &str); 5] = [
    (
        "docs/rust-rewrite/o01/coverage.json",
        "76d216907f00ac637505101e83454f43d9a812213432e1c40a20d5a8d75934b0",
        SOURCE_PATH,
    ),
    (
        "docs/rust-rewrite/a09/coverage.json",
        "d3dbf8be85061348dded0b783a55dd11f5d79e789857e2046ebe808504f162c8",
        SOURCE_PATH,
    ),
    (
        "docs/rust-rewrite/a09/coverage.json",
        "d3dbf8be85061348dded0b783a55dd11f5d79e789857e2046ebe808504f162c8",
        MAIN_SOURCE_PATH,
    ),
    (
        "docs/rust-rewrite/a12/qualification-v1.json",
        "37bdd69e0c26437f902e9aad3acb60908fbae920dcc10e42aeefaadbb0a40320",
        SOURCE_PATH,
    ),
    (
        "docs/rust-rewrite/a12/qualification-v1.json",
        "37bdd69e0c26437f902e9aad3acb60908fbae920dcc10e42aeefaadbb0a40320",
        MAIN_SOURCE_PATH,
    ),
];

fn shared_binding(container_path: &str, source_path: &str) -> Option<&'static str> {
    SHARED_BINDINGS
        .iter()
        .find_map(|(container, digest, source)| {
            (*container == container_path && *source == source_path).then_some(*digest)
        })
}

fn validate_shared_binding(
    container_path: &str,
    container_sha: &str,
    container: &[u8],
    source_path: &str,
    expected_sha: &str,
    expected_bytes: Option<usize>,
) -> Result<()> {
    let Some(bound_sha) = shared_binding(container_path, source_path) else {
        return reject("unreviewed shared consumer/source binding");
    };
    if container_sha != bound_sha || sha256(container) != bound_sha {
        return reject("shared consumer manifest identity changed");
    }
    let historical_sha = if source_path == SOURCE_PATH {
        HISTORICAL_SHA256
    } else {
        MAIN_HISTORICAL_SHA256
    };
    // All five exact frozen rows omit `bytes`; the actual callers pass None.
    // Archive/material validation separately enforces the historical/current lengths.
    if expected_sha != historical_sha || expected_bytes.is_some() {
        return reject("shared historical source digest or bytes-field contract changed");
    }
    Ok(())
}

/// Only these five frozen consumer/source pairs receive the additive proof.
/// `None` preserves the existing A13 validator for every other caller/path.
pub(super) fn verify_shared(
    root: &Path,
    container_path: &str,
    container_sha: &str,
    source_path: &str,
    expected_sha: &str,
    expected_bytes: Option<usize>,
) -> Option<Result<()>> {
    shared_binding(container_path, source_path)?;
    Some((|| {
        let container = fs::read(root.join(container_path))
            .map_err(|error| format!("W06 shared consumer manifest: {error}"))?;
        validate_shared_binding(
            container_path,
            container_sha,
            &container,
            source_path,
            expected_sha,
            expected_bytes,
        )?;
        shared_positive_controls(root)?;
        shared_negative_controls(
            container_path,
            container_sha,
            &container,
            source_path,
            expected_sha,
            expected_bytes,
        )?;
        let mut commands = Vec::new();
        if source_path == SOURCE_PATH {
            verify(root, expected_sha, &mut commands)?;
        } else {
            verify_main(root, expected_sha, &mut commands)?;
        }
        // The A13 wrapper has no command-record parameter. Preserve compact
        // actual Git proof in the gate's captured stderr instead of discarding it.
        let proof_commands = commands
            .iter()
            .map(|record| {
                let stdout = record["stdout"].as_str().unwrap_or_default();
                serde_json::json!({
                    "program": record["program"], "args": record["args"],
                    "exit_code": record["exit_code"],
                    "stdout_bytes": stdout.len(), "stdout_sha256": sha256(stdout.as_bytes())
                })
            })
            .collect::<Vec<_>>();
        eprintln!(
            "W07_SOURCE_AMENDMENT_PROOF {}",
            serde_json::json!({
                "container_path": container_path, "container_sha256": container_sha,
                "source_path": source_path, "historical_sha256": expected_sha,
                "consumer_bytes_field": expected_bytes,
            "historical_bytes": if source_path == SOURCE_PATH { 5525 } else { 38817 },
            "current_bytes": if source_path == SOURCE_PATH { 5561 } else { 38843 },
                "current_sha256": if source_path == SOURCE_PATH {
                    "1ff0647013d66e6711587bbd103659959ab860bd3bd2d2b42d10b7a640a5544e"
                } else { MAIN_CURRENT_SHA256 },
                "git_checks": proof_commands, "positive_binding_controls": 5, "negative_controls": "passed"
            })
        );
        Ok(())
    })())
}

fn shared_negative_controls(
    container_path: &str,
    container_sha: &str,
    container: &[u8],
    source_path: &str,
    expected_sha: &str,
    expected_bytes: Option<usize>,
) -> Result<()> {
    let rejected = |scope: &str,
                    digest: &str,
                    bytes: &[u8],
                    source: &str,
                    expected: &str,
                    length: Option<usize>| {
        if validate_shared_binding(scope, digest, bytes, source, expected, length).is_ok() {
            return reject("shared binding negative control was accepted");
        }
        Ok(())
    };
    rejected(
        "docs/rust-rewrite/unreviewed/coverage.json",
        container_sha,
        container,
        source_path,
        expected_sha,
        expected_bytes,
    )?;
    rejected(
        container_path,
        container_sha,
        container,
        "crates/unreviewed/src/lib.rs",
        expected_sha,
        expected_bytes,
    )?;
    rejected(
        container_path,
        &"0".repeat(64),
        container,
        source_path,
        expected_sha,
        expected_bytes,
    )?;
    let mut tampered = container.to_vec();
    tampered[0] ^= 1;
    rejected(
        container_path,
        container_sha,
        &tampered,
        source_path,
        expected_sha,
        expected_bytes,
    )?;
    rejected(
        container_path,
        container_sha,
        container,
        source_path,
        &"0".repeat(64),
        expected_bytes,
    )?;
    rejected(
        container_path,
        container_sha,
        container,
        source_path,
        expected_sha,
        Some(if source_path == SOURCE_PATH {
            5525
        } else {
            38817
        }),
    )?;
    rejected(
        container_path,
        container_sha,
        container,
        source_path,
        expected_sha,
        Some(0),
    )?;
    if SHARED_BINDINGS.len() != 5
        || shared_binding("docs/rust-rewrite/o01/coverage.json", MAIN_SOURCE_PATH).is_some()
        || shared_binding("docs/rust-rewrite/a13/coverage.json", SOURCE_PATH).is_some()
    {
        return reject("shared consumer whitelist expanded");
    }
    Ok(())
}

fn shared_positive_controls(root: &Path) -> Result<()> {
    let mut checked = 0;
    for (container_path, container_sha, source_path) in SHARED_BINDINGS {
        let bytes = fs::read(root.join(container_path))
            .map_err(|error| format!("W06 shared positive manifest: {error}"))?;
        if sha256(&bytes) != container_sha {
            return reject("shared positive frozen manifest identity changed");
        }
        let value: Value =
            serde_json::from_slice(&bytes).map_err(|_| "W06 shared positive manifest malformed")?;
        // These are the exact arrays iterated by the three existing callers.
        let array = if container_path == "docs/rust-rewrite/a12/qualification-v1.json" {
            "sources"
        } else {
            "artifacts"
        };
        let rows = value[array]
            .as_array()
            .ok_or("shared positive source rows absent")?;
        let matching = rows
            .iter()
            .filter(|row| row["path"] == source_path)
            .collect::<Vec<_>>();
        if matching.len() != 1 {
            return reject("shared positive source binding missing or duplicated");
        }
        let pin = matching[0];
        let expected_sha = pin["sha256"]
            .as_str()
            .ok_or("shared positive digest absent")?;
        let expected_bytes = pin["bytes"].as_u64().map(|length| length as usize);
        validate_shared_binding(
            container_path,
            container_sha,
            &bytes,
            source_path,
            expected_sha,
            expected_bytes,
        )?;
        // Do not manufacture lengths in the caller contract. The independent
        // exact material proofs below still enforce 5525/5561 and38817/38843.
        if pin.get("bytes").is_some() || expected_bytes.is_some() {
            return reject("shared frozen caller unexpectedly supplies bytes");
        }
        checked += 1;
    }
    if checked != 5 {
        return reject("shared positive control count changed");
    }
    Ok(())
}
