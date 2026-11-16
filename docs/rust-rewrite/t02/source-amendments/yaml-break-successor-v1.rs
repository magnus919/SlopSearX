//! Exact three-source repair after 6120 qualification. Frozen HTTP oracles stay unchanged.
use super::{Inputs, Result, Transition, git, need, read, reconstruct, sha};
use serde::Deserialize;
use std::path::Path;
const CAPSULE: &str = "docs/rust-rewrite/t02/source-amendments/yaml-break-successor-v1.json";
const CAPSULE_SHA: &str = "edacbf9fff22342414237ac0c7e46c5d589ce93b8245f326f8db2f2f7e4a88a8";
const BASE: &str = "6120c8604bd96a560cda1e8b2b2374c6ab6abeb6";
const ROWS: [(&str, &str); 3] = [
    (
        "crates/ssx-http/src/formats.rs",
        "a4232f4daa997aedce09658c60b97324854e0dc6587125c216ed392310b17a15",
    ),
    (
        "crates/ssx-http/examples/t02_fixture.rs",
        "3ea961f5760bfccde11ef632b90d4766952c834fcfa97508947095342b900838",
    ),
    (
        "crates/ssx-http/tests/t02_http.rs",
        "b178e40ccaa4ebd73567df9ed1994097858237ae430f097d7dcb27efec0cfbc7",
    ),
];
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Control {
    path: String,
    sha256: String,
    bytes: usize,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Capsule {
    schema_version: u64,
    kind: String,
    coverage_sha256: String,
    baseline_commit: String,
    controls: Control,
    transitions: Vec<Transition>,
}
fn validate(c: &Capsule) -> Result<()> {
    need(
        c.schema_version == 1
            && c.kind == "T02_YAML_BREAK_SUCCESSOR_V1"
            && c.coverage_sha256 == super::COVERAGE_SHA
            && c.baseline_commit == BASE
            && c.transitions.len() == ROWS.len(),
        "T02 YAML exact capsule header/scope",
    )?;
    need(
        c.controls.path == "docs/rust-rewrite/t02/yaml-break-source-controls-v1.json"
            && c.controls.sha256
                == "5e00f07a6b613a8d2bca60010dac90499cc1d09698a764d3c466abcc4223051c"
            && c.controls.bytes == 15077,
        "T02 YAML source controls identity",
    )?;
    for (t, (path, pin)) in c.transitions.iter().zip(ROWS) {
        need(
            t.path == path
                && t.old_sha256 == pin
                && t.mode == 0o644
                && t.baseline_commit == BASE
                && !t.edits.is_empty()
                && t.archive
                    == format!(
                        "docs/rust-rewrite/t02/source-amendments/yaml-break-6120/{}.frozen",
                        Path::new(path).file_name().unwrap().to_string_lossy()
                    ),
            "T02 YAML exact transition/caller scope",
        )?;
    }
    Ok(())
}
fn load(root: &Path) -> Result<Capsule> {
    let raw = read(root, CAPSULE)?;
    need(
        raw.mode == 0o644 && sha(&raw.bytes) == CAPSULE_SHA,
        "T02 YAML capsule mode/hash",
    )?;
    let c: Capsule = serde_json::from_slice(&raw.bytes).map_err(|e| e.to_string())?;
    validate(&c)?;
    let controls = read(root, &c.controls.path)?;
    need(
        controls.mode == 0o644
            && controls.bytes.len() == c.controls.bytes
            && sha(&controls.bytes) == c.controls.sha256,
        "T02 YAML source controls material",
    )?;
    Ok(c)
}
fn inputs(root: &Path, c: &Capsule) -> Result<Vec<Inputs>> {
    c.transitions
        .iter()
        .map(|t| {
            Ok(Inputs {
                archive: read(root, &t.archive)?,
                current: read(root, &t.path)?,
                git_archive: git(root, &["show", &format!("{BASE}:{}", t.path)])?,
                git_tree: String::from_utf8(git(root, &["ls-tree", BASE, "--", &t.path])?)
                    .map_err(|e| e.to_string())?,
                current_blob: String::from_utf8(git(root, &["hash-object", "--", &t.path])?)
                    .map_err(|e| e.to_string())?,
            })
        })
        .collect()
}
fn materials(c: &Capsule, items: &[Inputs]) -> Result<()> {
    need(items.len() == ROWS.len(), "T02 YAML material scope")?;
    for (t, m) in c.transitions.iter().zip(items) {
        let checked = m.archive.mode == 0o644
            && m.current.mode == 0o644
            && m.archive.bytes.len() == t.old_bytes
            && sha(&m.archive.bytes) == t.old_sha256
            && m.git_archive == m.archive.bytes
            && m.git_tree == format!("100644 blob {}\t{}\n", t.baseline_blob, t.path)
            && m.current.bytes.len() == t.current_bytes
            && sha(&m.current.bytes) == t.current_sha256
            && m.current_blob == format!("{}\n", t.current_blob);
        need(
            checked,
            &format!("T02 YAML exact source material differs:{}", t.path),
        )?;
        need(
            reconstruct(&m.archive.bytes, &t.edits)? == m.current.bytes,
            &format!("T02 YAML ordered reconstruction differs:{}", t.path),
        )?;
    }
    Ok(())
}
pub(super) fn verify(root: &Path, path: &str, old: &str, coverage: &str) -> Option<Result<()>> {
    let (_, pin) = ROWS.iter().find(|(p, _)| *p == path)?;
    Some((|| {
        need(
            old == *pin && coverage == super::COVERAGE_SHA,
            "T02 YAML exact caller",
        )?;
        let c = load(root)?;
        materials(&c, &inputs(root, &c)?)
    })())
}
pub(super) fn controls(root: &Path) -> Result<()> {
    let c = load(root)?;
    let good = inputs(root, &c)?;
    materials(&c, &good)?;
    for (path, pin) in ROWS {
        need(
            verify(root, path, pin, super::COVERAGE_SHA).is_some_and(|r| r.is_ok()),
            "T02 YAML positive route",
        )?;
        need(
            verify(root, path, "wrong", super::COVERAGE_SHA).is_some_and(|r| r.is_err()),
            "T02 YAML wrong caller accepted",
        )?;
        need(
            verify(root, path, pin, "wrong").is_some_and(|r| r.is_err()),
            "T02 YAML wrong container accepted",
        )?;
    }
    need(
        verify(root, "unlisted", "wrong", super::COVERAGE_SHA).is_none(),
        "T02 YAML unlisted route selected",
    )?;
    for index in 0..good.len() {
        for case in 0..8 {
            let mut bad = good.clone();
            match case {
                0 => bad[index].archive.bytes[0] ^= 1,
                1 => bad[index].current.bytes.push(b'x'),
                2 => bad[index].archive.mode = 0o600,
                3 => bad[index].current.mode = 0o4644,
                4 => bad[index].git_archive[0] ^= 1,
                5 => bad[index].git_tree.push('x'),
                6 => bad[index].current_blob.push('x'),
                _ => bad[index].current = bad[index].archive.clone(),
            }
            need(
                materials(&c, &bad).is_err(),
                "T02 YAML material mutation accepted",
            )?;
        }
        let mut bad = good.clone();
        bad.remove(index);
        need(
            materials(&c, &bad).is_err(),
            "T02 YAML missing material accepted",
        )?;
        for case in 0..4 {
            let mut edits = c.transitions[index].edits.clone();
            match case {
                0 => {
                    edits.pop();
                }
                1 => edits[0].delete = "wrong preimage".into(),
                2 => edits.insert(1, edits[0].clone()),
                _ => edits.reverse(),
            }
            // A single-edit reversal is the identity, not a negative mutation.
            if case == 3 && edits.len() == 1 {
                continue;
            }
            need(
                !reconstruct(&good[index].archive.bytes, &edits)
                    .is_ok_and(|r| r == good[index].current.bytes),
                "T02 YAML edit mutation accepted",
            )?;
        }
    }
    println!(
        "T02_YAML_BREAK_SOURCE_CONTROLS three:Git:ordered-edits:modes:closed-caller:source-controls"
    );
    Ok(())
}
