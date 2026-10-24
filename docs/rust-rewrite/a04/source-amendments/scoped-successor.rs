//! Unapplied PR1034 draft. Current identity and historical evidence are distinct.
#[path = "../../a06/source-amendments/shared-successor.rs"]
pub(crate) mod a06_source_successor;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{fs, path::Path};
const MANIFEST: &str = "docs/rust-rewrite/a04/source-amendments/scoped-successor-v1.json";
const MANIFEST_SHA: &str = "31d418472c38c2bf5182f7deb76fafab4876cbd3dd58d332fa2e64fc9bb6b18c";
// V1 remains the immutable predecessor, also referenced by A13 composition.
const CURRENT_MANIFEST: &str = "docs/rust-rewrite/a04/source-amendments/scoped-successor-v2.json";
const CURRENT_MANIFEST_SHA: &str =
    "13b6e48152b4316693159995d9179350605326a57b0fed0dee01d8413efcfdf5";
const CURRENT_HTTP_SHA: &str = "c5116d9a0081a0497a12643440053f001b6d68b48e9b4bf8c5feabf81eed5d10";
const HTTP_GUARD_BEFORE: &[u8] = b"            #[cfg(test)]\n            if let Some(endpoint) = &self.replay_endpoint {\n                if request.url.scheme() != endpoint.scheme()\n                    || request.url.host() != endpoint.host()\n                    || request.url.port_or_known_default() != endpoint.port_or_known_default()\n                {\n                    return Err(Error::InvalidRequest);\n                }\n            }\n";
const HTTP_GUARD_AFTER: &[u8] = b"            #[cfg(test)]\n            if let Some(endpoint) = &self.replay_endpoint\n                && (request.url.scheme() != endpoint.scheme()\n                    || request.url.host() != endpoint.host()\n                    || request.url.port_or_known_default() != endpoint.port_or_known_default())\n            {\n                return Err(Error::InvalidRequest);\n            }\n";
const HTTP: &str = "crates/ssx-infra/src/http.rs";
const DEHASHED: &str = "crates/ssx-infra/tests/a13_dehashed.rs";
const REMAINING: &str = "crates/ssx-infra/tests/a13_remaining_fixture/mod.rs";
const CLOCK: &str = "crates/ssx-infra/tests/support/a10_http_clock.rs";
const CONTAINERS: [&str; 6] = [
    "docs/rust-rewrite/a10/qualification.json",
    "docs/rust-rewrite/a11/qualification.json",
    "docs/rust-rewrite/a12/qualification-v1.json",
    "docs/rust-rewrite/a13/qualification-v1.json",
    "docs/rust-rewrite/a13/dehashed-source-map-v1.json",
    "docs/rust-rewrite/a13/remaining/source-map.json",
];
fn selected(container: &str, source: &str) -> bool {
    (source == HTTP && CONTAINERS.contains(&container))
        || (source == CLOCK && [CONTAINERS[0], CONTAINERS[1], CONTAINERS[3]].contains(&container))
        || ([DEHASHED, REMAINING].contains(&source) && container == CONTAINERS[3])
}
fn require(ok: bool, message: &str) -> Result<(), String> {
    if ok { Ok(()) } else { Err(message.into()) }
}
fn digest(raw: &[u8]) -> String {
    let digest = Sha256::digest(raw);
    let mut hex = String::with_capacity(digest.len() * 2);
    const HEX: &[u8; 16] = b"0123456789abcdef";
    for byte in digest.iter().copied() {
        hex.push(HEX[(byte >> 4) as usize] as char);
        hex.push(HEX[(byte & 0x0f) as usize] as char);
    }
    hex
}
#[cfg(unix)]
fn mode(path: &Path) -> Result<String, String> {
    use std::os::unix::fs::PermissionsExt;
    Ok(format!(
        "{:04o}",
        fs::metadata(path)
            .map_err(|e| e.to_string())?
            .permissions()
            .mode()
            & 0o777
    ))
}
#[cfg(not(unix))]
fn mode(_: &Path) -> Result<String, String> {
    Err("PR1034 successor filesystem-mode proof unsupported on this platform".into())
}
fn text<'a>(row: &'a Value, key: &str) -> Result<&'a str, String> {
    row[key]
        .as_str()
        .ok_or_else(|| format!("PR1034 missing {key}"))
}
fn check_material(row: &Value, raw: &[u8], actual_mode: &str, stage: &str) -> Result<(), String> {
    require(
        row["sha256"] == digest(raw)
            && row["bytes"].as_u64() == Some(raw.len() as u64)
            && row["mode"] == actual_mode,
        stage,
    )
}
fn read_material(root: &Path, row: &Value, stage: &str) -> Result<Vec<u8>, String> {
    if text(row, "path")? == "crates/ssx-infra/tests/a09_factories.rs" {
        let prior = a06_source_successor::default_ca_fixture_operand(
            root,
            text(row, "path")?,
            text(row, "sha256")?,
            row["bytes"].as_u64().map(|n| n as usize),
        )
        .ok_or("A09 default-CA fixture tuple not selected")??;
        check_material(row, &prior, "0644", stage)?;
        return Ok(prior);
    }

    let path = root.join(text(row, "path")?);
    let metadata = fs::symlink_metadata(&path).map_err(|e| e.to_string())?;
    require(
        metadata.file_type().is_file() && !metadata.file_type().is_symlink(),
        "PR1034 material must be a regular non-symlink file",
    )?;
    let bytes = fs::read(&path).map_err(|e| format!("{stage}: {e}"))?;
    check_material(row, &bytes, &mode(&path)?, stage)?;
    Ok(bytes)
}
// This one V1 material acquired W07's reviewed wrapper prefix on main. Its
// original row remains historical premerge evidence; actual CURRENT is read
// independently from the merged source path, never from an archive.
const XTASK_A13: &str = "crates/xtask/src/a13.rs";
const A13_COMPOSITION: &str = "docs/rust-rewrite/a04/source-amendments/a13-w07-composition-v1.json";
const A13_COMPOSITION_SHA: &str =
    "98263270d3e7386eafe9dd78d0abc13f0fd8a3cfa48e0b3b7b845dd4fc4daa4b";
const W07_A13_INSERT: &[u8] = b"    if let Some(result) = crate::w06_source_amendment::verify_shared(\n        root,\n        container_path,\n        container_sha,\n        path,\n        expected_sha,\n        expected_bytes,\n    ) {\n        return result;\n    }\n";
struct HistoricalMergedA13 {
    bytes: Vec<u8>,
}
struct HistoricalPremergeMaterial {
    bytes: Vec<u8>,
}
fn a13_composition_payload(
    v1_row: &Value,
    composition: &Value,
    actual_current: &[u8],
    actual_mode: &str,
) -> Result<(HistoricalMergedA13, HistoricalPremergeMaterial), String> {
    require(
        composition["kind"] == "PR1034_W07_A13_EXACT_COMPOSITION_V1"
            && composition["frozen_http_manifest"]["path"] == MANIFEST
            && composition["frozen_http_manifest"]["sha256"] == MANIFEST_SHA
            && composition["historical_revision"] == "c253e60438a4750512089fb11fae412ca28a4f8a"
            && v1_row["path"] == XTASK_A13
            && composition["historical_premerge"] == *v1_row
            && composition["current"]["path"] == XTASK_A13,
        "PR1034 A13 composition scope or historical V1 relation changed",
    )?;
    check_material(
        &composition["current"],
        actual_current,
        actual_mode,
        "PR1034 actual CURRENT merged A13 bytes/mode",
    )?;
    let current = HistoricalMergedA13 {
        bytes: actual_current.to_vec(),
    };
    let insertion = text(&composition["insertion"], "utf8")?.as_bytes();
    require(
        insertion == W07_A13_INSERT
            && insertion.len() == 228
            && composition["insertion"]["bytes"].as_u64() == Some(228)
            && composition["insertion"]["sha256"] == digest(insertion),
        "PR1034 A13 composition exact W07 insertion changed",
    )?;
    let at = composition["insertion"]["at"]
        .as_u64()
        .and_then(|n| usize::try_from(n).ok())
        .ok_or("PR1034 A13 composition offset absent/overflow")?;
    let end = at
        .checked_add(insertion.len())
        .ok_or("PR1034 A13 insertion overflow")?;
    require(
        current.bytes.get(at..end) == Some(insertion)
            && current
                .bytes
                .windows(insertion.len())
                .filter(|w| *w == insertion)
                .count()
                == 1,
        "PR1034 A13 current must contain exactly one insertion at the reviewed offset",
    )?;
    let mut historical_bytes = current.bytes[..at].to_vec();
    historical_bytes.extend_from_slice(&current.bytes[end..]);
    // V1 seals this historical identity and mode. This reconstructed operand
    // proves the premerge byte relation; it is never read as live/current code.
    require(
        v1_row["sha256"] == "81b358ff63a0dceb773cdd353f99fb9322d017897af007e290409ac598a10465"
            && v1_row["bytes"].as_u64() == Some(35360)
            && v1_row["mode"] == "0644"
            && digest(&historical_bytes) == v1_row["sha256"]
            && historical_bytes.len() == 35360,
        "PR1034 HISTORICAL premerge A13 is not the sealed V1 operand",
    )?;
    Ok((
        current,
        HistoricalPremergeMaterial {
            bytes: historical_bytes,
        },
    ))
}
fn a13_composition_document(raw: &[u8]) -> Result<Value, String> {
    require(
        digest(raw) == A13_COMPOSITION_SHA,
        "PR1034 A13 composition manifest changed",
    )?;
    serde_json::from_slice(raw).map_err(|e| e.to_string())
}
fn read_historical_a13_composition(
    root: &Path,
    v1_row: &Value,
) -> Result<(Value, HistoricalMergedA13, HistoricalPremergeMaterial), String> {
    let raw = fs::read(root.join(A13_COMPOSITION)).map_err(|e| e.to_string())?;
    let composition = a13_composition_document(&raw)?;
    require(
        composition["current"]["path"] == XTASK_A13,
        "PR1034 A13 actual current path changed",
    )?;
    // This old W07 relation is now checked only on an explicitly archived predecessor.
    let next = a13_composition_v2_document(root)?;
    let actual = read_material(
        root,
        &next["predecessor_archive"],
        "PR1034 HISTORICAL merged A13",
    )?;
    let actual_mode = mode(&root.join(text(&next["predecessor_archive"], "path")?))?;
    let (current, historical) =
        a13_composition_payload(v1_row, &composition, &actual, &actual_mode)?;
    Ok((composition, current, historical))
}
fn verify_merged_a13(root: &Path, current_row: &Value) -> Result<(), String> {
    let prior_row = historical_a13_row(root)?;
    let (_, previous, historical) = read_historical_a13_composition(root, &prior_row)?;
    let next = a13_composition_v2_document(root)?;
    let (accepted_main_row, accepted_main_bytes) =
        a06_source_successor::a13_baseline(root, current_row)?;
    require(
        next["current"] == accepted_main_row,
        "PR1034 A13 accepted-main predecessor row differs",
    )?;
    let actual = read_material(root, current_row, "A06 actual CURRENT composed A13")?;
    check_source_transition(
        &next,
        &previous.bytes,
        &accepted_main_bytes,
        "0644", // Archive mode checked by the new transition before this old relation.
        XTASK_A13,
        A13_EDITS_SHA,
    )?;
    eprintln!(
        "PR1034_ADDITIVE_COMPOSITION_PROOF {}",
        serde_json::json!({
        "current_path":XTASK_A13,"current_sha256":digest(&actual),"current_bytes":actual.len(),
        "historical_w07_current_sha256":digest(&previous.bytes),
        "historical_premerge_sha256":digest(&historical.bytes),
        "predecessor_composition_sha256":A13_COMPOSITION_SHA})
    );
    Ok(())
}
#[cfg(test)]
fn a13_composition_controls(root: &Path, manifest: &Value) -> Result<(), String> {
    let historical_row = historical_a13_row(root)?;
    let row = &historical_row;
    require(
        manifest["a13_composition"]["sha256"] == A13_COMPOSITION_V2_SHA,
        "PR1034 A13 V2 binding differs",
    )?;
    let (composition, current, historical) = read_historical_a13_composition(root, row)?;
    let mut changed_manifest = fs::read(root.join(A13_COMPOSITION)).map_err(|e| e.to_string())?;
    changed_manifest.push(0);
    require(
        a13_composition_document(&changed_manifest).is_err(),
        "PR1034 A13 changed composition manifest accepted",
    )?;
    let rejects = |candidate: &Value, bytes: &[u8], permissions: &str| {
        require(
            a13_composition_payload(row, candidate, bytes, permissions).is_err(),
            "PR1034 A13 composition negative control accepted",
        )
    };
    let mut changed = current.bytes.clone();
    changed[0] ^= 1;
    rejects(&composition, &changed, "0644")?;
    rejects(&composition, &historical.bytes, "0644")?;
    rejects(&composition, &current.bytes, "0755")?;
    for replacement in ["", "unreviewed insertion"] {
        let mut changed_delta = composition.clone();
        changed_delta["insertion"]["utf8"] = serde_json::json!(replacement);
        rejects(&changed_delta, &current.bytes, "0644")?;
    }
    let mut moved_delta = composition.clone();
    moved_delta["insertion"]["at"] =
        serde_json::json!(composition["insertion"]["at"].as_u64().unwrap() + 1);
    rejects(&moved_delta, &current.bytes, "0644")?;
    let mut wrong_scope = composition.clone();
    wrong_scope["current"]["path"] = serde_json::json!(HTTP);
    rejects(&wrong_scope, &current.bytes, "0644")?;
    // Recompute the claimed current digest to exercise the relation itself:
    // pin checks alone must not hide an unrelated addition or duplicate prefix.
    for extra in [&b"unreviewed"[..], W07_A13_INSERT] {
        let mut extra_current = current.bytes.clone();
        extra_current.extend_from_slice(extra);
        let mut changed_claim = composition.clone();
        changed_claim["current"]["sha256"] = serde_json::json!(digest(&extra_current));
        changed_claim["current"]["bytes"] = serde_json::json!(extra_current.len());
        rejects(&changed_claim, &extra_current, "0644")?;
    }
    eprintln!(
        "PR1034_COMPOSITION_CONTROLS historical_w07_positive=true premerge_as_w07_rejected=true altered_byte_delta_offset_mode_scope_and_relation_negatives=passed"
    );
    Ok(())
}

/// Always loaded from the actual source path, never from historical evidence.
struct CurrentSuccessor {
    path: String,
    bytes: Vec<u8>,
}
/// Explicit archived material. It cannot be supplied as a CurrentSuccessor.
struct HistoricalEvidence {
    path: String,
    bytes: Vec<u8>,
}
fn current(root: &Path, successor: &Value) -> Result<CurrentSuccessor, String> {
    let row = &successor["current"];
    let path = text(row, "path")?;
    require(
        [HTTP, CLOCK, DEHASHED, REMAINING].contains(&path),
        "PR1034 current source scope",
    )?;
    Ok(CurrentSuccessor {
        path: path.into(),
        bytes: read_material(root, row, "PR1034 CURRENT bytes/mode")?,
    })
}
fn historical(root: &Path, successor: &Value) -> Result<HistoricalEvidence, String> {
    let current_path = text(&successor["current"], "path")?;
    let archive = &successor["historical191"];
    require(
        archive["path"]
            == format!("docs/rust-rewrite/a04/source-amendments/191537e/{current_path}"),
        "PR1034 historical archive path",
    )?;
    Ok(HistoricalEvidence {
        path: current_path.into(),
        bytes: read_material(root, archive, "PR1034 HISTORICAL191 bytes/mode")?,
    })
}
fn patch_matches(
    successor: &Value,
    current: &CurrentSuccessor,
    prior: &HistoricalEvidence,
) -> Result<(), String> {
    require(
        current.path == prior.path,
        "PR1034 distinct material source mismatch",
    )?;
    let edits = successor["edits"].as_array().ok_or("PR1034 edits absent")?;
    let mut output = Vec::new();
    let mut end = 0;
    for edit in edits {
        let at = edit["at"].as_u64().ok_or("PR1034 edit offset")? as usize;
        let removed: Vec<u8> =
            serde_json::from_value(edit["remove"].clone()).map_err(|e| e.to_string())?;
        let inserted: Vec<u8> =
            serde_json::from_value(edit["insert"].clone()).map_err(|e| e.to_string())?;
        let next = at
            .checked_add(removed.len())
            .ok_or("PR1034 edit overflow")?;
        require(
            at >= end && next <= prior.bytes.len() && prior.bytes[at..next] == removed,
            "PR1034 reviewed edit preimage differs",
        )?;
        output.extend_from_slice(&prior.bytes[end..at]);
        output.extend_from_slice(&inserted);
        end = next;
    }
    output.extend_from_slice(&prior.bytes[end..]);
    require(
        output == current.bytes,
        "PR1034 exact reviewed patch differs",
    )
}
fn check_caller(
    binding: &Value,
    container_sha: &str,
    container: &[u8],
    expected_sha: &str,
    expected_bytes: Option<usize>,
) -> Result<(), String> {
    require(
        binding["container_sha256"] == container_sha
            && digest(container) == container_sha
            && binding["caller_expected_sha256"] == expected_sha
            && binding["caller_expected_bytes"]
                .as_u64()
                .map(|n| n as usize)
                == expected_bytes,
        &format!(
            "PR1034 caller tuple changed: container={} source={} sealed_container_sha={} supplied_container_sha={} actual_operand_sha={} sealed_source_sha={} supplied_source_sha={} sealed_bytes={} supplied_bytes={:?}",
            binding["container_path"],
            binding["source_path"],
            binding["container_sha256"],
            container_sha,
            digest(container),
            binding["caller_expected_sha256"],
            expected_sha,
            binding["caller_expected_bytes"],
            expected_bytes
        ),
    )?;
    let document: Value = serde_json::from_slice(container).map_err(|e| e.to_string())?;
    let pointer = format!("/{}", text(binding, "row")?);
    let actual = document
        .pointer(&pointer)
        .ok_or("PR1034 original row absent")?;
    require(
        actual["path"] == binding["source_path"]
            && actual["sha256"] == expected_sha
            && actual["bytes"].as_u64().map(|n| n as usize) == expected_bytes,
        "PR1034 actual original row differs",
    )
}
fn verify_history(
    root: &Path,
    manifest: &Value,
    binding: &Value,
    prior: &HistoricalEvidence,
) -> Result<(), String> {
    // Every V1–V6 manifest and every original/previous-successor archive is
    // independently sealed. No historical file is asserted to be current.
    for row in manifest["historical_chain_materials"]
        .as_array()
        .ok_or("PR1034 history absent")?
    {
        read_material(root, row, "PR1034 HISTORICAL chain material")?;
    }
    require(
        binding["prior191_sha256"] == digest(&prior.bytes)
            && binding["prior191_bytes"].as_u64() == Some(prior.bytes.len() as u64),
        "PR1034 caller historical191 relation",
    )?;
    if prior.path == CLOCK {
        return require(
            binding["caller_expected_sha256"] == digest(&prior.bytes),
            "PR1034 original clock archive differs",
        );
    }
    let v6: Value =
        serde_json::from_slice(
            &fs::read(root.join(
                "docs/rust-rewrite/a13/source-amendments/a13-current-source-amendment-v6.json",
            ))
            .map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
    let row = v6["amendments"]
        .as_array()
        .ok_or("PR1034 V6 amendments absent")?
        .iter()
        .find(|r| r["source_path"] == prior.path)
        .ok_or("PR1034 V6 exact source absent")?;
    require(
        row["successor"]["sha256"] == digest(&prior.bytes)
            && row["successor"]["bytes"].as_u64() == Some(prior.bytes.len() as u64),
        "PR1034 historical191 is not accepted V6 successor",
    )?;
    let matches: Vec<_> = row["historical_pins"]
        .as_array()
        .ok_or("PR1034 V6 pins absent")?
        .iter()
        .filter(|r| r["container_path"] == binding["container_path"])
        .collect();
    require(matches.len() == 1, "PR1034 historical container membership")?;
    let pin = matches[0];
    require(
        pin["container_sha256"] == binding["container_sha256"]
            && pin["source_sha256"] == binding["caller_expected_sha256"]
            && binding["caller_expected_bytes"]
                .as_u64()
                .is_none_or(|n| pin["source_bytes"].as_u64() == Some(n))
            && pin["source_sha256"] == row["archive"]["sha256"]
            && pin["source_bytes"] == row["archive"]["bytes"]
            && pin["source_mode"] == row["archive"]["mode"],
        "PR1034 V6 historical source relation",
    )
}
/// The only V1 -> V2 changes are kind, actual HTTP SHA/length, and the exact
/// equivalent test-only guard inserted at the existing historical edit offset.
fn validate_manifest_transition(previous: &Value, current: &Value) -> Result<(), String> {
    require(
        previous["kind"] == "PR1034_SCOPED_CURRENT_AND_HISTORY_V1"
            && current["kind"] == "PR1034_SCOPED_CURRENT_AND_HISTORY_V2"
            && previous["successors"][0]["current"]["path"] == HTTP
            && previous["successors"][0]["current"]["sha256"]
                == "3f47a040aa52b4e26260b055e0f31bbe1022fb6974fb97b493d0ebdb5f2b1d91"
            && previous["successors"][0]["current"]["bytes"].as_u64() == Some(54800)
            && previous["successors"][0]["edits"][7]["at"].as_u64() == Some(19532)
            && previous["successors"][0]["edits"][7]["insert"]
                == serde_json::json!(HTTP_GUARD_BEFORE)
            && current["successors"][0]["current"]["sha256"] == CURRENT_HTTP_SHA
            && current["successors"][0]["current"]["bytes"].as_u64() == Some(54774)
            && current["successors"][0]["current"]["mode"] == "0644"
            && current["successors"][0]["edits"][7]["insert"]
                == serde_json::json!(HTTP_GUARD_AFTER),
        "PR1034 finite V1/V2 HTTP guard transition differs",
    )?;
    let mut reverted = current.clone();
    reverted["kind"] = previous["kind"].clone();
    reverted["successors"][0]["current"]["sha256"] =
        previous["successors"][0]["current"]["sha256"].clone();
    reverted["successors"][0]["current"]["bytes"] =
        previous["successors"][0]["current"]["bytes"].clone();
    reverted["successors"][0]["edits"][7]["insert"] =
        previous["successors"][0]["edits"][7]["insert"].clone();
    require(
        reverted == *previous,
        "PR1034 unrelated V1/V2 manifest material changed",
    )
}
fn current_manifest_document(raw: &[u8]) -> Result<Value, String> {
    require(
        digest(raw) == CURRENT_MANIFEST_SHA,
        "PR1034 V2 manifest changed",
    )?;
    serde_json::from_slice(raw).map_err(|e| e.to_string())
}
#[cfg(test)]
fn manifest_transition_controls(root: &Path, current: &Value) -> Result<(), String> {
    let previous_raw = fs::read(root.join(MANIFEST)).map_err(|e| e.to_string())?;
    require(
        digest(&previous_raw) == MANIFEST_SHA,
        "PR1034 V1 controls identity changed",
    )?;
    let previous: Value = serde_json::from_slice(&previous_raw).map_err(|e| e.to_string())?;
    validate_manifest_transition(&previous, current)?;
    let rejects = |mutant: &Value| {
        require(
            validate_manifest_transition(&previous, mutant).is_err(),
            "PR1034 finite V2 transition negative control accepted",
        )
    };
    rejects(&previous)?; // A frozen V1 manifest cannot describe actual V2 current.
    let mut changed_kind = current.clone();
    changed_kind["kind"] = previous["kind"].clone();
    rejects(&changed_kind)?;
    let mut changed_current_sha = current.clone();
    changed_current_sha["successors"][0]["current"]["sha256"] = serde_json::json!("unreviewed");
    rejects(&changed_current_sha)?;
    let mut changed_current_length = current.clone();
    changed_current_length["successors"][0]["current"]["bytes"] = serde_json::json!(54800);
    rejects(&changed_current_length)?;
    let mut old_nested_guard = current.clone();
    old_nested_guard["successors"][0]["edits"][7]["insert"] =
        previous["successors"][0]["edits"][7]["insert"].clone();
    rejects(&old_nested_guard)?;
    let mut wrong_index = current.clone();
    wrong_index["successors"][0]["edits"]
        .as_array_mut()
        .unwrap()
        .swap(6, 7);
    rejects(&wrong_index)?;
    let mut changed_remove = current.clone();
    changed_remove["successors"][0]["edits"][7]["remove"] = serde_json::json!([1]);
    rejects(&changed_remove)?;
    let mut extra_edit = current.clone();
    extra_edit["successors"][0]["edits"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({"at":0,"remove":[],"insert":[]}));
    rejects(&extra_edit)?;
    let mut changed_binding = current.clone();
    changed_binding["bindings"][0]["container_sha256"] = serde_json::json!("unreviewed");
    rejects(&changed_binding)?;
    let mut changed_other_source = current.clone();
    changed_other_source["successors"][1]["current"]["bytes"] = serde_json::json!(0);
    rejects(&changed_other_source)?;
    let mut changed_mode = current.clone();
    changed_mode["successors"][0]["current"]["mode"] = serde_json::json!("0755");
    rejects(&changed_mode)?;
    let mut changed_delta = current.clone();
    changed_delta["successors"][0]["edits"][7]["insert"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!(0));
    rejects(&changed_delta)?;
    let mut moved_delta = current.clone();
    moved_delta["successors"][0]["edits"][7]["at"] = serde_json::json!(19533);
    rejects(&moved_delta)?;
    let mut changed_capture = current.clone();
    changed_capture["frozen_captures"][0]["sha256"] = serde_json::json!("unreviewed");
    rejects(&changed_capture)?;
    let mut changed_history = current.clone();
    changed_history["historical_chain_materials"][0]["sha256"] = serde_json::json!("unreviewed");
    rejects(&changed_history)?;
    let mut changed_additional = current.clone();
    changed_additional["additional_current_materials"][0]["sha256"] =
        serde_json::json!("unreviewed");
    rejects(&changed_additional)?;
    require(
        current_manifest_document(&previous_raw).is_err(),
        "PR1034 frozen V1 bytes accepted as V2 current manifest",
    )?;
    let mut changed_manifest = fs::read(root.join(CURRENT_MANIFEST)).map_err(|e| e.to_string())?;
    changed_manifest.push(0);
    require(
        current_manifest_document(&changed_manifest).is_err(),
        "PR1034 altered V2 bytes accepted",
    )?;
    eprintln!(
        "PR1034_V2_TRANSITION_CONTROLS positive=true unrelated_bindings_sources_mode_delta_offset_capture_history_additional_and_manifest_negatives=passed"
    );
    Ok(())
}
fn load_v2(root: &Path) -> Result<Value, String> {
    let raw = fs::read(root.join(MANIFEST)).map_err(|e| e.to_string())?;
    require(
        digest(&raw) == MANIFEST_SHA,
        "PR1034 successor manifest changed",
    )?;
    let value: Value = serde_json::from_slice(&raw).map_err(|e| e.to_string())?;
    require(
        value["kind"] == "PR1034_SCOPED_CURRENT_AND_HISTORY_V1"
            && value["bindings"].as_array().is_some_and(|a| a.len() == 11)
            && value["successors"].as_array().is_some_and(|a| a.len() == 4),
        "PR1034 fixed source membership",
    )?;
    let current_raw = fs::read(root.join(CURRENT_MANIFEST)).map_err(|e| e.to_string())?;
    let current = current_manifest_document(&current_raw)?;
    validate_manifest_transition(&value, &current)?;
    Ok(current)
}
/// Unlisted pairs return None before reading any successor material. Existing
/// verification is called unchanged by the proposed wrapper for that branch.
pub(crate) fn try_verify(
    root: &Path,
    container_path: &str,
    container_sha: &str,
    source_path: &str,
    expected_sha: &str,
    expected_bytes: Option<usize>,
) -> Option<Result<(), String>> {
    if !selected(container_path, source_path) {
        return None;
    }
    Some((|| {
        let manifest = load(root)?;
        let binding = manifest["bindings"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["container_path"] == container_path && r["source_path"] == source_path)
            .ok_or("PR1034 exact binding absent")?;
        let container =
            a06_source_successor::historical_caller(root, container_path, container_sha)?;
        check_caller(
            binding,
            container_sha,
            &container,
            expected_sha,
            expected_bytes,
        )?;
        for key in ["additional_current_materials", "frozen_captures"] {
            for row in manifest[key]
                .as_array()
                .ok_or("PR1034 payload set absent")?
            {
                if key == "additional_current_materials" && row["path"] == XTASK_A13 {
                    verify_merged_a13(root, row)?;
                } else if key == "additional_current_materials" && row["path"] == VALIDATOR {
                    verify_validator_composition(root, &manifest, row)?;
                } else {
                    read_material(root, row, "PR1034 current/capture payload")?;
                }
            }
        }
        let mut proof = Vec::new();
        for successor in manifest["successors"].as_array().unwrap() {
            let current = current(root, successor)?;
            let prior = historical(root, successor)?;
            patch_matches(successor, &current, &prior)?;
            if current.path == source_path {
                verify_history(root, &manifest, binding, &prior)?;
            }
            proof.push(serde_json::json!({"current_path":current.path,"current_sha256":digest(&current.bytes),"current_bytes":current.bytes.len(),"historical191_sha256":digest(&prior.bytes),"historical191_bytes":prior.bytes.len()}));
        }
        eprintln!(
            "PR1034_SCOPED_CURRENT_HISTORY_PROOF {}",
            serde_json::json!({"container":container_path,"container_sha256":container_sha,"caller_sha256":expected_sha,"caller_bytes":expected_bytes,"materials":proof})
        );
        Ok(())
    })())
}
#[cfg(test)]
pub(crate) fn controls(root: &Path) -> Result<(), String> {
    factory_controls(root)?;
    let manifest = load(root)?;
    manifest_transition_controls(root, &load_v2(root)?)?;
    // Original version/control relations execute on explicitly archived accepted
    // main material; new actual-current controls are separate and mandatory.
    let accepted_main_manifest = load_v3(root)?;
    additive_composition_controls(root, &accepted_main_manifest)?;
    a13_composition_controls(root, &accepted_main_manifest)?;
    a06_source_successor::controls(root)?;
    for binding in manifest["bindings"].as_array().unwrap() {
        let container_path = text(binding, "container_path")?;
        let source = text(binding, "source_path")?;
        let sha = text(binding, "container_sha256")?;
        let expected = text(binding, "caller_expected_sha256")?;
        let length = binding["caller_expected_bytes"]
            .as_u64()
            .map(|n| n as usize);
        try_verify(root, container_path, sha, source, expected, length)
            .ok_or("missing real positive route")??;
        verify_current(root, container_path, sha, source, expected, length)?;
        require(
            super::verify(root, container_path, sha, source, expected, length).is_err(),
            "old actual-current policy unexpectedly accepted a reviewed successor",
        )?;
        let container = a06_source_successor::historical_caller(root, container_path, sha)?;
        require(
            check_caller(binding, "wrong", &container, expected, length).is_err(),
            "wrong container accepted",
        )?;
        require(
            check_caller(binding, sha, &container, "wrong", length).is_err(),
            "wrong original accepted",
        )?;
        let mut changed_container = container.clone();
        changed_container.push(0);
        require(
            check_caller(binding, sha, &changed_container, expected, length).is_err(),
            "changed actual container bytes accepted",
        )?;
        let bad_length = if length.is_some() { None } else { Some(0) };
        require(
            check_caller(binding, sha, &container, expected, bad_length).is_err(),
            "wrong Some/None accepted",
        )?;
        require(
            try_verify(root, "unknown.json", sha, source, expected, length).is_none(),
            "unknown caller intercepted",
        )?;
        require(
            try_verify(root, container_path, sha, "unknown.rs", expected, length).is_none(),
            "unknown source intercepted",
        )?;
    }
    for successor in manifest["successors"].as_array().unwrap() {
        let current = current(root, successor)?;
        let prior = historical(root, successor)?;
        let mut bad_current = current.bytes.clone();
        bad_current.push(0);
        require(
            check_material(&successor["current"], &bad_current, "0644", "CURRENT").is_err(),
            "current mutation accepted",
        )?;
        let mut bad_archive = prior.bytes.clone();
        bad_archive.push(0);
        require(
            check_material(
                &successor["historical191"],
                &bad_archive,
                "0644",
                "HISTORICAL",
            )
            .is_err(),
            "archive mutation accepted",
        )?;
        require(
            check_material(&successor["current"], &current.bytes, "0755", "CURRENT").is_err(),
            "current mode mutation accepted",
        )?;
        let mut bad = successor.clone();
        bad["edits"] = serde_json::json!([]);
        require(
            patch_matches(&bad, &current, &prior).is_err(),
            "omitted patch accepted",
        )?;
    }
    for row in manifest["historical_chain_materials"].as_array().unwrap() {
        let mut raw = read_material(root, row, "history control")?;
        raw.push(0);
        require(
            check_material(row, &raw, "0644", "HISTORY").is_err(),
            "history chain mutation accepted",
        )?;
    }
    Ok(())
}

/// Only this module can construct validated historical control evidence.
#[cfg(test)]
pub(crate) struct HistoricalControlEvidence(HistoricalEvidence);
#[cfg(test)]
impl HistoricalControlEvidence {
    pub(crate) fn accepted_v6_successor_bytes(&self) -> &[u8] {
        &self.0.bytes
    }
}
/// Exact candidate scope only. All other actual-current contexts retain the
/// original rejects_mutants entrypoint, including its original current read.
#[cfg(test)]
pub(crate) fn control_bridge(root: &Path) -> Result<(), String> {
    let actual = fs::read(root.join(HTTP)).map_err(|e| e.to_string())?;
    if digest(&actual) != CURRENT_HTTP_SHA && !a06_source_successor::current_http(root, &actual)? {
        return super::rejects_mutants(root);
    }
    let manifest = load(root)?;
    let binding = manifest["bindings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["source_path"] == HTTP)
        .ok_or("control bridge HTTP binding absent")?;
    let container = text(binding, "container_path")?;
    let sha = text(binding, "container_sha256")?;
    let old = text(binding, "caller_expected_sha256")?;
    let length = binding["caller_expected_bytes"]
        .as_u64()
        .map(|n| n as usize);
    try_verify(root, container, sha, HTTP, old, length)
        .ok_or("control bridge successor scope absent")??;
    require(
        super::verify(root, container, sha, HTTP, old, length).is_err(),
        "original V6 policy unexpectedly accepted actual new CURRENT",
    )?;
    let successor = manifest["successors"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["current"]["path"] == HTTP)
        .ok_or("control bridge successor absent")?;
    let real_current = current(root, successor)?;
    let historical = historical(root, successor)?;
    patch_matches(successor, &real_current, &historical)?;
    verify_history(root, &manifest, binding, &historical)?;
    let evidence = HistoricalControlEvidence(historical);
    super::rejects_mutants_on_historical_evidence(root, &evidence)?;
    controls(root)?;
    original_routing_equivalence(root)?;
    eprintln!(
        "PR1034_CONTROL_BRIDGE legacy_policy_rejected_actual_current=true historical_v6_original_negatives=passed actual_current_successor_controls=passed"
    );
    Ok(())
}

/// Explicit opt-in routing. The original verifier remains callable with actual
/// current bytes; all unlisted caller/source pairs retain that exact path.
pub(crate) fn verify_current(
    root: &Path,
    container_path: &str,
    container_sha: &str,
    source_path: &str,
    expected_sha: &str,
    expected_bytes: Option<usize>,
) -> Result<(), String> {
    if let Some(result) = a06_source_successor::try_verify(
        root,
        container_path,
        container_sha,
        source_path,
        expected_sha,
        expected_bytes,
    ) {
        return result;
    }
    if factory_selected(container_path, source_path) {
        return verify_factory_current(root, container_sha, expected_sha, expected_bytes);
    }
    match try_verify(
        root,
        container_path,
        container_sha,
        source_path,
        expected_sha,
        expected_bytes,
    ) {
        Some(result) => result,
        None => super::verify(
            root,
            container_path,
            container_sha,
            source_path,
            expected_sha,
            expected_bytes,
        ),
    }
}

#[cfg(test)]
fn original_routing_equivalence(root: &Path) -> Result<(), String> {
    let v6: Value = serde_json::from_slice(
        &fs::read(root.join(super::MANIFEST_PATH)).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    let mut compared = 0;
    for amendment in v6["amendments"]
        .as_array()
        .ok_or("V6 routing scope absent")?
    {
        let source = text(amendment, "source_path")?;
        for pin in amendment["historical_pins"]
            .as_array()
            .ok_or("V6 routing pins absent")?
        {
            let container = text(pin, "container_path")?;
            if selected(container, source) || a06_source_successor::selected(container, source) {
                continue;
            }
            let hash = text(pin, "container_sha256")?;
            let source_hash = text(pin, "source_sha256")?;
            let length = pin["source_bytes"].as_u64().map(|n| n as usize);
            for caller_hash in [hash, "mutated-container-hash"] {
                require(
                    verify_current(root, container, caller_hash, source, source_hash, length)
                        == super::verify(root, container, caller_hash, source, source_hash, length),
                    "nonmatching current-read path changed",
                )?;
                compared += 1;
            }
        }
    }
    require(compared > 0, "no original fallback callers compared")?;
    eprintln!("PR1034_ORIGINAL_ROUTING_EQUIVALENCE comparisons={compared}");
    Ok(())
}

// Finite security/A07 successor: old manifests remain immutable historical evidence.
const CURRENT_V3: &str = "docs/rust-rewrite/a04/source-amendments/scoped-successor-v3.json";
const CURRENT_V3_SHA: &str = "b90370a9d8bd50e79e8468c73eef863647dd0c18d5c649f6baf38e8f3423d6d4";
const A13_COMPOSITION_V2: &str =
    "docs/rust-rewrite/a04/source-amendments/a13-w07-composition-v2.json";
const A13_COMPOSITION_V2_SHA: &str =
    "11de2279f9f204fa6b68ea1fd7a2a0198e2ae3c2910b9750b07d700af6cc9e8d";
const VALIDATOR: &str = "docs/rust-rewrite/a13/source-amendments/validator.rs";
const VALIDATOR_EDITS_SHA: &str =
    "ccf2197e181a1ea16f951040cf8910e0d5643ea7fd40c2acadd01ccb8f05fe5d";
const A13_EDITS_SHA: &str = "7ba41c83724c7537750e782867b8f9a2341140c35636e5f250c72e93026129b6";

fn historical_a13_row(root: &Path) -> Result<Value, String> {
    let raw = fs::read(root.join(MANIFEST)).map_err(|e| e.to_string())?;
    require(digest(&raw) == MANIFEST_SHA, "PR1034 historical V1 differs")?;
    let document: Value = serde_json::from_slice(&raw).map_err(|e| e.to_string())?;
    document["additional_current_materials"]
        .as_array()
        .ok_or("V1 materials absent")?
        .iter()
        .find(|row| row["path"] == XTASK_A13)
        .cloned()
        .ok_or("V1 A13 row absent".into())
}
fn a13_composition_v2_document(root: &Path) -> Result<Value, String> {
    let raw = fs::read(root.join(A13_COMPOSITION_V2)).map_err(|e| e.to_string())?;
    require(
        digest(&raw) == A13_COMPOSITION_V2_SHA,
        "PR1034 A13 composition V2 differs",
    )?;
    let document: Value = serde_json::from_slice(&raw).map_err(|e| e.to_string())?;
    require(
        document["kind"] == "PR1034_W07_SECURITY_A07_COMPOSITION_V2"
            && document["predecessor_composition"]["path"] == A13_COMPOSITION
            && document["predecessor_composition"]["sha256"] == A13_COMPOSITION_SHA,
        "PR1034 A13 composition predecessor/scope differs",
    )?;
    Ok(document)
}
fn check_source_transition(
    transition: &Value,
    before: &[u8],
    actual: &[u8],
    actual_mode: &str,
    source: &str,
    edits_sha: &str,
) -> Result<(), String> {
    require(
        [VALIDATOR, XTASK_A13].contains(&source)
            && transition["current"]["path"] == source
            && transition["predecessor_archive"]["source_path"] == source,
        "PR1034 source transition scope differs",
    )?;
    check_material(
        &transition["predecessor_archive"],
        before,
        "0644",
        "PR1034 source transition predecessor differs",
    )?;
    check_material(
        &transition["current"],
        actual,
        actual_mode,
        "PR1034 source transition CURRENT differs",
    )?;
    let edits = transition["edits"]
        .as_array()
        .ok_or("PR1034 transition edits absent")?;
    require(
        !edits.is_empty()
            && digest(&serde_json::to_vec(edits).map_err(|e| e.to_string())?) == edits_sha,
        "PR1034 exact reviewed transition edits differ",
    )?;
    let mut output = Vec::new();
    let mut end = 0;
    for edit in edits {
        let at = edit["at"]
            .as_u64()
            .and_then(|n| usize::try_from(n).ok())
            .ok_or("PR1034 transition offset")?;
        let remove: Vec<u8> =
            serde_json::from_value(edit["remove"].clone()).map_err(|e| e.to_string())?;
        let insert: Vec<u8> =
            serde_json::from_value(edit["insert"].clone()).map_err(|e| e.to_string())?;
        let next = at
            .checked_add(remove.len())
            .ok_or("PR1034 transition overflow")?;
        require(
            at >= end
                && next <= before.len()
                && before[at..next] == remove
                && (!remove.is_empty() || !insert.is_empty()),
            "PR1034 transition preimage/order differs",
        )?;
        output.extend_from_slice(&before[end..at]);
        output.extend_from_slice(&insert);
        end = next;
    }
    output.extend_from_slice(&before[end..]);
    require(
        output == actual,
        "PR1034 reconstructed source differs from actual CURRENT",
    )
}
fn validate_v3(previous: &Value, next: &Value) -> Result<(), String> {
    require(
        previous["kind"] == "PR1034_SCOPED_CURRENT_AND_HISTORY_V2"
            && next["kind"] == "PR1034_SCOPED_CURRENT_AND_HISTORY_V3"
            && next["a13_composition"]["path"] == A13_COMPOSITION_V2
            && next["a13_composition"]["sha256"] == A13_COMPOSITION_V2_SHA,
        "PR1034 V3 kind/composition differs",
    )?;
    let mut reverted = next.clone();
    let old = previous["additional_current_materials"]
        .as_array()
        .ok_or("PR1034 V2 materials absent")?;
    let rows = reverted["additional_current_materials"]
        .as_array_mut()
        .ok_or("PR1034 V3 materials absent")?;
    require(rows.len() == old.len(), "PR1034 V3 material count differs")?;
    let mut changed = 0;
    for (row, prior) in rows.iter_mut().zip(old) {
        if [VALIDATOR, XTASK_A13]
            .iter()
            .any(|path| prior["path"] == *path)
        {
            require(
                row["path"] == prior["path"] && row["mode"] == prior["mode"],
                "PR1034 V3 material path/mode differs",
            )?;
            *row = prior.clone();
            changed += 1;
        }
    }
    require(
        changed == 2,
        "PR1034 V3 exact two-material transition absent",
    )?;
    reverted["kind"] = previous["kind"].clone();
    for key in [
        "validator_transition",
        "a13_composition",
        "helper_predecessor_archive",
    ] {
        require(
            reverted
                .as_object_mut()
                .ok_or("PR1034 V3 object absent")?
                .remove(key)
                .is_some(),
            "PR1034 V3 required proof absent",
        )?;
    }
    require(
        reverted == *previous,
        "PR1034 unrelated V2/V3 material differs",
    )
}
fn load_v3(root: &Path) -> Result<Value, String> {
    let previous = load_v2(root)?;
    let raw = fs::read(root.join(CURRENT_V3)).map_err(|e| e.to_string())?;
    require(digest(&raw) == CURRENT_V3_SHA, "PR1034 V3 manifest differs")?;
    let next: Value = serde_json::from_slice(&raw).map_err(|e| e.to_string())?;
    validate_v3(&previous, &next)?;
    let helper = read_material(
        root,
        &next["helper_predecessor_archive"],
        "PR1034 historical helper differs",
    )?;
    require(
        digest(&helper) == "edb8ac754f54c78da747296bf0a115ef4d04d289e9ba10b8f76a32168c092c2a",
        "PR1034 helper predecessor identity differs",
    )?;
    Ok(next)
}
fn validate_validator_predecessor(previous: &Value, document: &Value) -> Result<(), String> {
    let prior = previous["additional_current_materials"]
        .as_array()
        .ok_or("PR1034 V2 materials absent")?
        .iter()
        .find(|row| row["path"] == VALIDATOR)
        .ok_or("PR1034 V2 validator absent")?;
    let archive = &document["validator_transition"]["predecessor_archive"];
    require(
        archive["source_path"] == prior["path"]
            && archive["sha256"] == prior["sha256"]
            && archive["bytes"] == prior["bytes"]
            && archive["mode"] == prior["mode"],
        "PR1034 validator predecessor is not the sealed V2 material",
    )
}
fn verify_validator_composition(root: &Path, document: &Value, row: &Value) -> Result<(), String> {
    validate_validator_predecessor(&load_v2(root)?, document)?;
    let transition = &document["validator_transition"];
    let (accepted_main_row, accepted_main_bytes) =
        a06_source_successor::validator_baseline(root, row)?;
    require(
        transition["current"] == accepted_main_row,
        "PR1034 validator accepted-main predecessor row differs",
    )?;
    let old = read_material(
        root,
        &transition["predecessor_archive"],
        "PR1034 HISTORICAL validator",
    )?;
    let actual = read_material(root, row, "PR1034 CURRENT validator")?;
    check_source_transition(
        transition,
        &old,
        &accepted_main_bytes,
        "0644", // Archived mode already checked by the new exact transition.
        VALIDATOR,
        VALIDATOR_EDITS_SHA,
    )?;
    eprintln!(
        "PR1034_VALIDATOR_ADDITIVE_COMPOSITION_PROOF current_sha256={} historical_sha256={}",
        digest(&actual),
        digest(&old)
    );
    Ok(())
}
#[cfg(test)]
fn additive_composition_controls(root: &Path, document: &Value) -> Result<(), String> {
    let previous = load_v2(root)?;
    validate_v3(&previous, document)?;
    validate_validator_predecessor(&previous, document)?;
    let mut wrong_predecessor = document.clone();
    let mut altered_predecessor = read_material(
        root,
        &document["validator_transition"]["predecessor_archive"],
        "PR1034 control validator predecessor",
    )?;
    altered_predecessor.push(0);
    wrong_predecessor["validator_transition"]["predecessor_archive"]["sha256"] =
        serde_json::json!(digest(&altered_predecessor));
    wrong_predecessor["validator_transition"]["predecessor_archive"]["bytes"] =
        serde_json::json!(altered_predecessor.len());
    require(
        validate_validator_predecessor(&previous, &wrong_predecessor)
            .err()
            .as_deref()
            == Some("PR1034 validator predecessor is not the sealed V2 material"),
        "PR1034 rehashed validator predecessor escaped the V2 link",
    )?;
    for key in [
        "bindings",
        "successors",
        "historical_chain_materials",
        "frozen_captures",
    ] {
        let mut bad = document.clone();
        bad[key]
            .as_array_mut()
            .ok_or("PR1034 control array absent")?
            .pop();
        require(
            validate_v3(&previous, &bad).is_err(),
            "PR1034 V3 unrelated scope mutation accepted",
        )?;
    }
    let a13 = a13_composition_v2_document(root)?;
    for (transition, source, edits_sha) in [
        (
            &document["validator_transition"],
            VALIDATOR,
            VALIDATOR_EDITS_SHA,
        ),
        (&a13, XTASK_A13, A13_EDITS_SHA),
    ] {
        let old = read_material(
            root,
            &transition["predecessor_archive"],
            "PR1034 control predecessor",
        )?;
        let actual = if source == XTASK_A13 {
            a06_source_successor::a13_baseline(root, &transition["current"])?.1
        } else if source == VALIDATOR {
            a06_source_successor::validator_baseline(root, &transition["current"])?.1
        } else {
            read_material(root, &transition["current"], "PR1034 control CURRENT")?
        };
        let positive = |claim: &Value, bytes: &[u8], permissions: &str| {
            check_source_transition(claim, &old, bytes, permissions, source, edits_sha)
        };
        positive(transition, &actual, "0644")?;
        require(
            positive(transition, &old, "0644").is_err(),
            "PR1034 predecessor accepted as CURRENT",
        )?;
        require(
            positive(transition, &actual, "0755").is_err(),
            "PR1034 changed mode accepted",
        )?;
        let mut altered = actual.clone();
        altered.push(0);
        let mut rehashed = transition.clone();
        rehashed["current"]["sha256"] = serde_json::json!(digest(&altered));
        rehashed["current"]["bytes"] = serde_json::json!(altered.len());
        require(
            positive(&rehashed, &altered, "0644").is_err(),
            "PR1034 hash-only unrelated successor accepted",
        )?;
        // Test-only recomputed digests bypass the authority check deliberately,
        // so these controls must reach the structural/reconstruction guards.
        let structural_error = |claim: &Value, bytes: &[u8]| -> Result<String, String> {
            let proposed = digest(
                &serde_json::to_vec(claim["edits"].as_array().ok_or("control edits absent")?)
                    .map_err(|e| e.to_string())?,
            );
            check_source_transition(claim, &old, bytes, "0644", source, &proposed)
                .err()
                .ok_or("structurally invalid transition accepted".into())
        };
        let mut missing_edit = transition.clone();
        missing_edit["edits"].as_array_mut().unwrap().pop();
        require(
            structural_error(&missing_edit, &actual)?
                == "PR1034 reconstructed source differs from actual CURRENT",
            "PR1034 missing edit did not reach reconstruction guard",
        )?;
        let mut wrong_preimage = transition.clone();
        let at = wrong_preimage["edits"][0]["at"].as_u64().unwrap() as usize;
        wrong_preimage["edits"][0]["remove"] = serde_json::json!([old[at] ^ 1]);
        require(
            structural_error(&wrong_preimage, &actual)?
                == "PR1034 transition preimage/order differs",
            "PR1034 wrong preimage did not reach structural guard",
        )?;
        let mut reversed = transition.clone();
        reversed["edits"].as_array_mut().unwrap().swap(0, 1);
        require(
            structural_error(&reversed, &actual)? == "PR1034 transition preimage/order differs",
            "PR1034 reordered edits did not reach structural guard",
        )?;
        let mut moved = transition.clone();
        moved["edits"][0]["at"] = serde_json::json!(old.len() + 1);
        require(
            structural_error(&moved, &actual)? == "PR1034 transition preimage/order differs",
            "PR1034 moved edit did not reach bounds guard",
        )?;
        let mut repeated = transition.clone();
        let first = repeated["edits"][0].clone();
        repeated["edits"].as_array_mut().unwrap().insert(1, first);
        let repeated_error = structural_error(&repeated, &actual)?;
        require(
            [
                "PR1034 transition preimage/order differs",
                "PR1034 reconstructed source differs from actual CURRENT",
            ]
            .contains(&repeated_error.as_str()),
            "PR1034 duplicate edit did not reach structural/reconstruction guard",
        )?;
        require(
            structural_error(&rehashed, &altered)?
                == "PR1034 reconstructed source differs from actual CURRENT",
            "PR1034 rehashed unrelated CURRENT did not reach reconstruction guard",
        )?;
        for mutation in 0..6 {
            let mut bad = transition.clone();
            match mutation {
                0 => {
                    bad["edits"].as_array_mut().unwrap().pop();
                }
                1 => {
                    let edit = bad["edits"][0].clone();
                    bad["edits"].as_array_mut().unwrap().push(edit);
                }
                2 => {
                    bad["edits"][0]["at"] =
                        serde_json::json!(bad["edits"][0]["at"].as_u64().unwrap() + 1);
                }
                3 => {
                    bad["current"]["path"] = serde_json::json!("unreviewed.rs");
                }
                4 => {
                    bad["predecessor_archive"]["sha256"] = serde_json::json!("unreviewed");
                }
                _ => {
                    bad["edits"][0]["insert"]
                        .as_array_mut()
                        .unwrap()
                        .push(serde_json::json!(0));
                }
            }
            require(
                positive(&bad, &actual, "0644").is_err(),
                "PR1034 exact source transition negative accepted",
            )?;
        }
    }
    eprintln!(
        "PR1034_SECURITY_A07_COMPOSITION_CONTROLS accepted_main_predecessor_positive=true actual_A06_current_separately_reconstructed=true historical_as_current_mode_scope_missing_duplicate_moved_unrelated_edits_rejected=true"
    );
    Ok(())
}

// One exact A09 fixture successor. Frozen coverage and every legacy route remain
// unchanged; only this caller/source pair opts into the reconstructive sidecar.
const FACTORY_CONTAINER: &str = "docs/rust-rewrite/a09/coverage.json";
const FACTORY_CONTAINER_SHA: &str =
    "d3dbf8be85061348dded0b783a55dd11f5d79e789857e2046ebe808504f162c8";
const FACTORY_SOURCE: &str = "crates/ssx-infra/tests/a09_factories.rs";
const FACTORY_OLD_SHA: &str = "c7eb4cb261ab85764d3ebe3e7bd1c6648469aadaa1e61fb9b6b51077f3faa8da";
const FACTORY_ARCHIVE: &str =
    "docs/rust-rewrite/a09/source-amendments/52c3ce7d-a09-factories.rs.frozen";
const FACTORY_MANIFEST: &str =
    "docs/rust-rewrite/a09/source-amendments/factory-harness-successor-v1.json";
const FACTORY_MANIFEST_SHA: &str =
    "5b35cc651429a2db741782354841bfa398ecb10ddabdc8cd2a15b73dc0ec33b4";
const FACTORY_EDITS_SHA: &str = "c167ea6c274384e276b2e144d58590c0824141e55b0e94fdbfa19fe3e57a030c";
fn factory_selected(container: &str, source: &str) -> bool {
    container == FACTORY_CONTAINER && source == FACTORY_SOURCE
}
fn factory_scope(document: &Value) -> Result<(), String> {
    require(
        document["kind"] == "PR1034_A09_FACTORY_HARNESS_SUCCESSOR_V1"
            && document["baseline_revision"] == "52c3ce7d481c35e74eb1afe1740ec840b5571567"
            && document["binding"]
                == serde_json::json!({
                    "container_path": FACTORY_CONTAINER, "container_sha256": FACTORY_CONTAINER_SHA,
                    "source_path": FACTORY_SOURCE, "caller_expected_sha256": FACTORY_OLD_SHA,
                    "caller_expected_bytes": null, "row": "artifacts/79"
                })
            && document["current"]["path"] == FACTORY_SOURCE
            && document["current"]["mode"] == "0644"
            && document["historical_archive"]
                == serde_json::json!({
                    "path": FACTORY_ARCHIVE, "source_path": FACTORY_SOURCE,
                    "sha256": FACTORY_OLD_SHA, "bytes": 28678, "mode": "0644"
                }),
        "PR1034 factory harness caller/archive scope differs",
    )
}
fn factory_document(raw: &[u8]) -> Result<Value, String> {
    require(
        digest(raw) == FACTORY_MANIFEST_SHA,
        "PR1034 factory harness manifest differs",
    )?;
    let document: Value = serde_json::from_slice(raw).map_err(|e| e.to_string())?;
    factory_scope(&document)?;
    Ok(document)
}
fn factory_transition(
    document: &Value,
    current: &CurrentSuccessor,
    previous: &HistoricalEvidence,
    current_mode: &str,
    previous_mode: &str,
    edits_sha: &str,
) -> Result<(), String> {
    factory_scope(document)?;
    require(
        current.path == FACTORY_SOURCE && previous.path == FACTORY_SOURCE,
        "PR1034 factory harness typed material scope differs",
    )?;
    check_material(
        &document["historical_archive"],
        &previous.bytes,
        previous_mode,
        "PR1034 factory harness historical bytes/mode differ",
    )?;
    check_material(
        &document["current"],
        &current.bytes,
        current_mode,
        "PR1034 factory harness CURRENT bytes/mode differ",
    )?;
    let edits = document["edits"]
        .as_array()
        .ok_or("PR1034 factory harness edits absent")?;
    require(
        !edits.is_empty()
            && digest(&serde_json::to_vec(edits).map_err(|e| e.to_string())?) == edits_sha,
        "PR1034 exact factory harness edits differ",
    )?;
    patch_matches(document, current, previous)
}
fn verify_factory_current(
    root: &Path,
    container_sha: &str,
    expected_sha: &str,
    expected_bytes: Option<usize>,
) -> Result<(), String> {
    let raw = fs::read(root.join(FACTORY_MANIFEST)).map_err(|e| e.to_string())?;
    let document = factory_document(&raw)?;
    let container =
        a06_source_successor::historical_caller(root, FACTORY_CONTAINER, container_sha)?;
    check_caller(
        &document["binding"],
        container_sha,
        &container,
        expected_sha,
        expected_bytes,
    )?;
    // CURRENT is always read from the actual fixture path. The archive is only
    // the historical operand, even if a caller tries to supply its old digest.
    let current = CurrentSuccessor {
        path: FACTORY_SOURCE.into(),
        bytes: read_material(root, &document["current"], "PR1034 factory CURRENT")?,
    };
    let previous = HistoricalEvidence {
        path: FACTORY_SOURCE.into(),
        bytes: read_material(
            root,
            &document["historical_archive"],
            "PR1034 factory HISTORICAL",
        )?,
    };
    factory_transition(
        &document,
        &current,
        &previous,
        &mode(&root.join(FACTORY_SOURCE))?,
        &mode(&root.join(FACTORY_ARCHIVE))?,
        FACTORY_EDITS_SHA,
    )?;
    eprintln!(
        "PR1034_FACTORY_HARNESS_SOURCE_PROOF current_sha256={} historical_sha256={}",
        digest(&current.bytes),
        digest(&previous.bytes)
    );
    Ok(())
}
#[cfg(test)]
fn factory_controls(root: &Path) -> Result<(), String> {
    let raw = fs::read(root.join(FACTORY_MANIFEST)).map_err(|e| e.to_string())?;
    let document = factory_document(&raw)?;
    let container =
        a06_source_successor::historical_caller(root, FACTORY_CONTAINER, FACTORY_CONTAINER_SHA)?;
    let current = CurrentSuccessor {
        path: FACTORY_SOURCE.into(),
        bytes: read_material(root, &document["current"], "factory control CURRENT")?,
    };
    let previous = HistoricalEvidence {
        path: FACTORY_SOURCE.into(),
        bytes: read_material(
            root,
            &document["historical_archive"],
            "factory control HISTORICAL",
        )?,
    };
    let current_mode = mode(&root.join(FACTORY_SOURCE))?;
    let previous_mode = mode(&root.join(FACTORY_ARCHIVE))?;
    factory_transition(
        &document,
        &current,
        &previous,
        &current_mode,
        &previous_mode,
        FACTORY_EDITS_SHA,
    )?;
    verify_current(
        root,
        FACTORY_CONTAINER,
        FACTORY_CONTAINER_SHA,
        FACTORY_SOURCE,
        FACTORY_OLD_SHA,
        None,
    )?;
    require(
        super::verify(
            root,
            FACTORY_CONTAINER,
            FACTORY_CONTAINER_SHA,
            FACTORY_SOURCE,
            FACTORY_OLD_SHA,
            None,
        )
        .is_err(),
        "original unamended policy unexpectedly accepted actual fixture CURRENT",
    )?;
    for (hash, source_hash, length) in [
        ("wrong-container", FACTORY_OLD_SHA, None),
        (FACTORY_CONTAINER_SHA, "wrong-original", None),
        (FACTORY_CONTAINER_SHA, FACTORY_OLD_SHA, Some(28678)),
    ] {
        require(
            check_caller(&document["binding"], hash, &container, source_hash, length).is_err(),
            "factory caller hash/original/Some-None mutation accepted",
        )?;
    }
    let mut altered_container = container.clone();
    altered_container.push(0);
    require(
        check_caller(
            &document["binding"],
            FACTORY_CONTAINER_SHA,
            &altered_container,
            FACTORY_OLD_SHA,
            None,
        )
        .is_err(),
        "factory changed actual container accepted",
    )?;
    require(
        !factory_selected("unknown.json", FACTORY_SOURCE)
            && !factory_selected(FACTORY_CONTAINER, "unknown.rs"),
        "factory intercepted an unlisted caller/source",
    )?;
    let mut altered_manifest = raw.clone();
    altered_manifest.push(b'\n');
    require(
        factory_document(&altered_manifest).is_err(),
        "factory changed sidecar accepted",
    )?;
    require(
        factory_transition(
            &document,
            &current,
            &previous,
            "0755",
            &previous_mode,
            FACTORY_EDITS_SHA,
        )
        .is_err(),
        "factory CURRENT mode mutation accepted",
    )?;
    require(
        factory_transition(
            &document,
            &current,
            &previous,
            &current_mode,
            "0755",
            FACTORY_EDITS_SHA,
        )
        .is_err(),
        "factory historical mode mutation accepted",
    )?;
    let historical_as_current = CurrentSuccessor {
        path: FACTORY_SOURCE.into(),
        bytes: previous.bytes.clone(),
    };
    require(
        factory_transition(
            &document,
            &historical_as_current,
            &previous,
            &current_mode,
            &previous_mode,
            FACTORY_EDITS_SHA,
        )
        .is_err(),
        "factory historical bytes accepted as CURRENT",
    )?;
    let mut wrong_archive = HistoricalEvidence {
        path: FACTORY_SOURCE.into(),
        bytes: previous.bytes.clone(),
    };
    wrong_archive.bytes.push(0);
    require(
        factory_transition(
            &document,
            &current,
            &wrong_archive,
            &current_mode,
            &previous_mode,
            FACTORY_EDITS_SHA,
        )
        .is_err(),
        "factory changed archive accepted",
    )?;
    let mut altered = CurrentSuccessor {
        path: FACTORY_SOURCE.into(),
        bytes: current.bytes.clone(),
    };
    altered.bytes.push(0);
    let mut rehashed = document.clone();
    rehashed["current"]["sha256"] = serde_json::json!(digest(&altered.bytes));
    rehashed["current"]["bytes"] = serde_json::json!(altered.bytes.len());
    require(
        factory_transition(
            &rehashed,
            &altered,
            &previous,
            &current_mode,
            &previous_mode,
            FACTORY_EDITS_SHA,
        )
        .is_err_and(|error| error == "PR1034 exact reviewed patch differs"),
        "factory unrelated rehashed CURRENT bypassed reconstruction",
    )?;
    // Recompute the digest only in these structural controls. Production always
    // supplies the fixed reviewed FACTORY_EDITS_SHA above.
    let mut missing = document.clone();
    missing["edits"].as_array_mut().unwrap().remove(0);
    let mut duplicate = document.clone();
    duplicate["edits"]
        .as_array_mut()
        .unwrap()
        .push(document["edits"][0].clone());
    let mut reordered = document.clone();
    reordered["edits"].as_array_mut().unwrap().swap(0, 1);
    let mut moved = document.clone();
    moved["edits"][0]["at"] = serde_json::json!(moved["edits"][0]["at"].as_u64().unwrap() + 1);
    let mut preimage = document.clone();
    let i = preimage["edits"]
        .as_array()
        .unwrap()
        .iter()
        .position(|edit| !edit["remove"].as_array().unwrap().is_empty())
        .unwrap();
    preimage["edits"][i]["remove"][0] =
        serde_json::json!(preimage["edits"][i]["remove"][0].as_u64().unwrap() ^ 1);
    for claim in [missing, duplicate, reordered, moved, preimage] {
        let edits_sha = digest(&serde_json::to_vec(&claim["edits"]).map_err(|e| e.to_string())?);
        require(
            factory_transition(
                &claim,
                &current,
                &previous,
                &current_mode,
                &previous_mode,
                &edits_sha,
            )
            .is_err_and(|error| {
                error == "PR1034 reviewed edit preimage differs"
                    || error == "PR1034 exact reviewed patch differs"
            }),
            "factory structural edit control did not reach preimage/reconstruction guard",
        )?;
    }
    for (key, value) in [
        ("kind", serde_json::json!("wrong-kind")),
        ("baseline_revision", serde_json::json!("wrong-baseline")),
    ] {
        let mut wrong = document.clone();
        wrong[key] = value;
        require(
            factory_scope(&wrong).is_err(),
            "factory scope mutation accepted",
        )?;
    }
    let mut wrong_row = document.clone();
    wrong_row["binding"]["row"] = serde_json::json!("artifacts/78");
    require(
        check_caller(
            &wrong_row["binding"],
            FACTORY_CONTAINER_SHA,
            &container,
            FACTORY_OLD_SHA,
            None,
        )
        .is_err(),
        "factory moved historical row accepted",
    )?;
    eprintln!(
        "PR1034_FACTORY_HARNESS_SOURCE_CONTROLS actual_current=true historical_as_current_container_mode_preimage_order_missing_duplicate_moved_unrelated_rehashed_rejected=true"
    );
    Ok(())
}

// The immutable V3 remains historical authority. This effective-current value
// is derived only after exact main-preimage/actual-current reconstruction.
fn load(root: &Path) -> Result<Value, String> {
    a06_source_successor::compose_scoped(root, &load_v3(root)?)
}
