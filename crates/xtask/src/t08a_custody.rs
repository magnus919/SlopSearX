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
fn opened_source_mode(root: &Path, path: &str, mode: u32) -> Result<&'static str> {
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
    need(
        out.status.success() && String::from_utf8_lossy(&out.stdout).starts_with("100644 "),
        "Windows current source index requires100644",
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
pub(crate) const CURRENT_LOCK_SHA: &str =
    "be8d91b79e48175ea1d3d523c05e5e21a9e8cac815bfb4d49db6fd188b8ca4d0";
pub(crate) const CURRENT_LOCK_BYTES: usize = 94358;
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
    need(
        current == physical
            && physical.len() == CURRENT_LOCK_BYTES
            && digest(&physical) == CURRENT_LOCK_SHA,
        "issue903 exact physical current lock",
    )?;
    let prior = project_issue903_config(root, "Cargo.lock", &physical, opened_mode)?;
    portal_health_lock_frame_prior(&prior)
}
fn portal_health_lock_frame_prior(current: &[u8]) -> Result<Vec<u8>> {
    need(
        current.len() == upstream_contracts_successor::LOCK_BYTES
            && digest(current) == upstream_contracts_successor::LOCK_SHA,
        "security exact projected prior lock",
    )?;
    let security = upstream_contracts_successor::frame("Cargo.lock", current, 0o644)?;
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
        return native_health_lock_prior(&portal_health_lock_frame(root, physical)?);
    }
    if path == "Cargo.toml" {
        return rmcp_security_prior(path, physical);
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
    need(
        mode == "0644"
            && supplied.len() == CURRENT_LOCK_BYTES
            && digest(supplied) == CURRENT_LOCK_SHA,
        "authenticated physical current lock bytes/mode",
    )?;
    let index = ROWS
        .iter()
        .position(|row| row.0 == path)
        .ok_or("unowned W07 Cargo.lock successor path")?;
    let logical = cli_logical_input(root, path, supplied, 0o644)?;
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
fn configured_control_scratch_root() -> Result<std::path::PathBuf> {
    let tmp_dir =
        std::env::var_os("TMPDIR").ok_or("TMPDIR must be configured for T08A controls")?;
    let fixture_root = std::env::var_os("SLOPSEARX_TEST_TEMP_ROOT")
        .ok_or("SLOPSEARX_TEST_TEMP_ROOT must be configured for T08A controls")?;
    let tmp_dir = std::path::PathBuf::from(tmp_dir);
    let fixture_root = std::path::PathBuf::from(fixture_root);
    need(
        tmp_dir.is_absolute() && fixture_root.is_absolute(),
        "T08A control scratch roots must be absolute",
    )?;
    let tmp_meta = std::fs::symlink_metadata(&tmp_dir).map_err(|error| error.to_string())?;
    let fixture_meta =
        std::fs::symlink_metadata(&fixture_root).map_err(|error| error.to_string())?;
    need(
        tmp_meta.is_dir()
            && !tmp_meta.file_type().is_symlink()
            && fixture_meta.is_dir()
            && !fixture_meta.file_type().is_symlink(),
        "T08A control scratch roots must be existing non-symlink directories",
    )?;
    let tmp_dir = tmp_dir.canonicalize().map_err(|error| error.to_string())?;
    let fixture_root = fixture_root
        .canonicalize()
        .map_err(|error| error.to_string())?;
    need(
        tmp_dir == fixture_root,
        "TMPDIR and SLOPSEARX_TEST_TEMP_ROOT must resolve to one registered scratch root",
    )?;
    Ok(tmp_dir)
}
fn upstream_controls_with_projected_lock(root: &Path) -> Result<()> {
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };
    let scratch_root = configured_control_scratch_root()?;
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_nanos();
    let overlay = scratch_root.join(format!("ssx-issue903-o02-{nonce}"));
    fs::create_dir(&overlay).map_err(|error| error.to_string())?;
    let populate = || -> Result<()> {
        let git_dir = Command::new("git")
            .args(["rev-parse", "--absolute-git-dir"])
            .current_dir(root)
            .output()
            .map_err(|error| error.to_string())?;
        need(git_dir.status.success(), "overlay Git directory lookup")?;
        let git_dir = String::from_utf8(git_dir.stdout)
            .map_err(|error| error.to_string())?
            .trim()
            .to_owned();
        fs::write(overlay.join(".git"), format!("gitdir: {git_dir}\n"))
            .map_err(|error| error.to_string())?;
        for (path, bytes) in [
            (
                "docs/rust-rewrite/o02/UPSTREAM_CONTRACTS_SOURCE_SUCCESSOR.json",
                regular(
                    root,
                    "docs/rust-rewrite/o02/UPSTREAM_CONTRACTS_SOURCE_SUCCESSOR.json",
                )?,
            ),
            (
                "crates/xtask/src/main.rs",
                regular(root, "crates/xtask/src/main.rs")?,
            ),
        ] {
            let destination = overlay.join(path);
            fs::create_dir_all(destination.parent().ok_or("overlay source parent")?)
                .map_err(|error| error.to_string())?;
            fs::write(&destination, bytes).map_err(|error| error.to_string())?;
            #[cfg(unix)]
            fs::set_permissions(&destination, fs::Permissions::from_mode(0o644))
                .map_err(|error| error.to_string())?;
        }
        let (physical_lock, mode) = regular_material(root, "Cargo.lock")?;
        let projected_lock = project_issue903_config(root, "Cargo.lock", &physical_lock, mode)?;
        let destination = overlay.join("Cargo.lock");
        fs::write(&destination, projected_lock).map_err(|error| error.to_string())?;
        #[cfg(unix)]
        fs::set_permissions(&destination, fs::Permissions::from_mode(0o644))
            .map_err(|error| error.to_string())?;
        Ok(())
    };
    let result = populate().and_then(|()| upstream_contracts_successor::controls(&overlay));
    fs::remove_dir_all(&overlay).map_err(|error| error.to_string())?;
    result
}

pub(crate) fn controls(root: &Path) -> Result<()> {
    issue903_config_controls(root)?;
    w07_test_dependency_source_successor::controls(root)?;
    upstream_controls_with_projected_lock(root)?;
    rmcp_security_controls(root)?;
    let (actual, mode) = regular_material(root, "Cargo.lock")?;
    let projected_prior_lock = project_issue903_config(root, "Cargo.lock", &actual, mode)?;
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
        opened_source_mode(root, path, opened_mode)?;
        let actual = if *path == "Cargo.lock" {
            let projected = project_issue903_config(root, path, &physical, opened_mode)?;
            upstream_contracts_successor::frame(path, &projected, 0o644)?
        } else {
            physical
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
