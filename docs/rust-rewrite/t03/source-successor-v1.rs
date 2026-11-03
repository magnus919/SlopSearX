//! Closed T03 source-only successor. Historical capsules and qualification stay fixed.
//! `prior` owns physical/capsule/Git admission; `frame` is a pure byte proof only.
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
const LIMIT: u64 = 1_048_576;
struct Pin<'a> {
    path: &'a str,
    current_sha: &'a str,
    current_bytes: usize,
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
        need(
            keys(
                row,
                &["path", "current", "prior", "prior_git_blob", "edits"],
            ),
            "closed source row shape",
        )?;
        let path = row["path"].as_str().ok_or("T03 source path missing")?;
        need(
            safe_path(path) && previous.is_none_or(|old| path > old),
            "safe unique lexically ordered source paths",
        )?;
        let (current_sha, current_bytes) = material(&row["current"])?;
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
    need(
        row["path"] == pin.path
            && row["current"]
                == json!({"bytes":pin.current_bytes,"sha256":pin.current_sha,"mode":"0644"})
            && row["prior"]
                == json!({"bytes":pin.prior_bytes,"sha256":pin.prior_sha,"mode":"0644"})
            && row["prior_git_blob"] == pin.blob,
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
fn git_prior(root: &Path, pin: &Pin<'_>, prior: &[u8]) -> Result<()> {
    need(
        git(root, &["show", &format!("{BASE}:{}", pin.path)])? == prior,
        "exact base Git source bytes",
    )?;
    need(
        git(root, &["ls-tree", BASE, "--", pin.path])?
            == format!("100644 blob {}\t{}\n", pin.blob, pin.path).as_bytes(),
        "exact base Git tree/blob/mode",
    )
}
/// Full custody: authenticate current physical source and capsule before exposing its parent.
/// A sealed historical buffer supplied as current is rejected even if its hash is known.
pub(crate) fn prior(root: &Path, path: &str, supplied: &[u8]) -> Result<Vec<u8>> {
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
    let mut capsule_mutant = CAPSULE.to_vec();
    capsule_mutant.push(b' ');
    need(decode(&capsule_mutant).is_err(), "changed capsule rejected")?;
    need(
        prior(root, "unlisted.rs", b"").is_err(),
        "wrong physical caller rejected",
    )?;
    let all_pins = pins(&doc)?;
    for pin in &all_pins {
        let physical = regular(root, pin.path, pin.current_bytes)?;
        let prior_bytes = prior(root, pin.path, &physical)?;
        let source_row = row(&doc, pin.path)?;
        need(
            inverse(pin, source_row, &physical, 0o644)? == prior_bytes,
            "pure positive precedes controls",
        )?;
        need(
            frame(pin.path, &physical, 0o644)? == prior_bytes,
            "same pure frame as full custody",
        )?;
        need(
            prior(root, pin.path, &prior_bytes).is_err()
                && frame(pin.path, &prior_bytes, 0o644).is_err(),
            "stale parent is not current",
        )?;
        for mode in [0o600, 0o755, 0o4644] {
            need(
                inverse(pin, source_row, &physical, mode).is_err(),
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
                inverse(pin, &bad, &physical, 0o644).is_err(),
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
                inverse(pin, &bad, &physical, 0o644).is_err(),
                "reordered edits rejected",
            )?;
            let mut bad = source_row.clone();
            bad["edits"][1] = bad["edits"][0].clone();
            need(
                inverse(pin, &bad, &physical, 0o644).is_err(),
                "same-count duplicate span rejected",
            )?;
        }
    }
    Ok(())
}
