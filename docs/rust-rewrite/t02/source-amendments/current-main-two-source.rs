//! Finite source-only T02 f7 -> accepted-main integration proof; no original execution.
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{fs, path::Path, process::Command};
type Result<T> = std::result::Result<T, String>;
const CAPSULE: &str = "docs/rust-rewrite/t02/source-amendments/current-main-two-source-v1.json";
const CAPSULE_SHA: &str = "1ec7cda3b896f602078d50156792ae22c3140329bf1cc608383a411983bf1200";
const COVERAGE_SHA: &str = "49c113e42009501981348382d8b866e8379579f47f6bd63b25aad93405cc4b71";
const PR: &str = "f7cb4ad1e6d8a72b053cf09cfffb29cc607d9827";
const MAIN: &str = "50de34d821cc94d538ac5cf1c5149552da15222d";
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
        current_sha256: "4aec7730d0c6dd7d7620c983d7decab67b9a6aa249066e3db2af107553600331",
        current_bytes: 86609,
        current_blob: "403f287b856eba2b6a05968c453d394456eba605",
    },
    Binding {
        path: "Cargo.lock",
        baseline_blob: "6246845bf0e379123b31aba5fba3bab79415be2e",
        archive: "docs/rust-rewrite/t02/source-amendments/f7cb4ad1/Cargo.lock",
        old_sha256: "5633a2999f932cc1e91a451c3bc6b6ff68baa9d9fcbe68f8b5d5dfbae3fb8a35",
        old_bytes: 87455,
        current_sha256: "6fb367877c645aaa4ee0fe078b630b180e08096074c35412c8c5932abd64a3f0",
        current_bytes: 87456,
        current_blob: "738fad8fb323ca10a1bb787c35541d59dc4d4ea8",
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
            && c.kind == "T02_CURRENT_MAIN_TWO_SOURCE_SUCCESSOR_V1"
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
fn load_inputs(root: &Path, c: &Capsule) -> Result<Vec<Inputs>> {
    collect(
        c,
        |p| read(root, p),
        |t| {
            let old = git(root, &["cat-file", "blob", &t.baseline_blob])?;
            let tree = String::from_utf8(git(root, &["ls-tree", PR, "--", &t.path])?)
                .map_err(|e| e.to_string())?;
            let current = String::from_utf8(git(root, &["hash-object", "--", &t.path])?)
                .map_err(|e| e.to_string())?;
            Ok((old, tree, current))
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
    let raw = read(root, CAPSULE)?;
    need(raw.mode == 0o644, "T02 capsule mode")?;
    let c = decode(&raw.bytes, CAPSULE_SHA)?;
    let good = load_inputs(root, &c)?;
    verify_materials(&c, &good)?;
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
