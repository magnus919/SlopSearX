//! Finite Page/main receipt-control composition; original proof assets remain intact.
//! Copied reader: main96f, SHA792876948f3885663a72373a733e7b6c312a45de0b3c2cda431fbf9fae1cbef1.
//! Actual complete source bytes authenticate before exact Git endpoint reconstruction.
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{fs, path::Path, process::Command};
type Result<T> = std::result::Result<T, String>;
const CAPSULE: &str = "docs/rust-rewrite/t06/source-amendments/receipt-control-successor.json";
const CAPSULE_SHA: &str = "d6fb5b119b7a20b810a5cf2f45bb523d882bce928329ec9b417ae804aeb728b0";
const BASELINE: &str = "e9c5911a468b892fe2adef9027374cb867a2535d";
const PATHS: &[&str] = &[
    "crates/ssx-mcp/src/bin/search-qualification.rs",
    "crates/ssx-mcp/tests/t06_support/server.rs",
    "crates/xtask/src/t06.rs",
    "crates/ssx-mcp/tests/t06_source_bindings.rs",
];
#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct Material {
    sha256: String,
    bytes: usize,
    mode: u32,
}
#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct Edit {
    current_offset: usize,
    old: String,
    new: String,
}
#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct Row {
    path: String,
    prior: Material,
    prior_git_blob: String,
    current: Material,
    edits: Vec<Edit>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Frozen {
    path: String,
    sha256: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Capsule {
    schema_version: u32,
    baseline: String,
    rows: Vec<Row>,
    frozen_predecessors: Vec<Frozen>,
}
fn need(ok: bool, message: &str) -> Result<()> {
    if ok { Ok(()) } else { Err(message.into()) }
}
fn digest(raw: &[u8]) -> String {
    Sha256::digest(raw)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
fn regular(root: &Path, path: &str) -> Result<(Vec<u8>, u32)> {
    need(
        !Path::new(path).is_absolute() && !path.split('/').any(|part| part == ".."),
        "receipt-control unsafe path",
    )?;
    let path = root.join(path);
    let meta = fs::symlink_metadata(&path).map_err(|e| e.to_string())?;
    need(
        meta.is_file() && !meta.file_type().is_symlink(),
        "receipt-control source must be regular",
    )?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        Ok((
            fs::read(path).map_err(|e| e.to_string())?,
            meta.permissions().mode() & 0o7777,
        ))
    }
    #[cfg(not(unix))]
    {
        Err("receipt-control source mode requires Unix".into())
    }
}
fn load(root: &Path) -> Result<Capsule> {
    let _ = current_load(root)?;
    let (raw, mode) = regular(root, CAPSULE)?;
    need(
        mode == 0o644 && digest(&raw) == CAPSULE_SHA,
        "receipt-control capsule changed",
    )?;
    let capsule: Capsule = serde_json::from_slice(&raw).map_err(|e| e.to_string())?;
    need(
        capsule.schema_version == 1
            && capsule.baseline == BASELINE
            && capsule
                .rows
                .iter()
                .map(|row| row.path.as_str())
                .eq(PATHS.iter().copied()),
        "receipt-control closed membership/baseline differs",
    )?;
    Ok(capsule)
}
fn material(pin: &Material, raw: &[u8], mode: u32) -> Result<()> {
    need(
        pin.mode == 0o644
            && mode == pin.mode
            && raw.len() == pin.bytes
            && digest(raw) == pin.sha256,
        "receipt-control complete source bytes/mode differ",
    )
}
fn inverse(row: &Row, current: &[u8], mode: u32) -> Result<Vec<u8>> {
    material(&row.current, current, mode)?;
    need(!row.edits.is_empty(), "receipt-control empty inverse")?;
    let mut prior = Vec::new();
    let mut end = 0;
    for edit in &row.edits {
        let stop = edit
            .current_offset
            .checked_add(edit.new.len())
            .ok_or("receipt-control offset overflow")?;
        need(
            edit.current_offset >= end
                && current.get(edit.current_offset..stop) == Some(edit.new.as_bytes()),
            "receipt-control exact ordered span differs",
        )?;
        prior.extend_from_slice(&current[end..edit.current_offset]);
        prior.extend_from_slice(edit.old.as_bytes());
        end = stop;
    }
    prior.extend_from_slice(&current[end..]);
    material(&row.prior, &prior, 0o644)?;
    Ok(prior)
}
pub(crate) fn parent(
    root: &Path,
    path: &str,
    expected: &str,
    supplied: &[u8],
) -> Result<Option<Vec<u8>>> {
    if !PATHS.contains(&path) {
        return Ok(None);
    }
    let capsule = load(root)?;
    let row = capsule
        .rows
        .iter()
        .find(|row| row.path == path)
        .ok_or("receipt-control unlisted source")?;
    if row.prior.sha256 != expected {
        return Ok(None);
    }
    let (physical, mode) = regular(root, path)?;
    need(
        supplied == physical,
        "receipt-control supplied buffer is not actual physical source",
    )?;
    let framed = current_frame(root, path, &physical, mode, CurrentBoundary::Main)?;
    material(&row.current, &framed, mode)?;
    let prior = inverse(row, &framed, mode)?;
    let blob = Command::new("git")
        .args(["cat-file", "blob", &row.prior_git_blob])
        .current_dir(root)
        .output()
        .map_err(|e| e.to_string())?;
    let tree = Command::new("git")
        .args(["ls-tree", BASELINE, "--", path])
        .current_dir(root)
        .output()
        .map_err(|e| e.to_string())?;
    need(
        blob.status.success()
            && blob.stdout == prior
            && tree.status.success()
            && tree.stdout == format!("100644 blob {}\t{path}\n", row.prior_git_blob).as_bytes(),
        "receipt-control exact predecessor Git bytes/mode/blob differ",
    )?;
    Ok(Some(prior))
}
pub(crate) fn reader_parent(
    root: &Path,
    path: &str,
    expected: &str,
    supplied: &[u8],
) -> Result<Option<Vec<u8>>> {
    if !PATHS[2..].contains(&path) {
        return Ok(None);
    }
    parent(root, path, expected, supplied)
}
pub(crate) fn controls(root: &Path) -> Result<()> {
    current_controls(root)?;
    let capsule = load(root)?;
    for frozen in &capsule.frozen_predecessors {
        let (raw, mode) = regular(root, &frozen.path)?;
        let raw = current_frame(root, &frozen.path, &raw, mode, CurrentBoundary::Main)?;
        need(
            mode == 0o644 && digest(&raw) == frozen.sha256,
            "receipt-control frozen predecessor changed",
        )?;
    }
    for row in &capsule.rows {
        let (physical, mode) = regular(root, &row.path)?;
        let framed = current_frame(root, &row.path, &physical, mode, CurrentBoundary::Main)?;
        let prior = parent(root, &row.path, &row.prior.sha256, &physical)?
            .ok_or("receipt-control positive tuple absent")?;
        need(
            inverse(row, &framed, mode)? == prior,
            "receipt-control positive inverse differs",
        )?;
        if PATHS[2..].contains(&row.path.as_str()) {
            need(
                reader_parent(root, &row.path, &row.prior.sha256, &physical)?
                    == Some(prior.clone()),
                "receipt-control exact reader route differs",
            )?;
        }
        need(
            parent(root, &row.path, &row.prior.sha256, &prior).is_err(),
            "receipt-control old bytes accepted as current",
        )?;
        need(
            inverse(row, &framed, 0o755).is_err(),
            "receipt-control wrong mode accepted",
        )?;
        let mut changed = framed.clone();
        changed[0] ^= 1;
        let mut extra = framed.clone();
        extra.push(b' ');
        for bad in [changed, extra] {
            need(
                inverse(row, &bad, mode).is_err(),
                "receipt-control changed/extra bytes accepted",
            )?;
        }
        let mut missing = row.clone();
        missing.edits.remove(0);
        need(
            inverse(&missing, &framed, mode).is_err(),
            "receipt-control missing inverse accepted",
        )?;
        let mut duplicate = row.clone();
        duplicate.edits.insert(0, row.edits[0].clone());
        need(
            inverse(&duplicate, &framed, mode).is_err(),
            "receipt-control duplicate inverse accepted",
        )?;
        let mut moved = row.clone();
        moved.edits[0].current_offset += 1;
        need(
            inverse(&moved, &framed, mode).is_err(),
            "receipt-control relocated inverse accepted",
        )?;
        need(
            parent(root, &row.path, &"0".repeat(64), &physical)?.is_none(),
            "receipt-control wrong caller hash accepted",
        )?;
    }
    need(
        parent(root, "unlisted.rs", "", b"")?.is_none(),
        "receipt-control unlisted caller accepted",
    )
}

// This closed adapter is code, not an allowlist-only waiver. Complete physical
// generation is authenticated before any frame, then exact spans and Git tuples
// reconstruct the independently accepted main and page boundaries.
const CURRENT_CAPSULE: &str =
    "docs/rust-rewrite/t06/source-amendments/page-main-receipt-control-composition-v1.json";
const CURRENT_SHA: &str = "06908948e7fba37ac52d68489e8c110c4dede6e96da6e8ee2bd07d740c41df24";
const CURRENT_MAIN: &str = "96f8541f18368e6ef9162b6163644a3ff0b12b73";
const CURRENT_PAGE: &str = "3f6329f3ecaaa425f9df0c7c2eae54aebe8aec0c";
const CURRENT_PATHS: &[&str] = &[
    "crates/xtask/src/t06.rs",
    "crates/ssx-mcp/tests/t06_source_bindings.rs",
    "docs/rust-rewrite/w07/qualification-completion-v1.json",
];
const IMMUTABLE_ASSETS: &[(&str, &str, usize)] = &[
    (
        "docs/rust-rewrite/t06/source-amendments/receipt-control-successor.rs",
        "792876948f3885663a72373a733e7b6c312a45de0b3c2cda431fbf9fae1cbef1",
        8372,
    ),
    (
        "docs/rust-rewrite/t06/source-amendments/receipt-control-successor.json",
        "d6fb5b119b7a20b810a5cf2f45bb523d882bce928329ec9b417ae804aeb728b0",
        9067,
    ),
    (
        "docs/rust-rewrite/t06/source-amendments/receipt-control-closure.rs",
        "9c9ae2b2def2310f68811ec4235c8bf859085d5ffc1448c1cadb88d356d42ceb",
        2646,
    ),
];
#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct CurrentEndpoint {
    prior: Material,
    prior_git_blob: String,
    edits: Vec<Edit>,
}
#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct CurrentRow {
    path: String,
    current: Material,
    main: CurrentEndpoint,
    page: CurrentEndpoint,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CurrentComposition {
    schema_version: u32,
    main: String,
    page: String,
    paths: Vec<String>,
    rows: Vec<CurrentRow>,
}
#[derive(Clone, Copy)]
enum CurrentBoundary {
    Main,
    Page,
}
fn current_load(root: &Path) -> Result<CurrentComposition> {
    for (path, expected, bytes) in IMMUTABLE_ASSETS {
        let (actual, mode) = regular(root, path)?;
        need(
            mode == 0o644 && actual.len() == *bytes && digest(&actual) == *expected,
            "combined receipt immutable main asset differs",
        )?;
    }
    let (raw, mode) = regular(root, CURRENT_CAPSULE)?;
    need(
        mode == 0o644 && digest(&raw) == CURRENT_SHA,
        "combined receipt capsule differs",
    )?;
    let doc: CurrentComposition = serde_json::from_slice(&raw).map_err(|e| e.to_string())?;
    need(
        doc.schema_version == 1
            && doc.main == CURRENT_MAIN
            && doc.page == CURRENT_PAGE
            && doc
                .paths
                .iter()
                .map(String::as_str)
                .eq(CURRENT_PATHS.iter().copied())
            && doc
                .rows
                .iter()
                .map(|row| row.path.as_str())
                .eq(CURRENT_PATHS.iter().copied()),
        "combined receipt closed membership/boundaries differ",
    )?;
    for row in &doc.rows {
        let (physical, mode) = regular(root, &row.path)?;
        material(&row.current, &physical, mode)?;
    }
    Ok(doc)
}
fn current_inverse(
    row: &CurrentRow,
    endpoint: &CurrentEndpoint,
    physical: &[u8],
    mode: u32,
) -> Result<Vec<u8>> {
    need(
        CURRENT_PATHS.contains(&row.path.as_str()),
        "combined receipt unlisted inverse path",
    )?;
    material(&row.current, physical, mode)?;
    if endpoint.edits.is_empty() {
        // The unchanged page registry is an explicit identity endpoint only.
        material(&endpoint.prior, physical, mode)?;
        return Ok(physical.to_vec());
    }
    inverse(
        &Row {
            path: row.path.clone(),
            prior: endpoint.prior.clone(),
            prior_git_blob: endpoint.prior_git_blob.clone(),
            current: row.current.clone(),
            edits: endpoint.edits.clone(),
        },
        physical,
        mode,
    )
}
fn current_frame(
    root: &Path,
    path: &str,
    supplied: &[u8],
    mode: u32,
    boundary: CurrentBoundary,
) -> Result<Vec<u8>> {
    if !CURRENT_PATHS.contains(&path) {
        return Ok(supplied.to_vec());
    }
    let doc = current_load(root)?;
    let row = doc
        .rows
        .iter()
        .find(|row| row.path == path)
        .ok_or("combined receipt path absent")?;
    let (physical, actual_mode) = regular(root, path)?;
    need(
        mode == actual_mode && supplied == physical,
        "combined receipt supplied bytes/mode not actual physical generation",
    )?;
    let (endpoint, revision) = match boundary {
        CurrentBoundary::Main => (&row.main, CURRENT_MAIN),
        CurrentBoundary::Page => (&row.page, CURRENT_PAGE),
    };
    let prior = current_inverse(row, endpoint, &physical, mode)?;
    let blob = Command::new("git")
        .args(["cat-file", "blob", &endpoint.prior_git_blob])
        .current_dir(root)
        .output()
        .map_err(|e| e.to_string())?;
    let tree = Command::new("git")
        .args(["ls-tree", revision, "--", path])
        .current_dir(root)
        .output()
        .map_err(|e| e.to_string())?;
    need(
        blob.status.success()
            && blob.stdout == prior
            && tree.status.success()
            && tree.stdout
                == format!("100644 blob {}\t{path}\n", endpoint.prior_git_blob).as_bytes(),
        "combined receipt accepted Git boundary differs",
    )?;
    Ok(prior)
}
fn current_controls(root: &Path) -> Result<()> {
    let doc = current_load(root)?;
    for row in &doc.rows {
        let (physical, mode) = regular(root, &row.path)?;
        for boundary in [CurrentBoundary::Main, CurrentBoundary::Page] {
            let endpoint = match boundary {
                CurrentBoundary::Main => &row.main,
                CurrentBoundary::Page => &row.page,
            };
            let prior = current_frame(root, &row.path, &physical, mode, boundary)?;
            if matches!(boundary, CurrentBoundary::Page) && PATHS[2..].contains(&row.path.as_str())
            {
                need(
                    reader_parent(root, &row.path, &endpoint.prior.sha256, &physical)?
                        == Some(prior.clone()),
                    "combined receipt page caller route differs",
                )?;
                need(
                    reader_parent(root, &row.path, &"0".repeat(64), &physical)?.is_none(),
                    "combined receipt wrong page caller hash accepted",
                )?;
                if prior != physical {
                    need(
                        reader_parent(root, &row.path, &endpoint.prior.sha256, &prior).is_err(),
                        "combined receipt historical page bytes accepted as current",
                    )?;
                }
            }
            need(
                current_inverse(row, endpoint, &physical, mode)? == prior,
                "combined receipt positive inverse differs",
            )?;
            need(
                current_frame(root, &row.path, &physical, 0o755, boundary).is_err(),
                "combined receipt wrong mode accepted",
            )?;
            let mut changed = physical.clone();
            changed[0] ^= 1;
            let mut extra = physical.clone();
            extra.push(b' ');
            for bad in [changed, extra] {
                need(
                    current_frame(root, &row.path, &bad, mode, boundary).is_err(),
                    "combined receipt changed/extra supplied bytes accepted",
                )?;
                need(
                    current_inverse(row, endpoint, &bad, mode).is_err(),
                    "combined receipt changed/extra inverse accepted",
                )?;
            }
            if prior != physical {
                need(
                    current_frame(root, &row.path, &prior, mode, boundary).is_err(),
                    "combined receipt old bytes accepted as actual",
                )?;
            }
            let mut wrong = row.clone();
            wrong.path = "unlisted.rs".into();
            need(
                current_inverse(&wrong, endpoint, &physical, mode).is_err(),
                "combined receipt unlisted inverse accepted",
            )?;
            if !endpoint.edits.is_empty() {
                let mut missing = endpoint.clone();
                missing.edits.remove(0);
                need(
                    current_inverse(row, &missing, &physical, mode).is_err(),
                    "combined receipt missing inverse accepted",
                )?;
                let mut duplicate = endpoint.clone();
                duplicate.edits.insert(0, endpoint.edits[0].clone());
                need(
                    current_inverse(row, &duplicate, &physical, mode).is_err(),
                    "combined receipt duplicate inverse accepted",
                )?;
                let mut moved = endpoint.clone();
                moved.edits[0].current_offset += 1;
                need(
                    current_inverse(row, &moved, &physical, mode).is_err(),
                    "combined receipt relocated inverse accepted",
                )?;
            }
        }
        let other = doc
            .rows
            .iter()
            .find(|other| other.path != row.path)
            .ok_or("combined receipt paired path missing")?;
        need(
            current_frame(root, &other.path, &physical, mode, CurrentBoundary::Main).is_err(),
            "combined receipt wrong physical path accepted",
        )?;
    }
    need(
        parent(root, "unlisted.rs", "", b"")?.is_none(),
        "combined receipt unlisted caller accepted",
    )
}
