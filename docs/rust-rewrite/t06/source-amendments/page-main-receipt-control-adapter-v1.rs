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
    if path == RAW_GATEWAY {
        let (physical, mode) = regular(root, path)?;
        need(
            supplied == physical,
            "T06 raw gateway requires actual current source",
        )?;
        return raw_gateway_prior(root, &physical, mode, expected).map(Some);
    }
    if path == RMCP_TRANSPORT {
        let (physical, mode) = regular(root, path)?;
        need(
            supplied == physical,
            "rmcp transport requires actual current source",
        )?;
        return rmcp_transport_prior(&physical, mode, expected).map(Some);
    }
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
    if path == T06_REGISTRAR {
        let doc = current_load(root)?;
        if let Some(row) = doc.rows.iter().find(|row| row.path == path)
            && let Some(endpoint) = &row.qualification
            && endpoint.prior.sha256 == expected
        {
            return current_frame(root, path, supplied, 0o644, CurrentBoundary::Qualification)
                .map(Some);
        }
    }
    parent(root, path, expected, supplied)
}
pub(crate) fn controls(root: &Path) -> Result<()> {
    rmcp_transport_controls(root)?;
    raw_gateway_controls(root)?;
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
const CURRENT_SHA: &str = "1b74f37f78765d1c73fdd7ef598d844f371cacc16a7a5bb2bf6fb784fa4574cb";
const CURRENT_MAIN: &str = "96f8541f18368e6ef9162b6163644a3ff0b12b73";
const CURRENT_PAGE: &str = "3f6329f3ecaaa425f9df0c7c2eae54aebe8aec0c";
const CURRENT_QUALIFICATION: &str = "24db49cb5ba2c00cd15e7acb36242ae297983975";
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
    #[serde(default)]
    physical: Option<Material>,
    #[serde(default)]
    physical_to_current: Vec<Edit>,
    main: CurrentEndpoint,
    page: CurrentEndpoint,
    #[serde(default)]
    qualification: Option<CurrentEndpoint>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CurrentComposition {
    schema_version: u32,
    main: String,
    page: String,
    qualification: String,
    paths: Vec<String>,
    rows: Vec<CurrentRow>,
}
#[derive(Clone, Copy)]
enum CurrentBoundary {
    Main,
    Page,
    Qualification,
}

// Exact source projections for the W08/Page registry and the qualified T06
// registrar feed the unchanged Page/main proof. Each private route admits only
// its listed path, physical bytes and pinned capsule. No crate-root imports.
const W08_REGISTRY: &str = "docs/rust-rewrite/w07/qualification-completion-v1.json";
const T06_REGISTRAR: &str = "crates/xtask/src/t06.rs";
const W08_JOIN: &str = "docs/rust-rewrite/snapshot-fixture-amendment/w08-page-join-v1.json";
const W08_JOIN_SHA: &str = "2201b3d459bc0e386d3fb847a02a23f5f30fcf4b7e82ca85fd056bead5567847";
pub(crate) const W07_REGISTRY_T06_EDIT: (usize, &str, &str) = (
    19299,
    "    \"crates/xtask/src/t06.rs\": \"be30918a7ac4365b2538b58fd1c3c933e6b2bea96d6222d267c4a87d29e413e0\",\n",
    "    \"crates/xtask/src/t06.rs\": \"00a0a0ba122cd15da453cad72a025750e476db71d7ff327bef5b8646ade4cdd7\",\n",
);
pub(crate) fn w07_registry_to_w08(physical: &[u8], mode: u32) -> Result<Vec<u8>> {
    inverse(
        &Row {
            path: W08_REGISTRY.into(),
            prior: Material {
                sha256: "a651a9d36c1d5409edbcb7d79a44ac599468ca493fdd735982bb1d1894e4461d".into(),
                bytes: 21848,
                mode: 0o644,
            },
            prior_git_blob: "ead390da6eae22e2aa94f66b954efae101342151".into(),
            current: Material {
                sha256: "4d9cb197718db1d06086ee1557f1eb4953bf53b9938eb4bce41da762f70ad1a7".into(),
                bytes: 21848,
                mode: 0o644,
            },
            edits: vec![Edit {
                current_offset: W07_REGISTRY_T06_EDIT.0,
                old: W07_REGISTRY_T06_EDIT.1.into(),
                new: W07_REGISTRY_T06_EDIT.2.into(),
            }],
        },
        physical,
        mode,
    )
}
// Authenticate the exact EXP-124 reader changes before composing them with
// the accepted receipt generation. Older receipt files remain immutable.
const SCHOLARLY_READERS: &[(&str, usize, &str, &str, &str)] = &[
    (
        "crates/xtask/src/t06.rs",
        20100,
        "f726de530415b13b44d8df3b9b5d5bd04b07614e1dd27c782f04aa4cb757cd15",
        "7125a8c5245f56eea34dd2f5f6d08f082c421b14",
        "f28475c15f8c25eb159d3239f2ce0d78a523fe86bdea865a8173609af7b04a94",
    ),
    (
        "crates/ssx-mcp/tests/t06_source_bindings.rs",
        6025,
        "dbc1cbb7feb7d5d266bc919ec12e1908cf2a17597f7d2078e67b3ea5a97a11cf",
        "fce1581aa6b987ed1ee5ab5834e2c70596da5b3f",
        "6b84c5f9d1532f4ba4a66906d3031bd0b68db32ce4593bc4c240c11df32d55bf",
    ),
];

fn scholarly_reader_prior<'a>(
    root: &Path,
    path: &str,
    physical: &'a [u8],
    mode: u32,
) -> Result<std::borrow::Cow<'a, [u8]>> {
    let Some((_, bytes, sha, blob, prior_sha)) =
        SCHOLARLY_READERS.iter().find(|(name, ..)| *name == path)
    else {
        return Ok(std::borrow::Cow::Borrowed(physical));
    };
    need(
        mode == 0o644 && physical.len() == *bytes && digest(physical) == *sha,
        "EXP-124 T06 reader does not match reviewed current bytes",
    )?;
    let prior = Command::new("git")
        .args(["cat-file", "blob", blob])
        .current_dir(root)
        .output()
        .map_err(|error| error.to_string())?;
    need(
        prior.status.success() && digest(&prior.stdout) == *prior_sha,
        "EXP-124 T06 historical reader Git object differs",
    )?;
    Ok(std::borrow::Cow::Owned(prior.stdout))
}

pub(crate) fn scholarly_serializer(root: &Path, path: &str, expected: &str) -> Result<bool> {
    const SOURCE: &str = "slopsearx/mcp/result_serialization.py";
    if path != SOURCE {
        return Ok(false);
    }
    need(
        expected == "170d358b3d0b7da2b5aa61c18736ca424b3764f82c8d81d8e0d54329066fe669",
        "EXP-124 T06 historical serializer SHA differs",
    )?;
    let (current, mode) = regular(root, SOURCE)?;
    need(
        mode == 0o644
            && current.len() == 11523
            && digest(&current)
                == "347336f96e3ccfeab9dac48651a002b454c57061f9d0a5e99f76c2dd713f67b0",
        "EXP-124 T06 current serializer differs",
    )?;
    let original = Command::new("git")
        .args([
            "show",
            "6c44976444340cc8f343fef61e2e8d723b189c19:slopsearx/mcp/result_serialization.py",
        ])
        .current_dir(root)
        .output()
        .map_err(|error| error.to_string())?;
    need(
        original.status.success() && digest(&original.stdout) == expected,
        "EXP-124 T06 original serializer Git bytes differ",
    )?;
    Ok(true)
}

fn current_operand(root: &Path, row: &CurrentRow, supplied: &[u8], mode: u32) -> Result<Vec<u8>> {
    let generation = scholarly_reader_prior(root, &row.path, supplied, mode)?;
    let supplied = generation.as_ref();
    if row.path == T06_REGISTRAR {
        let physical = row
            .physical
            .as_ref()
            .ok_or("T06 physical source pin absent")?;
        need(
            !row.physical_to_current.is_empty(),
            "T06 physical projection absent",
        )?;
        return inverse(
            &Row {
                path: row.path.clone(),
                prior: row.current.clone(),
                prior_git_blob: "t06-physical-to-page-main".into(),
                current: physical.clone(),
                edits: row.physical_to_current.clone(),
            },
            supplied,
            mode,
        );
    }
    if row.path != W08_REGISTRY {
        return Ok(supplied.to_vec());
    }
    need(
        row.current.mode == 0o644
            && row.current.sha256
                == "a8b900be65b70bc6a4f382c25d0cabfb667408d81ddf793c80e2f807d268ed80",
        "exact accepted Page registry caller",
    )?;
    let (physical, actual_mode) = regular(root, W08_REGISTRY)?;
    need(
        mode == 0o644 && actual_mode == mode && physical == supplied,
        "outer receipt registry is not physical current",
    )?;
    let physical = w07_registry_to_w08(&physical, mode)?;
    let (capsule, capsule_mode) = regular(root, W08_JOIN)?;
    need(
        capsule_mode == 0o644 && digest(&capsule) == W08_JOIN_SHA,
        "outer receipt registry capsule differs",
    )?;
    let doc: serde_json::Value =
        serde_json::from_slice(&capsule).map_err(|error| error.to_string())?;
    need(
        doc["schema"] == "W08_PAGE_FOUR_OPERAND_JOIN_V1"
            && doc["parents"]
                == serde_json::json!({"w08":"8c12d02ae5e7741410e41de4795a235700fc71c2","page":"72987c50283eb70f70ff6d339c5d9f4ed8e00efd"}),
        "closed outer receipt registry parents",
    )?;
    let rows = doc["rows"].as_array().ok_or("outer receipt rows")?;
    let paths = [
        "crates/xtask/src/main.rs",
        "docs/rust-rewrite/a06/source-amendments/shared-successor.rs",
        "docs/rust-rewrite/a06/source-amendments/w07-t02-joint.rs",
        W08_REGISTRY,
    ];
    need(
        rows.len() == paths.len()
            && rows
                .iter()
                .zip(paths)
                .all(|(row, path)| row["current"]["path"] == path),
        "closed outer receipt membership",
    )?;
    let outer = rows.last().ok_or("outer receipt registry absent")?;
    let frame = &outer["page"];
    need(
        frame["current"] == outer["current"]
            && frame["current"]["path"] == W08_REGISTRY
            && frame["current"]["mode"] == "0644"
            && frame["current"]["sha256"] == digest(&physical)
            && frame["current"]["bytes"].as_u64() == Some(physical.len() as u64)
            && frame["prior"]["path"] == W08_REGISTRY
            && frame["prior"]["mode"] == "0644"
            && frame["prior"]["sha256"] == row.current.sha256
            && frame["prior"]["bytes"].as_u64() == Some(row.current.bytes as u64),
        "exact outer receipt current/prior registry",
    )?;
    let edits = frame["edits"].as_array().ok_or("outer receipt edits")?;
    let mut prior = Vec::new();
    let (mut old, mut cursor) = (0usize, 0usize);
    for edit in edits {
        let at = edit["at"]
            .as_u64()
            .and_then(|n| usize::try_from(n).ok())
            .ok_or("outer receipt offset")?;
        let remove = edit["remove_utf8"]
            .as_str()
            .ok_or("outer receipt removal")?
            .as_bytes();
        let insert = edit["insert_utf8"]
            .as_str()
            .ok_or("outer receipt insertion")?
            .as_bytes();
        need(
            !remove.is_empty() || !insert.is_empty(),
            "outer receipt vacuous edit",
        )?;
        let end = cursor
            .checked_add(at.checked_sub(old).ok_or("outer receipt edit order")?)
            .ok_or("outer receipt span overflow")?;
        prior.extend_from_slice(physical.get(cursor..end).ok_or("outer receipt prefix")?);
        let next = end
            .checked_add(insert.len())
            .ok_or("outer receipt insertion overflow")?;
        need(
            physical.get(end..next) == Some(insert),
            "outer receipt exact inserted span",
        )?;
        prior.extend_from_slice(remove);
        old = at
            .checked_add(remove.len())
            .ok_or("outer receipt removal overflow")?;
        cursor = next;
    }
    prior.extend_from_slice(physical.get(cursor..).ok_or("outer receipt suffix")?);
    material(&row.current, &prior, mode)?;
    let mut forward = Vec::new();
    let mut end = 0;
    for edit in edits {
        let at = edit["at"]
            .as_u64()
            .and_then(|n| usize::try_from(n).ok())
            .ok_or("outer receipt forward offset")?;
        let remove = edit["remove_utf8"]
            .as_str()
            .ok_or("outer receipt forward removal")?
            .as_bytes();
        let next = at
            .checked_add(remove.len())
            .ok_or("outer receipt forward overflow")?;
        need(
            at >= end && prior.get(at..next) == Some(remove),
            "outer receipt exact forward removal",
        )?;
        forward.extend_from_slice(prior.get(end..at).ok_or("outer receipt forward prefix")?);
        forward.extend_from_slice(
            edit["insert_utf8"]
                .as_str()
                .ok_or("outer receipt forward insertion")?
                .as_bytes(),
        );
        end = next;
    }
    forward.extend_from_slice(prior.get(end..).ok_or("outer receipt forward suffix")?);
    need(
        forward == physical,
        "outer receipt independent forward differs",
    )?;
    Ok(prior)
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
        doc.schema_version == 2
            && doc.main == CURRENT_MAIN
            && doc.page == CURRENT_PAGE
            && doc.qualification == CURRENT_QUALIFICATION
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
        need(
            row.physical.is_some() == row.qualification.is_some()
                && row.physical.is_some() == (row.path == T06_REGISTRAR),
            "combined receipt physical/qualification endpoint membership differs",
        )?;
        let (physical, mode) = regular(root, &row.path)?;
        if let Some(pin) = &row.physical {
            material(
                pin,
                &scholarly_reader_prior(root, &row.path, &physical, mode)?,
                mode,
            )?;
        }
        let logical = current_operand(root, row, &physical, mode)?;
        material(&row.current, &logical, mode)?;
    }
    Ok(doc)
}
fn current_inverse(
    row: &CurrentRow,
    endpoint: &CurrentEndpoint,
    physical: &[u8],
    mode: u32,
) -> Result<Vec<u8>> {
    current_inverse_from(row, endpoint, physical, mode, &row.current)
}
fn current_inverse_from(
    row: &CurrentRow,
    endpoint: &CurrentEndpoint,
    physical: &[u8],
    mode: u32,
    current: &Material,
) -> Result<Vec<u8>> {
    need(
        CURRENT_PATHS.contains(&row.path.as_str()),
        "combined receipt unlisted inverse path",
    )?;
    material(current, physical, mode)?;
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
            current: current.clone(),
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
    let logical = current_operand(root, row, &physical, mode)?;
    let generation = scholarly_reader_prior(root, path, &physical, mode)?;
    let (endpoint, revision, operand, operand_pin) = match boundary {
        CurrentBoundary::Main => (&row.main, CURRENT_MAIN, logical.as_slice(), &row.current),
        CurrentBoundary::Page => (&row.page, CURRENT_PAGE, logical.as_slice(), &row.current),
        CurrentBoundary::Qualification => (
            row.qualification
                .as_ref()
                .ok_or("T06 qualification endpoint absent")?,
            CURRENT_QUALIFICATION,
            generation.as_ref(),
            row.physical
                .as_ref()
                .ok_or("T06 physical source pin absent")?,
        ),
    };
    let prior = current_inverse_from(row, endpoint, operand, mode, operand_pin)?;
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
        let logical = current_operand(root, row, &physical, mode)?;
        if row.path == W08_REGISTRY {
            let w08_physical = w07_registry_to_w08(&physical, mode)?;
            need(
                w08_physical.len() == 21848
                    && digest(&w08_physical)
                        == "a651a9d36c1d5409edbcb7d79a44ac599468ca493fdd735982bb1d1894e4461d",
                "outer registry exact W07 generation projection differs",
            )?;
            let mut changed_pin = physical.clone();
            changed_pin[19299] ^= 1;
            need(
                w07_registry_to_w08(&changed_pin, mode).is_err(),
                "outer registry unreviewed T06 pin generation accepted",
            )?;
            need(
                logical != physical,
                "outer registry projection must be nonvacuous",
            )?;
            need(
                current_operand(root, row, &logical, mode).is_err(),
                "old Page registry supplied as physical current",
            )?;
            need(
                current_operand(root, row, &physical, 0o755).is_err(),
                "outer registry wrong mode",
            )?;
            let bytes = Command::new("git").args(["show", "72987c50283eb70f70ff6d339c5d9f4ed8e00efd:docs/rust-rewrite/w07/qualification-completion-v1.json"]).current_dir(root).output().map_err(|error|error.to_string())?;
            need(
                bytes.status.success() && bytes.stdout == logical,
                "outer registry exact accepted Page Git bytes",
            )?;
        }
        for boundary in [CurrentBoundary::Main, CurrentBoundary::Page] {
            let endpoint = match boundary {
                CurrentBoundary::Main => &row.main,
                CurrentBoundary::Page => &row.page,
                CurrentBoundary::Qualification => row
                    .qualification
                    .as_ref()
                    .ok_or("T06 qualification endpoint absent")?,
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
                current_inverse(row, endpoint, &logical, mode)? == prior,
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
            }
            let mut changed_logical = logical.clone();
            changed_logical[0] ^= 1;
            let mut extra_logical = logical.clone();
            extra_logical.push(b' ');
            for bad in [changed_logical, extra_logical] {
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
                current_inverse(&wrong, endpoint, &logical, mode).is_err(),
                "combined receipt unlisted inverse accepted",
            )?;
            if !endpoint.edits.is_empty() {
                let mut missing = endpoint.clone();
                missing.edits.remove(0);
                need(
                    current_inverse(row, &missing, &logical, mode).is_err(),
                    "combined receipt missing inverse accepted",
                )?;
                let mut duplicate = endpoint.clone();
                duplicate.edits.insert(0, endpoint.edits[0].clone());
                need(
                    current_inverse(row, &duplicate, &logical, mode).is_err(),
                    "combined receipt duplicate inverse accepted",
                )?;
                let mut moved = endpoint.clone();
                moved.edits[0].current_offset += 1;
                need(
                    current_inverse(row, &moved, &logical, mode).is_err(),
                    "combined receipt relocated inverse accepted",
                )?;
            }
        }
        if let Some(endpoint) = &row.qualification {
            let prior = current_frame(
                root,
                &row.path,
                &physical,
                mode,
                CurrentBoundary::Qualification,
            )?;
            need(
                reader_parent(root, &row.path, &endpoint.prior.sha256, &physical)?
                    == Some(prior.clone()),
                "qualification source reader route differs",
            )?;
            need(
                reader_parent(root, &row.path, &"0".repeat(64), &physical)?.is_none(),
                "qualification wrong caller hash accepted",
            )?;
            need(
                current_frame(
                    root,
                    &row.path,
                    &prior,
                    mode,
                    CurrentBoundary::Qualification,
                )
                .is_err(),
                "qualification historical bytes accepted as current physical source",
            )?;
            let physical_pin = row
                .physical
                .as_ref()
                .ok_or("qualification physical pin absent")?;
            let generation = scholarly_reader_prior(root, &row.path, &physical, mode)?;
            let mut missing = endpoint.clone();
            missing.edits.remove(0);
            need(
                current_inverse_from(row, &missing, &generation, mode, physical_pin).is_err(),
                "qualification missing inverse accepted",
            )?;
            let mut duplicate = endpoint.clone();
            duplicate.edits.insert(0, endpoint.edits[0].clone());
            need(
                current_inverse_from(row, &duplicate, &generation, mode, physical_pin).is_err(),
                "qualification duplicate inverse accepted",
            )?;
            let mut moved = endpoint.clone();
            moved.edits[0].current_offset += 1;
            need(
                current_inverse_from(row, &moved, &generation, mode, physical_pin).is_err(),
                "qualification relocated inverse accepted",
            )?;
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

// Exact security-upgrade diagnostic successor; historical captures stay frozen.
const RMCP_TRANSPORT: &str = "crates/ssx-mcp/tests/t06_transport.rs";
const RMCP_TRANSPORT_PRIOR: &str =
    "264983f5bffaba8116c7b92be99c33baa73423d99b118cc181c9729092bd4ceb";
const RMCP_TRANSPORT_CURRENT: &str =
    "399eb30c126d94673ec49a3a929acc9947fda63b57700a0154d18e8a3b9736eb";
const RMCP_TRANSPORT_OLD: &str = r###"                assert_eq!(
                    response.text().await?,
                    difference["native_body"].as_str().unwrap()
                );"###;
const RMCP_TRANSPORT_NEW: &str = r###"                // Preserve the frozen 3.5.1 observation; 3.9.0 rejects the same
                // malformed grammar with a new SDK diagnostic (HTTP415).
                assert_eq!(
                    difference["native_body"].as_str().unwrap(),
                    "fail to deserialize request body data did not match any variant of untagged enum JsonRpcMessage"
                );
                assert_eq!(
                    response.text().await?,
                    "fail to deserialize request body unrecognized JSON-RPC message"
                );"###;
fn rmcp_transport_prior(raw: &[u8], mode: u32, expected: &str) -> Result<Vec<u8>> {
    need(
        mode == 0o644
            && raw.len() == 43970
            && digest(raw) == RMCP_TRANSPORT_CURRENT
            && expected == RMCP_TRANSPORT_PRIOR,
        "rmcp transport exact current identity",
    )?;
    let text = std::str::from_utf8(raw).map_err(|e| e.to_string())?;
    need(
        text.matches(RMCP_TRANSPORT_NEW).count() == 1,
        "rmcp transport unique diagnostic span",
    )?;
    let prior = text.replacen(RMCP_TRANSPORT_NEW, RMCP_TRANSPORT_OLD, 1);
    need(
        prior.len() == 43564 && digest(prior.as_bytes()) == RMCP_TRANSPORT_PRIOR,
        "rmcp transport exact historical identity",
    )?;
    need(
        prior
            .replacen(RMCP_TRANSPORT_OLD, RMCP_TRANSPORT_NEW, 1)
            .as_bytes()
            == raw,
        "rmcp transport full forward reconstruction",
    )?;
    Ok(prior.into_bytes())
}
// Shared readers execute these negatives inside the existing closed inventory.
fn rmcp_transport_controls(root: &Path) -> Result<()> {
    let (raw, mode) = regular(root, RMCP_TRANSPORT)?;
    let prior = rmcp_transport_prior(&raw, mode, RMCP_TRANSPORT_PRIOR)?;
    need(
        rmcp_transport_prior(&prior, 0o644, RMCP_TRANSPORT_PRIOR).is_err(),
        "rmcp historical input rejected",
    )?;
    need(
        rmcp_transport_prior(&raw, 0o755, RMCP_TRANSPORT_PRIOR).is_err(),
        "rmcp altered mode rejected",
    )?;
    need(
        rmcp_transport_prior(&raw, 0o644, RMCP_TRANSPORT_CURRENT).is_err(),
        "rmcp altered expected identity rejected",
    )?;
    let mut changed = raw.clone();
    changed[0] ^= 1;
    need(
        rmcp_transport_prior(&changed, 0o644, RMCP_TRANSPORT_PRIOR).is_err(),
        "rmcp altered byte rejected",
    )?;
    changed = raw.clone();
    changed.push(b' ');
    need(
        rmcp_transport_prior(&changed, 0o644, RMCP_TRANSPORT_PRIOR).is_err(),
        "rmcp oversized input rejected",
    )?;
    Ok(())
}

// RawGateway now delegates URL validation and client construction to the
// shared exact-target client. Fix the source projection to that single edit.
const RAW_GATEWAY: &str = "crates/ssx-mcp/src/opaque.rs";
const RAW_GATEWAY_BASELINE: &str = "93da466abdb56e015474493337f537c2a6ec7b31";
const RAW_GATEWAY_BLOB: &str = "54314e969672a3c1ebac774da35ce3f634652da6";
const RAW_GATEWAY_PRIOR: &str = "c89e508d9d4d2c86f6d0f0f8cbf8f35690e83e63da94a4ca342c28da60526d41";
const RAW_GATEWAY_CURRENT: &str =
    "5800bb552fbca40f0a0bf6e11fcd8b5b5cf71aab402e2478067a22e2f45a1b17";
const RAW_GATEWAY_OLD: &str = r#"        let parsed = reqwest::Url::parse(&url)?;
        if !parsed.username().is_empty()
            || parsed.password().is_some()
            || parsed.fragment().is_some()
        {
            return Err("invalid MCP remote URL".into());
        }
        let http = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .build()?;"#;
const RAW_GATEWAY_NEW: &str = "        let http = crate::http_client::gateway_http(&url)?;";

// Startup cancellation keeps the accepted request-target change as its
// predecessor. The complete accepted and physical byte identities are pinned.
const STARTUP_BASELINE: &str = "1e18b746b45a7c2ec31be623911687767d87ed9f";
const STARTUP_PRIOR_BLOB: &str = "dfc36638d78ce22abb9b0da7ae0c90bf2784b684";
const STARTUP_CURRENT: &str = "fcbb1970c46a1394fb158a41e7a0e2d2cc2162a933e2a491ec58d1afda6ff36a";
const STARTUP_CURRENT_BYTES: usize = 87822;

const STDIO_BASELINE: &str = "316f1822ec93e0fe1dfe5939e3e9018833aa02e9";
const STDIO_PRIOR_BLOB: &str = "625c045e4f6a42dd4902e998088943bd82034805";
const STDIO_CURRENT: &str = "e2e944c50cac65e71935d6d47fcb2eb3fe444bca642c646ebac834da129def1c";
const STDIO_OLD: &str = r#"    pub async fn stdio(self) -> Result<()> {
        let wire: std::sync::Arc<std::sync::Mutex<WireState>> = Default::default();
        let handler = RawForwarder {
            gateway: self.clone(),
            wire: wire.clone(),
        };
        let result = stdio_handler(
            handler,
            std::sync::Arc::new(self.catalogs.clone()),
            wire,
            crate::output_limits::OutputLimits::HTTP,
        )
        .await;
        self.close().await?;
        result
    }
"#;
const STDIO_NEW: &str = r#"    pub async fn stdio(self) -> Result<()> {
        self.stdio_with_io(
            tokio::io::stdin(),
            tokio::io::stdout(),
            tokio_util::sync::CancellationToken::new(),
        )
        .await
    }

    /// Serve MCP over caller-owned streams and close the upstream session on exit.
    ///
    /// Cancellation stops the stdio service without waiting for input EOF.
    pub async fn stdio_with_io<I, O>(
        self,
        input: I,
        output: O,
        cancel: tokio_util::sync::CancellationToken,
    ) -> Result<()>
    where
        I: tokio::io::AsyncRead + Unpin,
        O: tokio::io::AsyncWrite + Unpin,
    {
        let wire: std::sync::Arc<std::sync::Mutex<WireState>> = Default::default();
        let stop = cancel.child_token();
        let _stop_on_drop = stop.clone().drop_guard();
        let forwarding = std::sync::Arc::new(tokio::sync::RwLock::new(()));
        let handler = RawForwarder {
            gateway: self.clone(),
            wire: wire.clone(),
            stop: stop.clone(),
            forwarding: forwarding.clone(),
        };
        let result = stdio_handler_io(
            handler,
            std::sync::Arc::new(self.catalogs.clone()),
            wire,
            crate::output_limits::OutputLimits::HTTP,
            cancel,
            input,
            output,
        )
        .await;
        // The SDK's response drain can finish before its handler tasks. Stop
        // and join their HTTP futures before retiring the shared session.
        stop.cancel();
        let _drained = forwarding.write().await;
        self.close().await?;
        result
    }
"#;
const FORWARDER_OLD: &str = r#"struct RawForwarder {
    gateway: RawGateway,
    wire: std::sync::Arc<std::sync::Mutex<WireState>>,
}
impl RawForwarder {
    fn decode<T: serde::de::DeserializeOwned>(
        value: Value,
    ) -> std::result::Result<T, rmcp::ErrorData> {
        serde_json::from_value(value)
            .map_err(|e| rmcp::ErrorData::internal_error(e.to_string(), None))
    }
    async fn forward<T: serde::de::DeserializeOwned>(
        &self,
        context: rmcp::service::RequestContext<rmcp::RoleServer>,
    ) -> std::result::Result<T, rmcp::ErrorData> {
"#;
const FORWARDER_NEW: &str = r#"struct RawForwarder {
    gateway: RawGateway,
    stop: tokio_util::sync::CancellationToken,
    forwarding: std::sync::Arc<tokio::sync::RwLock<()>>,
    wire: std::sync::Arc<std::sync::Mutex<WireState>>,
}
impl RawForwarder {
    fn decode<T: serde::de::DeserializeOwned>(
        value: Value,
    ) -> std::result::Result<T, rmcp::ErrorData> {
        serde_json::from_value(value)
            .map_err(|e| rmcp::ErrorData::internal_error(e.to_string(), None))
    }
    async fn forward<T: serde::de::DeserializeOwned>(
        &self,
        context: rmcp::service::RequestContext<rmcp::RoleServer>,
    ) -> std::result::Result<T, rmcp::ErrorData> {
        let _active = self.forwarding.read().await;
        tokio::select! {
            biased;
            _ = self.stop.cancelled() => Err(rmcp::ErrorData::internal_error("gateway stopping", None)),
            result = self.exchange_rpc(context) => result,
        }
    }

    async fn exchange_rpc<T: serde::de::DeserializeOwned>(
        &self,
        context: rmcp::service::RequestContext<rmcp::RoleServer>,
    ) -> std::result::Result<T, rmcp::ErrorData> {
"#;

const AUTH_BASELINE: &str = "40d2236234bec724c03a7e05448c411e715e46cb";
const AUTH_RECORD: &str = r#####"{
  "path": "crates/ssx-mcp/src/opaque.rs",
  "prior": {
    "sha256": "e2e944c50cac65e71935d6d47fcb2eb3fe444bca642c646ebac834da129def1c",
    "bytes": 89540,
    "mode": 420
  },
  "prior_git_blob": "b2cb99c1298edecd401f1316e7175f78cf16c833",
  "current": {
    "sha256": "bc106324406905af3e20915e02ad39f7088d3d00202b64830f35497cf683fdb1",
    "bytes": 100668,
    "mode": 420
  },
  "edits": [
    {
      "current_offset": 44043,
      "old": "",
      "new": "/// Supplies bearer credentials for an already configured remote MCP endpoint.\npub trait GatewayAuthorizer: Send + Sync {\n    /// Return the last cached bearer without doing I/O or waiting for refresh.\n    fn cached_bearer(&self) -> Option<String>;\n\n    /// Recognize an insufficient-scope challenge on HTTP 403 without doing I/O.\n    fn supports_scope_upgrade(&self, _challenges: &[reqwest::header::HeaderValue]) -> bool {\n        false\n    }\n\n    /// Respond to a 401 or a recognized 403 from the MCP endpoint. Cache any\n    /// returned bearer so concurrent requests and session cleanup can use it.\n    fn authorize<'a>(\n        &'a self,\n        status: reqwest::StatusCode,\n        sent_bearer: Option<&'a str>,\n        challenges: &'a [reqwest::header::HeaderValue],\n    ) -> futures_util::future::BoxFuture<'a, Result<Option<String>>>;\n}\n\n#[derive(Clone)]\nenum GatewayBearer {\n    Static(Option<String>),\n    Refreshable {\n        authorizer: std::sync::Arc<dyn GatewayAuthorizer>,\n        timeout: Duration,\n    },\n}\n\nimpl GatewayBearer {\n    fn cached(&self) -> Option<String> {\n        match self {\n            Self::Static(token) => token.clone(),\n            Self::Refreshable { authorizer, .. } => authorizer.cached_bearer(),\n        }\n    }\n}\n\n"
    },
    {
      "current_offset": 45386,
      "old": "    token: Option<String>,\n",
      "new": "    bearer: GatewayBearer,\n"
    },
    {
      "current_offset": 45701,
      "old": "        let deadline = tokio::time::Instant::now() + self.deadline;\n        let mut request = self\n            .http\n            .post(&self.url)\n            .header(\"accept\", \"application/json, text/event-stream\")\n            .header(\"content-type\", \"application/json\")\n            .header(\"mcp-protocol-version\", self.protocol.as_str())\n            .body(body.to_owned());\n        if let Some(token) = &self.token {\n            request = request.bearer_auth(token);\n",
      "new": "        let mut deadline = tokio::time::Instant::now() + self.deadline;\n        let request: Envelope = serde_json::from_str(body)?;\n        let attempted_session = self.session.lock().unwrap().clone();\n        let send = |bearer: Option<&str>, deadline| {\n            let mut outgoing = self\n                .http\n                .post(&self.url)\n                .header(\"accept\", \"application/json, text/event-stream\")\n                .header(\"content-type\", \"application/json\")\n                .header(\"mcp-protocol-version\", self.protocol.as_str())\n                .body(body.to_owned());\n            if let Some(token) = bearer {\n                outgoing = outgoing.bearer_auth(token);\n            }\n            if let Some(session) = &attempted_session {\n                outgoing = outgoing.header(\"mcp-session-id\", session.as_str());\n            }\n            tokio::time::timeout_at(deadline, outgoing.send())\n        };\n        let sent = self.bearer.cached();\n        let mut response = send(sent.as_deref(), deadline).await??;\n        let status = response.status();\n        if matches!(\n            status,\n            reqwest::StatusCode::UNAUTHORIZED | reqwest::StatusCode::FORBIDDEN\n        ) && request.method.as_deref() != Some(\"notifications/cancelled\")\n            && let GatewayBearer::Refreshable {\n                authorizer,\n                timeout,\n            } = &self.bearer\n        {\n            let challenges: Vec<_> = response\n                .headers()\n                .get_all(reqwest::header::WWW_AUTHENTICATE)\n                .iter()\n                .cloned()\n                .collect();\n            if status == reqwest::StatusCode::UNAUTHORIZED\n                || authorizer.supports_scope_upgrade(&challenges)\n            {\n                drop(response);\n                // Authorization may involve an interactive browser. Retain the\n                // remaining HTTP budget for replay and the resulting body.\n                let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());\n                let renewed = tokio::time::timeout(\n                    *timeout,\n                    authorizer.authorize(status, sent.as_deref(), &challenges),\n                )\n                .await?\n                .map_err(|_| \"remote MCP authorization failed\")?;\n                let token = renewed\n                    .as_deref()\n                    .filter(|token| !token.is_empty())\n                    .ok_or_else(|| format!(\"remote MCP HTTP status {status}\"))?;\n                deadline = tokio::time::Instant::now() + remaining;\n                response = send(Some(token), deadline).await??;\n            }\n"
    },
    {
      "current_offset": 48378,
      "old": "        let attempted_session = self.session.lock().unwrap().clone();\n        if let Some(session) = &attempted_session {\n            request = request.header(\"mcp-session-id\", session.as_str());\n",
      "new": "        if !response.status().is_success() {\n            return Err(format!(\"remote MCP HTTP status {}\", response.status()).into());\n"
    },
    {
      "current_offset": 48521,
      "old": "        let mut response = tokio::time::timeout_at(deadline, request.send()).await??;\n",
      "new": ""
    },
    {
      "current_offset": 49564,
      "old": "        if !response.status().is_success() {\n            return Err(format!(\"remote MCP HTTP status {}\", response.status()).into());\n        }\n",
      "new": ""
    },
    {
      "current_offset": 49983,
      "old": "        let request: Envelope = serde_json::from_str(body)?;\n",
      "new": ""
    },
    {
      "current_offset": 51658,
      "old": "",
      "new": "        Self::connect_with_bearer(url, GatewayBearer::Static(token), deadline, cancel).await\n    }\n\n    /// Connect using a cached bearer and allow one authorization retry per exchange.\n    /// Startup shares one overall budget for HTTP and interactive authorization.\n    pub async fn connect_with_authorizer(\n        url: String,\n        authorizer: std::sync::Arc<dyn GatewayAuthorizer>,\n        deadline: Duration,\n        authorization_timeout: Duration,\n        cancel: &tokio_util::sync::CancellationToken,\n    ) -> Result<Self> {\n        Self::connect_with_bearer(\n            url,\n            GatewayBearer::Refreshable {\n                authorizer,\n                timeout: authorization_timeout,\n            },\n            deadline,\n            cancel,\n        )\n        .await\n    }\n\n    async fn connect_with_bearer(\n        url: String,\n        bearer: GatewayBearer,\n        deadline: Duration,\n        cancel: &tokio_util::sync::CancellationToken,\n    ) -> Result<Self> {\n"
    },
    {
      "current_offset": 52705,
      "old": "",
      "new": "        let startup_timeout = match &bearer {\n            GatewayBearer::Static(_) => deadline,\n            GatewayBearer::Refreshable { timeout, .. } => deadline.saturating_add(*timeout),\n        };\n"
    },
    {
      "current_offset": 52973,
      "old": "            token,\n",
      "new": "            bearer,\n"
    },
    {
      "current_offset": 53186,
      "old": "        let initialize = tokio::time::timeout(deadline, async {\n",
      "new": "        let initialize = tokio::time::timeout(startup_timeout, async {\n"
    },
    {
      "current_offset": 61241,
      "old": "            if let Some(token) = &self.token {\n",
      "new": "            if let Some(token) = self.bearer.cached() {\n"
    },
    {
      "current_offset": 85864,
      "old": "                token: None,\n",
      "new": "                bearer: GatewayBearer::Static(None),\n"
    },
    {
      "current_offset": 86587,
      "old": "",
      "new": "    #[tokio::test]\n    async fn gateway_refresh_replays_rpc_once_and_keeps_control_requests_noninteractive()\n    -> Result<()> {\n        use axum::{\n            http::{Method, Request, StatusCode},\n            response::IntoResponse,\n        };\n        use std::sync::{Arc, Mutex};\n\n        const TARGET: &str = \"/keep/%2e%2E/mcp?\";\n        const RPC: &str =\n            r#\"{\"jsonrpc\":\"2.0\",\"id\":9,\"method\":\"tools/call\",\"params\":{\"name\":\"search\"}}\"#;\n        const CANCEL: &str =\n            r#\"{\"jsonrpc\":\"2.0\",\"method\":\"notifications/cancelled\",\"params\":{\"requestId\":9}}\"#;\n        const CHALLENGE: &str = r#\"Bearer error=\"invalid_token\"\"#;\n        const SCOPE: &str = r#\"Bearer error=\"insufficient_scope\", scope=\"search\"\"#;\n        const FORBIDDEN: &str =\n            r#\"{\"jsonrpc\":\"2.0\",\"id\":8,\"method\":\"tools/call\",\"params\":{\"name\":\"search\"}}\"#;\n        const SCOPED: &str =\n            r#\"{\"jsonrpc\":\"2.0\",\"id\":10,\"method\":\"tools/call\",\"params\":{\"name\":\"search\"}}\"#;\n\n        struct Authorizer(Mutex<Option<String>>);\n        impl GatewayAuthorizer for Authorizer {\n            fn cached_bearer(&self) -> Option<String> {\n                self.0.lock().unwrap().clone()\n            }\n\n            fn supports_scope_upgrade(&self, challenges: &[reqwest::header::HeaderValue]) -> bool {\n                challenges == [SCOPE]\n            }\n\n            fn authorize<'a>(\n                &'a self,\n                status: reqwest::StatusCode,\n                sent_bearer: Option<&'a str>,\n                challenges: &'a [reqwest::header::HeaderValue],\n            ) -> futures_util::future::BoxFuture<'a, Result<Option<String>>> {\n                Box::pin(async move {\n                    let replacement = match (status, sent_bearer) {\n                        (StatusCode::UNAUTHORIZED, Some(\"expired\")) => {\n                            assert_eq!(challenges, &[CHALLENGE]);\n                            // Make authorization take longer than the HTTP budget.\n                            tokio::time::sleep(Duration::from_millis(2100)).await;\n                            \"renewed\"\n                        }\n                        (StatusCode::FORBIDDEN, Some(\"renewed\")) => {\n                            assert_eq!(challenges, &[SCOPE]);\n                            \"denied\"\n                        }\n                        _ => panic!(\"unexpected credential authorization\"),\n                    };\n                    *self.0.lock().unwrap() = Some(replacement.into());\n                    Ok(Some(replacement.into()))\n                })\n            }\n        }\n\n        let (observed, mut requests) = tokio::sync::mpsc::unbounded_channel();\n        let router = axum::Router::new().fallback(move |request: Request<axum::body::Body>| {\n            let observed = observed.clone();\n            async move {\n                let (parts, body) = request.into_parts();\n                let bearer = parts.headers[\"authorization\"].to_str().unwrap().to_owned();\n                let body = axum::body::to_bytes(body, 4096).await.unwrap();\n                let body = String::from_utf8(body.to_vec()).unwrap();\n                let reply = match (parts.method.clone(), bearer.as_str(), body.as_str()) {\n                    (Method::POST, _, FORBIDDEN) => StatusCode::FORBIDDEN.into_response(),\n                    (Method::POST, _, SCOPED) => {\n                        (StatusCode::FORBIDDEN, [(\"www-authenticate\", SCOPE)]).into_response()\n                    }\n                    (Method::POST, \"Bearer renewed\", RPC) => {\n                        axum::Json(json!({\"jsonrpc\":\"2.0\",\"id\":9,\"result\":{\"content\":[]}}))\n                            .into_response()\n                    }\n                    (Method::DELETE, \"Bearer denied\", \"\") => StatusCode::NO_CONTENT.into_response(),\n                    _ => (\n                        StatusCode::UNAUTHORIZED,\n                        [\n                            (\"www-authenticate\", CHALLENGE),\n                            (\"mcp-session-id\", \"denied-session\"),\n                        ],\n                    )\n                        .into_response(),\n                };\n                observed\n                    .send((\n                        parts.method,\n                        parts.uri.path_and_query().unwrap().to_string(),\n                        parts.headers[\"mcp-session-id\"].to_str().unwrap().to_owned(),\n                        parts.headers[\"mcp-protocol-version\"]\n                            .to_str()\n                            .unwrap()\n                            .to_owned(),\n                        bearer,\n                        body,\n                    ))\n                    .unwrap();\n                reply\n            }\n        });\n        let host =\n            crate::harness::Host::start(router, tokio_util::sync::CancellationToken::new()).await?;\n        let url = format!(\"http://{}{TARGET}\", host.address);\n        let gateway = RawGateway {\n            http: crate::http_client::gateway_http(&url)?,\n            url,\n            bearer: GatewayBearer::Refreshable {\n                authorizer: Arc::new(Authorizer(Mutex::new(Some(\"expired\".into())))),\n                timeout: Duration::from_secs(3),\n            },\n            catalogs: BTreeMap::new(),\n            deadline: Duration::from_secs(2),\n            session: Arc::new(Mutex::new(Some(Arc::new(\"active-session\".into())))),\n            initialize_result: None,\n            protocol: \"2025-11-25\".into(),\n        };\n\n        assert!(gateway.exchange(CANCEL).await.is_err());\n        assert!(gateway.exchange(RPC).await?.is_some());\n        // A second RPC is denied even after replacement. Its failure ends this\n        // exchange without asking the authorizer for another bearer.\n        assert!(gateway.exchange(FORBIDDEN).await.is_err());\n        assert!(gateway.exchange(SCOPED).await.is_err());\n        gateway.close().await?;\n        assert!(gateway.session.lock().unwrap().is_none());\n\n        for (method, bearer, body) in [\n            (Method::POST, \"Bearer expired\", CANCEL),\n            (Method::POST, \"Bearer expired\", RPC),\n            (Method::POST, \"Bearer renewed\", RPC),\n            (Method::POST, \"Bearer renewed\", FORBIDDEN),\n            (Method::POST, \"Bearer renewed\", SCOPED),\n            (Method::POST, \"Bearer denied\", SCOPED),\n            (Method::DELETE, \"Bearer denied\", \"\"),\n        ] {\n            assert_eq!(\n                requests.try_recv()?,\n                (\n                    method,\n                    TARGET.into(),\n                    \"active-session\".into(),\n                    \"2025-11-25\".into(),\n                    bearer.into(),\n                    body.into(),\n                )\n            );\n        }\n        assert!(requests.try_recv().is_err());\n        Ok(())\n    }\n\n"
    },
    {
      "current_offset": 93544,
      "old": "            token: None,\n",
      "new": "            bearer: GatewayBearer::Static(None),\n"
    }
  ]
}"#####;

fn auth_source(root: &Path, raw: &[u8], mode: u32) -> Result<Vec<u8>> {
    let row: Row = serde_json::from_str(AUTH_RECORD).map_err(|error| error.to_string())?;
    let prior = inverse(&row, raw, mode)?;
    need(
        row.path == RAW_GATEWAY,
        "T06 raw gateway authorizer path differs",
    )?;
    let tree = Command::new("git")
        .args(["ls-tree", AUTH_BASELINE, "--", RAW_GATEWAY])
        .current_dir(root)
        .output()
        .map_err(|error| error.to_string())?;
    let blob = Command::new("git")
        .args(["cat-file", "blob", &row.prior_git_blob])
        .current_dir(root)
        .output()
        .map_err(|error| error.to_string())?;
    need(
        tree.status.success()
            && tree.stdout
                == format!("100644 blob {}\t{RAW_GATEWAY}\n", row.prior_git_blob).as_bytes()
            && blob.status.success()
            && blob.stdout == prior,
        "T06 raw gateway authorizer predecessor Git identity differs",
    )?;
    Ok(prior)
}

fn startup_source(raw: &[u8], mode: u32) -> Result<Vec<u8>> {
    inverse(
        &Row {
            path: RAW_GATEWAY.into(),
            prior: Material {
                sha256: STARTUP_CURRENT.into(),
                bytes: STARTUP_CURRENT_BYTES,
                mode: 0o644,
            },
            prior_git_blob: STDIO_PRIOR_BLOB.into(),
            current: Material {
                sha256: STDIO_CURRENT.into(),
                bytes: 89540,
                mode: 0o644,
            },
            edits: vec![
                Edit {
                    current_offset: 54626,
                    old: STDIO_OLD.into(),
                    new: STDIO_NEW.into(),
                },
                Edit {
                    current_offset: 57632,
                    old: FORWARDER_OLD.into(),
                    new: FORWARDER_NEW.into(),
                },
            ],
        },
        raw,
        mode,
    )
}

fn raw_gateway_prior(root: &Path, raw: &[u8], mode: u32, expected: &str) -> Result<Vec<u8>> {
    need(
        expected == RAW_GATEWAY_PRIOR,
        "T06 raw gateway historical binding differs",
    )?;
    startup_source(&auth_source(root, raw, mode)?, mode)?;
    let accepted = Command::new("git")
        .args(["cat-file", "blob", STARTUP_PRIOR_BLOB])
        .current_dir(root)
        .output()
        .map_err(|e| e.to_string())?;
    need(
        accepted.status.success(),
        "T06 raw gateway startup predecessor missing",
    )?;
    inverse(
        &Row {
            path: RAW_GATEWAY.into(),
            prior: Material {
                sha256: RAW_GATEWAY_PRIOR.into(),
                bytes: 85717,
                mode: 0o644,
            },
            prior_git_blob: RAW_GATEWAY_BLOB.into(),
            current: Material {
                sha256: RAW_GATEWAY_CURRENT.into(),
                bytes: 85398,
                mode: 0o644,
            },
            edits: vec![Edit {
                current_offset: 48167,
                old: RAW_GATEWAY_OLD.into(),
                new: RAW_GATEWAY_NEW.into(),
            }],
        },
        &accepted.stdout,
        mode,
    )
}

fn raw_gateway_controls(root: &Path) -> Result<()> {
    let (raw, mode) = regular(root, RAW_GATEWAY)?;
    let prior = raw_gateway_prior(root, &raw, mode, RAW_GATEWAY_PRIOR)?;
    let auth_predecessor = auth_source(root, &raw, mode)?;
    let stdio_predecessor = startup_source(&auth_predecessor, mode)?;
    let stdio_tree = Command::new("git")
        .args(["ls-tree", STDIO_BASELINE, "--", RAW_GATEWAY])
        .current_dir(root)
        .output()
        .map_err(|e| e.to_string())?;
    let stdio_blob = Command::new("git")
        .args(["cat-file", "blob", STDIO_PRIOR_BLOB])
        .current_dir(root)
        .output()
        .map_err(|e| e.to_string())?;
    need(
        stdio_tree.status.success()
            && stdio_tree.stdout
                == format!("100644 blob {STDIO_PRIOR_BLOB}\t{RAW_GATEWAY}\n").as_bytes()
            && stdio_blob.status.success()
            && stdio_blob.stdout == stdio_predecessor,
        "T06 raw gateway stdio predecessor Git identity differs",
    )?;
    let startup_tree = Command::new("git")
        .args(["ls-tree", STARTUP_BASELINE, "--", RAW_GATEWAY])
        .current_dir(root)
        .output()
        .map_err(|e| e.to_string())?;
    need(
        startup_tree.status.success()
            && startup_tree.stdout
                == format!("100644 blob {STARTUP_PRIOR_BLOB}\t{RAW_GATEWAY}\n").as_bytes(),
        "T06 raw gateway startup predecessor Git identity differs",
    )?;
    let tree = Command::new("git")
        .args(["ls-tree", RAW_GATEWAY_BASELINE, "--", RAW_GATEWAY])
        .current_dir(root)
        .output()
        .map_err(|e| e.to_string())?;
    let blob = Command::new("git")
        .args(["cat-file", "blob", RAW_GATEWAY_BLOB])
        .current_dir(root)
        .output()
        .map_err(|e| e.to_string())?;
    need(
        tree.status.success()
            && tree.stdout == format!("100644 blob {RAW_GATEWAY_BLOB}\t{RAW_GATEWAY}\n").as_bytes()
            && blob.status.success()
            && blob.stdout == prior,
        "T06 raw gateway exact Git predecessor differs",
    )?;
    let mut changed = raw.clone();
    changed[0] ^= 1;
    for (bytes, mode, expected) in [
        (changed.as_slice(), 0o644, RAW_GATEWAY_PRIOR),
        (prior.as_slice(), 0o644, RAW_GATEWAY_PRIOR),
        (auth_predecessor.as_slice(), 0o644, RAW_GATEWAY_PRIOR),
        (stdio_predecessor.as_slice(), 0o644, RAW_GATEWAY_PRIOR),
        (raw.as_slice(), 0o755, RAW_GATEWAY_PRIOR),
        (raw.as_slice(), 0o644, RAW_GATEWAY_CURRENT),
    ] {
        need(
            raw_gateway_prior(root, bytes, mode, expected).is_err(),
            "T06 raw gateway accepted altered bytes, mode, or historical binding",
        )?;
    }
    Ok(())
}
