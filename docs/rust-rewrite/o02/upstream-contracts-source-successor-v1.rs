//! Exact O02 additive frame before the unchanged T08A historical custody chains.
//! `frame` proves bytes only; callers retain physical read/mode authority.
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::Read,
    path::{Component, Path},
    process::Command,
};
type Result<T> = std::result::Result<T, String>;
const PATH: &str = "docs/rust-rewrite/o02/UPSTREAM_CONTRACTS_SOURCE_SUCCESSOR.json";
const RAW: &[u8] = include_bytes!("UPSTREAM_CONTRACTS_SOURCE_SUCCESSOR.json");
const SHA: &str = "7c2a0024e2d1d3abe37ab6e49337e9fcfd989c406c3bf154c6a7b44e29ed534b";
const BASE: &str = "661af83a1b95a3e7873cfda99c05d0fb6a02fe0a";
const SCOPE: &str = "Two exact additive upstream-contracts dispatcher and dependency insertions per path; reconstruct accepted main and lock before unchanged historical custody. No historical acceptance or runtime qualification is conferred.";
pub(crate) const LOCK_BYTES: usize = 94340;
pub(crate) const LOCK_SHA: &str =
    "fcc6f4375b5f967d6a1d96172a0707438957e95fbddd83a502105bf5fede4847";
struct Pin {
    path: &'static str,
    current_bytes: usize,
    current_sha: &'static str,
    prior_bytes: usize,
    prior_sha: &'static str,
    prior_blob: &'static str,
}
const PINS: [Pin; 2] = [
    Pin {
        path: "Cargo.lock",
        current_bytes: LOCK_BYTES,
        current_sha: LOCK_SHA,
        prior_bytes: 94314,
        prior_sha: "89545e8f75650c1d83c7aa5a36c92bc878184cf87cf42d3d0e93b2acc0a1ccd7",
        prior_blob: "46165dce1a345c7608b3d1ee6dfd249233e6e973",
    },
    Pin {
        path: "crates/xtask/src/main.rs",
        current_bytes: 45733,
        current_sha: "d1375df4b4210f8ac4f8b6c62b65c8d1c72531f70fe5ef593c556fa0c454bb91",
        prior_bytes: 45399,
        prior_sha: "8552ffd6647bca01c89cba514bb297d00ccfaca8ce4a656ef505222349012e36",
        prior_blob: "5e9e83d404bb649051c50b604ad2f1b267e211ab",
    },
];
fn need(ok: bool, reason: &str) -> Result<()> {
    if ok {
        Ok(())
    } else {
        Err(format!("O02 upstream source successor: {reason}"))
    }
}
fn digest(raw: &[u8]) -> String {
    Sha256::digest(raw)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
fn keys(value: &Value, expected: &[&str]) -> bool {
    value
        .as_object()
        .is_some_and(|o| o.len() == expected.len() && expected.iter().all(|k| o.contains_key(*k)))
}
fn scope(doc: &Value) -> Result<()> {
    need(
        keys(doc, &["schema", "baseline", "scope", "rows"])
            && doc["schema"] == "O02_UPSTREAM_CONTRACTS_SOURCE_SUCCESSOR_V1"
            && doc["baseline"] == BASE
            && doc["scope"] == SCOPE
            && doc["rows"]
                .as_array()
                .is_some_and(|rows| rows.len() == PINS.len()),
        "closed capsule header/two-path scope",
    )?;
    for (pin, row) in PINS.iter().zip(doc["rows"].as_array().ok_or("O02 rows")?) {
        need(
            keys(
                row,
                &["path", "prior", "prior_git_blob", "current", "edits"],
            ) && row["path"] == pin.path
                && row["prior"]
                    == json!({"bytes":pin.prior_bytes,"sha256":pin.prior_sha,"mode":"0644"})
                && row["prior_git_blob"] == pin.prior_blob
                && row["current"]
                    == json!({"bytes":pin.current_bytes,"sha256":pin.current_sha,"mode":"0644"})
                && row["edits"]
                    .as_array()
                    .is_some_and(|edits| edits.len() == 2),
            "closed ordered path/material/blob/two-edit row",
        )?;
        for edit in row["edits"].as_array().ok_or("O02 edits")? {
            need(
                keys(edit, &["at", "remove_utf8", "insert_utf8"])
                    && edit["at"]
                        .as_u64()
                        .and_then(|n| usize::try_from(n).ok())
                        .is_some()
                    && edit["remove_utf8"] == ""
                    && edit["insert_utf8"].as_str().is_some_and(|s| !s.is_empty()),
                "closed additive typed span",
            )?;
        }
    }
    Ok(())
}
fn decode(raw: &[u8]) -> Result<Value> {
    need(
        raw == RAW && digest(raw) == SHA,
        "compiled capsule bytes/hash",
    )?;
    let doc: Value = serde_json::from_slice(raw).map_err(|e| e.to_string())?;
    scope(&doc)?;
    Ok(doc)
}
fn inverse(pin: &Pin, row: &Value, current: &[u8], mode: u32) -> Result<Vec<u8>> {
    need(
        mode == 0o644 && current.len() == pin.current_bytes && digest(current) == pin.current_sha,
        "exact current bytes/hash/mode",
    )?;
    need(
        keys(
            row,
            &["path", "prior", "prior_git_blob", "current", "edits"],
        ) && row["path"] == pin.path
            && row["prior"]
                == json!({"bytes":pin.prior_bytes,"sha256":pin.prior_sha,"mode":"0644"})
            && row["prior_git_blob"] == pin.prior_blob
            && row["current"]
                == json!({"bytes":pin.current_bytes,"sha256":pin.current_sha,"mode":"0644"}),
        "exact inverse caller tuple",
    )?;
    let edits = row["edits"].as_array().ok_or("O02 edits missing")?;
    need(edits.len() == 2, "exact two additive spans")?;
    let mut prior = Vec::with_capacity(pin.prior_bytes);
    let (mut old_cursor, mut cursor) = (0usize, 0usize);
    let mut previous = None;
    for edit in edits {
        need(
            keys(edit, &["at", "remove_utf8", "insert_utf8"]) && edit["remove_utf8"] == "",
            "closed insertion fields",
        )?;
        let at = edit["at"]
            .as_u64()
            .and_then(|n| usize::try_from(n).ok())
            .ok_or("O02 span offset")?;
        need(
            previous.is_none_or(|old| at > old),
            "strict ordered unique offsets",
        )?;
        let inserted = edit["insert_utf8"]
            .as_str()
            .filter(|s| !s.is_empty())
            .ok_or("O02 nonempty span")?
            .as_bytes();
        let unchanged = at.checked_sub(old_cursor).ok_or("O02 overlapping span")?;
        let end = cursor.checked_add(unchanged).ok_or("O02 prefix overflow")?;
        prior.extend_from_slice(
            current
                .get(cursor..end)
                .ok_or("O02 prefix outside source")?,
        );
        let next = end
            .checked_add(inserted.len())
            .ok_or("O02 insertion overflow")?;
        need(
            current.get(end..next) == Some(inserted),
            "exact inserted bytes",
        )?;
        old_cursor = at;
        cursor = next;
        previous = Some(at);
    }
    prior.extend_from_slice(current.get(cursor..).ok_or("O02 suffix outside source")?);
    need(
        prior.len() == pin.prior_bytes && digest(&prior) == pin.prior_sha,
        "exact accepted predecessor bytes/hash",
    )?;
    let mut forward = Vec::with_capacity(current.len());
    let mut cursor = 0usize;
    for edit in edits {
        let at = edit["at"]
            .as_u64()
            .and_then(|n| usize::try_from(n).ok())
            .ok_or("O02 forward offset")?;
        forward.extend_from_slice(prior.get(cursor..at).ok_or("O02 forward ordered prefix")?);
        forward.extend_from_slice(
            edit["insert_utf8"]
                .as_str()
                .ok_or("O02 forward insertion")?
                .as_bytes(),
        );
        cursor = at;
    }
    forward.extend_from_slice(prior.get(cursor..).ok_or("O02 forward suffix")?);
    need(forward == current, "complete forward reconstruction")?;
    Ok(prior)
}
pub(crate) fn frame(path: &str, current: &[u8], mode: u32) -> Result<Vec<u8>> {
    let index = PINS
        .iter()
        .position(|pin| pin.path == path)
        .ok_or("O02 unowned source path")?;
    let doc = decode(RAW)?;
    inverse(&PINS[index], &doc["rows"][index], current, mode)
}
fn confined(root: &Path, path: &str) -> Result<std::path::PathBuf> {
    let root = root.canonicalize().map_err(|e| e.to_string())?;
    let mut full = root.clone();
    let components = Path::new(path).components().collect::<Vec<_>>();
    need(!components.is_empty(), "nonempty relative source path")?;
    for (index, component) in components.iter().enumerate() {
        let Component::Normal(name) = component else {
            return Err("O02 unsafe source path component".into());
        };
        full.push(name);
        let metadata = fs::symlink_metadata(&full).map_err(|e| e.to_string())?;
        need(
            !metadata.file_type().is_symlink()
                && if index + 1 == components.len() {
                    metadata.is_file()
                } else {
                    metadata.is_dir()
                },
            "regular source with no symlink ancestors",
        )?;
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            need(
                metadata.file_attributes() & 0x400 == 0,
                "no Windows reparse point",
            )?;
        }
    }
    need(
        full.canonicalize()
            .map_err(|e| e.to_string())?
            .starts_with(root),
        "source confined to root",
    )?;
    Ok(full)
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
fn physical(root: &Path, path: &str, expected_bytes: usize) -> Result<Vec<u8>> {
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
        return Err("O02 no-follow source open is not implemented for this Unix".into());
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
    return Err("O02 source admission requires Unix or Windows".into());
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
pub(crate) fn controls(root: &Path) -> Result<()> {
    let runtime = physical(root, PATH, RAW.len())?;
    let doc = decode(&runtime)?;
    need(
        decode(&[RAW, b" "].concat()).is_err(),
        "altered capsule rejected",
    )?;
    for (index, pin) in PINS.iter().enumerate() {
        let actual = physical(root, pin.path, pin.current_bytes)?;
        let row = &doc["rows"][index];
        let prior = frame(pin.path, &actual, 0o644)?;
        let git = Command::new("git")
            .args(["show", &format!("{BASE}:{}", pin.path)])
            .current_dir(root)
            .output()
            .map_err(|e| e.to_string())?;
        let tree = Command::new("git")
            .args(["ls-tree", BASE, "--", pin.path])
            .current_dir(root)
            .output()
            .map_err(|e| e.to_string())?;
        need(
            git.status.success()
                && git.stdout == prior
                && tree.status.success()
                && tree.stdout
                    == format!("100644 blob {}\t{}\n", pin.prior_blob, pin.path).as_bytes(),
            "exact predecessor Git bytes/mode/blob",
        )?;
        need(
            frame(pin.path, &prior, 0o644).is_err(),
            "historical predecessor rejected as current",
        )?;
        for mode in [0o600, 0o755, 0o4644] {
            need(
                frame(pin.path, &actual, mode).is_err(),
                "wrong source mode rejected",
            )?;
        }
        for mutation in 0..4 {
            let mut bad = actual.clone();
            match mutation {
                0 => {
                    bad[0] ^= 1;
                }
                1 => {
                    bad.push(b'\n');
                }
                2 => {
                    bad.pop();
                }
                _ => {
                    bad.splice(0..0, b"unexpected".iter().copied());
                }
            }
            need(
                frame(pin.path, &bad, 0o644).is_err(),
                "changed/truncated/appended current rejected",
            )?;
        }
        let mut shift = 0usize;
        for edit in row["edits"].as_array().ok_or("O02 control spans")? {
            let at = edit["at"]
                .as_u64()
                .and_then(|n| usize::try_from(n).ok())
                .ok_or("O02 control offset")?;
            let inserted = edit["insert_utf8"]
                .as_str()
                .ok_or("O02 control insertion")?
                .as_bytes();
            let start = at.checked_add(shift).ok_or("O02 control shift")?;
            let end = start.checked_add(inserted.len()).ok_or("O02 control end")?;
            let mut missing = actual.clone();
            missing.drain(start..end);
            need(
                frame(pin.path, &missing, 0o644).is_err(),
                "missing physical insertion rejected",
            )?;
            let mut duplicate = actual.clone();
            duplicate.splice(start..start, inserted.iter().copied());
            need(
                frame(pin.path, &duplicate, 0o644).is_err(),
                "duplicate physical insertion rejected",
            )?;
            let mut changed = actual.clone();
            changed[start] ^= 1;
            need(
                frame(pin.path, &changed, 0o644).is_err(),
                "changed physical insertion rejected",
            )?;
            shift = shift
                .checked_add(inserted.len())
                .ok_or("O02 control cumulative shift")?;
        }
        for mutation in 0..6 {
            let mut bad = row.clone();
            let edits = bad["edits"].as_array_mut().ok_or("O02 control edits")?;
            match mutation {
                0 => {
                    edits.pop();
                }
                1 => {
                    edits.push(edits[0].clone());
                }
                2 => {
                    edits.swap(0, 1);
                }
                3 => {
                    edits[0]["at"] = json!(usize::MAX);
                }
                4 => {
                    edits[0]["insert_utf8"] = json!("changed");
                }
                _ => {
                    edits[0]["at"] = json!(edits[0]["at"].as_u64().ok_or("O02 offset")? + 1);
                }
            }
            need(
                inverse(pin, &bad, &actual, 0o644).is_err(),
                "missing/duplicate/reordered/moved/changed/overflow span rejected",
            )?;
        }
        for key in ["prior_git_blob", "prior", "current", "path"] {
            let mut bad = row.clone();
            bad[key] = json!("changed");
            need(
                inverse(pin, &bad, &actual, 0o644).is_err(),
                "changed inverse authority rejected",
            )?;
        }
    }
    for mutation in 0..6 {
        let mut bad = doc.clone();
        match mutation {
            0 => {
                bad["rows"].as_array_mut().ok_or("O02 rows")?.pop();
            }
            1 => {
                let row = bad["rows"][0].clone();
                bad["rows"].as_array_mut().ok_or("O02 rows")?.push(row);
            }
            2 => {
                bad["rows"].as_array_mut().ok_or("O02 rows")?.swap(0, 1);
            }
            3 => {
                bad["baseline"] = json!("changed");
            }
            4 => {
                bad["rows"][0]["path"] = json!("unowned");
            }
            _ => {
                bad["unexpected"] = json!(true);
            }
        }
        need(scope(&bad).is_err(), "closed scope mutation rejected")?;
    }
    need(
        frame("unowned", b"", 0o644).is_err(),
        "unowned path rejected",
    )
}

// Separate one-path catalog frame; the main/lock capsule remains closed to two paths.
pub(crate) const GATES_PATH: &str = "docs/rust-rewrite/ci/gates.json";
const GATES_CAPSULE_PATH: &str = "docs/rust-rewrite/o02/UPSTREAM_CONTRACTS_GATES_SUCCESSOR.json";
const GATES_RAW: &[u8] = include_bytes!("UPSTREAM_CONTRACTS_GATES_SUCCESSOR.json");
const GATES_SHA: &str = "007d933a6e2d4b758a83078d35c4650035ff1a8433ccb2ac5519fa4301a2f58a";
const GATES_SCOPE: &str = "Only the upstream-contracts.yml:live-contracts catalog entry changes to the bounded native O02 successor; reconstruct exact accepted gates bytes before unchanged DNSpython source custody. Historical qualification remains unchanged.";
const GATES_PIN: Pin = Pin {
    path: GATES_PATH,
    current_bytes: 177321,
    current_sha: "4fe646332b6291b68f9423dffd4cfb5939c07cbf5988895c0577c20170fe6708",
    prior_bytes: 176465,
    prior_sha: "45c7f9f39791ee75c3e6d91f6da1ecca2e213ed29080dea13f853511dd0c655e",
    prior_blob: "a436239ec26a7af5de3c255e0824802dfc6f3787",
};
fn gates_row_scope(row: &Value) -> Result<()> {
    need(
        keys(
            row,
            &["path", "current", "prior", "prior_git_blob", "edits"],
        ) && row["path"] == GATES_PATH
            && row["current"]
                == json!({"bytes":GATES_PIN.current_bytes,"sha256":GATES_PIN.current_sha,"mode":"0644"})
            && row["prior"]
                == json!({"bytes":GATES_PIN.prior_bytes,"sha256":GATES_PIN.prior_sha,"mode":"0644"})
            && row["prior_git_blob"] == GATES_PIN.prior_blob
            && row["edits"]
                .as_array()
                .is_some_and(|edits| edits.len() == 7),
        "closed catalog path/material/blob/seven-edit row",
    )
}
fn gates_scope(doc: &Value) -> Result<()> {
    need(
        keys(doc, &["schema", "baseline", "scope", "row"])
            && doc["schema"] == "O02_UPSTREAM_CONTRACTS_GATES_SUCCESSOR_V1"
            && doc["baseline"] == BASE
            && doc["scope"] == GATES_SCOPE,
        "closed one-path catalog capsule",
    )?;
    gates_row_scope(&doc["row"])
}
fn gates_decode(raw: &[u8]) -> Result<Value> {
    need(
        raw == GATES_RAW && digest(raw) == GATES_SHA,
        "compiled catalog capsule bytes/hash",
    )?;
    let doc: Value = serde_json::from_slice(raw).map_err(|e| e.to_string())?;
    gates_scope(&doc)?;
    Ok(doc)
}
fn gates_inverse(row: &Value, current: &[u8], mode: u32) -> Result<Vec<u8>> {
    gates_row_scope(row)?;
    need(
        mode == 0o644
            && current.len() == GATES_PIN.current_bytes
            && digest(current) == GATES_PIN.current_sha,
        "exact physical catalog bytes/hash/mode",
    )?;
    let edits = row["edits"].as_array().ok_or("O02 catalog edits")?;
    let mut prior = Vec::with_capacity(GATES_PIN.prior_bytes);
    let (mut old_cursor, mut cursor) = (0usize, 0usize);
    let mut previous = None;
    for edit in edits {
        need(
            keys(edit, &["at", "remove_utf8", "insert_utf8"]),
            "closed catalog span fields",
        )?;
        let at = edit["at"]
            .as_u64()
            .and_then(|n| usize::try_from(n).ok())
            .ok_or("O02 catalog offset")?;
        let remove = edit["remove_utf8"]
            .as_str()
            .ok_or("O02 catalog prior span")?
            .as_bytes();
        let insert = edit["insert_utf8"]
            .as_str()
            .ok_or("O02 catalog current span")?
            .as_bytes();
        need(
            remove != insert && previous.is_none_or(|old| at > old),
            "nonvacuous ordered unique catalog spans",
        )?;
        let unchanged = at
            .checked_sub(old_cursor)
            .ok_or("O02 catalog overlapping spans")?;
        let end = cursor
            .checked_add(unchanged)
            .ok_or("O02 catalog prefix overflow")?;
        prior.extend_from_slice(
            current
                .get(cursor..end)
                .ok_or("O02 catalog prefix outside source")?,
        );
        let next = end
            .checked_add(insert.len())
            .ok_or("O02 catalog insertion overflow")?;
        need(
            current.get(end..next) == Some(insert),
            "exact current catalog span",
        )?;
        prior.extend_from_slice(remove);
        old_cursor = at
            .checked_add(remove.len())
            .ok_or("O02 catalog prior offset overflow")?;
        cursor = next;
        previous = Some(at);
    }
    prior.extend_from_slice(
        current
            .get(cursor..)
            .ok_or("O02 catalog suffix outside source")?,
    );
    need(
        prior.len() == GATES_PIN.prior_bytes && digest(&prior) == GATES_PIN.prior_sha,
        "exact accepted catalog predecessor bytes/hash",
    )?;
    let mut forward = Vec::with_capacity(current.len());
    let mut cursor = 0usize;
    for edit in edits {
        let at = edit["at"]
            .as_u64()
            .and_then(|n| usize::try_from(n).ok())
            .ok_or("O02 catalog forward offset")?;
        let remove = edit["remove_utf8"]
            .as_str()
            .ok_or("O02 catalog forward prior span")?
            .as_bytes();
        let end = at
            .checked_add(remove.len())
            .ok_or("O02 catalog forward overflow")?;
        need(
            prior.get(at..end) == Some(remove),
            "exact predecessor catalog span",
        )?;
        forward.extend_from_slice(
            prior
                .get(cursor..at)
                .ok_or("O02 catalog ordered forward prefix")?,
        );
        forward.extend_from_slice(
            edit["insert_utf8"]
                .as_str()
                .ok_or("O02 catalog forward insertion")?
                .as_bytes(),
        );
        cursor = end;
    }
    forward.extend_from_slice(prior.get(cursor..).ok_or("O02 catalog forward suffix")?);
    need(
        forward == current,
        "complete catalog forward reconstruction",
    )?;
    Ok(prior)
}
/// Pure exact catalog projection. Callers retain physical read and supplied-buffer checks.
pub(crate) fn gates_frame(current: &[u8], mode: u32) -> Result<Vec<u8>> {
    let doc = gates_decode(GATES_RAW)?;
    gates_inverse(&doc["row"], current, mode)
}
pub(crate) fn gates_current_at(physical_root: &Path, proof_root: &Path) -> Result<Vec<u8>> {
    let capsule = physical(physical_root, GATES_CAPSULE_PATH, GATES_RAW.len())?;
    let doc = gates_decode(&capsule)?;
    let current = physical(physical_root, GATES_PATH, GATES_PIN.current_bytes)?;
    let prior = gates_inverse(&doc["row"], &current, 0o644)?;
    let git = Command::new("git")
        .args(["show", &format!("{BASE}:{GATES_PATH}")])
        .current_dir(proof_root)
        .output()
        .map_err(|e| e.to_string())?;
    let tree = Command::new("git")
        .args(["ls-tree", BASE, "--", GATES_PATH])
        .current_dir(proof_root)
        .output()
        .map_err(|e| e.to_string())?;
    need(
        git.status.success()
            && git.stdout == prior
            && tree.status.success()
            && tree.stdout
                == format!("100644 blob {}\t{GATES_PATH}\n", GATES_PIN.prior_blob).as_bytes(),
        "exact accepted catalog Git bytes/mode/blob",
    )?;
    Ok(current)
}
pub(crate) fn gates_controls(root: &Path) -> Result<()> {
    #[cfg(test)]
    gates_physical_controls(root)?;
    let doc = gates_decode(GATES_RAW)?;
    let row = &doc["row"];
    let current = gates_current_at(root, root)?;
    let prior = gates_frame(&current, 0o644)?;
    need(
        gates_frame(&prior, 0o644).is_err(),
        "stale accepted catalog cannot be current",
    )?;
    need(
        gates_decode(&[GATES_RAW, b" "].concat()).is_err(),
        "altered catalog capsule rejected",
    )?;
    for mode in [0o600, 0o755, 0o4644] {
        need(
            gates_frame(&current, mode).is_err(),
            "wrong catalog mode rejected",
        )?;
    }
    for mutation in 0..4 {
        let mut bad = current.clone();
        match mutation {
            0 => {
                bad[0] ^= 1;
            }
            1 => {
                bad.push(b'\n');
            }
            2 => {
                bad.pop();
            }
            _ => {
                bad.splice(0..0, b"unexpected".iter().copied());
            }
        }
        need(
            gates_frame(&bad, 0o644).is_err(),
            "changed/truncated/appended physical catalog rejected",
        )?;
    }
    for mutation in 0..9 {
        let mut bad = row.clone();
        let edits = bad["edits"]
            .as_array_mut()
            .ok_or("O02 catalog control edits")?;
        match mutation {
            0 => {
                edits.pop();
            }
            1 => {
                edits.push(edits[0].clone());
            }
            2 => {
                edits.swap(0, 1);
            }
            3 => {
                edits[0]["at"] = json!(usize::MAX);
            }
            4 => {
                edits[0]["insert_utf8"] = json!("changed");
            }
            5 => {
                edits[0]["remove_utf8"] = json!("changed");
            }
            6 => {
                edits[0]["at"] = json!(
                    edits[0]["at"]
                        .as_u64()
                        .ok_or("O02 catalog control offset")?
                        + 1
                );
            }
            7 => {
                edits[1] = edits[0].clone();
            }
            _ => {
                edits[0]["unexpected"] = json!(true);
            }
        }
        need(
            gates_inverse(&bad, &current, 0o644).is_err(),
            "missing/duplicate/reordered/moved/changed/malformed catalog span rejected",
        )?;
    }
    for key in ["prior_git_blob", "prior", "current", "path"] {
        let mut bad = row.clone();
        bad[key] = json!("changed");
        need(
            gates_inverse(&bad, &current, 0o644).is_err(),
            "changed catalog authority rejected",
        )?;
    }
    for key in ["schema", "baseline", "scope"] {
        let mut bad = doc.clone();
        bad[key] = json!("changed");
        need(
            gates_scope(&bad).is_err(),
            "changed catalog capsule header rejected",
        )?;
    }
    Ok(())
}

#[cfg(test)]
fn gates_physical_controls(proof_root: &Path) -> Result<()> {
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
        let temp = std::env::var_os("SLOPSEARX_TEST_TEMP_ROOT")
            .or_else(|| std::env::var_os("TMPDIR"))
            .ok_or("O02 physical controls require an explicitly configured temporary root")?;
        need(
            Path::new(&temp).is_absolute(),
            "absolute configured temporary root",
        )?;
        let suffix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_nanos();
        let owned =
            Owned(Path::new(&temp).join(format!("ssx-o02-gates-{}-{suffix}", std::process::id())));
        fs::create_dir(&owned.0).map_err(|e| e.to_string())?;
        let source = gates_current_at(proof_root, proof_root)?;
        for (path, bytes) in [
            (GATES_CAPSULE_PATH, GATES_RAW),
            (GATES_PATH, source.as_slice()),
        ] {
            let full = owned.0.join(path);
            fs::create_dir_all(full.parent().ok_or("O02 fixture parent")?)
                .map_err(|e| e.to_string())?;
            fs::write(&full, bytes).map_err(|e| e.to_string())?;
            fs::set_permissions(full, fs::Permissions::from_mode(0o644))
                .map_err(|e| e.to_string())?;
        }
        let current = gates_current_at(&owned.0, proof_root)?;
        let prior = gates_frame(&current, 0o644)?;
        let full = owned.0.join(GATES_PATH);
        fs::write(&full, &prior).map_err(|e| e.to_string())?;
        need(
            gates_current_at(&owned.0, proof_root).is_err(),
            "stale physical catalog rejected",
        )?;
        fs::write(&full, &current).map_err(|e| e.to_string())?;
        fs::set_permissions(&full, fs::Permissions::from_mode(0o600)).map_err(|e| e.to_string())?;
        need(
            gates_current_at(&owned.0, proof_root).is_err(),
            "wrong physical catalog mode rejected",
        )?;
        fs::set_permissions(&full, fs::Permissions::from_mode(0o644)).map_err(|e| e.to_string())?;
        let mut changed = current.clone();
        changed[0] ^= 1;
        fs::write(&full, &changed).map_err(|e| e.to_string())?;
        need(
            gates_current_at(&owned.0, proof_root).is_err(),
            "same-size physical catalog edit rejected",
        )?;
        changed.push(b'\n');
        fs::write(&full, &changed).map_err(|e| e.to_string())?;
        need(
            gates_current_at(&owned.0, proof_root).is_err(),
            "oversized physical catalog rejected",
        )?;
        fs::remove_file(&full).map_err(|e| e.to_string())?;
        need(
            gates_current_at(&owned.0, proof_root).is_err(),
            "missing physical catalog rejected",
        )?;
        fs::create_dir(&full).map_err(|e| e.to_string())?;
        need(
            gates_current_at(&owned.0, proof_root).is_err(),
            "directory catalog rejected",
        )?;
        fs::remove_dir(&full).map_err(|e| e.to_string())?;
        symlink(proof_root.join(GATES_PATH), &full).map_err(|e| e.to_string())?;
        need(
            gates_current_at(&owned.0, proof_root).is_err(),
            "symlink catalog rejected",
        )?;
        fs::remove_file(&full).map_err(|e| e.to_string())?;
        fs::write(&full, &current).map_err(|e| e.to_string())?;
        fs::set_permissions(&full, fs::Permissions::from_mode(0o644)).map_err(|e| e.to_string())?;
        gates_current_at(&owned.0, proof_root)?;
        let capsule = owned.0.join(GATES_CAPSULE_PATH);
        let mut changed_capsule = GATES_RAW.to_vec();
        changed_capsule.push(b' ');
        fs::write(&capsule, changed_capsule).map_err(|e| e.to_string())?;
        need(
            gates_current_at(&owned.0, proof_root).is_err(),
            "changed physical catalog capsule rejected",
        )?;
        fs::write(&capsule, GATES_RAW).map_err(|e| e.to_string())?;
        gates_current_at(&owned.0, proof_root)?;
        let catalog_dir = full.parent().ok_or("O02 catalog parent")?;
        fs::remove_file(&full).map_err(|e| e.to_string())?;
        fs::remove_dir(catalog_dir).map_err(|e| e.to_string())?;
        let shadow = owned.0.join("catalog-shadow");
        fs::create_dir(&shadow).map_err(|e| e.to_string())?;
        fs::write(shadow.join("gates.json"), &current).map_err(|e| e.to_string())?;
        symlink(&shadow, catalog_dir).map_err(|e| e.to_string())?;
        need(
            gates_current_at(&owned.0, proof_root).is_err(),
            "symlink catalog ancestor rejected",
        )?;
    }
    Ok(())
}
