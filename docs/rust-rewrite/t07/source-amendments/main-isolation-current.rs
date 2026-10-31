//! Closed physical operands for the normal main merge and client-only relocation.
//! These twelve rows are input adapters to existing proofs, never a fallback reader.
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{fs, path::Path, process::Command};
const CAPSULE: &str =
    "docs/rust-rewrite/t07/source-amendments/main-isolation-current-operands-v1.json";
const SHA: &str = "3e5cb86a53e2060cd9782a812a3834ea23bea96fd0fb4c1f1b5652035a209e82";
const BYTES: usize = 101785;
const PATHS: [&str; 12] = [
    "Cargo.toml",
    "Cargo.lock",
    "crates/ssx-mcp/Cargo.toml",
    "crates/ssx-infra/Cargo.toml",
    "crates/xtask/Cargo.toml",
    "crates/xtask/src/main.rs",
    "crates/xtask/src/a13.rs",
    "crates/ssx-mcp/src/opaque.rs",
    "crates/ssx-infra/src/adapters/source_json.rs",
    "crates/ssx-infra/src/adapters/source_json_accounting.rs",
    "crates/ssx-infra/src/adapters/source_json_future.rs",
    "crates/ssx-infra/src/adapters/source_json_diagnostic.rs",
];
#[derive(Clone, Copy)]
pub(crate) enum Parent {
    T07,
    Main,
}
fn need(ok: bool, why: &str) -> Result<(), String> {
    if ok {
        Ok(())
    } else {
        Err(format!("T07 main/isolation: {why}"))
    }
}
fn sha(raw: &[u8]) -> String {
    Sha256::digest(raw)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
fn text<'a>(row: &'a Value, key: &str) -> Result<&'a str, String> {
    row[key]
        .as_str()
        .ok_or_else(|| format!("T07 main/isolation: missing {key}"))
}
#[cfg(unix)]
fn mode(path: &Path) -> Result<u32, String> {
    use std::os::unix::fs::PermissionsExt;
    Ok(fs::symlink_metadata(path)
        .map_err(|e| e.to_string())?
        .permissions()
        .mode()
        & 0o7777)
}
#[cfg(not(unix))]
fn mode(_: &Path) -> Result<u32, String> {
    Err("T07 source mode proof requires Unix".into())
}
fn regular(root: &Path, path: &str) -> Result<Vec<u8>, String> {
    let full = root.join(path);
    let metadata = fs::symlink_metadata(&full).map_err(|e| e.to_string())?;
    need(
        metadata.is_file() && !metadata.file_type().is_symlink() && mode(&full)? == 0o644,
        "regular non-symlink 0644 source",
    )?;
    fs::read(full).map_err(|e| e.to_string())
}
fn material(row: &Value, raw: &[u8]) -> Result<(), String> {
    need(
        row["sha256"] == sha(raw)
            && row["bytes"].as_u64() == Some(raw.len() as u64)
            && row["mode"] == "0644",
        "exact material identity",
    )
}
fn document(raw: &[u8]) -> Result<Value, String> {
    need(
        raw.len() == BYTES && sha(raw) == SHA,
        "fixed capsule identity",
    )?;
    let doc: Value = serde_json::from_slice(raw).map_err(|e| e.to_string())?;
    need(
        doc["kind"] == "T07_MAIN_AND_CLIENT_ISOLATION_CURRENT_OPERANDS_V1"
            && doc["source_parents"]["T07"] == "1c3a3d6cfd9b20334b46f6a73b8fb068fe78d958"
            && doc["source_parents"]["Main"] == "e9c5911a468b892fe2adef9027374cb867a2535d",
        "fixed Git parent scope",
    )?;
    let rows = doc["rows"].as_array().ok_or("T07 joint rows absent")?;
    need(
        rows.len() == PATHS.len() && rows.iter().zip(PATHS).all(|(r, p)| r["path"] == p),
        "closed ordered twelve paths",
    )?;
    Ok(doc)
}
fn load(root: &Path) -> Result<Value, String> {
    document(&regular(root, CAPSULE)?)
}
fn row<'a>(doc: &'a Value, path: &str) -> Result<&'a Value, String> {
    let index = PATHS
        .iter()
        .position(|p| *p == path)
        .ok_or("T07 joint unlisted path")?;
    Ok(&doc["rows"][index])
}
fn inverse(actual: &[u8], changes: &Value) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    let mut end = 0;
    let mut previous = None;
    for edit in changes.as_array().ok_or("T07 joint inverse absent")? {
        let at = edit["at"]
            .as_u64()
            .and_then(|n| usize::try_from(n).ok())
            .ok_or("T07 joint offset")?;
        let remove = text(edit, "remove_utf8")?.as_bytes();
        let insert = text(edit, "insert_utf8")?.as_bytes();
        let next = at
            .checked_add(remove.len())
            .ok_or("T07 joint span overflow")?;
        need(
            at >= end
                && previous.is_none_or(|p| at > p)
                && actual.get(at..next) == Some(remove)
                && (!remove.is_empty() || !insert.is_empty()),
            "ordered literal inverse span",
        )?;
        out.extend_from_slice(&actual[end..at]);
        out.extend_from_slice(insert);
        end = next;
        previous = Some(at);
    }
    out.extend_from_slice(&actual[end..]);
    Ok(out)
}
fn git_material(root: &Path, parent: &Value, raw: &[u8]) -> Result<(), String> {
    let path = text(parent, "path")?;
    let commit = text(parent, "commit")?;
    let output = Command::new("git")
        .args(["show", &format!("{commit}:{path}")])
        .current_dir(root)
        .output()
        .map_err(|e| e.to_string())?;
    need(
        output.status.success() && output.stdout == raw,
        "proved parent Git bytes",
    )?;
    let output = Command::new("git")
        .args(["ls-tree", commit, "--", path])
        .current_dir(root)
        .output()
        .map_err(|e| e.to_string())?;
    need(
        output.status.success()
            && output.stdout
                == format!("100644 blob {}\t{path}\n", text(parent, "git_blob")?).as_bytes(),
        "proved parent Git blob/mode",
    )
}
fn validate(
    row: &Value,
    actual: &[u8],
    supplied: Option<&[u8]>,
    selected: Parent,
) -> Result<Vec<u8>, String> {
    material(&row["physical"], actual)?;
    if let Some(supplied) = supplied {
        need(
            supplied == actual,
            "supplied bytes are not physical current",
        )?;
    }
    let prior = &row["parents"][match selected {
        Parent::T07 => "T07",
        Parent::Main => "Main",
    }];
    let proof = inverse(actual, &prior["inverse"])?;
    material(prior, &proof)?;
    Ok(proof)
}
/// Authenticate the same current buffer before deriving one exact accepted Git frame.
/// The four relocated files authenticate the new leaf and the old reexport/absence too.
pub(crate) fn operands(
    root: &Path,
    path: &str,
    supplied: Option<&[u8]>,
    selected: Parent,
) -> Result<(Vec<u8>, Vec<u8>), String> {
    let doc = load(root)?;
    let row = row(&doc, path)?;
    let physical = text(&row["physical"], "path")?;
    let actual = regular(root, physical)?;
    if physical != path {
        if row["old_path_state"] == "absent" {
            need(
                matches!(fs::symlink_metadata(root.join(path)), Err(e) if e.kind()==std::io::ErrorKind::NotFound),
                "relocated old leaf must remain absent",
            )?;
        } else {
            material(&row["old_path_state"], &regular(root, path)?)?;
        }
        let relocation = regular(
            root,
            "docs/rust-rewrite/t07/client-dependency-relocation.json",
        )?;
        need(
            doc["relocation_manifest_sha256"] == sha(&relocation),
            "exact relocation and seventeen-selector manifest",
        )?;
    }
    let proof_input = if path == "crates/xtask/src/main.rs" {
        if let Some(buffer) = supplied {
            need(
                buffer == actual,
                "supplied gateway main is not physical current",
            )?;
        }
        super::w07_t02_joint::snapshot_helper_amendment::gateway_main(root, &actual)?
    } else {
        actual.clone()
    };
    let proof = validate(
        row,
        &proof_input,
        if path == "crates/xtask/src/main.rs" {
            supplied.map(|_| proof_input.as_slice())
        } else {
            supplied
        },
        selected,
    )?;
    git_material(
        root,
        &row["parents"][match selected {
            Parent::T07 => "T07",
            Parent::Main => "Main",
        }],
        &proof,
    )?;
    Ok((actual, proof))
}
pub(crate) fn parent(
    root: &Path,
    path: &str,
    supplied: Option<&[u8]>,
    selected: Parent,
) -> Result<Vec<u8>, String> {
    Ok(operands(root, path, supplied, selected)?.1)
}
/// Only the relocated parser leaves and three directly affected existing
/// A06 current operands are admitted here. Archives and all other caller tuples
/// keep the original strict reader, including an unlisted hash of these paths.
pub(crate) fn a06_current_operand(
    root: &Path,
    expected: &Value,
) -> Option<Result<Vec<u8>, String>> {
    let path = expected["path"].as_str()?;
    if ![
        "crates/ssx-infra/src/adapters/source_json.rs",
        "crates/ssx-infra/src/adapters/source_json_accounting.rs",
        "crates/ssx-infra/src/adapters/source_json_future.rs",
        "crates/ssx-infra/src/adapters/source_json_diagnostic.rs",
        "crates/ssx-infra/Cargo.toml",
        "crates/xtask/Cargo.toml",
        "crates/xtask/src/a13.rs",
    ]
    .contains(&path)
    {
        return None;
    }
    let doc = match load(root) {
        Ok(doc) => doc,
        Err(e) => return Some(Err(e)),
    };
    let r = match row(&doc, path) {
        Ok(r) => r,
        Err(e) => return Some(Err(e)),
    };
    let prior = &r["parents"]["Main"];
    if expected["sha256"] != prior["sha256"] {
        return None;
    }
    Some((|| {
        need(
            expected["bytes"] == prior["bytes"] && expected["mode"] == prior["mode"],
            "selected A06 current complete tuple",
        )?;
        let proof = parent(root, path, None, Parent::Main)?;
        material(expected, &proof)?;
        Ok(proof)
    })())
}
/// Exact physical relocation for the sixteen historical parser identities and
/// the current decoded-integer control. No name is inferred from a count alone.
pub(crate) fn source_json_tests(
    root: &Path,
) -> Result<std::collections::BTreeMap<String, String>, String> {
    let doc = load(root)?;
    let raw = regular(
        root,
        "docs/rust-rewrite/t07/client-dependency-relocation.json",
    )?;
    need(
        doc["relocation_manifest_sha256"] == sha(&raw),
        "fixed relocation manifest",
    )?;
    let relocation: Value = serde_json::from_slice(&raw).map_err(|e| e.to_string())?;
    let rows = relocation["test_relocation"]
        .as_array()
        .ok_or("T07 relocation tests")?;
    need(rows.len() == 17, "exact seventeen relocated identities")?;
    let mut result = std::collections::BTreeMap::new();
    let mut physical = std::collections::BTreeSet::new();
    for row in rows {
        let old = text(row, "old_selector")?;
        let new = text(row, "new_selector")?;
        need(
            row["old_package"] == "ssx-infra"
                && row["new_package"] == "ssx-source-json"
                && old.strip_prefix("adapters::") == Some(new)
                && result.insert(old.to_owned(), new.to_owned()).is_none()
                && physical.insert(new.to_owned()),
            "exact unique source parser relocation",
        )?;
    }
    Ok(result)
}
/// A06's four immutable helper rows name the old files. The new leaves and
/// exact old reexport/absence are authenticated before their old Git frame.
pub(crate) fn a06_helper(root: &Path, path: &str, expected: &str) -> Result<(), String> {
    need(
        PATHS[8..].contains(&path),
        "exact A06 relocated helper scope",
    )?;
    let proof = parent(root, path, None, Parent::Main)?;
    need(sha(&proof) == expected, "exact A06 historical helper hash")
}
#[cfg(test)]
pub(crate) fn controls(root: &Path) -> Result<(), String> {
    let doc = load(root)?;
    source_json_tests(root)?;
    for path in PATHS {
        let row = row(&doc, path)?;
        let physical = regular(root, text(&row["physical"], "path")?)?;
        let actual = if path == "crates/xtask/src/main.rs" {
            super::w07_t02_joint::snapshot_helper_amendment::gateway_main(root, &physical)?
        } else {
            physical.clone()
        };
        for selected in [Parent::T07, Parent::Main] {
            let (reported, proof) = operands(root, path, Some(&physical), selected)?;
            need(
                reported == physical,
                "current operand reporting retains physical bytes",
            )?;
            need(
                validate(row, &actual, Some(&actual), selected)? == proof,
                "original twelve-row pure positive before mutants",
            )?;
            if actual != physical {
                need(
                    parent(root, path, Some(&actual), selected).is_err(),
                    "gateway logical main supplied as physical current",
                )?;
            }
            if proof != actual {
                need(
                    validate(row, &actual, Some(&proof), selected).is_err(),
                    "parent bytes supplied as current",
                )?;
            }
            let mut wrong = actual.clone();
            wrong.push(0);
            need(
                validate(row, &wrong, None, selected).is_err()
                    && validate(row, &actual, Some(&wrong), selected).is_err(),
                "altered current/supplied rejection",
            )?;
            let key = match selected {
                Parent::T07 => "T07",
                Parent::Main => "Main",
            };
            let changes = row["parents"][key]["inverse"]
                .as_array()
                .ok_or("T07 joint control spans")?;
            for index in 0..changes.len() {
                let mut bad = row.clone();
                bad["parents"][key]["inverse"]
                    .as_array_mut()
                    .unwrap()
                    .remove(index);
                need(
                    validate(&bad, &actual, None, selected).is_err(),
                    "missing inverse span",
                )?;
                let mut bad = row.clone();
                bad["parents"][key]["inverse"]
                    .as_array_mut()
                    .unwrap()
                    .insert(index, changes[index].clone());
                need(
                    validate(&bad, &actual, None, selected).is_err(),
                    "duplicate inverse span",
                )?;
                let mut bad = row.clone();
                bad["parents"][key]["inverse"][index]["insert_utf8"] =
                    Value::String("changed".into());
                need(
                    validate(&bad, &actual, None, selected).is_err(),
                    "altered inverse span",
                )?;
            }
            let mut bad = row.clone();
            bad["physical"]["mode"] = Value::String("0755".into());
            need(
                validate(&bad, &actual, None, selected).is_err(),
                "wrong claimed source mode",
            )?;
        }
    }
    for path in [
        "crates/ssx-infra/src/adapters/source_json.rs",
        "crates/ssx-infra/src/adapters/source_json_accounting.rs",
        "crates/ssx-infra/src/adapters/source_json_future.rs",
        "crates/ssx-infra/src/adapters/source_json_diagnostic.rs",
        "crates/ssx-infra/Cargo.toml",
        "crates/xtask/Cargo.toml",
        "crates/xtask/src/a13.rs",
    ] {
        let prior = &row(&doc, path)?["parents"]["Main"];
        let mut expected = serde_json::json!({"path":path,"sha256":prior["sha256"],"bytes":prior["bytes"],"mode":"0644"});
        need(
            a06_current_operand(root, &expected)
                .ok_or("T07 selected A06 current route")?
                .is_ok(),
            "A06 exact selected tuple positive",
        )?;
        expected["bytes"] = Value::from(0);
        need(
            a06_current_operand(root, &expected)
                .ok_or("T07 wrong tuple remained selected")?
                .is_err(),
            "A06 wrong current length",
        )?;
        expected["sha256"] = Value::String("unlisted".into());
        need(
            a06_current_operand(root, &expected).is_none(),
            "A06 unlisted caller retained strict reader",
        )?;
    }
    need(
        a06_helper(root, PATHS[8], "unlisted").is_err(),
        "A06 wrong frozen helper caller",
    )?;
    need(
        parent(root, "unlisted.rs", None, Parent::T07).is_err(),
        "unlisted current path",
    )?;
    let mut bad = regular(root, CAPSULE)?;
    bad.push(0);
    need(document(&bad).is_err(), "changed capsule")
}
