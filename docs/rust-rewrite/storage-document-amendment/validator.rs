//! Exact original/current binding for two reviewed storage-policy documents.
//! Coverage files and the historical A13 code-amendment chain stay immutable.
use sha2::{Digest, Sha256};

struct Binding {
    path: &'static str,
    before: &'static str,
    after: &'static str,
    archive_path: &'static str,
}
const BINDINGS: &[Binding] = &[
    Binding {
        path: "AGENTS.md",
        before: "d710c84ebad4582dc607cc3bdb806a5a5253259e4cabd5c515f51e55c3588af8",
        after: "7d681485699a897464cf248d22fa83bd81e7b85d96a0163f6c9dabc019f988f6",
        archive_path: "docs/rust-rewrite/storage-document-amendment/AGENTS.before.md",
    },
    Binding {
        path: "CONTRIBUTING.md",
        before: "2df6be141a3b06c662a4062af79eb074248ea70f297a7bf4c65432c10feba681",
        after: "5526fb36792e2e14c7fab4f476b557c63e3d9e8aaab324078152d732e9dd3016",
        archive_path: "docs/rust-rewrite/storage-document-amendment/CONTRIBUTING.before.md",
    },
];
fn digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
pub fn is_path(path: &str) -> bool {
    BINDINGS.iter().any(|binding| binding.path == path)
}
pub fn verify_historical(
    path: &str,
    original: &str,
    original_bytes: Option<usize>,
    current: &[u8],
    archive: &[u8],
) -> Result<(), String> {
    let binding = BINDINGS
        .iter()
        .find(|binding| binding.path == path)
        .ok_or("Unreviewed storage document path")?;
    if original != binding.before
        || digest(archive) != binding.before
        || original_bytes.is_some_and(|bytes| bytes != archive.len())
    {
        return Err("Storage document original/archive binding differs".into());
    }
    if digest(current) != binding.after {
        return Err("Reviewed storage document successor differs".into());
    }
    Ok(())
}

// The old d710 -> accepted504 relation remains above, explicitly historical.
// Live AGENTS bytes must instead reconstruct the one reviewed next successor.
const AGENTS_SUCCESSOR_SHA256: &str =
    "f88d4d259df22ac0f140bb42549157dd7e536346e55943efcfb305ea85d72e6e";
const ACCEPTED_AGENTS: &[u8] = include_bytes!("AGENTS.accepted-504.md.frozen");
const AGENTS_SUCCESSOR: &[u8] = include_bytes!("agents-successor-v2.json");
pub fn verify_successor(
    path: &str,
    expected_baseline: &str,
    baseline: &[u8],
    current: &[u8],
    raw: &[u8],
) -> Result<(), String> {
    if path != "AGENTS.md" || expected_baseline != BINDINGS[0].after {
        return Err("Unreviewed AGENTS successor caller".into());
    }
    if digest(raw) != AGENTS_SUCCESSOR_SHA256 {
        return Err("AGENTS successor capsule differs".into());
    }
    let doc: serde_json::Value = serde_json::from_slice(raw).map_err(|e| e.to_string())?;
    if doc["kind"] != "SLOPSEARX_STORAGE_AGENTS_SUCCESSOR_V2"
        || doc["source_path"] != path
        || doc["baseline"]["path"] != path
        || doc["current"]["path"] != path
        || doc["archive"]["source_path"] != path
        || doc["archive"]["path"]
            != "docs/rust-rewrite/storage-document-amendment/AGENTS.accepted-504.md.frozen"
        || doc["baseline"]["sha256"] != expected_baseline
        || doc["archive"]["sha256"] != expected_baseline
        || digest(baseline) != expected_baseline
        || doc["baseline"]["bytes"].as_u64() != Some(baseline.len() as u64)
        || doc["archive"]["bytes"].as_u64() != Some(baseline.len() as u64)
    {
        return Err("AGENTS accepted504 archive/caller differs".into());
    }
    if doc["current"]["sha256"] != digest(current)
        || doc["current"]["bytes"].as_u64() != Some(current.len() as u64)
    {
        return Err("AGENTS actual current bytes differ".into());
    }
    let edits = doc["edits"]
        .as_array()
        .ok_or("AGENTS finite edits absent")?;
    if edits.is_empty() {
        return Err("AGENTS finite edits empty".into());
    }
    let mut rebuilt = Vec::new();
    let mut cursor = 0;
    for edit in edits {
        let at = edit["at"]
            .as_u64()
            .and_then(|v| usize::try_from(v).ok())
            .ok_or("AGENTS edit offset invalid")?;
        let remove = edit["remove_utf8"]
            .as_str()
            .ok_or("AGENTS edit preimage absent")?
            .as_bytes();
        let insert = edit["insert_utf8"]
            .as_str()
            .ok_or("AGENTS edit insertion absent")?
            .as_bytes();
        let end = at.checked_add(remove.len()).ok_or("AGENTS edit overflow")?;
        if at < cursor || end > baseline.len() || baseline.get(at..end) != Some(remove) {
            return Err("AGENTS finite edit preimage/order differs".into());
        }
        rebuilt.extend_from_slice(&baseline[cursor..at]);
        rebuilt.extend_from_slice(insert);
        cursor = end;
    }
    rebuilt.extend_from_slice(&baseline[cursor..]);
    if rebuilt != current {
        return Err("AGENTS reconstructed successor differs".into());
    }
    Ok(())
}
pub fn verify(
    path: &str,
    original: &str,
    original_bytes: Option<usize>,
    current: &[u8],
    archive: &[u8],
) -> Result<(), String> {
    if path != "AGENTS.md" {
        return verify_historical(path, original, original_bytes, current, archive);
    }
    verify_historical(path, original, original_bytes, ACCEPTED_AGENTS, archive)?;
    verify_successor(
        path,
        BINDINGS[0].after,
        ACCEPTED_AGENTS,
        current,
        AGENTS_SUCCESSOR,
    )
}

pub fn archive_path(path: &str) -> Result<&'static str, String> {
    BINDINGS
        .iter()
        .find(|binding| binding.path == path)
        .map(|binding| binding.archive_path)
        .ok_or_else(|| "Unreviewed storage document path".into())
}
