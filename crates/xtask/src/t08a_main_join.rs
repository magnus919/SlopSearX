//! Closed accepted-CLI/W08-main join. No calls into either historical reader.
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{fs, path::Path, process::Command};
type Result<T> = std::result::Result<T, String>;
pub(crate) const CAPSULE_PATH: &str = "docs/rust-rewrite/t08a/accepted-w08-main-join-v1.json";
const CAPSULE_SHA: &str = "e7206a5a5b3b9f882519c861e85dbcb71771ed48a156c68b96d06edc0d1e4d21";
const CLI_PARENT: &str = "c9a943f00eb94c739ec6f4d5433ce2c6e1f62b42";
const MAIN_PARENT: &str = "d9d42277657394e9f62061072a95671345a94f38";
const ROWS: [(&str, usize, usize); 4] = [
    ("crates/xtask/src/main.rs", 3, 6),
    (
        "docs/rust-rewrite/a06/source-amendments/shared-successor.rs",
        6,
        1,
    ),
    (
        "docs/rust-rewrite/a06/source-amendments/w07-t02-joint.rs",
        38,
        3,
    ),
    ("crates/xtask/src/w07.rs", 18, 2),
];
#[derive(Clone, Copy)]
pub(crate) enum Parent {
    Cli,
    Main,
}
pub(crate) fn owns(path: &str) -> bool {
    ROWS.iter().any(|row| row.0 == path)
}
fn need(ok: bool, why: &str) -> Result<()> {
    if ok {
        Ok(())
    } else {
        Err(format!("T08A accepted main join: {why}"))
    }
}
fn digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
fn regular(root: &Path, path: &str) -> Result<Vec<u8>> {
    let full = root.join(path);
    let m = fs::symlink_metadata(&full).map_err(|e| e.to_string())?;
    need(
        m.is_file() && !m.file_type().is_symlink() && m.len() <= 1048576,
        "regular bounded source leaf",
    )?;
    need(
        full.canonicalize()
            .map_err(|e| e.to_string())?
            .starts_with(root.canonicalize().map_err(|e| e.to_string())?),
        "source stays within root",
    )?;
    fs::read(full).map_err(|e| e.to_string())
}
fn source_mode(root: &Path, path: &str) -> Result<&'static str> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        need(
            fs::symlink_metadata(root.join(path))
                .map_err(|e| e.to_string())?
                .permissions()
                .mode()
                & 0o7777
                == 0o644,
            "physical source requires regular0644",
        )?;
    }
    #[cfg(not(unix))]
    {
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
    }
    Ok("0644")
}
fn scope(doc: &Value) -> Result<()> {
    need(
        doc["schema"] == "T08A_CLI_ACCEPTED_W08_MAIN_JOIN_V1"
            && doc.as_object().is_some_and(|fields| fields.len() == 3)
            && doc["parents"] == serde_json::json!({"cli":CLI_PARENT,"main":MAIN_PARENT})
            && doc["rows"]
                .as_array()
                .is_some_and(|rows| rows.len() == ROWS.len()),
        "closed parents and four-row scope",
    )?;
    for (index, (path, cli_count, main_count)) in ROWS.iter().enumerate() {
        let row = &doc["rows"][index];
        need(
            row["path"] == *path
                && row.as_object().is_some_and(|fields| fields.len() == 4)
                && row["current"]["path"] == *path
                && row["current"]["mode"] == "0644",
            "exact ordered current path/mode",
        )?;
        for (name, count, commit) in [
            ("cli", cli_count, CLI_PARENT),
            ("main", main_count, MAIN_PARENT),
        ] {
            let frame = &row[name];
            need(
                frame["current"] == row["current"]
                    && frame["prior"]["path"] == *path
                    && frame["prior"]["mode"] == "0644"
                    && frame["prior"]["commit"] == commit
                    && frame["edits"]
                        .as_array()
                        .is_some_and(|edits| edits.len() == *count),
                "closed endpoint path/parent/mode/span count",
            )?;
        }
    }
    Ok(())
}
fn capsule(root: &Path) -> Result<Value> {
    source_mode(root, CAPSULE_PATH)?;
    let raw = regular(root, CAPSULE_PATH)?;
    need(
        digest(&raw) == CAPSULE_SHA,
        "compiled outer capsule identity",
    )?;
    let doc: Value = serde_json::from_slice(&raw).map_err(|e| e.to_string())?;
    scope(&doc)?;
    Ok(doc)
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
// The #984 PR changes W07's verifier after the accepted CLI/W08 join was sealed.
// Bind the exact physical change, then invert its three finite spans before the
// existing join check so both accepted parents remain independently authentic.
fn pre_984_w07(path: &str, physical: &[u8]) -> Result<Vec<u8>> {
    if path != "crates/xtask/src/w07.rs" {
        return Ok(physical.to_vec());
    }
    need(
        physical.len() == 42023
            && digest(physical)
                == "55c93c8914529eb5c26df0943068b6fd97200f67c288f63bbafea6033729812c",
        "#984 exact physical W07 verifier",
    )?;
    let inverse_row = serde_json::json!({
        "current": {
            "sha256": "55c93c8914529eb5c26df0943068b6fd97200f67c288f63bbafea6033729812c",
            "bytes": 42023,
            "mode": "0644",
        },
        "prior": {
            "sha256": "4cc2a95645fa639d0873ea1a8803693d63eef074782f41c54540ea04632e6916",
            "bytes": 41464,
            "mode": "0644",
        },
        "edits": [
            {
                "at": 854,
                "remove_utf8": "const REGISTRY_SHA: &str = \"a651a9d36c1d5409edbcb7d79a44ac599468ca493fdd735982bb1d1894e4461d\";",
                "insert_utf8": "const REGISTRY_SHA: &str = \"4d9cb197718db1d06086ee1557f1eb4953bf53b9938eb4bce41da762f70ad1a7\";",
            },
            {
                "at": 10275,
                "remove_utf8": "",
                "insert_utf8": "    // #984 adds exact real-adapter MCP/HTTP suites; all historical W07 suites\n    // and source pins remain unchanged except this reviewed T06 registrar.\n    if expected[\"sources\"][\"crates/xtask/src/t06.rs\"]\n        != \"be30918a7ac4365b2538b58fd1c3c933e6b2bea96d6222d267c4a87d29e413e0\"\n    {\n        return Err(\"W07 original T06 registrar pin changed\".into());\n    }\n    expected[\"sources\"][\"crates/xtask/src/t06.rs\"] =\n        Value::String(\"00a0a0ba122cd15da453cad72a025750e476db71d7ff327bef5b8646ade4cdd7\".into());\n\n",
            },
            {
                "at": 39511,
                "remove_utf8": "",
                "insert_utf8": "            \"crates/xtask/src/t06.rs\",\n",
            },
        ],
    });
    inverse(&inverse_row, physical, "0644", 3)
}

fn accepted_current(path: &str, physical: &[u8]) -> Result<Vec<u8>> {
    let w07 = pre_984_w07(path, physical)?;
    pre_t08_registration(path, &w07)
}

// Exact additive T08 registration precedes the unchanged accepted CLI/W08 join.
// This is a closed three-span current-source amendment, not an alternate root.
fn pre_t08_registration(path: &str, physical: &[u8]) -> Result<Vec<u8>> {
    if path != "crates/xtask/src/main.rs" {
        return Ok(physical.to_vec());
    }
    need(
        physical.len() == 42077
            && digest(physical)
                == "dbce267dc183e0f092eb880f9600676d217ca6b4e8f2ccc39bf7baf322080ceb",
        "T08 exact physical main registration",
    )?;
    let frames: [(&str, &str); 3] = [
        (
            "ent;\nmod t01;\nmod t02;\nmod t04;\nmod t05;\nmod t06;\nmod t08a;\nmod t08a_child;\npub(crate) use a13::",
            "ent;\nmod t01;\nmod t02;\nmod t04;\nmod t05;\nmod t06;\nmod t08;\nmod t08a;\nmod t08a_child;\npub(crate) use a13::",
        ),
        (
            "\"T09\" => t09::check(root, records),\n        \"T08A\" => t08a::check(root, records),\n        \"T04\" ",
            "\"T09\" => t09::check(root, records),\n        \"T08\" => t08::check(root, records),\n        \"T08A\" => t08a::check(root, records),\n        \"T04\" ",
        ),
        (
            "e = a08::report_scope(&records);\n    let scope = if id == \"T08A\" {\n        \"standalone native ex",
            "e = a08::report_scope(&records);\n    let scope = if id == \"T08\" {\n        \"native default/installed CLI against separate production HTTP/service/npm/private-Valkey qualification processes; finite17 integration cases and O04 artifact handoff; no production startup, provider or release/deployment claim\"\n    } else if id == \"T08A\" {\n        \"standalone native ex",
        ),
    ];
    let current = std::str::from_utf8(physical).map_err(|e| e.to_string())?;
    let mut prior = current.to_owned();
    for (before, after) in frames {
        need(
            prior.matches(after).count() == 1,
            "T08 unique inverse registration span",
        )?;
        prior = prior.replacen(after, before, 1);
    }
    need(
        prior.len() == 41759
            && digest(prior.as_bytes())
                == "7aaacf8aaafda17fbc676f6344720197b77be3be583882bf8d4fff2ac6695bd6",
        "T08 exact pre-registration main",
    )?;
    let mut forward = prior.clone();
    for (before, after) in frames {
        need(
            forward.matches(before).count() == 1,
            "T08 unique forward registration span",
        )?;
        forward = forward.replacen(before, after, 1);
    }
    need(
        forward.as_bytes() == physical,
        "T08 complete forward registration",
    )?;
    Ok(prior.into_bytes())
}
fn interpret(
    doc: &Value,
    path: &str,
    physical: &[u8],
    mode: &str,
    parent: Parent,
) -> Result<Vec<u8>> {
    let logical = accepted_current(path, physical)?;
    let physical = logical.as_slice();
    scope(doc)?;
    let index = ROWS
        .iter()
        .position(|row| row.0 == path)
        .ok_or("unlisted join path")?;
    let row = &doc["rows"][index];
    // Both complete inverses and independent forwards run before either result.
    let cli = inverse(&row["cli"], physical, mode, ROWS[index].1)?;
    let main = inverse(&row["main"], physical, mode, ROWS[index].2)?;
    Ok(match parent {
        Parent::Cli => cli,
        Parent::Main => main,
    })
}
/// Exact physical source admission before any logical parent frame is exposed.
pub(crate) fn project(root: &Path, path: &str, supplied: &[u8], parent: Parent) -> Result<Vec<u8>> {
    need(owns(path), "unlisted physical join path")?;
    let mode = source_mode(root, path)?;
    let physical = regular(root, path)?;
    need(
        physical == supplied,
        "join supplied buffer is not physical current",
    )?;
    interpret(&capsule(root)?, path, &physical, mode, parent)
}
/// Same controls are reached by owning CLI and W08 composition routes.
pub(crate) fn controls(root: &Path) -> Result<()> {
    let doc = capsule(root)?;
    for (index, (path, cli_count, main_count)) in ROWS.iter().enumerate() {
        let physical = regular(root, path)?;
        let logical = accepted_current(path, &physical)?;
        if *path == "crates/xtask/src/main.rs" {
            need(
                accepted_current(path, &logical).is_err(),
                "pre-T08 frame substituted as physical registration",
            )?;
        }
        for (name, parent, count, other) in [
            ("cli", Parent::Cli, *cli_count, "main"),
            ("main", Parent::Main, *main_count, "cli"),
        ] {
            let prior = project(root, path, &physical, parent)?;
            let row = &doc["rows"][index][name];
            need(
                inverse(row, &logical, "0644", count)? == prior,
                "pure outer positive before controls",
            )?;
            let commit = text(&row["prior"], "commit")?;
            let git = Command::new("git")
                .args(["show", &format!("{commit}:{path}")])
                .current_dir(root)
                .output()
                .map_err(|e| e.to_string())?;
            let tree = Command::new("git")
                .args(["ls-tree", commit, "--", path])
                .current_dir(root)
                .output()
                .map_err(|e| e.to_string())?;
            need(
                git.status.success()
                    && git.stdout == prior
                    && tree.status.success()
                    && tree.stdout
                        == format!("100644 blob {}\t{path}\n", text(&row["prior"], "git_blob")?)
                            .as_bytes(),
                "actual exact parent Git bytes/mode/blob",
            )?;
            need(
                prior != physical && project(root, path, &prior, parent).is_err(),
                "parent substituted as physical current",
            )?;
            let mut changed = physical.clone();
            changed.push(0);
            need(
                project(root, path, &changed, parent).is_err(),
                "changed supplied physical bytes",
            )?;
            for mode in ["0600", "0755", "4644"] {
                need(
                    interpret(&doc, path, &physical, mode, parent).is_err(),
                    "outer wrong mode",
                )?;
            }
            for mutation in 0..4 {
                let mut bad = doc.clone();
                let edits = bad["rows"][index][name]["edits"]
                    .as_array_mut()
                    .ok_or("outer edit array")?;
                match mutation {
                    0 => {
                        edits.pop();
                    }
                    1 => {
                        edits.push(edits[0].clone());
                    }
                    2 => edits[0]["at"] = serde_json::json!(usize::MAX),
                    _ => edits[0]["insert_utf8"] = serde_json::json!("changed"),
                }
                need(
                    interpret(&bad, path, &physical, "0644", parent).is_err(),
                    "outer missing/duplicate/moved/changed span",
                )?;
            }
            let mut unselected = doc.clone();
            unselected["rows"][index][other]["prior"]["sha256"] = serde_json::json!("changed");
            need(
                interpret(&unselected, path, &physical, "0644", parent).is_err(),
                "unselected endpoint must also authenticate",
            )?;
        }
    }
    for mutation in 0..4 {
        let mut bad = doc.clone();
        match mutation {
            0 => {
                bad["rows"].as_array_mut().ok_or("rows")?.pop();
            }
            1 => {
                let row = bad["rows"][0].clone();
                bad["rows"].as_array_mut().ok_or("rows")?.push(row);
            }
            2 => bad["rows"][0]["path"] = serde_json::json!("unlisted"),
            _ => bad["parents"]["main"] = serde_json::json!(CLI_PARENT),
        }
        need(
            scope(&bad).is_err(),
            "outer missing/extra/path/parent scope rejected",
        )?;
    }
    need(
        project(root, "unlisted", b"", Parent::Cli).is_err(),
        "unlisted physical source rejected",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pre_984_w07_reuses_exact_inverse_for_the_registered_projection() {
        let physical = include_bytes!("w07.rs");
        let prior = pre_984_w07("crates/xtask/src/w07.rs", physical).unwrap();
        assert_eq!(prior.len(), 41464);
        assert_eq!(
            digest(&prior),
            "4cc2a95645fa639d0873ea1a8803693d63eef074782f41c54540ea04632e6916"
        );
        let mut changed = physical.to_vec();
        changed.push(b' ');
        assert!(pre_984_w07("crates/xtask/src/w07.rs", &changed).is_err());
    }
}
