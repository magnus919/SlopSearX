//! Two-path CLI successor. Physical current bytes precede historical interpretation.
#[path = "t08a_main_join.rs"]
pub(crate) mod accepted_main_join;
#[path = "../../../docs/rust-rewrite/t08a/issue903-config-edge-successor.rs"]
mod issue903_config_successor;
#[allow(
    dead_code,
    clippy::duplicate_mod,
    reason = "Independent T08A and DNSpython roots share this stateless source proof; each exercises its owned paths"
)]
#[path = "../../../docs/rust-rewrite/o02/upstream-contracts-source-successor-v1.rs"]
pub(crate) mod upstream_contracts_successor;
#[path = "../../../docs/rust-rewrite/t08a/w07-test-dependency-source-successor-v1.rs"]
mod w07_test_dependency_source_successor;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{path::Path, process::Command};
type Result<T> = std::result::Result<T, String>;
const CAPSULE: &str = "docs/rust-rewrite/t08a/shared-source-successor.json";
const CAPSULE_SHA: &str = "bb6ae04922975769b8baa8fc0b1f1751326a9c2c15fb9b2052239493e5c98c38";
const ROWS: [(&str, usize); 2] = [("crates/xtask/src/main.rs", 6), ("Cargo.lock", 23)];
fn need(ok: bool, why: &str) -> Result<()> {
    if ok {
        Ok(())
    } else {
        Err(format!("T08A shared source custody: {why}"))
    }
}
fn digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
#[allow(
    clippy::duplicate_mod,
    reason = "Shared standalone reader is independently included by sealed Rust source gates"
)]
#[path = "../../../docs/rust-rewrite/t08a/bounded_source_reader.rs"]
mod bounded_source_reader;
pub(crate) use bounded_source_reader::bounded_read;

fn regular_material(root: &Path, path: &str) -> Result<(Vec<u8>, u32)> {
    bounded_read(root, path, 1048576)
}
fn regular(root: &Path, path: &str) -> Result<Vec<u8>> {
    regular_material(root, path).map(|(bytes, _)| bytes)
}
pub(crate) fn opened_source_mode(root: &Path, path: &str, mode: u32) -> Result<&'static str> {
    #[cfg(unix)]
    {
        let _ = (root, path);
        need(mode == 0o644, "opened physical source requires regular0644")?;
        Ok("0644")
    }
    #[cfg(not(unix))]
    {
        let _ = mode;
        source_mode(root, path)
    }
}

pub(crate) fn project_issue903_config(
    root: &Path,
    path: &str,
    current: &[u8],
    opened_mode: u32,
) -> Result<Vec<u8>> {
    opened_source_mode(root, path, opened_mode)?;
    issue903_config_successor::prior(root, path, current, opened_mode)
}
#[allow(
    dead_code,
    reason = "W07/O02 consumers use the identity; the standalone checker compiles this module as well"
)]
pub(crate) fn current_lock_identity() -> Result<(usize, String)> {
    issue903_config_successor::current_lock_identity()
}

pub(crate) fn issue903_config_controls(root: &Path) -> Result<()> {
    issue903_config_successor::controls(root)
}

#[cfg(not(unix))]
fn source_mode(root: &Path, path: &str) -> Result<&'static str> {
    // Windows does not expose Unix permission bits. Preserve the committed
    // source mode through the current index and independently reject links.
    let out = Command::new("git")
        .args(["ls-files", "--stage", "--", path])
        .current_dir(root)
        .output()
        .map_err(|e| e.to_string())?;
    let index = std::str::from_utf8(&out.stdout).map_err(|error| error.to_string())?;
    let mut entries = index.lines();
    need(
        out.status.success()
            && entries.next().is_some_and(|entry| {
                entry.starts_with("100644 ") && entry.ends_with(&format!(" 0\t{path}"))
            })
            && entries.next().is_none(),
        "Windows current source index requires one stage-zero file with mode100644",
    )?;
    Ok("0644")
}
fn capsule(root: &Path) -> Result<Value> {
    let raw = regular(root, CAPSULE)?;
    need(digest(&raw) == CAPSULE_SHA, "compiled capsule identity")?;
    let value: Value = serde_json::from_slice(&raw).map_err(|e| e.to_string())?;
    need(
        value["schema"] == "T08A_SHARED_MAIN_LOCK_V1"
            && value["rows"].as_array().is_some_and(|r| r.len() == 2),
        "closed two-path capsule",
    )?;
    for (index, (path, count)) in ROWS.iter().enumerate() {
        let row = &value["rows"][index];
        need(
            row["path"] == *path
                && row["edits"].as_array().is_some_and(|r| r.len() == *count)
                && row["prior"]["mode"] == "0644"
                && row["current"]["mode"] == "0644",
            "closed path/mode/edit count",
        )?;
    }
    Ok(value)
}
fn text<'a>(value: &'a Value, key: &str) -> Result<&'a str> {
    value[key].as_str().ok_or_else(|| format!("missing {key}"))
}
fn material(row: &Value, bytes: &[u8], mode: &str) -> Result<()> {
    need(
        row["sha256"] == digest(bytes)
            && row["bytes"].as_u64() == Some(bytes.len() as u64)
            && row["mode"] == mode,
        "exact bytes/length/mode",
    )
}
fn inverse(row: &Value, current: &[u8], mode: &str, count: usize) -> Result<Vec<u8>> {
    material(&row["current"], current, mode)?;
    let edits = row["edits"].as_array().ok_or("ordered edits missing")?;
    need(edits.len() == count, "finite edit count")?;
    let mut prior = Vec::new();
    let (mut old_cursor, mut cursor) = (0usize, 0usize);
    for edit in edits {
        let at = edit["at"]
            .as_u64()
            .and_then(|n| usize::try_from(n).ok())
            .ok_or("edit offset")?;
        let remove = text(edit, "remove_utf8")?.as_bytes();
        let insert = text(edit, "insert_utf8")?.as_bytes();
        need(remove != insert, "no no-op edit")?;
        let unchanged = at
            .checked_sub(old_cursor)
            .ok_or("overlapping prior spans")?;
        let end = cursor.checked_add(unchanged).ok_or("prefix overflow")?;
        prior.extend_from_slice(current.get(cursor..end).ok_or("current prefix")?);
        let next = end.checked_add(insert.len()).ok_or("insert overflow")?;
        need(
            current.get(end..next) == Some(insert),
            "exact inserted span",
        )?;
        prior.extend_from_slice(remove);
        old_cursor = at
            .checked_add(remove.len())
            .ok_or("prior offset overflow")?;
        cursor = next;
    }
    prior.extend_from_slice(current.get(cursor..).ok_or("current suffix")?);
    material(&row["prior"], &prior, mode)?;
    let mut forward = Vec::new();
    let mut cursor = 0;
    for edit in edits {
        let at = edit["at"].as_u64().unwrap() as usize;
        let remove = text(edit, "remove_utf8")?.as_bytes();
        let next = at.checked_add(remove.len()).ok_or("forward overflow")?;
        need(
            at >= cursor && prior.get(at..next) == Some(remove),
            "forward ordered span",
        )?;
        forward.extend_from_slice(&prior[cursor..at]);
        forward.extend_from_slice(text(edit, "insert_utf8")?.as_bytes());
        cursor = next;
    }
    forward.extend_from_slice(&prior[cursor..]);
    need(forward == current, "full forward reconstruction")?;
    Ok(prior)
}
// Security successor is authenticated before entering the unchanged historical chains.
const RMCP_SECURITY_CAPSULE: &str =
    include_str!("../../../docs/rust-rewrite/t08a/rmcp-security-successor.json");
const RMCP_SECURITY_CAPSULE_SHA: &str =
    "b348258e03bc90feafc68b721d583e5f977d660a90062bff20b8c0f8521aec09";
#[path = "../../../docs/rust-rewrite/t08a/rmcp-source-afterimage.rs"]
#[allow(
    dead_code,
    clippy::duplicate_mod,
    reason = "Independent proof roots use different entry points of the stateless inverse"
)]
mod rmcp_source_afterimage;
const RMCP_RUNTIME_SOURCES: &str =
    include_str!("../../../docs/rust-rewrite/t08a/rmcp-runtime-afterimages.json");
const RMCP_RUNTIME_SHA: &str = "562478fd0d40d0cfae8a2b93e1958d0e80b41feb44359caca9a79c15b899c402";

pub(crate) fn rmcp_historical_operand(
    path: &str,
    physical: &[u8],
    mode: &str,
) -> Result<Option<Vec<u8>>> {
    let mode = u32::from_str_radix(mode, 8).map_err(|error| error.to_string())?;
    let projected = if path == "crates/ssx-infra/Cargo.toml" {
        Some(issue903_config_successor::project_current(path, physical)?)
    } else {
        None
    };
    rmcp_source_afterimage::runtime_prior(
        RMCP_RUNTIME_SOURCES,
        RMCP_RUNTIME_SHA,
        path,
        projected.as_deref().unwrap_or(physical),
        mode,
    )
}

const OPTIONAL_CHRONO: &str =
    include_str!("../../../docs/rust-rewrite/t08a/rmcp-optional-chrono-successor.json");
const OPTIONAL_CHRONO_SHA: &str =
    "648bb813290fa1757c5ab48cc8b85a21e8deb418d3ce9f74424a0d6febf82471";
const OPTIONAL_CHRONO_ROWS: [(&str, usize); 3] = [
    ("Cargo.lock", 23),
    ("Cargo.toml", 3),
    ("crates/ssx-infra/Cargo.toml", 1),
];
pub(crate) const CURRENT_LOCK_SHA: &str =
    "fd04cdf3d534a04be379d067583ff2dfb383fd98e6982092e922a00d56d4e7c9";
pub(crate) const CURRENT_LOCK_BYTES: usize = 96180;

pub(crate) fn optional_chrono_prior(path: &str, current: &[u8], mode: &str) -> Result<Vec<u8>> {
    need(
        digest(OPTIONAL_CHRONO.as_bytes()) == OPTIONAL_CHRONO_SHA,
        "compiled RMCP optional Chrono transition",
    )?;
    let doc: Value = serde_json::from_str(OPTIONAL_CHRONO).map_err(|e| e.to_string())?;
    need(
        doc["schema"] == "RMCP_OPTIONAL_CHRONO_SUCCESSOR_V1"
            && doc["rows"]
                .as_array()
                .is_some_and(|rows| rows.len() == OPTIONAL_CHRONO_ROWS.len()),
        "closed RMCP optional Chrono source set",
    )?;
    let (idx, (_, count)) = OPTIONAL_CHRONO_ROWS
        .iter()
        .enumerate()
        .find(|(_, (owned, _))| *owned == path)
        .ok_or("unowned RMCP optional Chrono source")?;
    let row = &doc["rows"][idx];
    need(
        row["path"] == path,
        "exact RMCP optional Chrono source path",
    )?;
    let workspace = (path == "Cargo.toml")
        .then(|| issue903_config_successor::gateway_workspace_prior(current))
        .transpose()?;
    inverse(row, workspace.as_deref().unwrap_or(current), mode, *count)
}

#[allow(dead_code, reason = "A08 caller is compiled only in xtask")]
pub(crate) fn prior_infra_manifest(root: &Path, supplied: &[u8]) -> Result<Vec<u8>> {
    const PATH: &str = "crates/ssx-infra/Cargo.toml";
    let (physical, opened_mode) = regular_material(root, PATH)?;
    need(
        physical == supplied,
        "RMCP infrastructure manifest must be physical current",
    )?;
    let projected = project_issue903_config(root, PATH, &physical, opened_mode)?;
    optional_chrono_prior(PATH, &projected, "0644")
}
const RMCP_ROWS: [(&str, usize); 2] = [("Cargo.toml", 1), ("Cargo.lock", 6)];
fn rmcp_security_capsule(raw: &str) -> Result<Value> {
    need(
        digest(raw.as_bytes()) == RMCP_SECURITY_CAPSULE_SHA,
        "compiled rmcp security capsule identity",
    )?;
    let doc: Value = serde_json::from_str(raw).map_err(|e| e.to_string())?;
    need(
        doc["schema"] == "RMCP_SECURITY_SUCCESSOR_V1"
            && doc["rows"].as_array().is_some_and(|rows| rows.len() == 2),
        "closed rmcp security capsule",
    )?;
    for (index, (path, count)) in RMCP_ROWS.iter().enumerate() {
        let row = &doc["rows"][index];
        need(
            row["path"] == *path
                && row["edits"]
                    .as_array()
                    .is_some_and(|edits| edits.len() == *count),
            "exact rmcp path and edit count",
        )?;
    }
    Ok(doc)
}
fn rmcp_security_prior(path: &str, physical: &[u8]) -> Result<Vec<u8>> {
    let index = RMCP_ROWS
        .iter()
        .position(|row| row.0 == path)
        .ok_or("unowned rmcp successor path")?;
    let doc = rmcp_security_capsule(RMCP_SECURITY_CAPSULE)?;
    inverse(&doc["rows"][index], physical, "0644", RMCP_ROWS[index].1)
}

const RMCP_AUTH_DIAGNOSTICS: &str =
    include_str!("../../../docs/rust-rewrite/t08a/rmcp-auth-diagnostics-successor.json");
const RMCP_AUTH_DIAGNOSTICS_SHA: &str =
    "f4116ccda0db1d057200475a266e2041f283272b99c6b59ec8cb1758c6e726cf";
const RMCP_AUTH_PARENT: &str = include_str!("../../../docs/rust-rewrite/t08a/rmcp-provenance.json");
const RMCP_AUTH_ROWS: [(&str, usize); 2] = [
    ("vendor/rmcp/src/transport/auth.rs", 28),
    ("vendor/rmcp/src/transport/auth/metadata_resolver.rs", 1),
];

const RMCP_AUTH_REDIRECT: &str =
    include_str!("../../../docs/rust-rewrite/t08a/rmcp-auth-redirect-successor.json");
const RMCP_AUTH_REDIRECT_SHA: &str =
    "4369485b29c893e40c3f339c544976a5f27863feb339191e28410642a061b399";
const RMCP_AUTH_REDIRECT_EDITS: usize = 13;

const RMCP_AUTH_ADDITIVE_ROOTS: &str =
    include_str!("../../../docs/rust-rewrite/t07/additive-roots-rmcp-successor-v1.json");
const RMCP_AUTH_ADDITIVE_ROOTS_SHA: &str =
    "159170ce928f5dbb7d0bfa3229d3c2539b5b9a5df896605e690506007a51d02d";
const RMCP_AUTH_ADDITIVE_ROOTS_EDITS: usize = 7;

fn rmcp_auth_redirect_prior(path: &str, current: &[u8], mode: &str) -> Result<Vec<u8>> {
    need(
        digest(RMCP_AUTH_REDIRECT.as_bytes()) == RMCP_AUTH_REDIRECT_SHA,
        "compiled RMCP auth redirect patch identity",
    )?;
    let patch: Value = serde_json::from_str(RMCP_AUTH_REDIRECT).map_err(|e| e.to_string())?;
    let row = &patch["row"];
    need(
        patch["schema"] == "RMCP_AUTH_REDIRECT_SUCCESSOR_V1"
            && patch["prior_diagnostics_sha256"] == RMCP_AUTH_DIAGNOSTICS_SHA
            && path == "vendor/rmcp/src/transport/auth.rs"
            && row["path"] == path,
        "closed RMCP auth redirect patch and predecessor",
    )?;
    need(
        digest(RMCP_AUTH_ADDITIVE_ROOTS.as_bytes()) == RMCP_AUTH_ADDITIVE_ROOTS_SHA,
        "compiled RMCP auth additive-root patch identity",
    )?;
    let roots: Value = serde_json::from_str(RMCP_AUTH_ADDITIVE_ROOTS).map_err(|e| e.to_string())?;
    let roots_row = &roots["row"];
    need(
        roots["schema"] == "T07_RMCP_AUTH_ADDITIVE_ROOTS_SUCCESSOR_V1"
            && roots["prior_redirect_sha256"] == RMCP_AUTH_REDIRECT_SHA
            && roots_row["path"] == path
            && roots_row["prior"] == row["current"],
        "closed RMCP auth additive-root patch and predecessor",
    )?;
    let current = inverse(roots_row, current, mode, RMCP_AUTH_ADDITIVE_ROOTS_EDITS)?;
    let prior = inverse(row, &current, mode, RMCP_AUTH_REDIRECT_EDITS)?;
    let mut altered = current;
    altered.push(b'\n');
    need(
        inverse(row, &altered, mode, RMCP_AUTH_REDIRECT_EDITS).is_err(),
        "altered RMCP auth redirect source rejected",
    )?;
    Ok(prior)
}

fn rmcp_auth_diagnostics_controls(root: &Path) -> Result<()> {
    need(
        digest(RMCP_AUTH_DIAGNOSTICS.as_bytes()) == RMCP_AUTH_DIAGNOSTICS_SHA,
        "compiled RMCP auth diagnostic patch identity",
    )?;
    let patch: Value = serde_json::from_str(RMCP_AUTH_DIAGNOSTICS).map_err(|e| e.to_string())?;
    need(
        patch["schema"] == "RMCP_AUTH_DIAGNOSTICS_SUCCESSOR_V1"
            && patch["prior_provenance_sha256"] == digest(RMCP_AUTH_PARENT.as_bytes())
            && patch["rows"]
                .as_array()
                .is_some_and(|rows| rows.len() == RMCP_AUTH_ROWS.len()),
        "closed RMCP auth diagnostic patch and predecessor",
    )?;
    let parent: Value = serde_json::from_str(RMCP_AUTH_PARENT).map_err(|e| e.to_string())?;
    for (index, (path, edit_count)) in RMCP_AUTH_ROWS.into_iter().enumerate() {
        let row = &patch["rows"][index];
        need(
            row["path"] == path,
            "exact RMCP auth diagnostic source path",
        )?;
        let (bytes, physical_mode) = regular_material(root, path)?;
        let mode = opened_source_mode(root, path, physical_mode)?;
        let bytes = if path == "vendor/rmcp/src/transport/auth.rs" {
            rmcp_auth_redirect_prior(path, &bytes, mode)?
        } else {
            bytes
        };
        let prior = inverse(row, &bytes, mode, edit_count)?;
        let member = path
            .strip_prefix("vendor/rmcp/")
            .ok_or("RMCP member path")?;
        need(
            parent["files"][member] == digest(&prior),
            "RMCP auth diagnostic predecessor matches package provenance",
        )?;
        let mut altered = bytes;
        altered.push(b'\n');
        need(
            inverse(row, &altered, mode, edit_count).is_err(),
            "altered RMCP auth diagnostic source rejected",
        )?;
    }
    Ok(())
}

// One additive std-only health package; immutable CLI lock capsule stays unchanged.
pub(crate) const HEALTH_LOCK_SHA: &str =
    "53fb4619c6e4fd12eae7928073c5062929498d2db29b270e456514a855a5bdd9";
pub(crate) const HEALTH_LOCK_BYTES: usize = 94250;
const HEALTH_LOCK_STANZA: &str = "[[package]]\nname = \"ssx-healthcheck\"\nversion = \"0.1.0\"\n\n";
// T03 adds three edges to already-pinned workspace packages. Authenticate
// physical current first, then reconstruct the immutable accepted health frame.
pub(crate) const PORTAL_LOCK_SHA: &str =
    "37415fcb0b57ae7ab3f502067485cb39143827aee6e54a8acb2e87b6a7e24549";
pub(crate) const PORTAL_LOCK_BYTES: usize = 94296;
const PORTAL_LOCK_EDITS: [(usize, &[u8]); 3] = [
    (65174, b" \"unicode-normalization\",\n"),
    (66903, b" \"scraper\",\n"),
    (91635, b" \"url\",\n"),
];
pub(crate) fn portal_health_lock_frame(root: &Path, current: &[u8]) -> Result<Vec<u8>> {
    let (physical, opened_mode) = regular_material(root, "Cargo.lock")?;
    need(current == physical, "issue903 exact physical current lock")?;
    portal_health_lock_from_admitted(root, &physical, opened_mode)
}

fn portal_health_lock_from_admitted(root: &Path, physical: &[u8], mode: u32) -> Result<Vec<u8>> {
    let prior = project_issue903_config(root, "Cargo.lock", physical, mode)?;
    portal_health_lock_frame_prior(&prior)
}
fn portal_health_lock_frame_prior(current: &[u8]) -> Result<Vec<u8>> {
    need(
        current.len() == CURRENT_LOCK_BYTES && digest(current) == CURRENT_LOCK_SHA,
        "security exact projected prior lock",
    )?;
    let accepted = optional_chrono_prior("Cargo.lock", current, "0644")?;
    let security = upstream_contracts_successor::frame("Cargo.lock", &accepted, 0o644)?;
    let portal = rmcp_security_prior("Cargo.lock", &security)?;
    let physical = portal.as_slice();
    need(
        physical.len() == PORTAL_LOCK_BYTES && digest(physical) == PORTAL_LOCK_SHA,
        "portal exact current lock",
    )?;
    let mut health = Vec::with_capacity(HEALTH_LOCK_BYTES);
    let (mut prior_cursor, mut current_cursor) = (0usize, 0usize);
    for (at, inserted) in PORTAL_LOCK_EDITS {
        let end = current_cursor + (at - prior_cursor);
        health.extend_from_slice(&physical[current_cursor..end]);
        need(
            physical.get(end..end + inserted.len()) == Some(inserted),
            "portal exact dependency insertion",
        )?;
        prior_cursor = at;
        current_cursor = end + inserted.len();
    }
    health.extend_from_slice(&physical[current_cursor..]);
    need(
        health.len() == HEALTH_LOCK_BYTES && digest(&health) == HEALTH_LOCK_SHA,
        "portal exact accepted health lock",
    )?;
    let mut forward = Vec::with_capacity(PORTAL_LOCK_BYTES);
    let mut cursor = 0;
    for (at, inserted) in PORTAL_LOCK_EDITS {
        forward.extend_from_slice(&health[cursor..at]);
        forward.extend_from_slice(inserted);
        cursor = at;
    }
    forward.extend_from_slice(&health[cursor..]);
    need(forward == physical, "portal full forward reconstruction")?;
    Ok(health)
}
fn native_health_lock_prior(physical: &[u8]) -> Result<Vec<u8>> {
    need(
        physical.len() == HEALTH_LOCK_BYTES && digest(physical) == HEALTH_LOCK_SHA,
        "native health exact current lock",
    )?;
    let current = std::str::from_utf8(physical).map_err(|e| e.to_string())?;
    need(
        current.matches(HEALTH_LOCK_STANZA).count() == 1,
        "native health unique package",
    )?;
    let at = current
        .find(HEALTH_LOCK_STANZA)
        .ok_or("native health package missing")?;
    let mut prior = physical.to_vec();
    prior.drain(at..at + HEALTH_LOCK_STANZA.len());
    need(
        prior.len() == 94194
            && digest(&prior) == "7f64930ad5267129bcd5f750de6bba4249c8a3bfde3a3de1753a853b5e1db9d9",
        "native health exact accepted-main lock",
    )?;
    let mut forward = prior.clone();
    forward.splice(at..at, HEALTH_LOCK_STANZA.bytes());
    need(
        forward == physical,
        "native health full forward reconstruction",
    )?;
    Ok(prior)
}
fn cli_logical_input(
    root: &Path,
    path: &str,
    physical: &[u8],
    opened_mode: u32,
) -> Result<Vec<u8>> {
    if path == "Cargo.lock" {
        return native_health_lock_prior(&portal_health_lock_from_admitted(
            root,
            physical,
            opened_mode,
        )?);
    }
    if path == "Cargo.toml" {
        let accepted =
            optional_chrono_prior(path, physical, opened_source_mode(root, path, opened_mode)?)?;
        return rmcp_security_prior(path, &accepted);
    }
    if w07_test_dependency_source_successor::owns(path) {
        return w07_test_dependency_source_successor::prior(root, path, physical, opened_mode);
    }
    if accepted_main_join::owns(path) {
        accepted_main_join::project(root, path, physical, accepted_main_join::Parent::Cli)
    } else {
        Ok(physical.to_vec())
    }
}
/// Returns one exact pre-CLI frame only after authenticating physical current.
/// Historical bytes supplied as current are rejected, even if the caller owns them.
pub(crate) fn prior(root: &Path, path: &str, supplied: &[u8]) -> Result<Vec<u8>> {
    let index = ROWS
        .iter()
        .position(|r| r.0 == path)
        .ok_or("unowned CLI successor path")?;
    let (current, mode) = regular_material(root, path)?;
    need(
        current == supplied,
        "supplied bytes are not physical current",
    )?;
    let logical = cli_logical_input(root, path, &current, mode)?;
    let doc = capsule(root)?;
    inverse(
        &doc["rows"][index],
        &logical,
        opened_source_mode(root, path, mode)?,
        ROWS[index].1,
    )
}
/// Reconstruct the retained W07 Cargo.lock frame from bytes already admitted
/// through the platform-specific bounded physical read. This path deliberately does not
/// reopen Cargo.lock; only the exact current lock and W07 predecessor tuple is
/// accepted.
#[allow(
    dead_code,
    reason = "W07-only caller; this shared custody module is also compiled by ssx-cli-check"
)]
pub(crate) fn w07_lock_prior_from_physical(
    root: &Path,
    path: &str,
    expected_prior: &str,
    supplied: &[u8],
    mode: &str,
) -> Result<Vec<u8>> {
    const W07_PRIOR_SHA: &str = "61be734ee8259d76108383154509e8c261e2cf064dc9ebf4def23a9d2a8332dc";
    const W07_PRIOR_COMMIT: &str = "e763c77397cad717d47de690a0dcbba4d0420aa3";
    const W07_PRIOR_BLOB: &str = "356b40d5915deaec9113e754deb7a9d8663ab146";
    need(
        path == "Cargo.lock" && expected_prior == W07_PRIOR_SHA,
        "exact W07 lock caller tuple",
    )?;
    need(mode == "0644", "authenticated physical current lock mode")?;
    let index = ROWS
        .iter()
        .position(|row| row.0 == path)
        .ok_or("unowned W07 Cargo.lock successor path")?;
    let mode_bits = if cfg!(windows) { 0 } else { 0o644 };
    let logical = cli_logical_input(root, path, supplied, mode_bits)?;
    let doc = capsule(root)?;
    let prior = inverse(&doc["rows"][index], &logical, mode, ROWS[index].1)?;
    need(
        prior.len() == 87572 && digest(&prior) == expected_prior,
        "exact historical W07 joined lock frame",
    )?;
    let git = Command::new("git")
        .args(["show", &format!("{W07_PRIOR_COMMIT}:{path}")])
        .current_dir(root)
        .output()
        .map_err(|error| error.to_string())?;
    need(
        git.status.success() && git.stdout == prior,
        "historical W07 Cargo.lock Git bytes",
    )?;
    let tree = Command::new("git")
        .args(["ls-tree", W07_PRIOR_COMMIT, "--", path])
        .current_dir(root)
        .output()
        .map_err(|error| error.to_string())?;
    need(
        tree.status.success()
            && tree.stdout == format!("100644 blob {W07_PRIOR_BLOB}\t{path}\n").as_bytes(),
        "historical W07 Cargo.lock Git mode/blob",
    )?;
    Ok(prior)
}
/// Portable byte/Git controls. Unix also proves physical permission bits;
/// Windows proves current Git index mode, without claiming POSIX chmod semantics.
fn controls_for(
    root: &Path,
    rows: &[(&str, usize)],
    doc: &Value,
    normalize: fn(&Path, &str, &[u8]) -> Result<Vec<u8>>,
) -> Result<()> {
    for (index, (path, count)) in rows.iter().enumerate() {
        let row = &doc["rows"][index];
        let (actual, opened_mode) = regular_material(root, path)?;
        let accepted_mode = opened_source_mode(root, path, opened_mode)?;
        let old = normalize(root, path, &actual)?;
        let logical = cli_logical_input(root, path, &actual, opened_mode)?;
        need(
            inverse(row, &logical, accepted_mode, *count)? == old,
            "original pure positive after physical outer admission",
        )?;
        let commit = text(&row["prior"], "commit")?;
        let git = Command::new("git")
            .args(["show", &format!("{commit}:{path}")])
            .current_dir(root)
            .output()
            .map_err(|e| e.to_string())?;
        need(
            git.status.success() && git.stdout == old,
            "exact pre-CLI Git bytes",
        )?;
        let tree = Command::new("git")
            .args(["ls-tree", commit, "--", path])
            .current_dir(root)
            .output()
            .map_err(|e| e.to_string())?;
        need(
            tree.status.success()
                && tree.stdout
                    == format!("100644 blob {}\t{path}\n", text(&row["prior"], "git_blob")?)
                        .as_bytes(),
            "pre-CLI Git mode/blob",
        )?;
        need(
            normalize(root, path, &old).is_err(),
            "historical frame rejected as supplied current",
        )?;
        let mut changed = actual.clone();
        changed.push(b'\n');
        need(
            normalize(root, path, &changed).is_err(),
            "changed supplied current",
        )?;
        for mode in ["0600", "0755", "4644"] {
            need(
                inverse(row, &logical, mode, *count).is_err(),
                "mode substitution",
            )?;
        }
        for mutation in 0..3 {
            let mut wrong = row.clone();
            let edits = wrong["edits"].as_array_mut().unwrap();
            match mutation {
                0 => {
                    edits.pop();
                }
                1 => {
                    edits.push(edits[0].clone());
                }
                _ => {
                    edits[0]["at"] = serde_json::json!(usize::MAX);
                }
            }
            need(
                inverse(&wrong, &logical, "0644", *count).is_err(),
                "missing/duplicate/overflow span",
            )?;
        }
    }
    Ok(())
}

// Separate closed legacy reader admission; the original main/lock ROWS remain2.
pub(crate) const LEGACY_CAPSULE_PATH: &str =
    "docs/rust-rewrite/t08a/legacy-input-successor-v2.json";
const LEGACY_CAPSULE_SHA: &str = "ef0098cb7eb3f33cd7295bb982fe4c34b146a97644a612cf26c21cea48713479";
const LEGACY_ROWS: [(&str, usize); 5] = [
    ("Cargo.toml", 1),
    (
        "docs/rust-rewrite/a06/source-amendments/shared-successor.rs",
        1,
    ),
    (
        "docs/rust-rewrite/a06/source-amendments/w07-t02-joint.rs",
        5,
    ),
    ("crates/xtask/src/a13.rs", 1),
    (
        "docs/rust-rewrite/w07/source-amendments/test_dependency_successor.rs",
        7,
    ),
];
fn legacy_capsule(root: &Path) -> Result<Value> {
    let raw = regular(root, LEGACY_CAPSULE_PATH)?;
    need(
        digest(&raw) == LEGACY_CAPSULE_SHA,
        "compiled legacy capsule identity",
    )?;
    let doc: Value = serde_json::from_slice(&raw).map_err(|e| e.to_string())?;
    need(
        doc["schema"] == "T08A_FIVE_LEGACY_INPUTS_V2"
            && doc["rows"].as_array().is_some_and(|rows| rows.len() == 5),
        "closed legacy four-path schema",
    )?;
    for (index, (path, count)) in LEGACY_ROWS.iter().enumerate() {
        let row = &doc["rows"][index];
        need(
            row["path"] == *path
                && row["prior"]["mode"] == "0644"
                && row["current"]["mode"] == "0644"
                && row["edits"]
                    .as_array()
                    .is_some_and(|edits| edits.len() == *count),
            "exact legacy path/modes/edit count",
        )?;
    }
    Ok(doc)
}
pub(crate) fn legacy_prior(root: &Path, path: &str, supplied: &[u8]) -> Result<Vec<u8>> {
    let index = LEGACY_ROWS
        .iter()
        .position(|row| row.0 == path)
        .ok_or("unowned legacy successor path")?;
    let (physical, mode) = regular_material(root, path)?;
    need(
        physical == supplied,
        "legacy supplied is not physical current",
    )?;
    let logical = cli_logical_input(root, path, &physical, mode)?;
    let doc = legacy_capsule(root)?;
    inverse(
        &doc["rows"][index],
        &logical,
        opened_source_mode(root, path, mode)?,
        LEGACY_ROWS[index].1,
    )
}
/// One exact historical W07 caller; common custody authenticates current first.
pub(crate) const TEST_DEPENDENCY_SOURCE: &str =
    "docs/rust-rewrite/w07/source-amendments/test_dependency_successor.rs";
pub(crate) const TEST_DEPENDENCY_PRIOR_SHA: &str =
    "f2103c06807e1ba1490c1d972ebe3abaebdc09a3243845f4478fc40260608365";
const TEST_DEPENDENCY_ARCHIVE: &str =
    "docs/rust-rewrite/t08a/source-parent-frames/test_dependency_successor.rs";
pub(crate) fn test_dependency_prior(
    root: &Path,
    path: &str,
    expected: &str,
    supplied: &[u8],
) -> Result<Vec<u8>> {
    need(
        path == TEST_DEPENDENCY_SOURCE && expected == TEST_DEPENDENCY_PRIOR_SHA,
        "closed W07 dependency caller path/hash",
    )?;
    let prior = legacy_prior(root, path, supplied)?;
    let (archive, archive_mode) = regular_material(root, TEST_DEPENDENCY_ARCHIVE)?;
    opened_source_mode(root, TEST_DEPENDENCY_ARCHIVE, archive_mode)?;
    need(
        digest(&archive) == TEST_DEPENDENCY_PRIOR_SHA && archive == prior,
        "exact immutable W07 dependency archive",
    )?;
    Ok(prior)
}
#[cfg(any(all(test, unix), windows))]
fn configured_control_scratch_root() -> Result<std::path::PathBuf> {
    fn checked_root(value: std::ffi::OsString) -> Result<std::path::PathBuf> {
        let path = std::path::PathBuf::from(value);
        need(
            path.is_absolute(),
            "T08A control scratch root must be absolute",
        )?;
        let metadata = std::fs::symlink_metadata(&path).map_err(|error| error.to_string())?;
        need(
            metadata.is_dir() && !metadata.file_type().is_symlink(),
            "T08A control scratch root must be an existing non-symlink directory",
        )?;
        path.canonicalize().map_err(|error| error.to_string())
    }
    let scratch = match std::env::var_os("TMPDIR") {
        Some(path) => path,
        // Foundation's T06 job already has a runner-owned scratch directory.
        // Each custody control creates and retires its own child there.
        None if std::env::var("GITHUB_ACTIONS").as_deref() == Ok("true") => {
            std::env::var_os("RUNNER_TEMP").ok_or("RUNNER_TEMP is missing in GitHub Actions")?
        }
        None => return Err("TMPDIR must be configured for T08A controls".into()),
    };
    let owned = checked_root(scratch)?;
    if let Some(value) = std::env::var_os("SLOPSEARX_TEST_TEMP_ROOT") {
        need(
            owned == checked_root(value)?,
            "TMPDIR and SLOPSEARX_TEST_TEMP_ROOT must resolve to one registered scratch root",
        )?;
    }
    Ok(owned)
}
pub(crate) fn controls(root: &Path) -> Result<()> {
    #[cfg(any(all(test, unix), windows))]
    bounded_source_reader::controls(&configured_control_scratch_root()?);
    issue903_config_controls(root)?;
    w07_test_dependency_source_successor::controls(root)?;
    rmcp_security_controls(root)?;
    rmcp_auth_diagnostics_controls(root)?;
    let (actual, mode) = regular_material(root, "Cargo.lock")?;
    let projected_prior_lock = project_issue903_config(root, "Cargo.lock", &actual, mode)?;
    let accepted = optional_chrono_prior("Cargo.lock", &projected_prior_lock, "0644")?;
    upstream_contracts_successor::controls(root, &accepted)?;
    let health = portal_health_lock_frame(root, &actual)?;
    need(
        portal_health_lock_frame_prior(&health).is_err(),
        "portal historical health lock is not current",
    )?;
    for (at, inserted) in PORTAL_LOCK_EDITS {
        let shifted = at
            + PORTAL_LOCK_EDITS
                .iter()
                .filter(|edit| edit.0 < at)
                .map(|edit| edit.1.len())
                .sum::<usize>();
        let mut missing = projected_prior_lock.clone();
        missing.drain(shifted..shifted + inserted.len());
        need(
            portal_health_lock_frame_prior(&missing).is_err(),
            "portal missing dependency edge rejected",
        )?;
        let mut duplicate = projected_prior_lock.clone();
        duplicate.splice(shifted..shifted, inserted.iter().copied());
        need(
            portal_health_lock_frame_prior(&duplicate).is_err(),
            "portal duplicate dependency edge rejected",
        )?;
    }
    let mut unrelated = projected_prior_lock.clone();
    unrelated[0] ^= 1;
    need(
        portal_health_lock_frame_prior(&unrelated).is_err(),
        "portal unrelated lock mutation rejected",
    )?;
    let mut appended = projected_prior_lock.clone();
    appended.push(b'\n');
    need(
        portal_health_lock_frame_prior(&appended).is_err(),
        "portal appended lock bytes rejected",
    )?;
    let prior_lock = native_health_lock_prior(&health)?;
    need(
        native_health_lock_prior(&prior_lock).is_err(),
        "native health old lock is not current",
    )?;
    let mut duplicate = health.clone();
    duplicate.extend_from_slice(HEALTH_LOCK_STANZA.as_bytes());
    need(
        native_health_lock_prior(&duplicate).is_err(),
        "native health duplicate package",
    )?;
    let mut changed = health.clone();
    let at = changed
        .windows(b"ssx-healthcheck".len())
        .position(|v| v == b"ssx-healthcheck")
        .ok_or("native health package missing")?;
    changed[at] = b'x';
    need(
        native_health_lock_prior(&changed).is_err(),
        "native health changed package",
    )?;
    accepted_main_join::controls(root)?;
    controls_for(root, &ROWS, &capsule(root)?, prior)?;
    controls_for(root, &LEGACY_ROWS, &legacy_capsule(root)?, legacy_prior)?;
    let actual = regular(root, TEST_DEPENDENCY_SOURCE)?;
    test_dependency_prior(
        root,
        TEST_DEPENDENCY_SOURCE,
        TEST_DEPENDENCY_PRIOR_SHA,
        &actual,
    )?;
    Ok(())
}

// Run through the existing source-custody gate, preserving its closed test inventory.
fn rmcp_security_controls(root: &Path) -> Result<()> {
    let doc = rmcp_security_capsule(RMCP_SECURITY_CAPSULE)?;
    need(
        rmcp_security_capsule(&format!("{RMCP_SECURITY_CAPSULE} ")).is_err(),
        "altered security capsule rejected",
    )?;
    for (index, (path, count)) in RMCP_ROWS.iter().enumerate() {
        let (physical, opened_mode) = regular_material(root, path)?;
        let mode = opened_source_mode(root, path, opened_mode)?;
        let projected = if *path == "Cargo.lock" {
            project_issue903_config(root, path, &physical, opened_mode)?
        } else {
            physical
        };
        let accepted = optional_chrono_prior(path, &projected, mode)?;
        let actual = if *path == "Cargo.lock" {
            upstream_contracts_successor::frame(path, &accepted, 0o644)?
        } else {
            accepted
        };
        let prior = rmcp_security_prior(path, &actual)?;
        let git = Command::new("git")
            .args([
                "show",
                &format!("8ee37a22d393ccff7493c5196be268cd872c140f:{path}"),
            ])
            .current_dir(root)
            .output()
            .map_err(|e| e.to_string())?;
        need(
            git.status.success() && prior == git.stdout,
            "security exact Git predecessor",
        )?;
        need(
            rmcp_security_prior(path, &prior).is_err(),
            "security historical input rejected",
        )?;
        need(
            rmcp_security_prior("unowned", &actual).is_err(),
            "security unowned path rejected",
        )?;
        let mut changed = actual.clone();
        changed[0] ^= 1;
        need(
            rmcp_security_prior(path, &changed).is_err(),
            "security changed bytes rejected",
        )?;
        for mode in ["0600", "0755", "4644"] {
            need(
                inverse(&doc["rows"][index], &actual, mode, *count).is_err(),
                "security changed mode rejected",
            )?;
        }
        for mutation in 0..3 {
            let mut row = doc["rows"][index].clone();
            let edits = row["edits"]
                .as_array_mut()
                .ok_or("security control edits")?;
            match mutation {
                0 => {
                    edits.pop();
                }
                1 => {
                    edits.push(edits[0].clone());
                }
                _ => {
                    edits[0]["insert_utf8"] = Value::String("wrong".into());
                }
            }
            need(
                inverse(&row, &actual, "0644", *count).is_err(),
                "security altered spans rejected",
            )?;
        }
    }
    Ok(())
}
