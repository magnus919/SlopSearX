//! Exact T08 dependency and source successor edges; historical endpoints remain fixed.
#[allow(
    clippy::duplicate_mod,
    reason = "Independent proof roots share this exact inverse"
)]
#[path = "../a11/replay-scheduling-source.rs"]
mod a11_replay_source;
#[allow(
    clippy::duplicate_mod,
    reason = "Independent proof roots share the stateless exact-byte inverse"
)]
#[path = "rmcp-source-afterimage.rs"]
mod source_afterimage;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{path::Path, process::Command};
type Result<T> = std::result::Result<T, String>;
const BASE: &str = "7d904b9d46f4c4ba8c59dc1ba09828f9e7ea204d";
const CAPSULE: &[u8] = include_bytes!("issue903-config-edge-successor-v1.json");
const CAPSULE_SHA: &str = "35d67057cfc0ce960706f5b19ca473116d9ae2eac2f2a5e0c6d4466363f554c6";
const MAX_PHYSICAL_BYTES: u64 = 1_048_576;
// The gateway codec adds two edges to packages already present in Cargo.lock.
const T07_LOCK_BYTES: usize = 96228;
const T07_LOCK_SHA: &str = "d6ae7d6cfee09ca6cfdbffa6faaa7eb955d38d6de38b7d8d7c858c2491b2019f";
const GATEWAY_LOCK_BYTES: usize = 96107;
const GATEWAY_LOCK_SHA: &str = "e779fa77be4df46c9ec3005ce81fdf0c0c54319c252e8f055ca72f45506e7409";
const INSTALLED_GATEWAY_LOCK_BYTES: usize = 96154;
const INSTALLED_GATEWAY_LOCK_SHA: &str =
    "7685919743252d3b8642944fb494f81df2636af3e17a5b58b1767abf1b5e4b0d";
const INSTALLED_GATEWAY_LOCK_SOURCE: &str =
    include_str!("t07-installed-gateway-lock-afterimage.json");
const INSTALLED_GATEWAY_LOCK_SOURCE_SHA: &str =
    "6bc7592a2536777555b2976e51a133bf0cbf7a8f0da15788761110e888013d87";
const GATEWAY_LOCK_SOURCE: &str = include_str!("t07-gateway-reqwest-lock-afterimage.json");
const GATEWAY_LOCK_SOURCE_SHA: &str =
    "bbacc3197f28c25a837ea7a9e75af95eea1d7fdc40f5fdab69cf56bc76b62d90";
const GATEWAY_WORKSPACE_SOURCE: &str = include_str!("t07-gateway-workspace-afterimage.json");
const GATEWAY_WORKSPACE_SHA: &str =
    "8861f11c390df344505a3a4f13453514f4d61036311aec9db8c0d4c38cba6b1f";
const GATEWAY_MCP_SOURCE: &str = include_str!("t07-gateway-mcp-afterimage.json");
const GATEWAY_MCP_SHA: &str = "85fedc8fa655401a1899115274963b6b3f526f2e03ca5928e838b28ebdc2cc58";
const OAUTH_LOCK_SOURCE: &str = include_str!("t07-gateway-oauth-lock-afterimage.json");
const OAUTH_LOCK_SOURCE_SHA: &str =
    "9a129a5b09562ae193e757f2f2992fb0f9d4ad23bb969ba11e3b18f908753dd8";
const OAUTH_LOCK_BYTES: usize = 101588;
const OAUTH_LOCK_SHA: &str = "5aec5f07b1143105a5100447e16d1696f15515e1524b9d9feb55683dcc8c035b";
const OAUTH_MCP_SOURCE: &str = include_str!("t07-gateway-oauth-mcp-afterimage.json");
const OAUTH_MCP_SHA: &str = "0d5fc70fe07b34019f0a0aeffd9843bafd1c5e5ea831a922f89e55b00ad9764f";
const T07_IDNA_EDGE: usize = 70349;
const T07_TEXT_EDGE: usize = 70373;
const T03_READER_PATH: &str = "docs/rust-rewrite/t03/source-successor-v1.rs";
const T09_T03_READER_SOURCE: &str = include_str!("t09-t03-reader-afterimage.json");
const T09_T03_READER_SHA: &str = "e4113304ad4054a298f51cf2fabccd1749b2c423560c4e3c7bff9928fa4a5d39";

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Capsule {
    schema: String,
    baseline_commit: String,
    rows: Vec<Row>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Row {
    path: String,
    prior: Prior,
    current: Current,
    edits: Vec<Edit>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Prior {
    bytes: usize,
    sha256: String,
    mode: String,
    git_blob: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Current {
    bytes: usize,
    sha256: String,
    mode: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Edit {
    offset: usize,
    remove_utf8: String,
    insert_utf8: String,
}
fn need(ok: bool, reason: &str) -> Result<()> {
    if ok {
        Ok(())
    } else {
        Err(format!("T08 issue903 config successor: {reason}"))
    }
}
fn sha(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
fn capsule() -> Result<Capsule> {
    need(sha(CAPSULE) == CAPSULE_SHA, "compiled capsule digest")?;
    let capsule: Capsule = serde_json::from_slice(CAPSULE).map_err(|e| e.to_string())?;
    need(
        capsule.schema == "ISSUE903_CONFIG_EDGE_SUCCESSOR_V1"
            && capsule.baseline_commit == BASE
            && capsule.rows.len() == 10
            && capsule.rows[0].path == "Cargo.lock"
            && capsule.rows[1].path == "crates/ssx-cli-check/Cargo.toml"
            && capsule.rows[2].path == "crates/ssx-mcp/Cargo.toml"
            && capsule.rows[3].path
                == "docs/rust-rewrite/a06/source-amendments/native-search-reader.rs"
            && capsule.rows[4].path == "docs/rust-rewrite/a06/source-amendments/w07-t02-joint.rs"
            && capsule.rows[5].path == "docs/rust-rewrite/t03/source-successor-v1.rs"
            && capsule.rows[6].path == "crates/ssx-infra/Cargo.toml"
            && capsule.rows[7].path
                == "docs/rust-rewrite/a06/source-amendments/shared-successor.rs"
            && capsule.rows[8].path == "crates/xtask/src/w07.rs"
            && capsule.rows[9].path == "crates/xtask/Cargo.toml",
        "closed ten-path capsule identity/order",
    )?;
    for row in &capsule.rows {
        need(
            row.prior.mode == "0644" && row.current.mode == "0644" && !row.edits.is_empty(),
            "closed prior/current mode, size, and change spans",
        )?;
    }
    Ok(capsule)
}
#[allow(
    dead_code,
    reason = "W07/O02 use this authority; independent source gates also include this module"
)]
pub(crate) fn current_lock_identity() -> Result<(usize, String)> {
    let capsule = capsule()?;
    let row = capsule
        .rows
        .into_iter()
        .next()
        .ok_or("missing Cargo.lock row")?;
    need(
        row.current.bytes == 96206
            && row.current.sha256
                == "5f4d339df70efcfa71a6ad6d878d4078c6250cd1a54279c8716123f2276e0c99",
        "T07 lock predecessor identity",
    )?;
    need(
        sha(OAUTH_LOCK_SOURCE.as_bytes()) == OAUTH_LOCK_SOURCE_SHA,
        "T07 installed OAuth lock receipt identity",
    )?;
    Ok((OAUTH_LOCK_BYTES, OAUTH_LOCK_SHA.to_owned()))
}

fn git(root: &Path, args: &[&str]) -> Result<Vec<u8>> {
    let output = Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .map_err(|error| error.to_string())?;
    need(output.status.success(), "prior Git identity command")?;
    Ok(output.stdout)
}
#[allow(
    dead_code,
    reason = "Used by source-composition gates; the standalone checker also compiles this shared module"
)]
pub(crate) fn owns(path: &str) -> bool {
    matches!(
        path,
        "docs/rust-rewrite/a06/source-amendments/native-search-reader.rs"
            | "docs/rust-rewrite/a06/source-amendments/w07-t02-joint.rs"
            | "docs/rust-rewrite/t03/source-successor-v1.rs"
            | "docs/rust-rewrite/a06/source-amendments/shared-successor.rs"
            | "crates/xtask/src/w07.rs"
    )
}
fn t07_codec_lock_prior(current: &[u8]) -> Result<Vec<u8>> {
    let installed_gateway_lock = source_afterimage::prior(
        OAUTH_LOCK_SOURCE,
        OAUTH_LOCK_SOURCE_SHA,
        "Cargo.lock",
        current,
        0o644,
    )?;
    need(
        installed_gateway_lock.len() == INSTALLED_GATEWAY_LOCK_BYTES
            && sha(&installed_gateway_lock) == INSTALLED_GATEWAY_LOCK_SHA,
        "T07 installed OAuth lock predecessor identity",
    )?;
    let exact_target_lock = source_afterimage::prior(
        INSTALLED_GATEWAY_LOCK_SOURCE,
        INSTALLED_GATEWAY_LOCK_SOURCE_SHA,
        "Cargo.lock",
        &installed_gateway_lock,
        0o644,
    )?;
    need(
        exact_target_lock.len() == GATEWAY_LOCK_BYTES
            && sha(&exact_target_lock) == GATEWAY_LOCK_SHA,
        "T07 installed gateway lock predecessor identity",
    )?;
    let previous = source_afterimage::prior(
        GATEWAY_LOCK_SOURCE,
        GATEWAY_LOCK_SOURCE_SHA,
        "Cargo.lock",
        &exact_target_lock,
        0o644,
    )?;
    let current = previous.as_slice();
    need(
        current.len() == T07_LOCK_BYTES && sha(current) == T07_LOCK_SHA,
        "T07 exact current lock identity",
    )?;
    const IDNA: &[u8] = b" \"idna\",\n";
    const TEXT: &[u8] = b" \"ssx-json\",\n";
    let idna_end = T07_IDNA_EDGE + IDNA.len();
    let text_end = T07_TEXT_EDGE + TEXT.len();
    need(
        current.get(T07_IDNA_EDGE..idna_end) == Some(IDNA)
            && current.get(T07_TEXT_EDGE..text_end) == Some(TEXT),
        "T07 exact codec edges",
    )?;
    Ok([
        &current[..T07_IDNA_EDGE],
        &current[idna_end..T07_TEXT_EDGE],
        &current[text_end..],
    ]
    .concat())
}

#[allow(
    dead_code,
    reason = "The standalone shared custody reader owns workspace manifest replay"
)]
pub(crate) fn gateway_workspace_prior(current: &[u8]) -> Result<Vec<u8>> {
    source_afterimage::prior(
        GATEWAY_WORKSPACE_SOURCE,
        GATEWAY_WORKSPACE_SHA,
        "Cargo.toml",
        current,
        0o644,
    )
}

fn project_row(row: &Row, current: &[u8]) -> Result<Vec<u8>> {
    let lock_prior = (row.path == "Cargo.lock")
        .then(|| t07_codec_lock_prior(current))
        .transpose()?;
    let current = lock_prior.as_deref().unwrap_or(current);
    let preceding = match row.path.as_str() {
        "crates/ssx-mcp/Cargo.toml" => {
            let gateway_manifest = source_afterimage::prior(
                OAUTH_MCP_SOURCE,
                OAUTH_MCP_SHA,
                &row.path,
                current,
                0o644,
            )?;
            Some(source_afterimage::prior(
                GATEWAY_MCP_SOURCE,
                GATEWAY_MCP_SHA,
                &row.path,
                &gateway_manifest,
                0o644,
            )?)
        }
        a11_replay_source::NATIVE_READER | a11_replay_source::SHARED_READER => {
            Some(a11_replay_source::prior(&row.path, current)?)
        }
        T03_READER_PATH => Some(source_afterimage::prior(
            T09_T03_READER_SOURCE,
            T09_T03_READER_SHA,
            &row.path,
            current,
            0o644,
        )?),
        _ => None,
    };
    let current = preceding.as_deref().unwrap_or(current);
    need(
        current.len() == row.current.bytes,
        &format!("opened current byte length for {}", row.path),
    )?;
    need(
        sha(current) == row.current.sha256,
        &format!("opened current digest for {}", row.path),
    )?;
    let mut prior = Vec::new();
    let mut current_cursor = 0usize;
    let mut prior_cursor = 0usize;
    for edit in &row.edits {
        let remove = edit.remove_utf8.as_bytes();
        let insert = edit.insert_utf8.as_bytes();
        let unchanged = edit
            .offset
            .checked_sub(prior_cursor)
            .ok_or("successor edit order")?;
        let current_end = current_cursor
            .checked_add(unchanged)
            .ok_or("successor current cursor overflow")?;
        prior.extend_from_slice(
            current
                .get(current_cursor..current_end)
                .ok_or("successor unchanged current span")?,
        );
        let inserted_end = current_end
            .checked_add(insert.len())
            .ok_or("successor inserted span overflow")?;
        need(
            current.get(current_end..inserted_end) == Some(insert),
            "exact dependency/source insertion span",
        )?;
        prior.extend_from_slice(remove);
        current_cursor = inserted_end;
        prior_cursor = edit
            .offset
            .checked_add(remove.len())
            .ok_or("successor prior cursor overflow")?;
    }
    prior.extend_from_slice(
        current
            .get(current_cursor..)
            .ok_or("successor current suffix")?,
    );
    need(
        prior.len() == row.prior.bytes && sha(&prior) == row.prior.sha256,
        "exact projected prior bytes",
    )?;
    let mut forward = Vec::new();
    let mut prior_cursor = 0usize;
    let mut current_cursor = 0usize;
    for edit in &row.edits {
        let remove = edit.remove_utf8.as_bytes();
        let insert = edit.insert_utf8.as_bytes();
        let unchanged = edit
            .offset
            .checked_sub(prior_cursor)
            .ok_or("successor forward edit order")?;
        let prior_end = prior_cursor
            .checked_add(unchanged)
            .ok_or("successor prior span overflow")?;
        forward.extend_from_slice(
            prior
                .get(prior_cursor..prior_end)
                .ok_or("successor prior unchanged span")?,
        );
        let removed_end = prior_end
            .checked_add(remove.len())
            .ok_or("successor removed span overflow")?;
        need(
            prior.get(prior_end..removed_end) == Some(remove),
            "exact predecessor source span",
        )?;
        forward.extend_from_slice(insert);
        current_cursor = current_cursor
            .checked_add(unchanged)
            .and_then(|cursor| cursor.checked_add(insert.len()))
            .ok_or("successor current cursor overflow")?;
        prior_cursor = removed_end;
    }
    forward.extend_from_slice(prior.get(prior_cursor..).ok_or("successor prior suffix")?);
    need(
        current_cursor <= current.len() && forward == current,
        "full forward reconstruction",
    )?;
    Ok(prior)
}
/// Pure byte projection for source-frame controls; physical callers use `prior`.
#[allow(
    dead_code,
    reason = "Used by source-composition gates; the standalone checker also compiles this shared module"
)]
pub(crate) fn project_current(path: &str, current: &[u8]) -> Result<Vec<u8>> {
    let capsule = capsule()?;
    let row = capsule
        .rows
        .iter()
        .find(|row| row.path == path)
        .ok_or("unowned issue903 config path")?;
    project_row(row, current)
}

/// Reverse exact current source spans after the caller's bounded read.
pub(crate) fn prior(root: &Path, path: &str, current: &[u8], mode: u32) -> Result<Vec<u8>> {
    let capsule = capsule()?;
    let row = capsule
        .rows
        .iter()
        .find(|row| row.path == path)
        .ok_or("unowned issue903 config path")?;
    #[cfg(unix)]
    need(mode == 0o644, "opened Unix current mode")?;
    #[cfg(windows)]
    {
        need(mode == 0, "Windows handle has no Unix mode bits")?;
        let index = git(root, &["ls-files", "--stage", "--", path])?;
        let index = std::str::from_utf8(&index).map_err(|error| error.to_string())?;
        let entries: Vec<_> = index.lines().collect();
        need(
            entries.len() == 1
                && entries[0].starts_with("100644 ")
                && entries[0].ends_with(&format!(" 0\t{path}")),
            "Windows stage-zero index mode",
        )?;
    }
    #[cfg(not(any(unix, windows)))]
    need(false, "opened source mode proof requires Unix or Windows")?;
    let projected_prior = project_row(row, current)?;
    let git_prior = git(root, &["show", &format!("{BASE}:{path}")])?;
    let tree = git(root, &["ls-tree", BASE, "--", path])?;
    need(
        git_prior == projected_prior
            && tree == format!("100644 blob {}\t{path}\n", row.prior.git_blob).as_bytes(),
        "exact baseline Git bytes/mode/blob",
    )?;
    Ok(projected_prior)
}
/// Positive and adversarial controls over the finite config successor.
pub(crate) fn controls(root: &Path) -> Result<()> {
    let capsule = capsule()?;
    for row in &capsule.rows {
        let (current, mode) =
            super::bounded_source_reader::bounded_read(root, &row.path, MAX_PHYSICAL_BYTES)?;
        let projected_prior = prior(root, &row.path, &current, mode)?;
        need(
            projected_prior.len() == row.prior.bytes && sha(&projected_prior) == row.prior.sha256,
            "positive projection control",
        )?;
        need(
            prior(root, &row.path, &current, 0o600).is_err(),
            "wrong opened mode rejected",
        )?;
        let mut altered = current.clone();
        altered[0] ^= 1;
        need(
            prior(root, &row.path, &altered, mode).is_err(),
            "altered current bytes rejected",
        )?;
        need(
            prior(root, &format!("{} ", row.path), &current, mode).is_err(),
            "unowned path rejected",
        )?;
    }
    need(
        capsule.rows[0].edits.len() == 9
            && capsule.rows[0].edits[4].offset == 67341
            && capsule.rows[0].edits[4].remove_utf8.is_empty()
            && capsule.rows[0].edits[4].insert_utf8 == " \"serde\",\n"
            && capsule.rows[0].edits[5].offset == 68937
            && capsule.rows[0].edits[5].remove_utf8.is_empty()
            && capsule.rows[0].edits[5].insert_utf8 == " \"nix\",\n"
            && capsule.rows[0].edits[6].offset == 69483
            && capsule.rows[0].edits[6].remove_utf8.is_empty()
            && capsule.rows[0].edits[6].insert_utf8 == " \"nix\",\n"
            && capsule.rows[1].edits.len() == 2
            && capsule.rows[1].edits[0].offset == 205
            && capsule.rows[1].edits[0].remove_utf8.is_empty()
            && capsule.rows[1].edits[0].insert_utf8 == "serde.workspace = true\n"
            && capsule.rows[1].edits[1].offset == 616
            && capsule.rows[1].edits[1].remove_utf8 == "features = [\"signal\"]\n"
            && capsule.rows[1].edits[1].insert_utf8 == "features = [\"signal\", \"fs\"]\n"
            && capsule.rows[2].edits.len() == 1
            && capsule.rows[2].edits[0].remove_utf8.is_empty()
            && capsule.rows[2].edits[0].insert_utf8
                == "\n[target.'cfg(unix)'.dev-dependencies.nix]\nversion = \"=0.31.3\"\nfeatures = [\"fs\"]\n"
            && !capsule.rows[3].edits.is_empty()
            && capsule.rows[4].edits.len() >= 2
            && !capsule.rows[5].edits.is_empty()
            && capsule.rows[6].edits.len() == 1
            && capsule.rows[6].edits[0].offset == capsule.rows[6].prior.bytes
            && capsule.rows[6].edits[0].insert_utf8
                == "\n[target.'cfg(unix)'.dev-dependencies.nix]\nversion = \"=0.31.3\"\nfeatures = [\"fs\"]\n"
            && !capsule.rows[7].edits.is_empty()
            && !capsule.rows[8].edits.is_empty()
            && capsule.rows[9].edits.len() == 1
            && capsule.rows[9].edits[0].remove_utf8 == "features = [\"signal\"]\n"
            && capsule.rows[9].edits[0].insert_utf8 == "features = [\"signal\", \"fs\"]\n",
        "finite dependency and source successor spans",
    )?;
    Ok(())
}
