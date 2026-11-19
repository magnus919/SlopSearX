//! Exact #1129 runtime/dev audit successor over accepted main.
//! Current physical source is authenticated before reconstructing the fixed predecessor.
#[allow(
    dead_code,
    clippy::duplicate_mod,
    reason = "T08 bounded physical source reader shared with T08A and T06 custody roots"
)]
#[path = "../../t08a/bounded_source_reader.rs"]
mod bounded_source_reader;
#[path = "../../../../crates/xtask/src/o02_image_gate_successor.rs"]
mod o02_image_gate_successor;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::Read,
    path::{Component, Path},
    process::Command,
};
type Result<T> = std::result::Result<T, String>;
pub(crate) const CAPSULE_PATH: &str =
    "docs/rust-rewrite/t06/source-amendments/python-dependency-audit-current-main-v1/capsule.json";
const CAPSULE: &[u8] = include_bytes!("python-dependency-audit-current-main-v1/capsule.json");
const CAPSULE_SHA: &str = "04bbd43240f0d0663ec4e8fd993ddf7c616aeabcfa64e04df429751027b2b4e0";
const SOURCE_READER_OUTER: &[u8] =
    include_bytes!("python-dependency-audit-current-main-v1/source-reader-successor-v1.json");
const SOURCE_READER_OUTER_SHA: &str =
    "9a874473ee7a2956636fccb4abfe2141d3b5e69a6cd88cf5758b5f587c31be64";
const BASE: &str = "6cd400c2af9ec77613caf9ecab27a7264acac79f";
const LIMIT: u64 = 1_048_576;
const SOURCE_READER_PATH: &str =
    "docs/rust-rewrite/t06/source-amendments/python-dependency-audit-source-reader-v1.rs";
const SOURCE_READER_OLD_BYTES: usize = 31_817;
const SOURCE_READER_OLD_SHA: &str =
    "e66d91525c9b50a26d1fabd1eb2bf137d83e2fc9ed8c7cbe5a6e71dda8edfca9";
const SOURCE_READER_OLD_BLOB: &str = "40f336a08289b7324b464c4f65436ab09200488a";
const ROWS: [(&str, usize); 8] = [
    (".github/workflows/ci.yml", 3),
    (".github/workflows/nightly.yml", 1),
    ("docs/rust-rewrite/ci/gates.json", 7),
    (
        "docs/rust-rewrite/o02/python-dependency-audit-gates-reader-v1.rs",
        7,
    ),
    (
        "docs/rust-rewrite/t06/source-amendments/lock_amendment.rs",
        1,
    ),
    (
        "docs/rust-rewrite/t06/source-amendments/python-dependency-audit-source-reader-v1.rs",
        27,
    ),
    ("pyproject.toml", 5),
    ("uv.lock", 47),
];
struct Pin<'a> {
    path: &'a str,
    current_sha: &'a str,
    current_bytes: usize,
    prior_path: &'a str,
    prior_sha: &'a str,
    prior_bytes: usize,
    blob: &'a str,
    current_blob: &'a str,
    edits: usize,
}
fn need(ok: bool, reason: &str) -> Result<()> {
    if ok {
        Ok(())
    } else {
        Err(format!(
            "Python dependency audit source successor: {reason}"
        ))
    }
}
fn digest(raw: &[u8]) -> String {
    Sha256::digest(raw)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
pub(crate) fn owns(path: &str) -> bool {
    ROWS.iter().any(|row| row.0 == path)
}
/// Validate the mode from the caller's bounded open before pure projection.
pub(crate) fn admit_opened_mode(root: &Path, path: &str, opened_mode: u32) -> Result<()> {
    need(
        owns(path),
        "opened mode path is outside the finite source set",
    )?;
    #[cfg(unix)]
    {
        let _ = root;
        need(opened_mode == 0o644, "opened source mode0644")?;
    }
    #[cfg(windows)]
    {
        need(
            opened_mode == 0,
            "Windows handle mode has no Unix permission bits",
        )?;
        let index = git(root, &["ls-files", "--stage", "--", path])?;
        let index = std::str::from_utf8(&index).map_err(|error| error.to_string())?;
        need(
            index.lines().count() == 1
                && index.starts_with("100644 ")
                && index.ends_with(&format!(" 0\t{path}")),
            "Windows source mode from stage-zero index",
        )?;
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = (root, opened_mode);
        need(false, "source mode proof requires Unix or Windows")?;
    }
    Ok(())
}
/// Every owned source must be physical current before any caller hash shortcut.
#[allow(
    dead_code,
    reason = "W07 uses this shared leaf; standalone source tests also compile it"
)]
pub(crate) fn current_operand(root: &Path, path: &str) -> Result<Option<Vec<u8>>> {
    current_at(root, root, path)
}
/// Pure exact-byte outer projection for the updated bounded source reader.
pub(crate) fn project_source_reader(path: &str, current: &[u8]) -> Result<Vec<u8>> {
    project_source_reader_capsule(path, current, SOURCE_READER_OUTER)
}
fn project_source_reader_capsule(path: &str, current: &[u8], capsule: &[u8]) -> Result<Vec<u8>> {
    need(path == SOURCE_READER_PATH, "outer source-reader path")?;
    need(
        digest(capsule) == SOURCE_READER_OUTER_SHA,
        "outer source-reader capsule digest",
    )?;
    let doc: Value = serde_json::from_slice(capsule).map_err(|error| error.to_string())?;
    need(
        keys(&doc, &["schema", "baseline", "row"])
            && doc["schema"] == "PYTHON_DEPENDENCY_AUDIT_SOURCE_READER_SUCCESSOR_V1"
            && doc["baseline"] == BASE,
        "outer source-reader capsule header",
    )?;
    let row = &doc["row"];
    need(
        keys(row, &["path", "current", "prior", "edits"])
            && row["path"] == SOURCE_READER_PATH
            && keys(&row["current"], &["bytes", "sha256", "mode"])
            && keys(&row["prior"], &["bytes", "sha256", "mode", "git_blob"])
            && row["current"]["mode"] == "0644"
            && row["prior"]["mode"] == "0644"
            && row["prior"]["bytes"] == SOURCE_READER_OLD_BYTES
            && row["prior"]["sha256"] == SOURCE_READER_OLD_SHA
            && row["prior"]["git_blob"] == SOURCE_READER_OLD_BLOB,
        "outer source-reader exact endpoint metadata",
    )?;
    let current_bytes = row["current"]["bytes"]
        .as_u64()
        .and_then(|value| usize::try_from(value).ok())
        .ok_or("outer source-reader current byte count")?;
    let current_sha = row["current"]["sha256"]
        .as_str()
        .ok_or("outer source-reader current digest")?;
    need(
        current.len() == current_bytes && digest(current) == current_sha,
        "outer source-reader physical bytes",
    )?;
    let edits = row["edits"]
        .as_array()
        .filter(|edits| !edits.is_empty())
        .ok_or("outer source-reader exact edit list")?;
    let mut prior = Vec::with_capacity(SOURCE_READER_OLD_BYTES);
    let mut current_cursor = 0usize;
    let mut prior_cursor = 0usize;
    for edit in edits {
        need(
            keys(edit, &["at", "remove_utf8", "insert_utf8"]),
            "outer source-reader edit shape",
        )?;
        let at = edit["at"]
            .as_u64()
            .and_then(|value| usize::try_from(value).ok())
            .ok_or("outer source-reader edit offset")?;
        let remove = edit["remove_utf8"]
            .as_str()
            .ok_or("outer source-reader removed span")?
            .as_bytes();
        let insert = edit["insert_utf8"]
            .as_str()
            .ok_or("outer source-reader inserted span")?
            .as_bytes();
        let unchanged = at
            .checked_sub(prior_cursor)
            .ok_or("outer source-reader edit order")?;
        let end = current_cursor
            .checked_add(unchanged)
            .ok_or("outer source-reader current cursor overflow")?;
        prior.extend_from_slice(
            current
                .get(current_cursor..end)
                .ok_or("outer source-reader unchanged current bytes")?,
        );
        let inserted_end = end
            .checked_add(insert.len())
            .ok_or("outer source-reader inserted span overflow")?;
        need(
            current.get(end..inserted_end) == Some(insert),
            "outer source-reader exact inserted bytes",
        )?;
        prior.extend_from_slice(remove);
        current_cursor = inserted_end;
        prior_cursor = at
            .checked_add(remove.len())
            .ok_or("outer source-reader prior cursor overflow")?;
    }
    prior.extend_from_slice(
        current
            .get(current_cursor..)
            .ok_or("outer source-reader current suffix")?,
    );
    need(
        prior.len() == SOURCE_READER_OLD_BYTES && digest(&prior) == SOURCE_READER_OLD_SHA,
        "outer source-reader exact historical current bytes",
    )?;
    let mut forward = Vec::with_capacity(current.len());
    let mut prior_cursor = 0usize;
    for edit in edits {
        let at = edit["at"]
            .as_u64()
            .and_then(|value| usize::try_from(value).ok())
            .ok_or("outer source-reader forward offset")?;
        let remove = edit["remove_utf8"]
            .as_str()
            .ok_or("outer source-reader forward removed span")?
            .as_bytes();
        let insert = edit["insert_utf8"]
            .as_str()
            .ok_or("outer source-reader forward inserted span")?
            .as_bytes();
        let end = at
            .checked_add(remove.len())
            .ok_or("outer source-reader forward span overflow")?;
        need(
            at >= prior_cursor && prior.get(at..end) == Some(remove),
            "outer source-reader exact prior bytes",
        )?;
        forward.extend_from_slice(&prior[prior_cursor..at]);
        forward.extend_from_slice(insert);
        prior_cursor = end;
    }
    forward.extend_from_slice(&prior[prior_cursor..]);
    need(
        forward == current,
        "outer source-reader full forward reconstruction",
    )?;
    Ok(prior)
}
fn admit_source_reader(
    root: &Path,
    path: &str,
    current: &[u8],
    opened_mode: u32,
) -> Result<Vec<u8>> {
    #[cfg(unix)]
    need(opened_mode == 0o644, "outer source-reader opened Unix mode")?;
    #[cfg(windows)]
    {
        need(opened_mode == 0, "outer source-reader Windows handle mode")?;
        let staged = git(root, &["ls-files", "--stage", "--", path])?;
        let staged = std::str::from_utf8(&staged).map_err(|error| error.to_string())?;
        need(
            staged.lines().count() == 1
                && staged.starts_with("100644 ")
                && staged.ends_with(&format!(" 0\t{path}")),
            "outer source-reader Windows index mode",
        )?;
    }
    #[cfg(not(any(unix, windows)))]
    need(
        false,
        "outer source-reader mode proof requires Unix or Windows",
    )?;
    let prior = project_source_reader(path, current)?;
    let historical_doc = decode(CAPSULE)?;
    let historical = pin(&historical_doc, path)?;
    need(
        historical.current_bytes == SOURCE_READER_OLD_BYTES
            && historical.current_sha == SOURCE_READER_OLD_SHA
            && historical.current_blob == SOURCE_READER_OLD_BLOB
            && git(root, &["cat-file", "blob", SOURCE_READER_OLD_BLOB])? == prior,
        "outer source-reader historical accepted capsule bytes/blob",
    )?;
    Ok(prior)
}
fn physical_operand(
    physical_root: &Path,
    proof_root: &Path,
    path: &str,
    expected_bytes: usize,
) -> Result<(Vec<u8>, Vec<u8>)> {
    if path == SOURCE_READER_PATH {
        let (physical, mode) = bounded_source_reader::bounded_read(physical_root, path, LIMIT)?;
        let logical = admit_source_reader(proof_root, path, &physical, mode)?;
        Ok((physical, logical))
    } else {
        let physical_bytes = o02_image_gate_successor::physical_bytes(path, expected_bytes);
        let physical = regular(physical_root, path, physical_bytes)?;
        let logical = o02_image_gate_successor::project(path, expected_bytes, physical.clone())?;
        Ok((physical, logical))
    }
}
pub(crate) fn current_at(
    physical_root: &Path,
    proof_root: &Path,
    path: &str,
) -> Result<Option<Vec<u8>>> {
    if !owns(path) {
        return Ok(None);
    }
    let capsule = regular(physical_root, CAPSULE_PATH, CAPSULE.len())?;
    let doc = decode(&capsule)?;
    let selected = pin(&doc, path)?;
    let (physical, logical) =
        physical_operand(physical_root, proof_root, path, selected.current_bytes)?;
    let prior = inverse(&selected, row(&doc, path)?, &logical, 0o644)?;
    git_prior(proof_root, &selected, &prior)?;
    Ok(Some(physical))
}
/// Only the exact prior path/hash tuple may use this physical source proof.
#[allow(
    dead_code,
    reason = "xtask W07 uses this shared leaf; the standalone MCP source test also compiles it"
)]
pub(crate) fn frozen_operand(
    root: &Path,
    path: &str,
    expected: &str,
    supplied: &[u8],
) -> Result<Option<Vec<u8>>> {
    if !owns(path) {
        return Ok(None);
    }
    let doc = decode(CAPSULE)?;
    let selected = pin(&doc, path)?;
    if selected.prior_sha != expected {
        return Ok(None);
    }
    prior(root, path, supplied).map(Some)
}
pub(crate) fn current_git_blob(path: &str) -> Result<String> {
    let doc = decode(CAPSULE)?;
    Ok(pin(&doc, path)?.current_blob.to_owned())
}
fn pin<'a>(doc: &'a Value, path: &str) -> Result<Pin<'a>> {
    pins(doc)?
        .into_iter()
        .find(|pin| pin.path == path)
        .ok_or_else(|| "Python dependency audit unlisted source path".into())
}
fn safe_path(path: &str) -> bool {
    !path.is_empty()
        && !path.contains("//")
        && !path.contains('\\')
        && Path::new(path)
            .components()
            .all(|part| matches!(part, Component::Normal(_)))
}
fn hex(text: &str, len: usize) -> bool {
    text.len() == len
        && text
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}
fn material(row: &Value) -> Result<(&str, usize)> {
    need(
        keys(row, &["bytes", "sha256", "mode"]) && row["mode"] == "0644",
        "closed material shape/mode",
    )?;
    let sha = row["sha256"]
        .as_str()
        .ok_or("Python dependency audit missing material hash")?;
    let bytes = usize_field(row, "bytes")?;
    need(
        hex(sha, 64) && bytes > 0 && bytes as u64 <= LIMIT,
        "bounded material bytes/lowercase SHA256",
    )?;
    Ok((sha, bytes))
}
fn pins(doc: &Value) -> Result<Vec<Pin<'_>>> {
    let rows = doc["rows"]
        .as_array()
        .ok_or("Python dependency audit source rows missing")?;
    need(!rows.is_empty(), "nonempty closed source rows")?;
    let mut result = Vec::with_capacity(rows.len());
    let mut previous: Option<&str> = None;
    for row in rows {
        need(
            keys(
                row,
                &[
                    "path",
                    "current",
                    "current_git_blob",
                    "prior",
                    "prior_git_blob",
                    "prior_path",
                    "edits",
                ],
            ),
            "closed source row shape",
        )?;
        let path = row["path"]
            .as_str()
            .ok_or("Python dependency audit source path missing")?;
        need(
            safe_path(path) && previous.is_none_or(|old| path > old),
            "safe unique lexically ordered source paths",
        )?;
        let (current_sha, current_bytes) = material(&row["current"])?;
        let (prior_sha, prior_bytes) = material(&row["prior"])?;
        let blob = row["prior_git_blob"]
            .as_str()
            .ok_or("Python dependency audit prior blob missing")?;
        let current_blob = row["current_git_blob"]
            .as_str()
            .ok_or("Python dependency audit current blob missing")?;
        let prior_path = row["prior_path"]
            .as_str()
            .ok_or("audit predecessor path missing")?;
        let expected_parent = match path {
            "docs/rust-rewrite/o02/python-dependency-audit-gates-reader-v1.rs" => {
                "docs/rust-rewrite/o02/upstream-contracts-source-successor-v1.rs"
            }
            "docs/rust-rewrite/t06/source-amendments/python-dependency-audit-source-reader-v1.rs" => {
                "docs/rust-rewrite/t06/source-amendments/dnspython-security-current-main-v1.rs"
            }
            _ => path,
        };
        need(
            prior_path == expected_parent && safe_path(prior_path),
            "exact predecessor source path",
        )?;
        let edits = row["edits"]
            .as_array()
            .ok_or("Python dependency audit source edits missing")?;
        need(
            hex(blob, 40) && hex(current_blob, 40) && current_sha != prior_sha && !edits.is_empty(),
            "nonvacuous exact source transition",
        )?;
        for edit in edits {
            need(
                keys(edit, &["at", "remove_utf8", "insert_utf8"])
                    && edit["at"]
                        .as_u64()
                        .and_then(|n| usize::try_from(n).ok())
                        .is_some()
                    && edit["remove_utf8"].is_string()
                    && edit["insert_utf8"].is_string(),
                "closed typed source edit",
            )?;
        }
        result.push(Pin {
            path,
            current_sha,
            current_bytes,
            prior_path,
            prior_sha,
            prior_bytes,
            blob,
            current_blob,
            edits: edits.len(),
        });
        previous = Some(path);
    }
    Ok(result)
}
fn keys(value: &Value, expected: &[&str]) -> bool {
    value.as_object().is_some_and(|object| {
        object.len() == expected.len() && expected.iter().all(|key| object.contains_key(*key))
    })
}
fn decode(raw: &[u8]) -> Result<Value> {
    need(
        raw == CAPSULE && digest(raw) == CAPSULE_SHA,
        "compiled capsule bytes/hash",
    )?;
    let doc: Value = serde_json::from_slice(raw).map_err(|e| e.to_string())?;
    need(
        keys(&doc, &["schema", "baseline", "scope", "rows"])
            && doc["schema"] == "PYTHON_DEPENDENCY_AUDIT_SOURCE_SUCCESSOR_V1"
            && doc["baseline"] == BASE
            && doc["scope"]
                == "Issue #1129 runtime/dev dependency security floors and reproducible audit from accepted main 6cd400c2 only; historical qualification evidence remains unchanged and does not certify successor behavior.",
        "closed capsule header",
    )?;
    let selected = pins(&doc)?;
    need(
        selected.iter().map(|p| (p.path, p.edits)).eq(ROWS),
        "closed eight-path source and edit inventory",
    )?;
    Ok(doc)
}
fn row<'a>(doc: &'a Value, path: &str) -> Result<&'a Value> {
    doc["rows"]
        .as_array()
        .ok_or("Python dependency audit rows missing")?
        .iter()
        .find(|row| row["path"] == path)
        .ok_or_else(|| "Python dependency audit unlisted row".into())
}
fn usize_field(value: &Value, field: &str) -> Result<usize> {
    value[field]
        .as_u64()
        .and_then(|n| usize::try_from(n).ok())
        .ok_or_else(|| format!("Python dependency audit invalid {field}"))
}
// The exact physical afterimage is authenticated before any span is interpreted.
fn inverse(pin: &Pin<'_>, row: &Value, actual: &[u8], mode: u32) -> Result<Vec<u8>> {
    need(
        mode == 0o644 && actual.len() == pin.current_bytes && digest(actual) == pin.current_sha,
        "exact current bytes/hash/mode",
    )?;
    need(
        row["path"] == pin.path
            && row["current"]
                == json!({"bytes":pin.current_bytes,"sha256":pin.current_sha,"mode":"0644"})
            && row["prior"]
                == json!({"bytes":pin.prior_bytes,"sha256":pin.prior_sha,"mode":"0644"})
            && row["prior_git_blob"] == pin.blob
            && row["prior_path"] == pin.prior_path
            && row["current_git_blob"] == pin.current_blob,
        "exact inverse caller tuples",
    )?;
    let edits = row["edits"]
        .as_array()
        .ok_or("Python dependency audit missing edits")?;
    need(
        edits.len() == pin.edits && !edits.is_empty(),
        "exact nonempty edit count",
    )?;
    let mut prior = Vec::with_capacity(pin.prior_bytes);
    let mut cursor = 0usize;
    let mut previous = None;
    let mut forwards = Vec::with_capacity(edits.len());
    for edit in edits {
        need(
            keys(edit, &["at", "remove_utf8", "insert_utf8"]),
            "closed edit fields",
        )?;
        let at = usize_field(edit, "at")?;
        let remove = edit["remove_utf8"]
            .as_str()
            .ok_or("Python dependency audit missing current span")?
            .as_bytes();
        let insert = edit["insert_utf8"]
            .as_str()
            .ok_or("Python dependency audit missing prior span")?
            .as_bytes();
        let end = at
            .checked_add(remove.len())
            .ok_or("Python dependency audit current span overflow")?;
        need(
            at >= cursor
                && previous.is_none_or(|old| at > old)
                && actual.get(at..end) == Some(remove)
                && (!remove.is_empty() || !insert.is_empty()),
            "ordered nonoverlapping exact edit spans",
        )?;
        prior.extend_from_slice(&actual[cursor..at]);
        forwards.push((prior.len(), insert, remove));
        prior.extend_from_slice(insert);
        cursor = end;
        previous = Some(at);
    }
    prior.extend_from_slice(&actual[cursor..]);
    need(
        prior.len() == pin.prior_bytes && digest(&prior) == pin.prior_sha,
        "exact prior reconstruction",
    )?;
    let mut forward = Vec::with_capacity(pin.current_bytes);
    cursor = 0;
    for (at, remove, insert) in forwards {
        let end = at
            .checked_add(remove.len())
            .ok_or("Python dependency audit forward span overflow")?;
        need(
            at >= cursor && prior.get(at..end) == Some(remove),
            "ordered forward preimages",
        )?;
        forward.extend_from_slice(&prior[cursor..at]);
        forward.extend_from_slice(insert);
        cursor = end;
    }
    forward.extend_from_slice(&prior[cursor..]);
    need(forward == actual, "complete exact forward reconstruction")?;
    Ok(prior)
}
/// Pure exact-byte inverse/forward check. This does not claim filesystem or Git custody.
pub(crate) fn frame(path: &str, physical: &[u8], mode: u32) -> Result<Vec<u8>> {
    need(
        mode == 0o644,
        "pure dependency-audit frame requires canonical mode0644",
    )?;
    let doc = decode(CAPSULE)?;
    let pin = pin(&doc, path)?;
    if path == SOURCE_READER_PATH {
        let logical = project_source_reader(path, physical)?;
        inverse(&pin, row(&doc, path)?, &logical, 0o644)
    } else {
        let logical =
            o02_image_gate_successor::project(path, pin.current_bytes, physical.to_vec())?;
        inverse(&pin, row(&doc, path)?, &logical, mode)
    }
}
fn confined(root: &Path, path: &str) -> Result<std::path::PathBuf> {
    need(!Path::new(path).is_absolute(), "relative material path")?;
    let canonical_root = root.canonicalize().map_err(|e| e.to_string())?;
    let mut full = canonical_root.clone();
    let components = Path::new(path).components().collect::<Vec<_>>();
    need(!components.is_empty(), "nonempty material path")?;
    for (index, component) in components.iter().enumerate() {
        let Component::Normal(name) = component else {
            return Err("Python dependency audit unsafe material component".into());
        };
        full.push(name);
        let meta = fs::symlink_metadata(&full).map_err(|e| e.to_string())?;
        need(
            !meta.file_type().is_symlink()
                && if index + 1 == components.len() {
                    meta.is_file()
                } else {
                    meta.is_dir()
                },
            "regular material with no symlink ancestors",
        )?;
    }
    need(
        full.canonicalize()
            .map_err(|e| e.to_string())?
            .starts_with(&canonical_root),
        "material confined to root",
    )?;
    Ok(full)
}
fn git(root: &Path, args: &[&str]) -> Result<Vec<u8>> {
    let output = Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .map_err(|e| e.to_string())?;
    need(output.status.success(), "Git proof command succeeded")?;
    Ok(output.stdout)
}
fn same_source(before: &fs::Metadata, after: &fs::Metadata) -> bool {
    if !before.is_file() || !after.is_file() || before.len() != after.len() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        before.mode() & 0o7777 == 0o644
            && after.mode() & 0o7777 == 0o644
            && before.dev() == after.dev()
            && before.ino() == after.ino()
            && before.mtime() == after.mtime()
            && before.mtime_nsec() == after.mtime_nsec()
            && before.ctime() == after.ctime()
            && before.ctime_nsec() == after.ctime_nsec()
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        before.file_attributes() & 0x400 == 0
            && after.file_attributes() & 0x400 == 0
            && before.file_attributes() == after.file_attributes()
            && before.creation_time() == after.creation_time()
            && before.last_write_time() == after.last_write_time()
    }
    #[cfg(not(any(unix, windows)))]
    false
}
fn regular(root: &Path, path: &str, expected_bytes: usize) -> Result<Vec<u8>> {
    regular_observed(root, path, expected_bytes, || Ok(()))
}
fn regular_observed(
    root: &Path,
    path: &str,
    expected_bytes: usize,
    after_read: impl FnOnce() -> Result<()>,
) -> Result<Vec<u8>> {
    let full = confined(root, path)?;
    let before = fs::symlink_metadata(&full).map_err(|e| e.to_string())?;
    need(
        before.is_file()
            && !before.file_type().is_symlink()
            && before.len() == expected_bytes as u64
            && before.len() <= 1_048_576,
        "regular exact-size bounded physical source",
    )?;
    let mut options = fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
        need(before.mode() & 0o7777 == 0o644, "physical source mode0644")?;
        #[cfg(any(target_os = "linux", target_os = "android"))]
        options.custom_flags(0o400000 | 0o4000); // O_NOFOLLOW | O_NONBLOCK
        #[cfg(any(
            target_os = "macos",
            target_os = "ios",
            target_os = "freebsd",
            target_os = "dragonfly",
            target_os = "openbsd",
            target_os = "netbsd"
        ))]
        options.custom_flags(0x100 | 0x4); // O_NOFOLLOW | O_NONBLOCK
        #[cfg(not(any(
            target_os = "linux",
            target_os = "android",
            target_os = "macos",
            target_os = "ios",
            target_os = "freebsd",
            target_os = "dragonfly",
            target_os = "openbsd",
            target_os = "netbsd"
        )))]
        return Err(
            "Python dependency audit no-follow source open is not implemented for this Unix".into(),
        );
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        // Open the leaf itself and deny concurrent writers/deletion while held.
        options.custom_flags(0x00200000).share_mode(0x00000001); // OPEN_REPARSE_POINT, FILE_SHARE_READ
        let out = Command::new("git")
            .args(["ls-files", "--stage", "--", path])
            .current_dir(root)
            .output()
            .map_err(|e| e.to_string())?;
        let text = String::from_utf8_lossy(&out.stdout);
        need(
            out.status.success()
                && text.lines().count() == 1
                && text.starts_with("100644 ")
                && text.ends_with(&format!(" 0\t{path}\n")),
            "Windows exact current stage-zero Git mode/path",
        )?;
    }
    #[cfg(not(any(unix, windows)))]
    return Err("Python dependency audit source admission requires Unix or Windows".into());
    let file = options.open(&full).map_err(|e| e.to_string())?;
    let opened = file.metadata().map_err(|e| e.to_string())?;
    need(
        same_source(&before, &opened),
        "same opened regular source identity/mode",
    )?;
    let mut bytes = Vec::with_capacity(expected_bytes);
    (&file)
        .take(expected_bytes as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    after_read()?;
    let after = file.metadata().map_err(|e| e.to_string())?;
    let current_full = confined(root, path)?;
    let current = fs::symlink_metadata(current_full).map_err(|e| e.to_string())?;
    need(
        bytes.len() == expected_bytes
            && same_source(&opened, &after)
            && same_source(&after, &current)
            && !current.file_type().is_symlink(),
        "descriptor/path identity unchanged after size-plus-one read",
    )?;
    Ok(bytes)
}
fn git_prior(root: &Path, pin: &Pin<'_>, prior: &[u8]) -> Result<()> {
    need(
        git(root, &["show", &format!("{BASE}:{}", pin.prior_path)])? == prior,
        "exact base Git source bytes",
    )?;
    need(
        git(root, &["ls-tree", BASE, "--", pin.prior_path])?
            == format!("100644 blob {}\t{}\n", pin.blob, pin.prior_path).as_bytes(),
        "exact base Git tree/blob/mode",
    )
}
/// Full custody: authenticate current physical source and capsule before exposing its parent.
/// A sealed historical buffer supplied as current is rejected even if its hash is known.
pub(crate) fn predecessor(root: &Path, path: &str) -> Result<Vec<u8>> {
    prior_at(root, root, path, None)
}
pub(crate) fn prior(root: &Path, path: &str, supplied: &[u8]) -> Result<Vec<u8>> {
    prior_at(root, root, path, Some(supplied))
}
fn prior_at(
    physical_root: &Path,
    proof_root: &Path,
    path: &str,
    supplied: Option<&[u8]>,
) -> Result<Vec<u8>> {
    let capsule = regular(physical_root, CAPSULE_PATH, CAPSULE.len())?;
    let doc = decode(&capsule)?;
    let pin = pin(&doc, path)?;
    let (physical, logical) = physical_operand(physical_root, proof_root, path, pin.current_bytes)?;
    need(
        supplied.is_none_or(|bytes| physical == bytes),
        "supplied bytes are physical current",
    )?;
    let prior = inverse(&pin, row(&doc, path)?, &logical, 0o644)?;
    git_prior(proof_root, &pin, &prior)?;
    Ok(prior)
}
/// Additive controls for existing owning test identities. No historical test names change.
#[cfg(test)]
pub(crate) fn controls(root: &Path) -> Result<()> {
    let doc = decode(CAPSULE)?;
    let mut capsule_mutant = CAPSULE.to_vec();
    capsule_mutant.push(b' ');
    need(decode(&capsule_mutant).is_err(), "changed capsule rejected")?;
    need(
        prior(root, "unlisted.rs", b"").is_err(),
        "wrong physical caller rejected",
    )?;
    need(
        frozen_operand(root, "unlisted.rs", "wrong", b"")?.is_none(),
        "unlisted frozen operand has no authority",
    )?;
    need(
        current_operand(root, "unlisted.rs")?.is_none(),
        "unlisted current operand has no authority",
    )?;
    let all_pins = pins(&doc)?;
    for pin in &all_pins {
        let (physical, logical) = physical_operand(root, root, pin.path, pin.current_bytes)?;
        let prior_bytes = prior(root, pin.path, &physical)?;
        need(
            current_operand(root, pin.path)? == Some(physical.clone()),
            "early admission returns authenticated physical current",
        )?;
        need(
            frozen_operand(root, pin.path, "wrong", &physical)?.is_none(),
            "wrong prior caller has no authority",
        )?;
        need(
            frozen_operand(root, pin.path, pin.prior_sha, &physical)? == Some(prior_bytes.clone()),
            "exact frozen caller reaches same predecessor",
        )?;
        let source_row = row(&doc, pin.path)?;
        need(
            inverse(pin, source_row, &logical, 0o644)? == prior_bytes,
            "pure positive precedes controls",
        )?;
        need(
            frame(pin.path, &physical, 0o644)? == prior_bytes,
            "same pure frame as full custody",
        )?;
        need(
            frame(pin.path, &physical, 0o600).is_err(),
            "outer source-reader pure frame rejects wrong caller mode",
        )?;
        if pin.path == SOURCE_READER_PATH {
            need(
                admit_source_reader(root, pin.path, &physical, 0o600).is_err(),
                "outer source-reader changed opened mode rejected",
            )?;
            let mut altered = physical.clone();
            altered[0] ^= 1;
            need(
                project_source_reader(pin.path, &altered).is_err()
                    && admit_source_reader(root, pin.path, &altered, 0o644).is_err(),
                "outer source-reader changed current bytes rejected",
            )?;
            need(
                project_source_reader("unlisted-source-reader.rs", &physical).is_err(),
                "outer source-reader wrong path rejected",
            )?;
            let mut altered_capsule = SOURCE_READER_OUTER.to_vec();
            altered_capsule.push(b' ');
            need(
                project_source_reader_capsule(pin.path, &physical, &altered_capsule).is_err(),
                "outer source-reader changed capsule rejected",
            )?;
            #[cfg(windows)]
            need(
                frame(pin.path, &physical, 0).is_err(),
                "pure source-reader frame rejects physical Windows mode-zero metadata",
            )?;
        }
        need(
            prior(root, pin.path, &prior_bytes).is_err()
                && frame(pin.path, &prior_bytes, 0o644).is_err(),
            "stale parent is not current",
        )?;
        for mode in [0o600, 0o755, 0o4644] {
            need(
                inverse(pin, source_row, &logical, mode).is_err(),
                "changed current mode rejected",
            )?;
        }
        let mut changed = physical.clone();
        changed[0] ^= 1;
        need(
            frame(pin.path, &changed, 0o644).is_err() && prior(root, pin.path, &changed).is_err(),
            "changed supplied source rejected",
        )?;
        for mutation in 0..6 {
            let mut bad = source_row.clone();
            let edits = bad["edits"]
                .as_array_mut()
                .ok_or("Python dependency audit control edits")?;
            match mutation {
                0 => {
                    edits.pop();
                }
                1 => {
                    edits.push(edits[0].clone());
                }
                2 => {
                    edits[0]["at"] = json!(usize::MAX);
                }
                3 => {
                    edits[0]["remove_utf8"] = json!("changed exact preimage");
                }
                4 => {
                    edits[0]["insert_utf8"] = json!("changed exact prior");
                }
                _ => {
                    edits[0]["extra"] = json!(true);
                }
            }
            need(
                inverse(pin, &bad, &logical, 0o644).is_err(),
                "missing duplicate moved malformed or changed span rejected",
            )?;
        }
        if pin.edits > 1 {
            let mut bad = source_row.clone();
            bad["edits"]
                .as_array_mut()
                .ok_or("Python dependency audit control edits")?
                .swap(0, 1);
            need(
                inverse(pin, &bad, &logical, 0o644).is_err(),
                "reordered edits rejected",
            )?;
            let mut bad = source_row.clone();
            bad["edits"][1] = bad["edits"][0].clone();
            need(
                inverse(pin, &bad, &logical, 0o644).is_err(),
                "same-count duplicate span rejected",
            )?;
        }
    }
    for (path, before, after) in [
        ("pyproject.toml", "fastmcp>=4.1.2,<5", "fastmcp>=4.0.11,<5"),
        (
            "uv.lock",
            "name = \"fastmcp\"\nversion = \"4.1.2\"",
            "name = \"fastmcp\"\nversion = \"4.0.11\"",
        ),
        (
            ".github/workflows/ci.yml",
            "        run: pip-audit -r requirements-dev.txt\n",
            "",
        ),
        (
            ".github/workflows/nightly.yml",
            "        run: pip-audit -r requirements-dev.txt\n",
            "",
        ),
    ] {
        let selected = pin(&doc, path)?;
        let physical = regular(root, path, selected.current_bytes)?;
        let text = std::str::from_utf8(&physical).map_err(|e| e.to_string())?;
        need(
            text.matches(before).count() == 1,
            "security mutation target exists once",
        )?;
        let changed = text.replacen(before, after, 1);
        need(
            frame(path, changed.as_bytes(), 0o644).is_err(),
            "security floor or requirements audit removal rejected",
        )?;
    }
    physical_controls(root)?;
    Ok(())
}
#[cfg(test)]
fn physical_controls(proof_root: &Path) -> Result<()> {
    #[cfg(not(unix))]
    let _ = proof_root;
    #[cfg(unix)]
    {
        use std::os::unix::fs::{PermissionsExt, symlink};
        struct Owned(std::path::PathBuf);
        impl Drop for Owned {
            fn drop(&mut self) {
                let _ = fs::remove_dir_all(&self.0);
            }
        }
        let suffix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_nanos();
        let owned = Owned(
            std::env::temp_dir().join(format!("ssx-pyaudit-{}-{suffix}", std::process::id())),
        );
        fs::create_dir_all(
            owned
                .0
                .join(Path::new(CAPSULE_PATH).parent().ok_or("capsule parent")?),
        )
        .map_err(|e| e.to_string())?;
        fs::write(owned.0.join(CAPSULE_PATH), CAPSULE).map_err(|e| e.to_string())?;
        fs::set_permissions(
            owned.0.join(CAPSULE_PATH),
            fs::Permissions::from_mode(0o644),
        )
        .map_err(|e| e.to_string())?;
        let path = "pyproject.toml";
        let doc = decode(CAPSULE)?;
        let selected = pin(&doc, path)?;
        let physical = regular(proof_root, path, selected.current_bytes)?;
        let full = owned.0.join(path);
        fs::write(&full, &physical).map_err(|e| e.to_string())?;
        fs::set_permissions(&full, fs::Permissions::from_mode(0o644)).map_err(|e| e.to_string())?;
        prior_at(&owned.0, proof_root, path, Some(&physical))?;
        // Establish an actual same-length in-place mutation after the descriptor
        // has yielded valid bytes. Successful mutation must prevent admission.
        let mut same_length_changed = physical.clone();
        same_length_changed[0] ^= 1;
        let mutation_established = std::cell::Cell::new(false);
        let observed = regular_observed(&owned.0, path, selected.current_bytes, || {
            let writer = fs::OpenOptions::new()
                .write(true)
                .open(&full)
                .map_err(|e| e.to_string())?;
            use std::io::Write;
            (&writer)
                .write_all(&same_length_changed)
                .map_err(|e| e.to_string())?;
            writer.sync_all().map_err(|e| e.to_string())?;
            need(
                fs::read(&full).map_err(|e| e.to_string())? == same_length_changed,
                "same-length post-read mutation established",
            )?;
            mutation_established.set(true);
            Ok(())
        });
        need(
            mutation_established.get() && observed.is_err(),
            "established same-length in-place post-read mutation rejected",
        )?;
        fs::write(&full, &physical).map_err(|e| e.to_string())?;
        prior_at(&owned.0, proof_root, path, Some(&physical))?;
        need(
            current_at(&owned.0, proof_root, path)? == Some(physical.clone()),
            "owned early-read positive",
        )?;
        let stale = prior_at(&owned.0, proof_root, path, Some(&physical))?;
        fs::write(&full, &stale).map_err(|e| e.to_string())?;
        need(
            current_at(&owned.0, proof_root, path).is_err(),
            "physical exact predecessor rejected before caller equality",
        )?;
        fs::write(&full, &physical).map_err(|e| e.to_string())?;
        fs::set_permissions(&full, fs::Permissions::from_mode(0o600)).map_err(|e| e.to_string())?;
        need(
            prior_at(&owned.0, proof_root, path, Some(&physical)).is_err(),
            "physical changed mode rejected",
        )?;
        fs::set_permissions(&full, fs::Permissions::from_mode(0o644)).map_err(|e| e.to_string())?;
        let mut changed = physical.clone();
        changed[0] ^= 1;
        fs::write(&full, &changed).map_err(|e| e.to_string())?;
        need(
            prior_at(&owned.0, proof_root, path, Some(&changed)).is_err(),
            "actual same-size source mutation rejected",
        )?;
        fs::remove_file(&full).map_err(|e| e.to_string())?;
        need(
            prior_at(&owned.0, proof_root, path, Some(&physical)).is_err(),
            "missing physical source rejected",
        )?;
        fs::create_dir(&full).map_err(|e| e.to_string())?;
        need(
            prior_at(&owned.0, proof_root, path, Some(&physical)).is_err(),
            "physical directory rejected",
        )?;
        fs::remove_dir(&full).map_err(|e| e.to_string())?;
        symlink(proof_root.join(path), &full).map_err(|e| e.to_string())?;
        need(
            prior_at(&owned.0, proof_root, path, Some(&physical)).is_err(),
            "physical symlink leaf rejected",
        )?;
        fs::remove_file(&full).map_err(|e| e.to_string())?;
        fs::write(&full, &physical).map_err(|e| e.to_string())?;
        fs::set_permissions(&full, fs::Permissions::from_mode(0o644)).map_err(|e| e.to_string())?;
        prior_at(&owned.0, proof_root, path, Some(&physical))?;
        let mut wrong_capsule = CAPSULE.to_vec();
        wrong_capsule.push(b' ');
        fs::write(owned.0.join(CAPSULE_PATH), wrong_capsule).map_err(|e| e.to_string())?;
        need(
            prior_at(&owned.0, proof_root, path, Some(&physical)).is_err(),
            "changed actual capsule rejected",
        )?;
        fs::write(owned.0.join(CAPSULE_PATH), CAPSULE).map_err(|e| e.to_string())?;
        prior_at(&owned.0, proof_root, path, Some(&physical))?;
        let mut appended = physical.clone();
        appended.push(b' ');
        fs::write(&full, &appended).map_err(|e| e.to_string())?;
        need(
            prior_at(&owned.0, proof_root, path, Some(&appended)).is_err(),
            "actual extra source bytes rejected before admission",
        )?;
        need(
            current_at(&owned.0, proof_root, path).is_err(),
            "early current read rejects extra source bytes",
        )?;
        let ci_path = ".github/workflows/ci.yml";
        let selected = pin(&doc, ci_path)?;
        let ci = regular(proof_root, ci_path, selected.current_bytes)?;
        fs::create_dir_all(owned.0.join("shadow/workflows")).map_err(|e| e.to_string())?;
        fs::write(owned.0.join("shadow/workflows/ci.yml"), &ci).map_err(|e| e.to_string())?;
        symlink(owned.0.join("shadow"), owned.0.join(".github")).map_err(|e| e.to_string())?;
        need(
            prior_at(&owned.0, proof_root, ci_path, Some(&ci)).is_err(),
            "physical symlink ancestor rejected",
        )?;
        need(
            current_at(&owned.0, proof_root, ci_path).is_err(),
            "early current read rejects symlink ancestor",
        )?;
    }
    Ok(())
}
/// Seed the exact compiled outer capsule in an exclusively owned physical-control fixture.
#[cfg(test)]
pub(crate) fn seed_control_capsule(root: &Path) -> Result<()> {
    let path = root.join(CAPSULE_PATH);
    fs::create_dir_all(path.parent().ok_or("audit capsule parent")?).map_err(|e| e.to_string())?;
    fs::write(&path, CAPSULE).map_err(|e| e.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o644)).map_err(|e| e.to_string())?;
    }
    Ok(())
}
