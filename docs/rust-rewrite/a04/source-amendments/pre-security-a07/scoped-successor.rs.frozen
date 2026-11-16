//! Unapplied PR1034 draft. Current identity and historical evidence are distinct.
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
struct CurrentMergedA13 {
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
) -> Result<(CurrentMergedA13, HistoricalPremergeMaterial), String> {
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
    let current = CurrentMergedA13 {
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
fn read_a13_composition(
    root: &Path,
    v1_row: &Value,
) -> Result<(Value, CurrentMergedA13, HistoricalPremergeMaterial), String> {
    let raw = fs::read(root.join(A13_COMPOSITION)).map_err(|e| e.to_string())?;
    let composition = a13_composition_document(&raw)?;
    require(
        composition["current"]["path"] == XTASK_A13,
        "PR1034 A13 actual current path changed",
    )?;
    let actual = read_material(root, &composition["current"], "PR1034 CURRENT merged A13")?;
    let actual_mode = mode(&root.join(XTASK_A13))?;
    let (current, historical) =
        a13_composition_payload(v1_row, &composition, &actual, &actual_mode)?;
    Ok((composition, current, historical))
}
fn verify_merged_a13(root: &Path, v1_row: &Value) -> Result<(), String> {
    let (composition, current, historical) = read_a13_composition(root, v1_row)?;
    eprintln!(
        "PR1034_MAIN_COMPOSITION_PROOF {}",
        serde_json::json!({"current_path":XTASK_A13,"current_sha256":digest(&current.bytes),
            "current_bytes":current.bytes.len(),"historical_premerge_sha256":digest(&historical.bytes),
            "historical_premerge_bytes":historical.bytes.len(),"insertion":composition["insertion"]["sha256"],
            "frozen_http_manifest_sha256":MANIFEST_SHA})
    );
    Ok(())
}
#[cfg(test)]
fn a13_composition_controls(root: &Path, manifest: &Value) -> Result<(), String> {
    let rows: Vec<_> = manifest["additional_current_materials"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|row| row["path"] == XTASK_A13)
        .collect();
    require(
        rows.len() == 1,
        "PR1034 A13 composition V1 membership changed",
    )?;
    let row = rows[0];
    let (composition, current, historical) = read_a13_composition(root, row)?;
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
        "PR1034_COMPOSITION_CONTROLS actual_current_positive=true historical_as_current_rejected=true altered_byte_delta_offset_mode_scope_and_relation_negatives=passed"
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
        "PR1034 caller tuple changed",
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
fn load(root: &Path) -> Result<Value, String> {
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
        let container = fs::read(root.join(container_path)).map_err(|e| e.to_string())?;
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
    let manifest = load(root)?;
    manifest_transition_controls(root, &manifest)?;
    a13_composition_controls(root, &manifest)?;
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
        let container = fs::read(root.join(container_path)).map_err(|e| e.to_string())?;
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
    if digest(&actual) != CURRENT_HTTP_SHA {
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
            if selected(container, source) {
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
