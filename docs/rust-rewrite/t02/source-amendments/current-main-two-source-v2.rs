//! Finite source-only T02 f7 -> accepted-main integration proof; no original execution.
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{fs, path::Path, process::Command};
#[path = "yaml-break-successor-v1.rs"]
mod yaml_break;
type Result<T> = std::result::Result<T, String>;
const CAPSULE: &str = "docs/rust-rewrite/t02/source-amendments/current-main-two-source-v2.json";
const CAPSULE_SHA: &str = "6297b5145efd8e1761b7198ac7c0bfe1a3d854f0a34baf1b4fdb35576f056a30";
const COVERAGE_SHA: &str = "5ce1ab45a2e2006daa03ebf4a4fcd9ed62d9d59b8490ddc8f5eb82b19fc37f28";
const PR: &str = "f7cb4ad1e6d8a72b053cf09cfffb29cc607d9827";
const MAIN: &str = "51fda0f70e2051e145478783ca9c32e1f4de9596";
struct Binding {
    path: &'static str,
    baseline_blob: &'static str,
    archive: &'static str,
    old_sha256: &'static str,
    old_bytes: usize,
    current_sha256: &'static str,
    current_bytes: usize,
    current_blob: &'static str,
}
const BINDINGS: [Binding; 2] = [
    Binding {
        path: "crates/ssx-infra/src/search.rs",
        baseline_blob: "2f34a38467884895319ac7837c72a04708aee73b",
        archive: "docs/rust-rewrite/t02/source-amendments/f7cb4ad1/crates/ssx-infra/src/search.rs",
        old_sha256: "e5b4047c9a8b4a965eea97f643f8dcdec9bdd7c19b42b2e3a2f6f74a65746c6a",
        old_bytes: 83421,
        current_sha256: "a1b310e2ef8692c96109bfd780c0e37d7e50692231e11eceb48e05860e0b5927",
        current_bytes: 86610,
        current_blob: "f4ad63519cc8d62136dda49db3ad8087c6f7b5ea",
    },
    Binding {
        path: "Cargo.lock",
        baseline_blob: "6246845bf0e379123b31aba5fba3bab79415be2e",
        archive: "docs/rust-rewrite/t02/source-amendments/f7cb4ad1/Cargo.lock",
        old_sha256: "5633a2999f932cc1e91a451c3bc6b6ff68baa9d9fcbe68f8b5d5dfbae3fb8a35",
        old_bytes: 87455,
        current_sha256: "96e9a7760d163fc5ea2afa23033e58cfb7e59d0f2d93f66d8847ef352e0b765d",
        current_bytes: 87558,
        current_blob: "ec42b15cc6bd863230688af502921fd3f0947778",
    },
];
#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct Edit {
    offset: usize,
    delete: String,
    insert: String,
}
#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct Transition {
    path: String,
    baseline_commit: String,
    baseline_blob: String,
    archive: String,
    old_sha256: String,
    old_bytes: usize,
    current_sha256: String,
    current_bytes: usize,
    mode: u32,
    edits: Vec<Edit>,
    current_blob: String,
}
#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct Capsule {
    schema_version: u64,
    kind: String,
    coverage_sha256: String,
    baseline_pr: String,
    incoming_main: String,
    qualification: String,
    transitions: Vec<Transition>,
}
#[derive(Clone)]
struct Material {
    bytes: Vec<u8>,
    mode: u32,
}
#[derive(Clone)]
struct Inputs {
    archive: Material,
    current: Material,
    git_archive: Vec<u8>,
    git_tree: String,
    current_blob: String,
}
fn need(ok: bool, why: &str) -> Result<()> {
    if ok { Ok(()) } else { Err(why.into()) }
}
fn sha(raw: &[u8]) -> String {
    Sha256::digest(raw)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
fn permission_bits(raw: u32) -> u32 {
    raw & 0o7777
}
fn read(root: &Path, path: &str) -> Result<Material> {
    let full = root.join(path);
    let metadata = fs::symlink_metadata(&full).map_err(|e| format!("{path}:{e}"))?;
    need(
        metadata.is_file() && !metadata.file_type().is_symlink(),
        "T02 material is not regular",
    )?;
    #[cfg(unix)]
    let mode = {
        use std::os::unix::fs::PermissionsExt;
        permission_bits(metadata.permissions().mode())
    };
    #[cfg(not(unix))]
    return Err("T02 source mode proof requires Unix".into());
    #[cfg(unix)]
    Ok(Material {
        bytes: fs::read(full).map_err(|e| format!("{path}:{e}"))?,
        mode,
    })
}
fn decode(raw: &[u8], pin: &str) -> Result<Capsule> {
    need(sha(raw) == pin, "T02 successor capsule digest")?;
    let c: Capsule = serde_json::from_slice(raw).map_err(|e| e.to_string())?;
    need(
        c.schema_version == 1
            && c.kind == "T02_CURRENT_MAIN_TWO_SOURCE_SUCCESSOR_V2"
            && c.coverage_sha256 == COVERAGE_SHA
            && c.baseline_pr == PR
            && c.incoming_main == MAIN
            && c.qualification == "SOURCE_ONLY_CURRENT_MAIN_INTEGRATION_NATIVE_PENDING",
        "T02 successor header",
    )?;
    need(
        c.transitions.len() == BINDINGS.len(),
        "T02 successor closed scope",
    )?;
    for (t, b) in c.transitions.iter().zip(&BINDINGS) {
        need(
            t.path == b.path
                && t.baseline_commit == PR
                && t.baseline_blob == b.baseline_blob
                && t.archive == b.archive
                && t.old_sha256 == b.old_sha256
                && t.old_bytes == b.old_bytes
                && t.current_sha256 == b.current_sha256
                && t.current_bytes == b.current_bytes
                && t.current_blob == b.current_blob
                && t.mode == 0o644,
            "T02 successor exact binding",
        )?;
    }
    Ok(c)
}
fn git(root: &Path, args: &[&str]) -> Result<Vec<u8>> {
    let result = Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .map_err(|e| e.to_string())?;
    need(result.status.success(), "T02 source Git proof command")?;
    Ok(result.stdout)
}
fn collect(
    c: &Capsule,
    mut load: impl FnMut(&str) -> Result<Material>,
    mut git_proof: impl FnMut(&Transition) -> Result<(Vec<u8>, String, String)>,
) -> Result<Vec<Inputs>> {
    c.transitions
        .iter()
        .map(|t| {
            let archive = load(&t.archive)?;
            let current = load(&t.path)?;
            let (git_archive, git_tree, current_blob) = git_proof(t)?;
            Ok(Inputs {
                archive,
                current,
                git_archive,
                git_tree,
                current_blob,
            })
        })
        .collect()
}
// These are the frozen T02 parent-frame operands. Physical joint identity is
// proved first; the projected Git blob is never evidence of the physical file.
fn load_inputs(root: &Path, c: &Capsule) -> Result<Vec<Inputs>> {
    collect(
        c,
        |p| {
            let mut material = read(root, p)?;
            if BINDINGS.iter().any(|binding| binding.path == p) {
                material.bytes = crate::a13::a06_source_successor::w07_t02_joint::parent(
                    root,
                    p,
                    &material.bytes,
                    crate::a13::a06_source_successor::w07_t02_joint::Parent::T02,
                )?;
            }
            Ok(material)
        },
        |t| {
            let old = git(root, &["cat-file", "blob", &t.baseline_blob])?;
            let tree = String::from_utf8(git(root, &["ls-tree", PR, "--", &t.path])?)
                .map_err(|e| e.to_string())?;
            let physical = read(root, &t.path)?;
            let parent_blob = crate::a13::a06_source_successor::w07_t02_joint::parent_git_blob(
                root,
                &t.path,
                &physical.bytes,
                crate::a13::a06_source_successor::w07_t02_joint::Parent::T02,
            )?;
            let proved_parent_blob_line = format!("{parent_blob}\n");
            Ok((old, tree, proved_parent_blob_line))
        },
    )
}
fn reconstruct(old: &[u8], edits: &[Edit]) -> Result<Vec<u8>> {
    let text = std::str::from_utf8(old).map_err(|e| e.to_string())?;
    need(!edits.is_empty(), "T02 successor edits missing")?;
    let mut cursor = 0;
    let mut previous = None;
    let mut out = Vec::new();
    for e in edits {
        let end = e
            .offset
            .checked_add(e.delete.len())
            .ok_or("T02 edit overflow")?;
        need(
            e.offset >= cursor
                && previous.is_none_or(|p| e.offset > p)
                && end <= old.len()
                && text.is_char_boundary(e.offset)
                && text.is_char_boundary(end),
            "T02 edit order or boundary",
        )?;
        need(
            !e.delete.is_empty() || !e.insert.is_empty(),
            "T02 empty edit",
        )?;
        need(
            old[e.offset..end] == *e.delete.as_bytes(),
            "T02 edit preimage",
        )?;
        out.extend_from_slice(&old[cursor..e.offset]);
        out.extend_from_slice(e.insert.as_bytes());
        cursor = end;
        previous = Some(e.offset);
    }
    out.extend_from_slice(&old[cursor..]);
    Ok(out)
}
fn verify_materials(c: &Capsule, items: &[Inputs]) -> Result<()> {
    need(items.len() == BINDINGS.len(), "T02 material scope")?;
    for (t, m) in c.transitions.iter().zip(items) {
        need(
            m.archive.mode == 0o644 && m.current.mode == 0o644,
            "T02 material mode",
        )?;
        need(
            m.archive.bytes.len() == t.old_bytes && sha(&m.archive.bytes) == t.old_sha256,
            "T02 archive identity",
        )?;
        need(
            m.git_archive == m.archive.bytes
                && m.git_tree == format!("100644 blob {}\t{}\n", t.baseline_blob, t.path),
            "T02 historical Git identity",
        )?;
        let rebuilt = reconstruct(&m.archive.bytes, &t.edits)?;
        need(rebuilt == m.current.bytes, "T02 reconstruction differs")?;
        need(
            m.current.bytes.len() == t.current_bytes
                && sha(&m.current.bytes) == t.current_sha256
                && m.current_blob == format!("{}\n", t.current_blob),
            "T02 current identity",
        )?;
    }
    Ok(())
}
/// None is reserved for the other forty-one artifacts; a selected failure never falls through.
pub(super) fn verify(root: &Path, path: &str, old_sha: &str, coverage: &str) -> Option<Result<()>> {
    if let Some(result) = yaml_break::verify(root, path, old_sha, coverage) {
        return Some(result);
    }
    let b = BINDINGS.iter().find(|b| b.path == path)?;
    Some((|| {
        need(
            coverage == COVERAGE_SHA && old_sha == b.old_sha256,
            "T02 exact caller",
        )?;
        let raw = read(root, CAPSULE)?;
        need(raw.mode == 0o644, "T02 capsule mode")?;
        let c = decode(&raw.bytes, CAPSULE_SHA)?;
        verify_materials(&c, &load_inputs(root, &c)?)
    })())
}
/// Mandatory production gate controls, also called by the existing unit identity.
pub(super) fn controls(root: &Path) -> Result<()> {
    yaml_break::controls(root)?;
    let raw = read(root, CAPSULE)?;
    need(raw.mode == 0o644, "T02 capsule mode")?;
    let c = decode(&raw.bytes, CAPSULE_SHA)?;
    let good = load_inputs(root, &c)?;
    verify_materials(&c, &good)?;
    for (index, transition) in c.transitions.iter().enumerate() {
        let physical = read(root, &transition.path)?;
        need(
            physical.bytes != good[index].current.bytes,
            "T02 physical joint and parent operands were conflated",
        )?;
        need(
            crate::a13::a06_source_successor::w07_t02_joint::parent(
                root,
                &transition.path,
                &good[index].current.bytes,
                crate::a13::a06_source_successor::w07_t02_joint::Parent::T02,
            )
            .is_err(),
            "T02 parent bytes accepted as physical joint current",
        )?;
        need(
            crate::a13::a06_source_successor::w07_t02_joint::parent_git_blob(
                root,
                &transition.path,
                &good[index].current.bytes,
                crate::a13::a06_source_successor::w07_t02_joint::Parent::T02,
            )
            .is_err(),
            "T02 parent supplied as physical joint Git witness",
        )?;
        let mut wrong = good.clone();
        wrong[index].current.bytes = physical.bytes;
        need(
            verify_materials(&c, &wrong).is_err(),
            "T02 physical bytes accepted in projected parent slot",
        )?;
        let mut wrong = good.clone();
        wrong[index].current_blob =
            String::from_utf8(git(root, &["hash-object", "--", &transition.path])?)
                .map_err(|e| e.to_string())?;
        need(
            verify_materials(&c, &wrong).is_err(),
            "T02 physical Git blob accepted in projected parent slot",
        )?;
    }
    for b in &BINDINGS {
        need(
            verify(root, b.path, b.old_sha256, COVERAGE_SHA).is_some_and(|r| r.is_ok()),
            "T02 current positive",
        )?;
        need(
            verify(root, b.path, "wrong", COVERAGE_SHA).is_some_and(|r| r.is_err()),
            "T02 wrong caller pin accepted",
        )?;
        need(
            verify(root, b.path, b.old_sha256, "wrong").is_some_and(|r| r.is_err()),
            "T02 wrong caller container accepted",
        )?;
    }
    need(
        verify(root, "crates/ssx-infra/src/http.rs", "wrong", COVERAGE_SHA).is_none(),
        "T02 unlisted route selected",
    )?;
    need(
        decode(&raw.bytes, "wrong").is_err(),
        "T02 missing digest control",
    )?;
    for index in 0..good.len() {
        for mode in [0o600, permission_bits(0o104644)] {
            need(mode != 0o644, "T02 special mode extraction")?;
            let mut bad = good.clone();
            bad[index].current.mode = mode;
            need(
                verify_materials(&c, &bad).is_err(),
                "T02 current mode accepted",
            )?;
            let mut bad = good.clone();
            bad[index].archive.mode = mode;
            need(
                verify_materials(&c, &bad).is_err(),
                "T02 archive mode accepted",
            )?;
        }
        let mut bad = good.clone();
        bad[index].archive.bytes[0] ^= 1;
        need(
            verify_materials(&c, &bad).is_err(),
            "T02 altered archive accepted",
        )?;
        let mut bad = good.clone();
        bad[index].archive = bad[index].current.clone();
        need(
            verify_materials(&c, &bad).is_err(),
            "T02 current as old accepted",
        )?;
        let mut bad = good.clone();
        bad[index].git_tree = "wrong tree entry\n".into();
        need(
            verify_materials(&c, &bad).is_err(),
            "T02 wrong baseline tree accepted",
        )?;
        let mut bad = good.clone();
        bad[index].current = bad[index].archive.clone();
        need(
            verify_materials(&c, &bad).is_err(),
            "T02 old as current accepted",
        )?;
        let mut bad = good.clone();
        bad[index].git_archive[0] ^= 1;
        need(
            verify_materials(&c, &bad).is_err(),
            "T02 wrong Git archive accepted",
        )?;
        let mut bad = good.clone();
        bad[index].current_blob = "wrong\n".into();
        need(
            verify_materials(&c, &bad).is_err(),
            "T02 wrong current Git blob accepted",
        )?;
        let mut bad = good.clone();
        bad[index].current.bytes.push(b'x');
        need(
            verify_materials(&c, &bad).is_err(),
            "T02 unrelated current change accepted",
        )?;
        for missing in [&c.transitions[index].archive, &c.transitions[index].path] {
            let loaded = collect(
                &c,
                |p| {
                    if p == missing.as_str() {
                        return Err("intentional missing material".into());
                    }
                    for (t, m) in c.transitions.iter().zip(&good) {
                        if p == t.path {
                            return Ok(m.current.clone());
                        }
                        if p == t.archive {
                            return Ok(m.archive.clone());
                        }
                    }
                    Err("unlisted control material".into())
                },
                |t| {
                    let i = c
                        .transitions
                        .iter()
                        .position(|row| row.path == t.path)
                        .ok_or("missing control row")?;
                    Ok((
                        good[i].git_archive.clone(),
                        good[i].git_tree.clone(),
                        good[i].current_blob.clone(),
                    ))
                },
            );
            need(loaded.is_err(), "T02 missing material accepted")?;
        }
    }
    let original: serde_json::Value =
        serde_json::from_slice(&raw.bytes).map_err(|e| e.to_string())?;
    // Rehash test-only mutated capsules so these reach the structural guards.
    let rehashed = |v: &serde_json::Value| -> Result<Capsule> {
        let bytes = serde_json::to_vec(v).map_err(|e| e.to_string())?;
        decode(&bytes, &sha(&bytes))
    };
    for case in 0..15 {
        let mut v = original.clone();
        match case {
            0 => {
                v["transitions"].as_array_mut().ok_or("control rows")?.pop();
            }
            1 => {
                let r = v["transitions"][0].clone();
                v["transitions"]
                    .as_array_mut()
                    .ok_or("control rows")?
                    .push(r);
            }
            2 => v["transitions"]
                .as_array_mut()
                .ok_or("control rows")?
                .swap(0, 1),
            3 => v["transitions"][0]["path"] = "unlisted".into(),
            4 => v["transitions"][0]["archive"] = "unlisted".into(),
            5 => v["transitions"][0]["baseline_blob"] = "wrong".into(),
            6 => v["transitions"][0]["current_sha256"] = "wrong".into(),
            7 => v["transitions"][0]["mode"] = 0o4644.into(),
            8 => v["coverage_sha256"] = "wrong".into(),
            9 => v["unexpected"] = true.into(),
            10 => v["transitions"][0]["baseline_commit"] = MAIN.into(),
            11 => v["transitions"][0]["old_sha256"] = "wrong".into(),
            12 => v["transitions"][0]["old_bytes"] = 0.into(),
            13 => v["transitions"][0]["current_blob"] = "wrong".into(),
            _ => v["kind"] = "wrong".into(),
        }
        need(
            rehashed(&v).is_err(),
            "T02 rehashed binding mutation accepted",
        )?;
    }
    // A current afterimage plus a matching rehashed payload is not a new permitted source.
    let mut third = original.clone();
    let mut changed_lock = good[1].current.bytes.clone();
    changed_lock.extend_from_slice(
        b"\n[[package]]\nname = \"unlisted-third-package\"\nversion = \"1.0.0\"\n",
    );
    third["transitions"][1]["current_sha256"] = sha(&changed_lock).into();
    third["transitions"][1]["current_bytes"] = changed_lock.len().into();
    need(
        rehashed(&third).is_err(),
        "T02 rehashed third package accepted",
    )?;
    for index in 0..BINDINGS.len() {
        for case in 0..5 {
            let mut v = original.clone();
            let edits = v["transitions"][index]["edits"]
                .as_array_mut()
                .ok_or("control edits")?;
            match case {
                0 => { edits.pop(); },
                1 => { let e = edits[0].clone(); edits.insert(1,e); },
                2 => edits.swap(0,1),
                3 => edits[0]["delete"] = "wrong preimage".into(),
                _ => edits.push(serde_json::json!({"offset":good[index].archive.bytes.len(),"delete":"","insert":"x"})),
            }
            let mutated = rehashed(&v)?;
            need(
                verify_materials(&mutated, &good).is_err(),
                "T02 rehashed edit mutation accepted",
            )?;
        }
    }
    println!(
        "T02_CURRENT_MAIN_SOURCE_SUCCESSOR_CONTROLS two:archives:git:ordered-edits:modes:closed-caller"
    );
    Ok(())
}
