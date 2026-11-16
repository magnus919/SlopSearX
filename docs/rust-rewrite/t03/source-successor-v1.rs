//! Closed T03 source-only successor. Historical capsules and qualification stay fixed.
//! `prior` owns physical/capsule/Git admission; `frame` is a pure byte proof only.
#[path = "../t06/source-amendments/dnspython-security-current-main-v1.rs"]
#[allow(
    dead_code,
    clippy::duplicate_mod,
    reason = "Independent qualification roots intentionally compile this stateless T03 proof leaf"
)]
mod dnspython_security_outer;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::Read,
    path::{Component, Path},
    process::Command,
};
type Result<T> = std::result::Result<T, String>;
pub(crate) const CAPSULE_PATH: &str = "docs/rust-rewrite/t03/source-successor-v1.json";
const CAPSULE: &[u8] = include_bytes!("source-successor-v1.json");
const CAPSULE_SHA: &str = "a82ce22c82e2c465d565232e281b981d0fe5739f9951cf057c980de8dd8bbd8b";
const BASE: &str = "d7fc628d1069ab1f0a5fe8ee114796c235615eca";
const W07_CAPSULE_PATH: &str = "docs/rust-rewrite/t03/w07-lock-custody-successor-v2.json";
const W07_CAPSULE: &[u8] = include_bytes!("w07-lock-custody-successor-v2.json");
const W07_CAPSULE_SHA: &str = "5b8575ef5b9c6148c80aae54dfd04c9e3503a42f5243633e00f762d193d536c5";
const W07_BASE: &str = "5350376a510a7752f90731739e6267db395cd21d";
const W07_SOURCE: &str = "crates/xtask/src/w07.rs";
const W07_ACCEPTED_SHA: &str = "40fea132ea484e6e72947956b6404143916a87edea1725e1aba0a8815900f45b";
const W07_ACCEPTED_BYTES: usize = 42_272;
const W07_ACCEPTED_BLOB: &str = "3beccfb3ac143d0baf00a6af7e34df85426a0f76";
const W07_SCOPE: &str = "W07 physical-source reconciliation plus one finite std-only Windows reader addition; existing T03 and W08 historical evidence remains unchanged.";
const W07_ADDED_SOURCE: &str = "crates/xtask/src/w07_windows_reader.rs";
const W07_ADDED_SOURCE_BYTES: usize = 2_732;
const W07_ADDED_SOURCE_SHA: &str =
    "cd5b1ee98e68605d5106d2294ebdfc3027cf5dbe73eb3bf42a1c70a8c88ce5f6";
const W07_MAIN_CAPSULE_PATH: &str = "docs/rust-rewrite/t03/w07-current-source-successor-v1.json";
const W07_MAIN_CAPSULE: &[u8] = include_bytes!("w07-current-source-successor-v1.json");
const W07_MAIN_CAPSULE_SHA: &str =
    "9bbf67108fd13674b9be68fd8b152f4c4d10cc27c16511e0af5d27a201f0e9ce";
const W07_MAIN_BASE: &str = "5350376a510a7752f90731739e6267db395cd21d";
const W07_MAIN_PARENT: &str = "287d944d57721d39dd5eb6a2bdec0fbcb5624760";
const W07_MAIN_SCOPE: &str = "W07 current-source reconciliation only: the exact merged #1094 Science scope reason and #1082 native Rust healthcheck reader; no unrelated source changes or historical evidence mutation.";
const W07_MAIN_SOURCES: [&str; 2] = [
    "crates/ssx-mcp/src/search_surface/mod.rs",
    "docs/rust-rewrite/a06/source-amendments/native-search-reader.rs",
];
const LIMIT: u64 = 1_048_576;
struct Pin<'a> {
    path: &'a str,
    current_sha: &'a str,
    current_bytes: usize,
    current_blob: Option<&'a str>,
    prior_sha: &'a str,
    prior_bytes: usize,
    blob: &'a str,
    edits: usize,
}
fn need(ok: bool, reason: &str) -> Result<()> {
    if ok {
        Ok(())
    } else {
        Err(format!("T03 source successor: {reason}"))
    }
}
fn digest(raw: &[u8]) -> String {
    Sha256::digest(raw)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
pub(crate) fn owns(path: &str) -> bool {
    decode(CAPSULE)
        .map(|doc| {
            doc["rows"]
                .as_array()
                .is_some_and(|rows| rows.iter().any(|row| row["path"] == path))
        })
        .unwrap_or(false)
}
fn pin<'a>(doc: &'a Value, path: &str) -> Result<Pin<'a>> {
    pins(doc)?
        .into_iter()
        .find(|pin| pin.path == path)
        .ok_or_else(|| "T03 unlisted source path".into())
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
    let sha = row["sha256"].as_str().ok_or("T03 missing material hash")?;
    let bytes = usize_field(row, "bytes")?;
    need(
        hex(sha, 64) && bytes > 0 && bytes as u64 <= LIMIT,
        "bounded material bytes/lowercase SHA256",
    )?;
    Ok((sha, bytes))
}
fn pins(doc: &Value) -> Result<Vec<Pin<'_>>> {
    let rows = doc["rows"].as_array().ok_or("T03 source rows missing")?;
    need(!rows.is_empty(), "nonempty closed source rows")?;
    let mut result = Vec::with_capacity(rows.len());
    let mut previous: Option<&str> = None;
    for row in rows {
        let base_keys = ["path", "current", "prior", "prior_git_blob", "edits"];
        let current_blob_keys = [
            "path",
            "current",
            "current_git_blob",
            "prior",
            "prior_git_blob",
            "edits",
        ];
        need(
            keys(row, &base_keys) || keys(row, &current_blob_keys),
            "closed source row shape",
        )?;
        let path = row["path"].as_str().ok_or("T03 source path missing")?;
        need(
            safe_path(path) && previous.is_none_or(|old| path > old),
            "safe unique lexically ordered source paths",
        )?;
        let (current_sha, current_bytes) = material(&row["current"])?;
        let current_blob = match row.get("current_git_blob") {
            None => None,
            Some(value) => {
                let blob = value
                    .as_str()
                    .ok_or("T03 current Git blob must be a string")?;
                need(hex(blob, 40), "exact lowercase current Git blob")?;
                Some(blob)
            }
        };
        let (prior_sha, prior_bytes) = material(&row["prior"])?;
        let blob = row["prior_git_blob"]
            .as_str()
            .ok_or("T03 prior blob missing")?;
        let edits = row["edits"].as_array().ok_or("T03 source edits missing")?;
        need(
            hex(blob, 40) && current_sha != prior_sha && !edits.is_empty(),
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
            current_blob,
            prior_sha,
            prior_bytes,
            blob,
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
            && doc["schema"] == "T03_CLOSED_SOURCE_SUCCESSOR_V1"
            && doc["baseline"] == BASE
            && doc["scope"]
                == "Source identity reconciliation only; historical qualification evidence remains unchanged and does not certify successor behavior.",
        "closed capsule header",
    )?;
    pins(&doc)?;
    Ok(doc)
}
fn row<'a>(doc: &'a Value, path: &str) -> Result<&'a Value> {
    doc["rows"]
        .as_array()
        .ok_or("T03 rows missing")?
        .iter()
        .find(|row| row["path"] == path)
        .ok_or_else(|| "T03 unlisted row".into())
}
fn usize_field(value: &Value, field: &str) -> Result<usize> {
    value[field]
        .as_u64()
        .and_then(|n| usize::try_from(n).ok())
        .ok_or_else(|| format!("T03 invalid {field}"))
}
// The exact physical afterimage is authenticated before any span is interpreted.
fn inverse(pin: &Pin<'_>, row: &Value, actual: &[u8], mode: u32) -> Result<Vec<u8>> {
    need(
        mode == 0o644 && actual.len() == pin.current_bytes && digest(actual) == pin.current_sha,
        "exact current bytes/hash/mode",
    )?;
    let base_keys = ["path", "current", "prior", "prior_git_blob", "edits"];
    let current_blob_keys = [
        "path",
        "current",
        "current_git_blob",
        "prior",
        "prior_git_blob",
        "edits",
    ];
    need(
        (keys(row, &base_keys) || (pin.current_blob.is_some() && keys(row, &current_blob_keys)))
            && row["path"] == pin.path
            && row["current"]
                == json!({"bytes":pin.current_bytes,"sha256":pin.current_sha,"mode":"0644"})
            && row["prior"]
                == json!({"bytes":pin.prior_bytes,"sha256":pin.prior_sha,"mode":"0644"})
            && row["prior_git_blob"] == pin.blob
            && pin
                .current_blob
                .is_none_or(|blob| row["current_git_blob"] == blob),
        "exact inverse caller tuples",
    )?;
    let edits = row["edits"].as_array().ok_or("T03 missing edits")?;
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
            .ok_or("T03 missing current span")?
            .as_bytes();
        let insert = edit["insert_utf8"]
            .as_str()
            .ok_or("T03 missing prior span")?
            .as_bytes();
        let end = at
            .checked_add(remove.len())
            .ok_or("T03 current span overflow")?;
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
            .ok_or("T03 forward span overflow")?;
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
    let doc = decode(CAPSULE)?;
    let pin = pin(&doc, path)?;
    inverse(&pin, row(&doc, path)?, physical, mode)
}
fn confined(root: &Path, path: &str) -> Result<std::path::PathBuf> {
    need(!Path::new(path).is_absolute(), "relative material path")?;
    let canonical_root = root.canonicalize().map_err(|e| e.to_string())?;
    let mut full = canonical_root.clone();
    let components = Path::new(path).components().collect::<Vec<_>>();
    need(!components.is_empty(), "nonempty material path")?;
    for (index, component) in components.iter().enumerate() {
        let Component::Normal(name) = component else {
            return Err("T03 unsafe material component".into());
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
fn regular(root: &Path, path: &str, expected_bytes: usize) -> Result<Vec<u8>> {
    let full = confined(root, path)?;
    let before = fs::symlink_metadata(&full).map_err(|e| e.to_string())?;
    need(
        before.is_file()
            && !before.file_type().is_symlink()
            && before.len() == expected_bytes as u64
            && before.len() <= LIMIT,
        "bounded exact-size regular source",
    )?;
    let mut options = fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
        need(before.mode() & 0o7777 == 0o644, "physical Unix0644")?;
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
        return Err("T03 no-follow source open is not implemented for this Unix".into());
    }
    #[cfg(windows)]
    {
        let index = git(root, &["ls-files", "--stage", "--", path])?;
        let index = std::str::from_utf8(&index).map_err(|e| e.to_string())?;
        let lines = index.lines().collect::<Vec<_>>();
        need(
            lines.len() == 1
                && lines[0].starts_with("100644 ")
                && lines[0].ends_with(&format!(" 0\t{path}")),
            "Windows current stage-zero index100644",
        )?;
    }
    #[cfg(not(any(unix, windows)))]
    return Err("T03 source-mode proof requires Unix or Windows".into());
    let opened = options.open(&full).map_err(|e| e.to_string())?;
    let after = opened.metadata().map_err(|e| e.to_string())?;
    need(
        after.is_file() && after.len() == before.len(),
        "opened regular exact-size descriptor",
    )?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        need(
            after.mode() & 0o7777 == 0o644
                && before.dev() == after.dev()
                && before.ino() == after.ino(),
            "same opened Unix source identity/mode",
        )?;
    }
    let mut bytes = Vec::with_capacity(expected_bytes);
    opened
        .take(LIMIT + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    need(
        bytes.len() == expected_bytes && bytes.len() as u64 <= LIMIT,
        "finite opened exact byte count",
    )?;
    let current_full = confined(root, path)?;
    let current = fs::symlink_metadata(current_full).map_err(|e| e.to_string())?;
    need(
        current.is_file() && !current.file_type().is_symlink() && current.len() == after.len(),
        "physical source remains regular exact-size",
    )?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        need(
            current.dev() == after.dev()
                && current.ino() == after.ino()
                && current.mode() & 0o7777 == 0o644,
            "physical path retains opened Unix identity/mode",
        )?;
    }
    Ok(bytes)
}
fn git_prior_at(root: &Path, pin: &Pin<'_>, prior: &[u8], baseline: &str) -> Result<()> {
    need(
        git(root, &["show", &format!("{baseline}:{}", pin.path)])? == prior,
        "exact pinned predecessor Git source bytes",
    )?;
    need(
        git(root, &["ls-tree", baseline, "--", pin.path])?
            == format!("100644 blob {}\t{}\n", pin.blob, pin.path).as_bytes(),
        "exact pinned predecessor Git tree/blob/mode",
    )
}
fn git_prior(root: &Path, pin: &Pin<'_>, prior: &[u8]) -> Result<()> {
    git_prior_at(root, pin, prior, BASE)
}
/// Authenticate the current W07 source and finite additions before exposing
/// the exact frame accepted by the historical T03 capsule.
fn w07_added_source(doc: &Value) -> Result<(&str, usize, &str)> {
    let rows = doc["added_sources"]
        .as_array()
        .ok_or("T03 W07 added-source inventory missing")?;
    need(rows.len() == 1, "one exact W07 added-source row")?;
    let row = &rows[0];
    need(
        keys(row, &["path", "current", "prior"]) && row["path"] == W07_ADDED_SOURCE,
        "closed exact W07 added-source path",
    )?;
    let (sha, bytes) = material(&row["current"])?;
    need(
        bytes == W07_ADDED_SOURCE_BYTES && sha == W07_ADDED_SOURCE_SHA,
        "exact W07 added-source current identity",
    )?;
    need(
        keys(&row["prior"], &["status", "baseline"])
            && row["prior"]["status"] == "absent"
            && row["prior"]["baseline"] == W07_BASE,
        "W07 added source was absent from the exact baseline",
    )?;
    Ok((W07_ADDED_SOURCE, bytes, sha))
}
fn validate_w07_document(doc: &Value) -> Result<()> {
    need(
        keys(
            doc,
            &["schema", "baseline", "scope", "rows", "added_sources"],
        ) && doc["schema"] == "T03_W07_LOCK_CUSTODY_SUCCESSOR_V2"
            && doc["baseline"] == W07_BASE
            && doc["scope"] == W07_SCOPE,
        "W07 source successor closed v2 header",
    )?;
    let pins = pins(doc)?;
    need(
        pins.len() == 1
            && pins[0].path == W07_SOURCE
            && pins[0].prior_sha == W07_ACCEPTED_SHA
            && pins[0].prior_bytes == W07_ACCEPTED_BYTES
            && pins[0].blob == W07_ACCEPTED_BLOB,
        "W07 source successor exact accepted predecessor tuple",
    )?;
    w07_added_source(doc)?;
    Ok(())
}
fn decode_w07(raw: &[u8]) -> Result<Value> {
    need(
        raw == W07_CAPSULE && digest(raw) == W07_CAPSULE_SHA,
        "W07 source successor capsule bytes/hash",
    )?;
    let doc: Value = serde_json::from_slice(raw).map_err(|error| error.to_string())?;
    validate_w07_document(&doc)?;
    Ok(doc)
}
fn verify_w07_added_source_bytes(actual: &[u8]) -> Result<()> {
    need(
        actual.len() == W07_ADDED_SOURCE_BYTES && digest(actual) == W07_ADDED_SOURCE_SHA,
        "exact W07 added-source physical bytes/hash",
    )
}
fn w07_added_source_bytes(root: &Path, doc: &Value) -> Result<Vec<u8>> {
    let (path, bytes, _) = w07_added_source(doc)?;
    let physical = regular(root, path, bytes)?;
    verify_w07_added_source_bytes(&physical)?;
    need(
        git(root, &["ls-tree", W07_BASE, "--", path])?.is_empty(),
        "W07 added-source path was absent from baseline Git tree",
    )?;
    Ok(physical)
}
fn w07_source_frames(root: &Path, supplied: Option<&[u8]>) -> Result<(Vec<u8>, Vec<u8>, Vec<u8>)> {
    let capsule = regular(root, W07_CAPSULE_PATH, W07_CAPSULE.len())?;
    let doc = decode_w07(&capsule)?;
    let pins = pins(&doc)?;
    let pin = &pins[0];
    let row = row(&doc, W07_SOURCE)?;
    let physical = dnspython_security_outer::current_operand(root, W07_SOURCE)?
        .ok_or("T03 W07 outer physical operand missing")?;
    if let Some(supplied) = supplied {
        need(physical == supplied, "supplied bytes are physical current")?;
    }
    let accepted = dnspython_security_outer::prior(root, W07_SOURCE, &physical)?;
    let prior = inverse(pin, row, &accepted, 0o644)?;
    need(
        git(root, &["show", &format!("{W07_BASE}:{W07_SOURCE}")])? == prior,
        "W07 source successor exact parent Git bytes",
    )?;
    need(
        git(root, &["ls-tree", W07_BASE, "--", W07_SOURCE])?
            == format!("100644 blob {W07_ACCEPTED_BLOB}\t{W07_SOURCE}\n").as_bytes(),
        "W07 source successor exact parent Git mode/blob",
    )?;
    w07_added_source_bytes(root, &doc)?;
    Ok((physical, accepted, prior))
}
// Existing #1131 controls continue to exercise precisely their accepted bytes.
fn w07_current_frame(root: &Path) -> Result<(Vec<u8>, Vec<u8>)> {
    let (_, accepted, prior) = w07_source_frames(root, None)?;
    Ok((accepted, prior))
}
fn w07_physical_current_frame(root: &Path) -> Result<(Vec<u8>, Vec<u8>)> {
    let (physical, _, prior) = w07_source_frames(root, None)?;
    Ok((physical, prior))
}
fn w07_outer_controls(root: &Path) -> Result<()> {
    let (physical, accepted, logical) = w07_source_frames(root, None)?;
    let doc = decode(CAPSULE)?;
    let source = pin(&doc, W07_SOURCE)?;
    let predecessor = inverse(&source, row(&doc, W07_SOURCE)?, &logical, 0o644)?;
    need(
        physical != accepted
            && accepted != logical
            && dnspython_security_outer::prior(root, W07_SOURCE, &physical)? == accepted
            && prior(root, W07_SOURCE, &physical)? == predecessor,
        "W07 outer physical to c2d2b5c to #1131 to T03 composition positive",
    )?;
    need(
        dnspython_security_outer::prior(root, W07_SOURCE, &accepted).is_err()
            && prior(root, W07_SOURCE, &accepted).is_err()
            && prior(root, W07_SOURCE, &logical).is_err()
            && prior(root, W07_SOURCE, &predecessor).is_err(),
        "W07 reconstructed operands cannot substitute for outer physical source",
    )?;
    let mut changed = physical.clone();
    changed[0] ^= 1;
    need(
        dnspython_security_outer::prior(root, W07_SOURCE, &changed).is_err()
            && prior(root, W07_SOURCE, &changed).is_err(),
        "changed W07 outer supplied physical bytes rejected",
    )?;
    let w07_doc = decode_w07(W07_CAPSULE)?;
    let w07_pin = pin(&w07_doc, W07_SOURCE)?;
    need(
        inverse(&w07_pin, row(&w07_doc, W07_SOURCE)?, &physical, 0o644).is_err()
            && frame(W07_SOURCE, &accepted, 0o644).is_err()
            && frame(W07_SOURCE, &physical, 0o644).is_err(),
        "W07 outer and historical inverse layers cannot be skipped",
    )?;
    Ok(())
}
fn w07_controls(root: &Path) -> Result<()> {
    let mut changed_capsule = W07_CAPSULE.to_vec();
    changed_capsule.push(b' ');
    need(
        decode_w07(&changed_capsule).is_err(),
        "changed W07 source successor capsule rejected",
    )?;
    let capsule = regular(root, W07_CAPSULE_PATH, W07_CAPSULE.len())?;
    let doc = decode_w07(&capsule)?;
    need(
        validate_w07_document(&doc).is_ok(),
        "exact W07 source successor capsule positive",
    )?;
    let mut missing_additions = doc.clone();
    missing_additions
        .as_object_mut()
        .ok_or("W07 source successor document object")?
        .remove("added_sources");
    need(
        validate_w07_document(&missing_additions).is_err(),
        "missing W07 added-source inventory rejected",
    )?;
    let mut duplicate_addition = doc.clone();
    let additions = duplicate_addition["added_sources"]
        .as_array_mut()
        .ok_or("W07 added-source inventory")?;
    let addition = additions[0].clone();
    additions.push(addition);
    need(
        validate_w07_document(&duplicate_addition).is_err(),
        "duplicate W07 added-source entry rejected",
    )?;
    let mut out_of_scope_addition = doc.clone();
    out_of_scope_addition["added_sources"][0]["path"] = json!("crates/xtask/src/main.rs");
    need(
        validate_w07_document(&out_of_scope_addition).is_err(),
        "out-of-scope W07 added-source path rejected",
    )?;
    let mut altered_addition_hash = doc.clone();
    altered_addition_hash["added_sources"][0]["current"]["sha256"] = json!("0".repeat(64));
    need(
        validate_w07_document(&altered_addition_hash).is_err(),
        "altered W07 added-source hash rejected",
    )?;
    let mut altered_addition_mode = doc.clone();
    altered_addition_mode["added_sources"][0]["current"]["mode"] = json!("0755");
    need(
        validate_w07_document(&altered_addition_mode).is_err(),
        "altered W07 added-source mode rejected",
    )?;
    let mut altered_addition_prior = doc.clone();
    altered_addition_prior["added_sources"][0]["prior"]["baseline"] =
        json!("287d944d57721d39dd5eb6a2bdec0fbcb5624760");
    need(
        validate_w07_document(&altered_addition_prior).is_err(),
        "altered W07 added-source predecessor rejected",
    )?;
    let mut open_addition = doc.clone();
    open_addition["added_sources"][0]["unexpected"] = json!(true);
    need(
        validate_w07_document(&open_addition).is_err(),
        "open W07 added-source row rejected",
    )?;
    let (added_path, added_bytes, _) = w07_added_source(&doc)?;
    let added = w07_added_source_bytes(root, &doc)?;
    need(
        added.len() == added_bytes && added_path == W07_ADDED_SOURCE,
        "exact W07 added-source physical identity positive",
    )?;
    let mut changed_added = added.clone();
    changed_added[0] ^= 1;
    need(
        verify_w07_added_source_bytes(&changed_added).is_err(),
        "changed W07 added-source physical bytes rejected",
    )?;
    let w07_pin = pin(&doc, W07_SOURCE)?;
    let row = row(&doc, W07_SOURCE)?;
    let (physical, prior) = w07_current_frame(root)?;
    need(
        inverse(&w07_pin, row, &physical, 0o644)? == prior,
        "W07 source successor positive exact inverse",
    )?;
    need(
        inverse(&w07_pin, row, &prior, 0o644).is_err(),
        "historical W07 source is not current",
    )?;
    need(
        pin(&doc, "Cargo.lock").is_err(),
        "wrong W07 successor caller rejected",
    )?;
    for mode in [0o600, 0o755, 0o4644] {
        need(
            inverse(&w07_pin, row, &physical, mode).is_err(),
            "changed W07 successor mode rejected",
        )?;
    }
    let mut changed = physical.clone();
    changed[0] ^= 1;
    need(
        inverse(&w07_pin, row, &changed, 0o644).is_err(),
        "changed W07 successor current bytes rejected",
    )?;
    for mutation in 0..6 {
        let mut bad = row.clone();
        let edits = bad["edits"].as_array_mut().ok_or("W07 successor edits")?;
        match mutation {
            0 => {
                edits.pop();
            }
            1 => edits.push(edits[0].clone()),
            2 => edits[0]["at"] = json!(usize::MAX),
            3 => edits[0]["remove_utf8"] = json!("changed exact current span"),
            4 => edits[0]["insert_utf8"] = json!("changed exact predecessor span"),
            _ => edits[0]["extra"] = json!(true),
        }
        need(
            inverse(&w07_pin, &bad, &physical, 0o644).is_err(),
            "missing duplicate moved malformed or changed W07 span rejected",
        )?;
    }
    let mut reordered = row.clone();
    reordered["edits"]
        .as_array_mut()
        .ok_or("W07 successor edits")?
        .swap(0, 1);
    need(
        inverse(&w07_pin, &reordered, &physical, 0o644).is_err(),
        "reordered W07 spans rejected",
    )?;
    let mut duplicated = row.clone();
    duplicated["edits"][1] = duplicated["edits"][0].clone();
    need(
        inverse(&w07_pin, &duplicated, &physical, 0o644).is_err(),
        "same-count duplicate W07 spans rejected",
    )?;
    Ok(())
}
fn decode_w07_main(raw: &[u8]) -> Result<Value> {
    need(
        raw == W07_MAIN_CAPSULE && digest(raw) == W07_MAIN_CAPSULE_SHA,
        "W07 current-source capsule bytes/hash",
    )?;
    let doc: Value = serde_json::from_slice(raw).map_err(|error| error.to_string())?;
    need(
        keys(
            &doc,
            &["schema", "baseline", "prior_baseline", "scope", "rows"],
        ) && doc["schema"] == "T03_W07_CURRENT_SOURCE_SUCCESSOR_V1"
            && doc["baseline"] == W07_MAIN_BASE
            && doc["prior_baseline"] == W07_MAIN_PARENT
            && doc["scope"] == W07_MAIN_SCOPE,
        "W07 current-source capsule closed header",
    )?;
    let pins = pins(&doc)?;
    need(
        pins.len() == W07_MAIN_SOURCES.len()
            && pins.iter().map(|pin| pin.path).eq(W07_MAIN_SOURCES),
        "W07 current-source exact sorted path inventory",
    )?;
    for (
        path,
        current_sha,
        current_bytes,
        current_blob,
        prior_sha,
        prior_bytes,
        prior_blob,
        edit_count,
    ) in [
        (
            W07_MAIN_SOURCES[0],
            "8e6fa66c8affa33ffcca162e6afd3e6ae114c4f99c0b48d75aef3de363aedb3d",
            46_532,
            "c33ba2bd9552dd02b797f67021b0a89f59c63921",
            "1de423862041d7988ce020f801af73abd25db3f9dcdae83a3e362db58428496f",
            46_323,
            "d47bedb47e2098d8135d6b450b23d14febb74052",
            1,
        ),
        (
            W07_MAIN_SOURCES[1],
            "61fa2a5aa3037c383691a1a023a17f48c20c05dcda486a02cfd0090c8a25035a",
            39_955,
            "b6fcb6d4b0470517a064ca66690241ff7e8a441a",
            "bc678e7fc47ec3ec06d6d55e13e3eed87f963f1c0bee8ef47b84f28641d3257c",
            26_158,
            "b2f177992718c5670164d752028f9564c092d012",
            4,
        ),
    ] {
        let source = pins
            .iter()
            .find(|pin| pin.path == path)
            .ok_or("W07 source pin missing")?;
        need(
            source.current_sha == current_sha
                && source.current_bytes == current_bytes
                && source.current_blob == Some(current_blob)
                && source.prior_sha == prior_sha
                && source.prior_bytes == prior_bytes
                && source.blob == prior_blob
                && source.edits == edit_count,
            "W07 current-source exact byte/hash/blob/span tuple",
        )?;
    }
    Ok(doc)
}
fn w07_main_current_frame(root: &Path, path: &str) -> Result<(Vec<u8>, Vec<u8>)> {
    let capsule = regular(root, W07_MAIN_CAPSULE_PATH, W07_MAIN_CAPSULE.len())?;
    let doc = decode_w07_main(&capsule)?;
    let source = pin(&doc, path)?;
    let source_row = row(&doc, path)?;
    let physical = regular(root, path, source.current_bytes)?;
    let prior = inverse(&source, source_row, &physical, 0o644)?;
    let current_blob = source.current_blob.ok_or("W07 current Git blob missing")?;
    need(
        git(root, &["show", &format!("{W07_MAIN_BASE}:{path}")])? == physical,
        "W07 exact current baseline Git source bytes",
    )?;
    need(
        git(root, &["ls-tree", W07_MAIN_BASE, "--", path])?
            == format!("100644 blob {current_blob}\t{path}\n").as_bytes(),
        "W07 exact current baseline Git tree/blob/mode",
    )?;
    git_prior_at(root, &source, &prior, W07_MAIN_PARENT)?;
    let ancestry = Command::new("git")
        .args([
            "merge-base",
            "--is-ancestor",
            W07_MAIN_PARENT,
            W07_MAIN_BASE,
        ])
        .current_dir(root)
        .status()
        .map_err(|error| error.to_string())?;
    need(
        ancestry.success(),
        "W07 predecessor baseline is an ancestor of current baseline",
    )?;
    Ok((physical, prior))
}
fn w07_main_controls(root: &Path) -> Result<()> {
    let mut changed_capsule = W07_MAIN_CAPSULE.to_vec();
    changed_capsule.push(b' ');
    need(
        decode_w07_main(&changed_capsule).is_err(),
        "changed W07 current-source capsule rejected",
    )?;
    let capsule = regular(root, W07_MAIN_CAPSULE_PATH, W07_MAIN_CAPSULE.len())?;
    let doc = decode_w07_main(&capsule)?;
    need(
        pin(&doc, "Cargo.lock").is_err(),
        "wrong W07 source path rejected",
    )?;
    for source in pins(&doc)? {
        let source_row = row(&doc, source.path)?;
        let (physical, prior_bytes) = w07_main_current_frame(root, source.path)?;
        need(
            inverse(&source, source_row, &physical, 0o644)? == prior_bytes
                && prior(root, source.path, &physical)? == prior_bytes,
            "W07 current-source positive exact inverse",
        )?;
        need(
            prior(root, source.path, &prior_bytes).is_err(),
            "W07 historical source bytes are not physical current",
        )?;
        for mode in [0o600, 0o755, 0o4644] {
            need(
                inverse(&source, source_row, &physical, mode).is_err(),
                "changed W07 current-source mode rejected",
            )?;
        }
        let mut changed = physical.clone();
        changed[0] ^= 1;
        need(
            inverse(&source, source_row, &changed, 0o644).is_err()
                && prior(root, source.path, &changed).is_err(),
            "changed W07 current-source bytes rejected",
        )?;
        for mutation in 0..6 {
            let mut bad = source_row.clone();
            let edits = bad["edits"]
                .as_array_mut()
                .ok_or("W07 source edit controls")?;
            match mutation {
                0 => {
                    edits.pop();
                }
                1 => edits.push(edits[0].clone()),
                2 => edits[0]["at"] = json!(usize::MAX),
                3 => edits[0]["remove_utf8"] = json!("changed exact current span"),
                4 => edits[0]["insert_utf8"] = json!("changed exact predecessor span"),
                _ => edits[0]["extra"] = json!(true),
            }
            need(
                inverse(&source, &bad, &physical, 0o644).is_err(),
                "missing, duplicate, moved, malformed, or changed W07 source span rejected",
            )?;
        }
        if source.edits > 1 {
            let mut reordered = source_row.clone();
            reordered["edits"]
                .as_array_mut()
                .ok_or("W07 source edit controls")?
                .swap(0, 1);
            need(
                inverse(&source, &reordered, &physical, 0o644).is_err(),
                "reordered W07 current-source spans rejected",
            )?;
            let mut duplicate = source_row.clone();
            duplicate["edits"][1] = duplicate["edits"][0].clone();
            need(
                inverse(&source, &duplicate, &physical, 0o644).is_err(),
                "duplicate W07 current-source spans rejected",
            )?;
        }
    }
    Ok(())
}
/// Full custody: authenticate current physical source and capsule before exposing its parent.
/// A sealed historical buffer supplied as current is rejected even if its hash is known.
pub(crate) fn prior(root: &Path, path: &str, supplied: &[u8]) -> Result<Vec<u8>> {
    if path == W07_SOURCE {
        let capsule = regular(root, CAPSULE_PATH, CAPSULE.len())?;
        let doc = decode(&capsule)?;
        let source = pin(&doc, path)?;
        let source_row = row(&doc, path)?;
        let (_, _, logical) = w07_source_frames(root, Some(supplied))?;
        let predecessor = inverse(&source, source_row, &logical, 0o644)?;
        git_prior(root, &source, &predecessor)?;
        return Ok(predecessor);
    }
    if W07_MAIN_SOURCES.contains(&path) {
        let (physical, predecessor) = w07_main_current_frame(root, path)?;
        need(physical == supplied, "supplied bytes are physical current")?;
        return Ok(predecessor);
    }
    let capsule = regular(root, CAPSULE_PATH, CAPSULE.len())?;
    let doc = decode(&capsule)?;
    let pin = pin(&doc, path)?;
    let physical = regular(root, path, pin.current_bytes)?;
    need(physical == supplied, "supplied bytes are physical current")?;
    let prior = inverse(&pin, row(&doc, path)?, &physical, 0o644)?;
    git_prior(root, &pin, &prior)?;
    Ok(prior)
}
/// Additive controls for existing owning test identities. No historical test names change.
pub(crate) fn controls(root: &Path) -> Result<()> {
    let doc = decode(CAPSULE)?;
    w07_controls(root)?;
    w07_main_controls(root)?;
    w07_outer_controls(root)?;
    let mut capsule_mutant = CAPSULE.to_vec();
    capsule_mutant.push(b' ');
    need(decode(&capsule_mutant).is_err(), "changed capsule rejected")?;
    need(
        prior(root, "unlisted.rs", b"").is_err(),
        "wrong physical caller rejected",
    )?;
    let all_pins = pins(&doc)?;
    for pin in &all_pins {
        let (physical, logical) = if pin.path == W07_SOURCE {
            w07_physical_current_frame(root)?
        } else {
            let physical = regular(root, pin.path, pin.current_bytes)?;
            (physical.clone(), physical)
        };
        let prior_bytes = prior(root, pin.path, &physical)?;
        let source_row = row(&doc, pin.path)?;
        need(
            inverse(pin, source_row, &logical, 0o644)? == prior_bytes,
            "pure positive precedes controls",
        )?;
        need(
            frame(pin.path, &logical, 0o644)? == prior_bytes,
            "same pure frame as full custody",
        )?;
        need(
            prior(root, pin.path, &prior_bytes).is_err()
                && frame(pin.path, &prior_bytes, 0o644).is_err(),
            "stale parent is not current",
        )?;
        if pin.path == W07_SOURCE {
            need(
                frame(pin.path, &physical, 0o644).is_err()
                    && prior(root, pin.path, &logical).is_err(),
                "physical successor is distinct from historical T03 current",
            )?;
        }
        for mode in [0o600, 0o755, 0o4644] {
            need(
                inverse(pin, source_row, &logical, mode).is_err(),
                "changed current mode rejected",
            )?;
        }
        let mut changed = logical.clone();
        changed[0] ^= 1;
        need(
            frame(pin.path, &changed, 0o644).is_err() && prior(root, pin.path, &changed).is_err(),
            "changed supplied source rejected",
        )?;
        for mutation in 0..6 {
            let mut bad = source_row.clone();
            let edits = bad["edits"].as_array_mut().ok_or("T03 control edits")?;
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
                .ok_or("T03 control edits")?
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
    Ok(())
}
