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
// PR1046 adds only the two reviewed notices. The pre-pause documents remain
// independently archived and verified through the historical chains above.
const PAUSE_CAPSULE: &[u8] = include_bytes!("community-pr-pause-v1.json");
const PAUSE_CAPSULE_SHA256: &str =
    "e859ec18e37ad02c41d0391e204fa83a6d4ef35ec9106337dda82d0bc599b45a";
const PRE_PAUSE_AGENTS: &[u8] = include_bytes!("AGENTS.pre-pause-d623.md.frozen");
const PRE_PAUSE_CONTRIBUTING: &[u8] = include_bytes!("CONTRIBUTING.pre-pause-d623.md.frozen");

pub fn verify_pause_successor(
    path: &str,
    baseline: &[u8],
    current: &[u8],
    raw: &[u8],
) -> Result<(), String> {
    let (expected_baseline, archive) = match path {
        "AGENTS.md" => (
            "284b8126bc0e031af6b6b734e5d699a86e2bae8a92faabd90cd35696411aadb3",
            "docs/rust-rewrite/storage-document-amendment/AGENTS.pre-pause-d623.md.frozen",
        ),
        "CONTRIBUTING.md" => (
            BINDINGS[1].after,
            "docs/rust-rewrite/storage-document-amendment/CONTRIBUTING.pre-pause-d623.md.frozen",
        ),
        _ => return Err("Unreviewed community-pause document path".into()),
    };
    if digest(raw) != PAUSE_CAPSULE_SHA256 {
        return Err("Community-pause capsule differs".into());
    }
    let doc: serde_json::Value = serde_json::from_slice(raw).map_err(|e| e.to_string())?;
    if doc["kind"] != "SLOPSEARX_COMMUNITY_PR_PAUSE_V1"
        || doc["baseline_revision"] != "d623c739653fca4812aca982f8ab703e1fbf361a"
        || doc["candidate_revision"] != "9bae4746b27e7e33a7bad7a7ce048d6234c84f8a"
        || doc["pull_request"] != 1046
    {
        return Err("Community-pause provenance differs".into());
    }
    let documents = doc["documents"]
        .as_array()
        .ok_or("Pause document rows absent")?;
    if documents.len() != 2 {
        return Err("Pause document inventory differs".into());
    }
    let rows: Vec<_> = documents
        .iter()
        .filter(|row| row["source_path"] == path)
        .collect();
    if rows.len() != 1 {
        return Err("Pause document identity differs".into());
    }
    let row = rows[0];
    if row["archive_path"] != archive
        || row["baseline"]["sha256"] != expected_baseline
        || digest(baseline) != expected_baseline
        || row["baseline"]["bytes"].as_u64() != Some(baseline.len() as u64)
    {
        return Err("Pause baseline/archive binding differs".into());
    }
    if row["current"]["sha256"] != digest(current)
        || row["current"]["bytes"].as_u64() != Some(current.len() as u64)
    {
        return Err("Pause actual current bytes differ".into());
    }
    let edits = row["edits"].as_array().ok_or("Pause edit absent")?;
    if edits.len() != 1 {
        return Err("Pause requires exactly one notice insertion".into());
    }
    let edit = &edits[0];
    let at = edit["at"]
        .as_u64()
        .and_then(|v| usize::try_from(v).ok())
        .ok_or("Pause insertion offset invalid")?;
    let insert = edit["insert_utf8"]
        .as_str()
        .ok_or("Pause notice absent")?
        .as_bytes();
    let title_end = baseline
        .windows(2)
        .position(|w| w == b"\n\n")
        .ok_or("Pause title boundary absent")?
        + 2;
    if edit["remove_utf8"] != "" || at != title_end || insert.is_empty() {
        return Err("Pause is not a title-adjacent insertion".into());
    }
    let mut rebuilt = baseline[..at].to_vec();
    rebuilt.extend_from_slice(insert);
    rebuilt.extend_from_slice(&baseline[at..]);
    if rebuilt != current {
        return Err("Pause reconstructed successor differs".into());
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
    let baseline = match path {
        "AGENTS.md" => {
            verify_historical(path, original, original_bytes, ACCEPTED_AGENTS, archive)?;
            verify_successor(
                path,
                BINDINGS[0].after,
                ACCEPTED_AGENTS,
                PRE_PAUSE_AGENTS,
                AGENTS_SUCCESSOR,
            )?;
            PRE_PAUSE_AGENTS
        }
        "CONTRIBUTING.md" => {
            verify_historical(
                path,
                original,
                original_bytes,
                PRE_PAUSE_CONTRIBUTING,
                archive,
            )?;
            PRE_PAUSE_CONTRIBUTING
        }
        _ => return Err("Unreviewed storage document path".into()),
    };
    verify_pause_successor(path, baseline, current, PAUSE_CAPSULE)
}

pub fn archive_path(path: &str) -> Result<&'static str, String> {
    BINDINGS
        .iter()
        .find(|binding| binding.path == path)
        .map(|binding| binding.archive_path)
        .ok_or_else(|| "Unreviewed storage document path".into())
}
