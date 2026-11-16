//! Three exact historical caller bindings for one portable test-fixture selection.
//! Historical captures/fixtures stay byte-exact; this proves current file custody
//! before reconstructing the old observation operand. It never executes Python.
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Read, Write},
    path::Path,
    process::{Command, Stdio},
};
type Result<T> = std::result::Result<T, String>;
const SOURCE: &str = "tests/test_coverage_pipeline_inputs.py";
const CAPSULE: &str = "docs/rust-rewrite/snapshot-fixture-amendment/capsule.json";
include!("capsule-sha.rs");
fn need(condition: bool, message: &str) -> Result<()> {
    if condition {
        Ok(())
    } else {
        Err(message.into())
    }
}
fn digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
fn document(raw: &[u8]) -> Result<Value> {
    need(
        digest(raw) == SHA,
        "snapshot fixture sealed capsule differs",
    )?;
    let doc: Value = serde_json::from_slice(raw).map_err(|e| e.to_string())?;
    need(
        doc["schema"] == "PORTABLE_SNAPSHOT_FIXTURE_SOURCE_V1"
            && doc["baseline_commit"] == "e12744511013bd05370173e777daaef471bf9527"
            && doc["bindings"]
                .as_array()
                .is_some_and(|rows| rows.len() == 3)
            && doc["source"]["prior"]["path"] == SOURCE
            && doc["source"]["current"]["path"] == SOURCE,
        "snapshot fixture capsule closed scope differs",
    )?;
    Ok(doc)
}
#[cfg(unix)]
fn regular_with_bound(root: &Path, relative: &str, limit: usize) -> Result<Vec<u8>> {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    let path = root.join(relative);
    let before = fs::symlink_metadata(&path).map_err(|e| e.to_string())?;
    need(
        before.is_file() && before.permissions().mode() & 0o7777 == 0o644,
        "snapshot fixture requires regular0644 source",
    )?;
    let file = fs::File::open(path).map_err(|e| e.to_string())?;
    let opened = file.metadata().map_err(|e| e.to_string())?;
    need(
        opened.is_file()
            && opened.permissions().mode() & 0o7777 == 0o644
            && before.dev() == opened.dev()
            && before.ino() == opened.ino(),
        "snapshot fixture source custody changed",
    )?;
    let mut raw = Vec::new();
    file.take(
        limit
            .checked_add(1)
            .ok_or("snapshot fixture read limit overflow")? as u64,
    )
    .read_to_end(&mut raw)
    .map_err(|e| e.to_string())?;
    need(
        raw.len() <= limit && raw.len() as u64 == opened.len(),
        "snapshot fixture bounded source read differs",
    )?;
    Ok(raw)
}
#[cfg(not(unix))]
fn regular_with_bound(_: &Path, _: &str, _: usize) -> Result<Vec<u8>> {
    Err("snapshot fixture custody requires Unix".into())
}
fn regular(root: &Path, relative: &str) -> Result<Vec<u8>> {
    regular_with_bound(root, relative, 1_048_576)
}
fn load(root: &Path) -> Result<Value> {
    document(&regular(root, CAPSULE)?)
}
fn material(row: &Value, raw: &[u8], mode: &str) -> Result<()> {
    need(
        row["mode"] == mode
            && row["bytes"].as_u64() == Some(raw.len() as u64)
            && row["sha256"] == digest(raw),
        "snapshot fixture material identity differs",
    )
}
fn input_and_wait(mut child: std::process::Child, raw: &[u8]) -> Result<std::process::Output> {
    // Closing stdin and joining the owned child precede every error return.
    let write_result: Result<()> = (|| {
        let mut input = child
            .stdin
            .take()
            .ok_or("snapshot fixture Git stdin absent")?;
        input.write_all(raw).map_err(|e| e.to_string())
    })();
    if write_result.is_err() {
        let _ = child.kill();
    }
    let completed = child.wait_with_output().map_err(|e| e.to_string());
    write_result?;
    completed
}
fn git_blob(root: &Path, raw: &[u8]) -> Result<String> {
    let child = Command::new("git")
        .args(["hash-object", "--stdin"])
        .current_dir(root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| e.to_string())?;
    let output = input_and_wait(child, raw)?;
    need(
        output.status.success(),
        "snapshot fixture Git blob calculation failed",
    )?;
    Ok(String::from_utf8(output.stdout)
        .map_err(|e| e.to_string())?
        .trim()
        .to_owned())
}
fn source_payload(root: &Path, source: &Value, current: &[u8], mode: &str) -> Result<Vec<u8>> {
    need(
        source["prior"]["path"] == SOURCE && source["current"]["path"] == SOURCE,
        "snapshot fixture source path differs",
    )?;
    material(&source["current"], current, mode)?;
    need(
        source["current"]["git_blob"] == git_blob(root, current)?,
        "snapshot fixture current Git blob differs",
    )?;
    let at = source["edit"]["at"]
        .as_u64()
        .and_then(|n| usize::try_from(n).ok())
        .ok_or("snapshot fixture offset absent")?;
    let old = source["edit"]["remove_utf8"]
        .as_str()
        .ok_or("snapshot fixture old line absent")?
        .as_bytes();
    let new = source["edit"]["insert_utf8"]
        .as_str()
        .ok_or("snapshot fixture new line absent")?
        .as_bytes();
    let end = at
        .checked_add(new.len())
        .ok_or("snapshot fixture edit span overflow")?;
    need(
        !old.is_empty() && !new.is_empty() && current.get(at..end) == Some(new),
        "snapshot fixture exact replacement span differs",
    )?;
    let mut prior = current[..at].to_vec();
    prior.extend_from_slice(old);
    prior.extend_from_slice(&current[end..]);
    material(&source["prior"], &prior, "0644")?;
    need(
        source["prior"]["git_blob"] == git_blob(root, &prior)?,
        "snapshot fixture historical Git blob differs",
    )?;
    let mut forward = prior[..at].to_vec();
    forward.extend_from_slice(new);
    forward.extend_from_slice(&prior[at + old.len()..]);
    need(
        forward == current,
        "snapshot fixture forward reconstruction differs",
    )?;
    Ok(prior)
}
fn binding<'a>(doc: &'a Value, kind: &str) -> Result<&'a Value> {
    let rows: Vec<_> = doc["bindings"]
        .as_array()
        .ok_or("snapshot fixture bindings absent")?
        .iter()
        .filter(|row| row["kind"] == kind)
        .collect();
    need(
        rows.len() == 1,
        "snapshot fixture caller missing or duplicated",
    )?;
    Ok(rows[0])
}
fn prove(root: &Path, doc: &Value, kind: &str, claimed: &Value) -> Result<Vec<u8>> {
    let row = binding(doc, kind)?;
    need(
        row["row"] == *claimed,
        "snapshot fixture exact caller shape or pin differs",
    )?;
    let path = row["container"]["path"]
        .as_str()
        .ok_or("snapshot fixture container path absent")?;
    // The sealed A06 frame is 1,201,634 bytes; source/capsule reads retain
    // their 1 MiB ceiling while these three exact containers allow at most 2 MiB.
    let raw = regular_with_bound(root, path, 2_097_152)?;
    material(&row["container"], &raw, "0644")?;
    let container: Value = serde_json::from_slice(&raw).map_err(|e| e.to_string())?;
    need(
        container.pointer(
            row["pointer"]
                .as_str()
                .ok_or("snapshot fixture pointer absent")?,
        ) == Some(&row["row"]),
        "snapshot fixture original caller row moved or changed",
    )?;
    source_payload(root, &doc["source"], &regular(root, SOURCE)?, "0644")
}
pub(crate) fn f01(root: &Path, path: &str, blob: &str) -> Result<Option<()>> {
    if path != SOURCE {
        return Ok(None);
    }
    let doc = load(root)?;
    prove(root, &doc, "F01", &json!({"path":path,"git_blob":blob}))?;
    Ok(Some(()))
}
pub(crate) fn a06(root: &Path, row: &Value) -> Result<Option<()>> {
    if row["path"] != SOURCE {
        return Ok(None);
    }
    let doc = load(root)?;
    prove(root, &doc, "A06", row)?;
    Ok(Some(()))
}
pub(crate) fn w03(root: &Path, row: &Value) -> Result<Option<Vec<u8>>> {
    if row["path"] != SOURCE {
        return Ok(None);
    }
    let doc = load(root)?;
    prove(root, &doc, "W03", row).map(Some)
}
#[cfg(test)]
mod tests {
    use super::*;
    fn root() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
    }
    #[test]
    fn snapshot_fixture_amendment_finite_container_and_source_read_bounds() {
        let root = root();
        let doc = load(&root).unwrap();
        let frame = &binding(&doc, "A06").unwrap()["container"];
        let path = frame["path"].as_str().unwrap();
        let size = frame["bytes"].as_u64().unwrap() as usize;
        assert!(size > 1_048_576 && size < 2_097_152);
        assert!(regular(&root, path).is_err());
        assert!(regular_with_bound(&root, path, size - 1).is_err());
        let actual = regular_with_bound(&root, path, size).unwrap();
        material(frame, &actual, "0644").unwrap();
        let size = doc["source"]["current"]["bytes"].as_u64().unwrap() as usize;
        assert!(regular_with_bound(&root, SOURCE, size - 1).is_err());
        assert_eq!(
            regular_with_bound(&root, SOURCE, size).unwrap(),
            regular(&root, SOURCE).unwrap()
        );
    }
    #[cfg(unix)]
    #[test]
    fn snapshot_fixture_amendment_failed_pipe_write_reaps_owned_git_child() {
        let child = Command::new("git")
            .args([
                "hash-object",
                "--deliberately-invalid-snapshot-fixture-option",
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let pid = child.id();
        assert!(input_and_wait(child, &vec![0; 1_048_576]).is_err());
        let alive = Command::new("/bin/kill")
            .args(["-0", &pid.to_string()])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .unwrap()
            .success();
        assert!(!alive, "failed Git writer retained an unreaped child");
    }
    #[test]
    fn snapshot_fixture_amendment_actual_f01_source_consumer() {
        crate::source_check(&root(), "F01", &mut vec![]).unwrap();
    }
    #[test]
    fn snapshot_fixture_amendment_closed_callers_and_source_controls() {
        let root = root();
        let doc = load(&root).unwrap();
        let current = regular(&root, SOURCE).unwrap();
        let prior = source_payload(&root, &doc["source"], &current, "0644").unwrap();
        assert!(source_payload(&root, &doc["source"], &prior, "0644").is_err());
        for mode in ["0600", "0755", "4644"] {
            assert!(source_payload(&root, &doc["source"], &current, mode).is_err());
        }
        for kind in ["F01", "A06", "W03"] {
            let row = &binding(&doc, kind).unwrap()["row"];
            assert_eq!(prove(&root, &doc, kind, row).unwrap(), prior);
            for field in ["extra", "sha256", "git_blob", "bytes", "mode"] {
                let mut bad = row.clone();
                bad[field] = json!("wrong");
                assert!(prove(&root, &doc, kind, &bad).is_err());
            }
            let mut wrong = doc.clone();
            let index = wrong["bindings"]
                .as_array()
                .unwrap()
                .iter()
                .position(|r| r["kind"] == kind)
                .unwrap();
            wrong["bindings"][index]["pointer"] = json!("/sources/0");
            assert!(prove(&root, &wrong, kind, row).is_err());
            wrong["bindings"][index]["container"]["sha256"] = json!("wrong");
            assert!(prove(&root, &wrong, kind, row).is_err());
            let mut duplicate = doc.clone();
            duplicate["bindings"]
                .as_array_mut()
                .unwrap()
                .push(binding(&doc, kind).unwrap().clone());
            assert!(binding(&duplicate, kind).is_err());
            let mut missing = doc.clone();
            missing["bindings"]
                .as_array_mut()
                .unwrap()
                .retain(|r| r["kind"] != kind);
            assert!(binding(&missing, kind).is_err());
        }
        let mut changed = current.clone();
        changed.push(b'\n');
        let mut rehashed = doc["source"].clone();
        rehashed["current"]["sha256"] = json!(digest(&changed));
        rehashed["current"]["bytes"] = json!(changed.len());
        rehashed["current"]["git_blob"] = json!(git_blob(&root, &changed).unwrap());
        assert!(source_payload(&root, &rehashed, &changed, "0644").is_err());
        let at = doc["source"]["edit"]["at"].as_u64().unwrap() as usize;
        let line = doc["source"]["edit"]["insert_utf8"]
            .as_str()
            .unwrap()
            .as_bytes();
        for mutation in 0..3 {
            let mut changed = current.clone();
            match mutation {
                0 => {
                    changed.drain(at..at + line.len());
                }
                1 => {
                    changed.splice(at..at, line.iter().copied());
                }
                _ => {
                    changed.drain(at..at + line.len());
                    changed.extend_from_slice(line);
                }
            }
            let mut candidate = doc["source"].clone();
            candidate["current"]["sha256"] = json!(digest(&changed));
            candidate["current"]["bytes"] = json!(changed.len());
            candidate["current"]["git_blob"] = json!(git_blob(&root, &changed).unwrap());
            assert!(source_payload(&root, &candidate, &changed, "0644").is_err());
        }
        assert!(f01(&root, "unlisted.py", "wrong").unwrap().is_none());
        assert!(
            a06(&root, &json!({"path":"unlisted.py"}))
                .unwrap()
                .is_none()
        );
        assert!(
            w03(&root, &json!({"path":"unlisted.py"}))
                .unwrap()
                .is_none()
        );
        let mut raw = regular(&root, CAPSULE).unwrap();
        raw.push(b'\n');
        assert!(document(&raw).is_err());
        assert!(load(Path::new("/missing-snapshot-fixture-root")).is_err());
    }
}
