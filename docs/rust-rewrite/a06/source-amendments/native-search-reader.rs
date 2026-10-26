//! The existing sealed fcac -> a1b search transition, shared by A06 and T06.
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{fs, path::Path, process::Command};
type Result<T> = std::result::Result<T, String>;
pub(crate) const SOURCE: &str = "crates/ssx-infra/src/search.rs";
const OLD: &str = "fcac2929eb6637960d5e1086c321f0eac62fc0f8446cf73f22d6b43406137b12";
const CURRENT: &str = "a1b310e2ef8692c96109bfd780c0e37d7e50692231e11eceb48e05860e0b5927";
const CAPSULE: &str = "docs/rust-rewrite/a06/source-amendments/post-a06-generation-v1.json";
const CAPSULE_SHA: &str = "3ff125fdcbca12bf6312ef875a613360319fdd11349fd2524a0fb932f31979cd";
const ARCHIVE: &str = "docs/rust-rewrite/a06/source-amendments/post-a06-233/search.rs.frozen";
const COMMIT: &str = "233c7934f9e2da5c2e94f0c1213945335d786502";
const BLOB: &str = "793b3a7edbbddead1866f72f97cc3bf5c789f9e7";
fn need(ok: bool, why: &str) -> Result<()> {
    if ok {
        Ok(())
    } else {
        Err(format!("T06 exact search successor: {why}"))
    }
}
fn digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
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
    git_tree: Vec<u8>,
}
fn read(root: &Path, path: &str) -> Result<Material> {
    let full = root.join(path);
    let meta = fs::symlink_metadata(&full).map_err(|e| format!("{path}: {e}"))?;
    need(
        meta.is_file() && !meta.file_type().is_symlink(),
        "regular non-symlink material",
    )?;
    #[cfg(unix)]
    let mode = {
        use std::os::unix::fs::PermissionsExt;
        meta.permissions().mode() & 0o7777
    };
    #[cfg(not(unix))]
    return Err("T06 source mode proof requires Unix".into());
    #[cfg(unix)]
    Ok(Material {
        bytes: fs::read(full).map_err(|e| e.to_string())?,
        mode,
    })
}
fn row(raw: &[u8]) -> Result<Value> {
    need(
        raw.len() == 126027 && digest(raw) == CAPSULE_SHA,
        "sealed existing capsule",
    )?;
    let doc: Value = serde_json::from_slice(raw).map_err(|e| e.to_string())?;
    need(
        doc["schema"] == "T02_POST_A06_CLOSED_GENERATION_V1"
            && doc["source_parent"] == COMMIT
            && doc["continuation_head"] == "4de0d641ea6e80b398f2cc12b46b2c818608bbac"
            && doc["rows"].as_array().map(Vec::len) == Some(24)
            && doc["callers"].as_array().map(Vec::len) == Some(77),
        "closed capsule membership",
    )?;
    let rows: Vec<_> = doc["rows"]
        .as_array()
        .ok_or("search rows absent")?
        .iter()
        .filter(|r| r["previous"]["path"] == SOURCE && r["previous"]["sha256"] == OLD)
        .collect();
    need(rows.len() == 1, "one exact historical search tuple")?;
    Ok(rows[0].clone())
}
fn reconstruct(prior: &[u8], edits: &Value) -> Result<Vec<u8>> {
    need(
        edits.as_array().is_some_and(|v| v.len() == 6),
        "six literal search edits",
    )?;
    ordered_edits(prior, edits)
}
fn ordered_edits(prior: &[u8], edits: &Value) -> Result<Vec<u8>> {
    let edits = edits.as_array().ok_or("search edits absent")?;
    need(!edits.is_empty(), "literal edits absent")?;
    let mut out = Vec::new();
    let mut end = 0;
    let mut previous = None;
    for e in edits {
        let at = e["at"]
            .as_u64()
            .and_then(|v| usize::try_from(v).ok())
            .ok_or("search edit offset")?;
        let remove = e["remove_utf8"]
            .as_str()
            .ok_or("search edit preimage")?
            .as_bytes();
        let insert = e["insert_utf8"]
            .as_str()
            .ok_or("search edit insertion")?
            .as_bytes();
        let next = at.checked_add(remove.len()).ok_or("search edit overflow")?;
        need(
            at >= end
                && previous.is_none_or(|p| at > p)
                && next <= prior.len()
                && prior[at..next] == *remove
                && (!remove.is_empty() || !insert.is_empty()),
            "ordered literal edit preimages",
        )?;
        out.extend_from_slice(&prior[end..at]);
        out.extend_from_slice(insert);
        end = next;
        previous = Some(at);
    }
    out.extend_from_slice(&prior[end..]);
    Ok(out)
}
fn proof(row: &Value, m: &Inputs) -> Result<()> {
    need(
        row["previous"] == json!({"path":SOURCE,"sha256":OLD,"bytes":86121,"mode":"0644"})
            && row["archive"] == json!({"path":ARCHIVE,"sha256":OLD,"bytes":86121,"mode":"0644"})
            && row["current"]
                == json!({"path":SOURCE,"sha256":CURRENT,"bytes":86610,"mode":"0644"})
            && row["source_commit"] == COMMIT
            && row["source_blob"] == BLOB,
        "exact transition operands",
    )?;
    need(
        m.archive.mode == 0o644 && m.current.mode == 0o644,
        "exact archive/current modes",
    )?;
    need(
        m.archive.bytes.len() == 86121
            && digest(&m.archive.bytes) == OLD
            && m.current.bytes.len() == 86610
            && digest(&m.current.bytes) == CURRENT,
        "exact archive/current bytes",
    )?;
    need(
        m.git_archive == m.archive.bytes
            && m.git_tree == format!("100644 blob {BLOB}\t{SOURCE}\n").as_bytes(),
        "historical Git bytes/tree/blob",
    )?;
    need(
        reconstruct(&m.archive.bytes, &row["edits"])? == m.current.bytes,
        "exact current reconstruction",
    )
}
fn load(root: &Path) -> Result<(Value, Inputs)> {
    let raw = read(root, CAPSULE)?;
    need(raw.mode == 0o644, "capsule mode")?;
    let row = row(&raw.bytes)?;
    let git = |args: &[&str]| -> Result<Vec<u8>> {
        let out = Command::new("git")
            .args(args)
            .current_dir(root)
            .output()
            .map_err(|e| e.to_string())?;
        need(out.status.success(), "Git proof command")?;
        Ok(out.stdout)
    };
    let mut m = Inputs {
        archive: read(root, ARCHIVE)?,
        current: read(root, SOURCE)?,
        git_archive: git(&["show", &format!("{COMMIT}:{SOURCE}")])?,
        git_tree: git(&["ls-tree", COMMIT, "--", SOURCE])?,
    };
    if let Some(prior) = page_prior(root, &row["current"], Some(&m.current.bytes))? {
        m.current.bytes = prior;
    }
    proof(&row, &m)?;
    Ok((row, m))
}
/// Used by the original post-A06 historical route after its sealed tuple selection.
pub(crate) fn materials_for_row(root: &Path, expected: &Value) -> Result<(Vec<u8>, Vec<u8>)> {
    let (sealed, m) = load(root)?;
    need(expected == &sealed, "exact supplied transition row")?;
    Ok((m.archive.bytes, m.current.bytes))
}
/// Only this exact T06 historical operand can use the existing physical successor.
pub(crate) fn verify(root: &Path, path: &str, expected: &str, supplied: &[u8]) -> Result<()> {
    need(path == SOURCE && expected == OLD, "exact historical caller")?;
    let raw = read(root, CAPSULE)?;
    need(raw.mode == 0o644, "capsule mode")?;
    let selected = row(&raw.bytes)?;
    let prior = page_prior(root, &selected["current"], Some(supplied))?
        .ok_or("selected search page successor missing")?;
    let (_, current) = materials_for_row(root, &selected)?;
    need(
        prior == current,
        "authenticated supplied predecessor matches sealed search proof",
    )
}
/// Additive controls called by the existing T06 identities, without new test names.
#[cfg(test)]
pub(crate) fn controls(root: &Path) -> Result<()> {
    page_controls(root)?;
    let (r, good) = load(root)?;
    let physical = read(root, SOURCE)?;
    verify(root, SOURCE, OLD, &physical.bytes)?;
    need(
        verify(root, SOURCE, OLD, &good.current.bytes).is_err(),
        "e127 proof predecessor supplied as physical current",
    )?;
    need(
        verify(root, "Cargo.lock", OLD, &good.current.bytes).is_err(),
        "wrong path accepted",
    )?;
    need(
        verify(root, SOURCE, CURRENT, &good.current.bytes).is_err(),
        "current hash as historical caller accepted",
    )?;
    need(
        verify(root, SOURCE, "wrong", &good.current.bytes).is_err(),
        "unknown caller accepted",
    )?;
    need(
        verify(root, SOURCE, OLD, &good.archive.bytes).is_err(),
        "historical material as current accepted",
    )?;
    need(row(b"{}").is_err(), "unsealed capsule accepted")?;
    for mode in [0o600, 0o4644] {
        let mut bad = good.clone();
        bad.current.mode = mode;
        need(proof(&r, &bad).is_err(), "current mode accepted")?;
        let mut bad = good.clone();
        bad.archive.mode = mode;
        need(proof(&r, &bad).is_err(), "archive mode accepted")?;
    }
    let mut bad = good.clone();
    bad.current.bytes[0] ^= 1;
    need(proof(&r, &bad).is_err(), "changed current accepted")?;
    let mut bad = good.clone();
    bad.archive.bytes[0] ^= 1;
    need(proof(&r, &bad).is_err(), "changed archive accepted")?;
    let mut bad = good.clone();
    bad.git_archive[0] ^= 1;
    need(proof(&r, &bad).is_err(), "changed Git bytes accepted")?;
    let mut bad = good.clone();
    bad.git_tree[0] ^= 1;
    need(proof(&r, &bad).is_err(), "changed Git mode/blob accepted")?;
    let mut altered_row = r.clone();
    altered_row["source_blob"] = json!("wrong blob");
    need(
        materials_for_row(root, &altered_row).is_err(),
        "altered supplied transition row accepted",
    )?;
    let mut bad = r.clone();
    bad["current"]["sha256"] = json!(OLD);
    need(proof(&bad, &good).is_err(), "changed transition accepted")?;
    let mut bad = r.clone();
    bad["edits"][0]["remove_utf8"] = json!("wrong preimage");
    need(proof(&bad, &good).is_err(), "changed preimage accepted")?;
    let mut bad = r.clone();
    bad["edits"][0]["insert_utf8"] = json!("wrong insertion");
    need(proof(&bad, &good).is_err(), "changed insertion accepted")?;
    let mut bad = r.clone();
    bad["edits"].as_array_mut().unwrap().swap(0, 1);
    need(proof(&bad, &good).is_err(), "unordered edits accepted")?;
    let mut bad = r.clone();
    bad["edits"].as_array_mut().unwrap().pop();
    need(proof(&bad, &good).is_err(), "missing edit accepted")?;
    Ok(())
}

// Eleven explicit operands: existing frames, five closed A13 sources, and
// the two audited shared/artifact transition operands.
// They are selected only after their existing historical
// callers have been authenticated. Reuse the one accepted 33-path page capsule;
// never broaden the generic A06 read_exact or reinterpret an archive as physical.
const PAGE_CAPSULE: &str = "docs/rust-rewrite/t02/source-amendments/lossless-page-1049-v1.json";
const PAGE_SHA: &str = "dfd7494b0ee42ac6a9ca702020640aaf65454b6a57f99991ae761c571dd96fcf";
const PAGE_BASE: &str = "e12744511013bd05370173e777daaef471bf9527";
const PAGE_OPERANDS: &[(&str, &str, usize)] = &[
    (SOURCE, CURRENT, 86610),
    (
        "crates/ssx-infra/tests/a13_dehashed.rs",
        "54f41684a2f7534c581bee17f562af4d6d97eacc81766d6433c2d2a998c22ed6",
        65083,
    ),
    (
        "crates/ssx-infra/tests/a08_factories.rs",
        "c0978ad1eb426b73043d82ec9691e06f12bd636829d1adbe9275953708185259",
        29122,
    ),
    (
        "crates/ssx-infra/tests/a09_factories.rs",
        "a23c6ac515cd06ca31147041d8b49e9bc0f0ca41686c0385033e1d58d6b66df7",
        37495,
    ),
    (
        "crates/ssx-domain/src/records.rs",
        "5e08c70c9adb8dc1f12562d560191ccf7b262edca73c6e6154057620e353d5bc",
        11011,
    ),
    (
        "docs/rust-rewrite/CASE_STUDY.md",
        "da92206b79b788b468fc8c0ab3b19f68fe6747d86b4f73af7d01ec362ff47a39",
        21096,
    ),
    (
        "crates/ssx-infra/tests/a13_hibp.rs",
        "0bb58e68252f427dc501c3eb996d8f6ed01d254edbfad8a06deb87346a46486c",
        20344,
    ),
    (
        "crates/ssx-infra/tests/a13_intelx.rs",
        "d1bcceec580af8f236e8cfb24bb68e3f5587c7c0bab4acb3eb5fb426a26bbb27",
        24217,
    ),
    (
        "crates/ssx-infra/src/adapters/vulnerability_tests.rs",
        "71c351bb896af5091fc0c79d50b823438456d39a05e5ad30321ce3853cc98d3f",
        36757,
    ),
    (
        "crates/ssx-infra/tests/a09_catalog_media.rs",
        "e25e5b9e756e5143cb5570f6a811bafb451122a7343a7f7a5b81d4fa8415ec43",
        12926,
    ),
    (
        "crates/ssx-infra/tests/a07_biomedical.rs",
        "f50dad891fe6f16e2b2363536a4c0c4cfd48c932ee7229730af50de8649e514b",
        34801,
    ),
];
fn page_document(root: &Path) -> Result<Value> {
    let raw = read(root, PAGE_CAPSULE)?;
    need(
        raw.mode == 0o644 && digest(&raw.bytes) == PAGE_SHA,
        "exact shared page capsule mode/hash",
    )?;
    let doc: Value = serde_json::from_slice(&raw.bytes).map_err(|e| e.to_string())?;
    need(
        doc["schema_version"] == 1
            && doc["kind"] == "T02_LOSSLESS_PAGE_1049_SOURCE_SUCCESSOR"
            && doc["base"] == PAGE_BASE
            && doc["rows"].as_array().is_some_and(|r| r.len() == 33),
        "closed shared page capsule membership",
    )?;
    Ok(doc)
}
fn page_payload(row: &Value, physical: &Material) -> Result<Vec<u8>> {
    need(
        physical.mode == 0o644
            && row["mode"] == 420
            && row["after_bytes"].as_u64() == Some(physical.bytes.len() as u64)
            && row["after_sha256"] == digest(&physical.bytes),
        "exact physical page afterimage",
    )?;
    let inverse = row["inverse"]
        .as_array()
        .ok_or("page inverse missing")?
        .iter()
        .map(|e| json!({"at":e["offset"],"remove_utf8":e["delete"],"insert_utf8":e["insert"]}))
        .collect::<Vec<_>>();
    let prior = ordered_edits(&physical.bytes, &json!(inverse))?;
    need(
        row["before_bytes"].as_u64() == Some(prior.len() as u64)
            && row["before_sha256"] == digest(&prior),
        "exact e127 page inverse",
    )?;
    Ok(prior)
}
/// Recover only these eleven exact e127 proof operands from authenticated physical
/// page successors. A supplied buffer is always compared to physical bytes first.
pub(crate) fn page_prior(
    root: &Path,
    expected: &Value,
    supplied: Option<&[u8]>,
) -> Result<Option<Vec<u8>>> {
    let path = expected["path"].as_str().ok_or("page caller path absent")?;
    let Some((_, pin, len)) = PAGE_OPERANDS.iter().find(|r| r.0 == path) else {
        return Ok(None);
    };
    need(
        *expected == json!({"path":path,"sha256":pin,"bytes":len,"mode":"0644"}),
        "exact e127 page caller tuple",
    )?;
    let physical = read(root, path)?;
    if let Some(supplied) = supplied {
        need(
            supplied == physical.bytes,
            "page supplied physical identity",
        )?;
    }
    let doc = page_document(root)?;
    let rows = doc["rows"]
        .as_array()
        .ok_or("page rows absent")?
        .iter()
        .filter(|r| r["path"] == path)
        .collect::<Vec<_>>();
    need(rows.len() == 1, "one exact page source row")?;
    let r = rows[0];
    need(
        r["before_sha256"] == *pin && r["before_bytes"] == *len,
        "page predecessor matches selected historical operand",
    )?;
    let prior = page_payload(r, &physical)?;
    for (args, wanted) in [
        (
            vec!["show".to_owned(), format!("{PAGE_BASE}:{path}")],
            prior.clone(),
        ),
        (
            vec![
                "ls-tree".to_owned(),
                PAGE_BASE.to_owned(),
                "--".to_owned(),
                path.to_owned(),
            ],
            format!(
                "100644 blob {}\t{path}\n",
                r["before_blob"].as_str().ok_or("page source blob absent")?
            )
            .into_bytes(),
        ),
    ] {
        let out = Command::new("git")
            .args(args)
            .current_dir(root)
            .output()
            .map_err(|e| e.to_string())?;
        need(
            out.status.success() && out.stdout == wanted,
            "page exact e127 Git material",
        )?;
    }
    Ok(Some(prior))
}
/// Same existing test identities exercise actual physical/new-to-e127 and legacy
/// reconstructions separately, including wrong caller/mode/edit/supplied buffers.
pub(crate) fn page_controls(root: &Path) -> Result<()> {
    let doc = page_document(root)?;
    for &(path, pin, len) in PAGE_OPERANDS {
        let expected = json!({"path":path,"sha256":pin,"bytes":len,"mode":"0644"});
        let physical = read(root, path)?;
        let prior = page_prior(root, &expected, Some(&physical.bytes))?
            .ok_or("page control not selected")?;
        need(
            digest(&prior) == pin && prior.len() == len,
            "page caller positive",
        )?;
        need(
            page_prior(root, &expected, Some(&prior)).is_err(),
            "e127 accepted as physical page source",
        )?;
        let r = doc["rows"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["path"] == path)
            .unwrap();
        for field in ["path", "sha256", "bytes", "mode"] {
            let mut bad = expected.clone();
            bad[field] = json!("wrong");
            if field == "path" {
                need(
                    page_prior(root, &bad, None)?.is_none(),
                    "unlisted page source selected",
                )?;
            } else {
                need(
                    page_prior(root, &bad, None).is_err(),
                    "altered page caller accepted",
                )?;
            }
        }
        for mode in [0o600, 0o755, 0o4644] {
            let mut bad = physical.clone();
            bad.mode = mode;
            need(
                page_payload(r, &bad).is_err(),
                "altered physical page mode accepted",
            )?;
        }
        let mut bad = physical.clone();
        bad.bytes.push(b' ');
        need(
            page_payload(r, &bad).is_err(),
            "unlisted physical page change accepted",
        )?;
        let old = Material {
            bytes: prior,
            mode: 0o644,
        };
        need(
            page_payload(r, &old).is_err(),
            "historical page operand accepted as physical",
        )?;
        for mutation in 0..3 {
            let mut bad = r.clone();
            match mutation {
                0 => bad["inverse"] = json!([]),
                1 => {
                    bad["inverse"].as_array_mut().unwrap().pop();
                }
                _ => bad["inverse"][0]["insert"] = json!("wrong"),
            }
            need(
                page_payload(&bad, &physical).is_err(),
                "altered page inverse accepted",
            )?;
        }
    }
    let sealed = page_scoped_predecessor(root)?;
    let archive = read(root, sealed["scoped_historical"]["path"].as_str().unwrap())?;
    need(archive.mode == 0o644, "scoped historical archive mode")?;
    let caller = json!({"current":sealed["current"],"historical191":sealed["scoped_historical"],"edits":scoped_byte_edits(&sealed["scoped_edits"])?});
    let next = page_scoped(root, &caller, &archive.bytes)?;
    let physical = read(root, "crates/ssx-infra/tests/a13_dehashed.rs")?;
    need(
        next["current"]["sha256"] == digest(&physical.bytes)
            && next["current"] != caller["current"],
        "scoped actual physical advancement",
    )?;
    for field in ["current", "historical191", "edits"] {
        let mut bad = caller.clone();
        bad[field] = json!(null);
        if field == "current" {
            need(
                page_scoped(root, &bad, &archive.bytes)? == bad,
                "unselected scoped caller changed",
            )?;
        } else {
            need(
                page_scoped(root, &bad, &archive.bytes).is_err(),
                "altered scoped predecessor accepted",
            )?;
        }
    }
    let mut old_as_physical = next.clone();
    old_as_physical["current"] = caller["current"].clone();
    need(
        page_scoped(root, &old_as_physical, &archive.bytes).is_err(),
        "composed edits presented as original scoped predecessor",
    )?;
    Ok(())
}

fn page_scoped_predecessor(root: &Path) -> Result<Value> {
    let raw = read(root, CAPSULE)?;
    need(raw.mode == 0o644, "scoped predecessor capsule mode")?;
    row(&raw.bytes)?; // authenticates the existing 24/77 capsule, unchanged.
    let doc: Value = serde_json::from_slice(&raw.bytes).map_err(|e| e.to_string())?;
    let matches = doc["rows"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| {
            r["current"]["path"] == "crates/ssx-infra/tests/a13_dehashed.rs"
                && r["scoped_historical"].is_object()
        })
        .collect::<Vec<_>>();
    need(matches.len() == 1, "one exact DeHashed scoped predecessor")?;
    Ok(matches[0].clone())
}
fn scoped_byte_edits(edits: &Value) -> Result<Value> {
    Ok(json!(edits.as_array().ok_or("scoped predecessor edits absent")?.iter().map(|e| {
        Ok(json!({"at":e["at"],"remove":e["remove_utf8"].as_str().ok_or("scoped remove absent")?.as_bytes(),
            "insert":e["insert_utf8"].as_str().ok_or("scoped insert absent")?.as_bytes()}))
    }).collect::<Result<Vec<Value>>>()?))
}
/// Advance only the already-selected DeHashed scoped row. Both historical
/// relations are proved first; the A04 reader still consumes actual physical
/// current bytes and a current tuple fixed by the existing page capsule.
pub(crate) fn page_scoped(root: &Path, caller: &Value, historical: &[u8]) -> Result<Value> {
    const PATH: &str = "crates/ssx-infra/tests/a13_dehashed.rs";
    if caller["current"]["path"] != PATH {
        return Ok(caller.clone());
    }
    let sealed = page_scoped_predecessor(root)?;
    need(
        caller["current"] == sealed["current"]
            && caller["historical191"] == sealed["scoped_historical"]
            && caller["edits"] == scoped_byte_edits(&sealed["scoped_edits"])?
            && sealed["scoped_historical"]["bytes"].as_u64() == Some(historical.len() as u64)
            && sealed["scoped_historical"]["sha256"] == digest(historical),
        "exact authenticated scoped predecessor caller",
    )?;
    let physical = read(root, PATH)?;
    let prior = page_prior(root, &caller["current"], Some(&physical.bytes))?
        .ok_or("scoped page successor absent")?;
    need(
        ordered_edits(historical, &sealed["scoped_edits"])? == prior,
        "original historical-to-e127 scoped relation",
    )?;
    let old_text = std::str::from_utf8(historical).map_err(|e| e.to_string())?;
    let new_text = std::str::from_utf8(&physical.bytes).map_err(|e| e.to_string())?;
    let mut prefix = historical
        .iter()
        .zip(&physical.bytes)
        .take_while(|(a, b)| a == b)
        .count();
    while !old_text.is_char_boundary(prefix) || !new_text.is_char_boundary(prefix) {
        prefix -= 1;
    }
    let mut suffix = historical[prefix..]
        .iter()
        .rev()
        .zip(physical.bytes[prefix..].iter().rev())
        .take_while(|(a, b)| a == b)
        .count();
    while !old_text.is_char_boundary(historical.len() - suffix)
        || !new_text.is_char_boundary(physical.bytes.len() - suffix)
    {
        suffix -= 1;
    }
    let literal = json!([{"at":prefix,"remove_utf8":&old_text[prefix..historical.len()-suffix],
        "insert_utf8":&new_text[prefix..physical.bytes.len()-suffix]}]);
    need(
        ordered_edits(historical, &literal)? == physical.bytes,
        "composed scoped physical relation",
    )?;
    let doc = page_document(root)?;
    let page = doc["rows"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["path"] == PATH)
        .unwrap();
    let mut next = caller.clone();
    next["current"] = json!({"path":PATH,"sha256":page["after_sha256"],"bytes":page["after_bytes"],"mode":"0644"});
    next["edits"] = scoped_byte_edits(&literal)?;
    need(
        next["current"]["sha256"] == digest(&physical.bytes)
            && next["current"]["bytes"].as_u64() == Some(physical.bytes.len() as u64),
        "scoped current equals sealed physical page tuple",
    )?;
    Ok(next)
}
