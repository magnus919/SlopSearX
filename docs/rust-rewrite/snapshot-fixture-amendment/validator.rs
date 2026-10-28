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
// Ready059de changes exactly three F01 inventory anchors. Authenticate that
// physical container before the original snapshot caller proof consumes its frame.
const F01_CONTAINER: &str = "crates/xtask/fixtures/F01.json";
const F01_CONTAINER_EDITS: [(usize, &str, &str); 3] = [
    (
        16292,
        "      \"git_blob\": \"73fedcbf810ee7d8a2d0af8b115f2296f0b15008\"\n",
        "      \"git_blob\": \"3c6f652482d0a2acef948bf6462aa9af1f324947\"\n",
    ),
    (
        17121,
        "      \"git_blob\": \"6234517809ced8386354de2140dcb41ec76cb534\"\n",
        "      \"git_blob\": \"da94757effefc9f38fa80db8f75b3537c5c40fbc\"\n",
    ),
    (
        17264,
        "      \"git_blob\": \"f1f1666e803a55076e6892677d4f52df0a515cec\"\n",
        "      \"git_blob\": \"a007f68f91fffc97448bf238e61d3866ec20e0ef\"\n",
    ),
];
fn f01_container_inverse(
    frame: &Value,
    current: &[u8],
    mode: &str,
    edits: &[(usize, &str, &str)],
) -> Result<Vec<u8>> {
    need(
        *frame
            == json!({"path":F01_CONTAINER,"sha256":"eeb28862df43b91b1eb6d5e83255e9b1a1c1935b144b5cafcb0ed59fec3239a3","bytes":64639,"git_blob":"e0af06245b7dab8d5716005b0ddb6fcab44b8017","mode":"0644"}),
        "snapshot F01 exact historical container frame differs",
    )?;
    need(
        mode == "0644"
            && current.len() == 64639
            && digest(current)
                == "b0c9e59c57c81e97619b9a3432e7ca44926bb173bc9a8b82a6cab84b9d1d585e",
        "snapshot F01 exact Ready059de physical container differs",
    )?;
    need(edits.len() == 3, "snapshot F01 exact three inventory edits")?;
    let mut prior = current.to_vec();
    let mut end = 0;
    for (at, before, after) in edits {
        let next = at
            .checked_add(after.len())
            .ok_or("F01 container span overflow")?;
        need(
            *at >= end
                && !before.is_empty()
                && before.len() == after.len()
                && current.get(*at..next) == Some(after.as_bytes()),
            "snapshot F01 exact ordered replacement span differs",
        )?;
        prior
            .get_mut(*at..next)
            .ok_or("F01 container prior span")?
            .copy_from_slice(before.as_bytes());
        end = next;
    }
    material(frame, &prior, "0644")?;
    let mut forward = prior.clone();
    for (at, _, after) in edits {
        forward[*at..*at + after.len()].copy_from_slice(after.as_bytes());
    }
    need(
        forward == current,
        "snapshot F01 independent forward proof differs",
    )?;
    Ok(prior)
}
fn f01_container_proof(root: &Path, frame: &Value, supplied: &[u8]) -> Result<Vec<u8>> {
    let physical = regular(root, F01_CONTAINER)?;
    need(
        physical == supplied,
        "snapshot F01 supplied container is not physical current",
    )?;
    need(
        git_blob(root, &physical)? == "94a7de321d13f0efc051ab1e63f155d72207e95a",
        "snapshot F01 current container Git blob differs",
    )?;
    let prior = f01_container_inverse(frame, &physical, "0644", &F01_CONTAINER_EDITS)?;
    need(
        frame["git_blob"] == git_blob(root, &prior)?,
        "snapshot F01 prior container Git blob differs",
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
    let raw = if kind == "F01" {
        f01_container_proof(root, &row["container"], &raw)?
    } else {
        raw
    };
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
        let frame = &binding(&doc, "F01").unwrap()["container"];
        let physical = regular(&root, F01_CONTAINER).unwrap();
        let historical = f01_container_proof(&root, frame, &physical).unwrap();
        let git = Command::new("git")
            .args([
                "show",
                "8e71a83f9649d58e9a60e34810275b736e44151a:crates/xtask/fixtures/F01.json",
            ])
            .current_dir(&root)
            .output()
            .unwrap();
        assert!(git.status.success());
        assert_eq!(historical, git.stdout);
        assert_eq!(
            f01_container_inverse(frame, &physical, "0644", &F01_CONTAINER_EDITS).unwrap(),
            historical
        );
        assert!(f01_container_proof(&root, frame, &historical).is_err());
        for mode in ["0600", "0755", "4644"] {
            assert!(f01_container_inverse(frame, &physical, mode, &F01_CONTAINER_EDITS).is_err());
        }
        for field in ["path", "sha256", "bytes", "mode", "git_blob", "extra"] {
            let mut wrong = frame.clone();
            wrong[field] = json!("wrong");
            assert!(
                f01_container_inverse(&wrong, &physical, "0644", &F01_CONTAINER_EDITS).is_err()
            );
        }
        for mutation in 0..5 {
            let mut edits = F01_CONTAINER_EDITS.to_vec();
            match mutation {
                0 => {
                    edits.remove(0);
                }
                1 => edits.push(edits[0]),
                2 => edits.swap(0, 1),
                3 => edits[0].0 += 1,
                _ => edits[0].1 = "wrong",
            }
            assert!(f01_container_inverse(frame, &physical, "0644", &edits).is_err());
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let fixture = std::env::temp_dir()
                .join(format!("ssx-snapshot-f01-container-{}", std::process::id()));
            struct Owned(std::path::PathBuf);
            impl Drop for Owned {
                fn drop(&mut self) {
                    let _ = fs::remove_dir_all(&self.0);
                }
            }
            // Exclusive root acquisition precedes every write and cleanup ownership.
            fs::create_dir(&fixture).unwrap();
            let owned = Owned(fixture);
            let fixture = &owned.0;
            fs::create_dir_all(fixture.join("crates/xtask/fixtures")).unwrap();
            let path = fixture.join(F01_CONTAINER);
            fs::write(&path, &physical).unwrap();
            fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
            assert_eq!(
                f01_container_proof(fixture, frame, &physical).unwrap(),
                historical
            );
            for mode in [0o600, 0o755, 0o4644] {
                fs::set_permissions(&path, fs::Permissions::from_mode(mode)).unwrap();
                assert!(f01_container_proof(fixture, frame, &physical).is_err());
            }
            fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
            fs::write(&path, &historical).unwrap();
            assert!(f01_container_proof(fixture, frame, &historical).is_err());
            fs::write(&path, &physical).unwrap();
            let mut changed = physical.clone();
            changed.push(0);
            assert!(f01_container_proof(fixture, frame, &changed).is_err());
            fs::write(&path, &changed).unwrap();
            assert!(f01_container_proof(fixture, frame, &changed).is_err());
            drop(owned);
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
