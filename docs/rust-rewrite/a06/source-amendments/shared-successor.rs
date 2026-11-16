//! Finite pure-A06 successors of accepted main. Historical manifests stay immutable.
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{fs, path::Path};
use w07_t02_joint::w08_source_successor;
#[path = "a13-header-layout-successor.rs"]
mod a13_header_layout_successor;
#[path = "post-a06-generation.rs"]
mod post_a06_generation;
pub(crate) use post_a06_generation::native_search;
#[path = "real-clock-successor.rs"]
mod real_clock_successor;
#[path = "w07-evidence-successor.rs"]
pub(crate) mod w07_evidence_successor;
#[path = "../../w07/source-amendments/main_successor.rs"]
pub(crate) mod w07_main_successor;
#[path = "w07-t02-joint.rs"]
pub(crate) mod w07_t02_joint;
#[path = "../../w07/source-amendments/test_dependency_successor.rs"]
pub(crate) mod w07_test_dependency;
const MANIFEST: &str = "docs/rust-rewrite/a06/source-amendments/shared-successor-v2.json";
const PREDECESSOR: &str = "docs/rust-rewrite/a06/source-amendments/shared-successor-v1.json";
const VALIDATOR: &str = "docs/rust-rewrite/a13/source-amendments/validator.rs";
const MANIFEST_SHA: &str = "864e6158d6d51cb13793ed4abed3df577c795c1ffb08940eb7309f4ef21efb3b";
const BASELINE: &str = "d5615ace90b827f31789ccf04c72aaad4516e8f7";
#[path = "a13-json-inventory-successor.rs"]
mod a13_json_inventory_successor;
#[path = "a13-marker-successor-v2.rs"]
mod a13_marker_successor;
#[path = "a13-qualification-successor.rs"]
mod a13_qualification_successor;
const A13: &str = "crates/xtask/src/a13.rs";
const SHA_PAIRS: &[(&str, &str)] = &[
    ("docs/rust-rewrite/a09/coverage.json", "Cargo.lock"),
    (
        "docs/rust-rewrite/a09/coverage.json",
        "crates/ssx-application/src/lib.rs",
    ),
    (
        "docs/rust-rewrite/a09/coverage.json",
        "crates/ssx-infra/src/adapters/mod.rs",
    ),
    (
        "docs/rust-rewrite/a09/coverage.json",
        "crates/ssx-infra/src/adapters/source_payload.rs",
    ),
    (
        "docs/rust-rewrite/a09/coverage.json",
        "crates/ssx-infra/tests/a09_catalog_media.rs",
    ),
    (
        "docs/rust-rewrite/a09/coverage.json",
        "crates/xtask/src/main.rs",
    ),
    (
        "docs/rust-rewrite/a09/coverage.json",
        "docs/rust-rewrite/a09/README.md",
    ),
    ("docs/rust-rewrite/a10/qualification.json", "Cargo.lock"),
    ("docs/rust-rewrite/a10/qualification.json", "Cargo.toml"),
    (
        "docs/rust-rewrite/a10/qualification.json",
        "crates/ssx-infra/src/adapters/clinicaltrials.rs",
    ),
    (
        "docs/rust-rewrite/a10/qualification.json",
        "crates/ssx-infra/src/adapters/mod.rs",
    ),
    (
        "docs/rust-rewrite/a10/qualification.json",
        "crates/ssx-infra/src/adapters/source_json.rs",
    ),
    (
        "docs/rust-rewrite/a10/qualification.json",
        "crates/ssx-infra/src/adapters/source_payload.rs",
    ),
    (
        "docs/rust-rewrite/a10/qualification.json",
        "crates/ssx-infra/src/http/monotonic.rs",
    ),
    ("docs/rust-rewrite/a11/qualification.json", "Cargo.lock"),
    ("docs/rust-rewrite/a11/qualification.json", "Cargo.toml"),
    (
        "docs/rust-rewrite/a11/qualification.json",
        "crates/ssx-infra/Cargo.toml",
    ),
    (
        "docs/rust-rewrite/a11/qualification.json",
        "crates/ssx-infra/src/adapters/clinicaltrials.rs",
    ),
    (
        "docs/rust-rewrite/a11/qualification.json",
        "crates/ssx-infra/src/adapters/mod.rs",
    ),
    (
        "docs/rust-rewrite/a11/qualification.json",
        "crates/ssx-infra/src/adapters/source_json.rs",
    ),
    (
        "docs/rust-rewrite/a11/qualification.json",
        "crates/ssx-infra/src/adapters/source_payload.rs",
    ),
    (
        "docs/rust-rewrite/a11/qualification.json",
        "crates/ssx-infra/src/http/monotonic.rs",
    ),
    (
        "docs/rust-rewrite/a11/qualification.json",
        "crates/ssx-json/src/lib.rs",
    ),
    ("docs/rust-rewrite/a12/qualification-v1.json", "Cargo.lock"),
    ("docs/rust-rewrite/a12/qualification-v1.json", "Cargo.toml"),
    (
        "docs/rust-rewrite/a12/qualification-v1.json",
        "crates/ssx-application/src/lib.rs",
    ),
    (
        "docs/rust-rewrite/a12/qualification-v1.json",
        "crates/ssx-infra/src/adapters/mod.rs",
    ),
    (
        "docs/rust-rewrite/a12/qualification-v1.json",
        "crates/ssx-infra/src/adapters/source_json.rs",
    ),
    (
        "docs/rust-rewrite/a12/qualification-v1.json",
        "crates/ssx-infra/src/adapters/source_payload.rs",
    ),
    (
        "docs/rust-rewrite/a12/qualification-v1.json",
        "crates/xtask/src/main.rs",
    ),
    (
        "docs/rust-rewrite/a13/dehashed-source-map-v1.json",
        "Cargo.lock",
    ),
    (
        "docs/rust-rewrite/a13/dehashed-source-map-v1.json",
        "crates/ssx-infra/Cargo.toml",
    ),
    (
        "docs/rust-rewrite/a13/dehashed-source-map-v1.json",
        "crates/ssx-infra/src/adapters/clinicaltrials.rs",
    ),
    (
        "docs/rust-rewrite/a13/dehashed-source-map-v1.json",
        "crates/ssx-infra/src/adapters/mod.rs",
    ),
    (
        "docs/rust-rewrite/a13/dehashed-source-map-v1.json",
        "crates/ssx-infra/src/adapters/source_json.rs",
    ),
    (
        "docs/rust-rewrite/a13/dehashed-source-map-v1.json",
        "crates/ssx-infra/src/adapters/source_payload.rs",
    ),
    ("docs/rust-rewrite/a13/qualification-v1.json", "Cargo.lock"),
    (
        "docs/rust-rewrite/a13/qualification-v1.json",
        "crates/ssx-infra/Cargo.toml",
    ),
    (
        "docs/rust-rewrite/a13/qualification-v1.json",
        "crates/ssx-infra/src/adapters/clinicaltrials.rs",
    ),
    (
        "docs/rust-rewrite/a13/qualification-v1.json",
        "crates/ssx-infra/src/adapters/mod.rs",
    ),
    (
        "docs/rust-rewrite/a13/qualification-v1.json",
        "crates/ssx-infra/src/adapters/source_json.rs",
    ),
    (
        "docs/rust-rewrite/a13/qualification-v1.json",
        "crates/ssx-infra/src/adapters/source_payload.rs",
    ),
    (
        "docs/rust-rewrite/a13/qualification-v1.json",
        "crates/ssx-json/src/lib.rs",
    ),
    (
        "docs/rust-rewrite/a13/qualification-v1.json",
        "docs/rust-rewrite/f08/parity.md",
    ),
    (
        "docs/rust-rewrite/a13/remaining/source-map.json",
        "Cargo.lock",
    ),
    (
        "docs/rust-rewrite/a13/remaining/source-map.json",
        "crates/ssx-infra/Cargo.toml",
    ),
    (
        "docs/rust-rewrite/a13/remaining/source-map.json",
        "crates/ssx-infra/src/adapters/clinicaltrials.rs",
    ),
    (
        "docs/rust-rewrite/a13/remaining/source-map.json",
        "crates/ssx-infra/src/adapters/mod.rs",
    ),
    (
        "docs/rust-rewrite/a13/remaining/source-map.json",
        "crates/ssx-infra/src/adapters/source_json.rs",
    ),
    (
        "docs/rust-rewrite/a13/remaining/source-map.json",
        "crates/ssx-infra/src/adapters/source_payload.rs",
    ),
    (
        "docs/rust-rewrite/a13/remaining/source-map.json",
        "crates/ssx-json/src/lib.rs",
    ),
    (
        "docs/rust-rewrite/a13/remaining/source-map.json",
        "docs/rust-rewrite/f08/parity.md",
    ),
    ("docs/rust-rewrite/o01/coverage.json", "Cargo.lock"),
    (
        "docs/rust-rewrite/o01/coverage.json",
        "crates/ssx-application/src/lib.rs",
    ),
    (
        "docs/rust-rewrite/o01/coverage.json",
        "crates/ssx-infra/src/lib.rs",
    ),
    ("docs/rust-rewrite/w06/qualification.json", "Cargo.lock"),
    (
        "docs/rust-rewrite/w06/qualification.json",
        "crates/ssx-application/src/lib.rs",
    ),
    (
        "docs/rust-rewrite/w06/qualification.json",
        "crates/ssx-infra/Cargo.toml",
    ),
    (
        "docs/rust-rewrite/w06/qualification.json",
        "crates/ssx-infra/src/lib.rs",
    ),
];
const GIT_PAIRS: &[(&str, &str)] = &[
    (
        "docs/rust-rewrite/a14/cases.json",
        "crates/ssx-infra/src/adapters/clinicaltrials.rs",
    ),
    (
        "docs/rust-rewrite/a14/cases.json",
        "crates/ssx-infra/src/adapters/source_json.rs",
    ),
    (
        "docs/rust-rewrite/a14/cases.json",
        "crates/ssx-infra/src/adapters/source_payload.rs",
    ),
];
const TRANSITION_PATHS: &[&str] = &[
    "Cargo.lock",
    "Cargo.toml",
    "crates/ssx-application/src/lib.rs",
    "crates/ssx-infra/Cargo.toml",
    "crates/ssx-infra/src/adapters/clinicaltrials.rs",
    "crates/ssx-infra/src/adapters/mod.rs",
    "crates/ssx-infra/src/adapters/source_json.rs",
    "crates/ssx-infra/src/adapters/source_payload.rs",
    "crates/ssx-infra/src/http.rs",
    "crates/ssx-infra/src/http/monotonic.rs",
    "crates/ssx-infra/src/lib.rs",
    "crates/ssx-infra/tests/a09_catalog_media.rs",
    "crates/ssx-infra/tests/a13_dehashed.rs",
    "crates/ssx-json/src/lib.rs",
    "crates/xtask/src/a04.rs",
    "crates/xtask/src/a13.rs",
    "crates/xtask/src/main.rs",
    "docs/rust-rewrite/a09/README.md",
    "docs/rust-rewrite/f08/parity.md",
    VALIDATOR,
];
fn need(ok: bool, message: &str) -> Result<(), String> {
    if ok { Ok(()) } else { Err(message.into()) }
}
fn digest(raw: &[u8]) -> String {
    Sha256::digest(raw)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
fn raw_mode(mode: u32) -> String {
    format!("{:04o}", mode & 0o7777)
}
#[cfg(unix)]
fn permissions(path: &Path) -> Result<String, String> {
    use std::os::unix::fs::PermissionsExt;
    Ok(raw_mode(
        fs::symlink_metadata(path)
            .map_err(|e| e.to_string())?
            .permissions()
            .mode(),
    ))
}
#[cfg(not(unix))]
fn permissions(_: &Path) -> Result<String, String> {
    Err("A06 source mode proof requires Unix".into())
}
fn text<'a>(row: &'a Value, key: &str) -> Result<&'a str, String> {
    row[key]
        .as_str()
        .ok_or_else(|| format!("A06 missing {key}"))
}
fn material(row: &Value, raw: &[u8], mode: &str) -> Result<(), String> {
    need(
        row["sha256"] == digest(raw)
            && row["bytes"].as_u64() == Some(raw.len() as u64)
            && row["mode"] == mode,
        &format!(
            "A06 exact source material differs: path={:?} expected_sha={} actual_sha={} expected_bytes={:?} actual_bytes={} expected_mode={} actual_mode={}",
            row["path"].as_str().unwrap_or("<missing>"),
            row["sha256"]
                .as_str()
                .filter(|s| s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit()))
                .unwrap_or("<invalid>"),
            digest(raw),
            row["bytes"].as_u64(),
            raw.len(),
            row["mode"]
                .as_str()
                .filter(|s| s.len() == 4 && s.bytes().all(|b| (b'0'..=b'7').contains(&b)))
                .unwrap_or("<invalid>"),
            mode
        ),
    )
}
fn read(root: &Path, row: &Value) -> Result<Vec<u8>, String> {
    if let Some(prior) = post_a06_generation::historical(root, row)? {
        return Ok(prior);
    }
    read_exact(root, row)
}
// Default-pool fixture isolation only. These exact afterimages preserve every
// original assertion; historical oracles/capsules keep their original hashes.
struct DefaultCaFixture {
    path: &'static str,
    before_sha: &'static str,
    before_len: usize,
    current_sha: &'static str,
    current_len: usize,
    insertions: &'static [(usize, &'static [u8])],
}
const DEFAULT_CA_FIXTURES: &[DefaultCaFixture] = &[
    DefaultCaFixture {
        path: "crates/ssx-infra/tests/a08_factories.rs",
        before_sha: "329ff395b82e91eed0a0ab384367bbb745e1011812584391ce03f44210c24a92", before_len: 28889,
        current_sha: "c0978ad1eb426b73043d82ec9691e06f12bd636829d1adbe9275953708185259", current_len: 29122,
        insertions: &[
            (0, b"#[path = \"support/default_ca_environment.rs\"]\nmod default_ca_environment;\n"),
            (14396, b"    if default_ca_environment::default_ca_test_environment(\n        \"all_four_real_network_timers_match_actual_source_loopback\",\n    ) {\n        return;\n    }\n"),
        ],
    },
    DefaultCaFixture {
        path: "crates/ssx-infra/tests/a09_factories.rs",
        before_sha: "09212420429e4017cac93d8918427b0f50bea11304e5f9113d48dce2d8f9e79d", before_len: 36934,
        current_sha: "a23c6ac515cd06ca31147041d8b49e9bc0f0ca41686c0385033e1d58d6b66df7", current_len: 37495,
        insertions: &[
            (79, b"#[path = \"support/default_ca_environment.rs\"]\nmod default_ca_environment;\n"),
            (21076, b"    if default_ca_environment::default_ca_test_environment(\n        \"five_real_timeout_and_expired_context_cleanup\",\n    ) {\n        return;\n    }\n"),
            (23458, b"    if default_ca_environment::default_ca_test_environment(\n        \"archive_real_cdx_timer_one_fallback_and_aggregate_context\",\n    ) {\n        return;\n    }\n"),
            (30830, b"    if default_ca_environment::default_ca_test_environment(\n        \"actual_source_archive_configured_timers_keep_error_and_context_timeout_distinct\",\n    ) {\n        return;\n    }\n"),
        ],
    },
    DefaultCaFixture {
        path: "crates/ssx-infra/tests/support/default_ca_environment.rs",
        before_sha: "00aee0178f3f111449e5a6e99f495538a1eafa29ec95ced7d8343a3a8ee1108c", before_len: 959,
        current_sha: "78a715a6f8323551c993e0f79c3bde8174116a568cd312fa3ea2b2c8adb01b54", current_len: 1039,
        insertions: &[
            (948, b"    std::io::Write::write_all(&mut std::io::stdout(), &output.stdout).unwrap();\n"),
        ],
    },
];
fn default_ca_fixture_inverse(
    row: &DefaultCaFixture,
    physical: &[u8],
    mode: &str,
    insertions: &[(usize, &[u8])],
) -> Result<Vec<u8>, String> {
    need(
        mode == "0644" && physical.len() == row.current_len && digest(physical) == row.current_sha,
        "default-CA fixture actual current identity differs",
    )?;
    let mut prior = Vec::new();
    let mut cursor = 0;
    for &(at, inserted) in insertions {
        need(
            !inserted.is_empty() && at >= cursor && at <= physical.len(),
            "default-CA fixture insertion order differs",
        )?;
        let end = at
            .checked_add(inserted.len())
            .ok_or("default-CA fixture span overflow")?;
        need(
            physical.get(at..end) == Some(inserted),
            "default-CA fixture exact insertion differs",
        )?;
        prior.extend_from_slice(&physical[cursor..at]);
        cursor = end;
    }
    prior.extend_from_slice(&physical[cursor..]);
    need(
        prior.len() == row.before_len && digest(&prior) == row.before_sha,
        "default-CA fixture insertion inverse differs",
    )?;
    Ok(prior)
}
/// Only these exact old tuples select the new physical three-file frame. Every
/// selected call authenticates all three physical regular0644 afterimages first.
/// A caller cannot supply historical bytes as actual current or mix helper bodies.
pub(crate) fn default_ca_fixture_operand(
    root: &Path,
    path: &str,
    expected: &str,
    expected_bytes: Option<usize>,
) -> Option<Result<Vec<u8>, String>> {
    let row = DEFAULT_CA_FIXTURES.iter().find(|r| r.path == path)?;
    Some((|| {
        need(
            expected == row.before_sha && expected_bytes.is_none_or(|n| n == row.before_len),
            "default-CA fixture caller tuple differs",
        )?;
        let mut selected = None;
        for physical_row in DEFAULT_CA_FIXTURES {
            let file = root.join(physical_row.path);
            let metadata = fs::symlink_metadata(&file).map_err(|e| e.to_string())?;
            need(
                metadata.is_file() && !metadata.file_type().is_symlink(),
                "default-CA fixture source must be regular, not symlink",
            )?;
            let physical = fs::read(&file).map_err(|e| e.to_string())?;
            let frame = json!({"path":physical_row.path,"sha256":physical_row.current_sha,"bytes":physical_row.current_len,"mode":"0644"});
            let bytes =
                native_search::page_prior(root, &frame, Some(&physical))?.unwrap_or(physical);
            let mode = permissions(&file)?;
            need(
                mode == "0644"
                    && bytes.len() == physical_row.current_len
                    && digest(&bytes) == physical_row.current_sha,
                "default-CA fixture physical frame differs",
            )?;
            if physical_row.path == path {
                selected = Some((bytes, mode));
            }
        }
        let (physical, mode) = selected.ok_or("default-CA fixture actual source missing")?;
        default_ca_fixture_inverse(row, &physical, &mode, row.insertions)
    })())
}
#[cfg(test)]
fn default_ca_fixture_controls(root: &Path) -> Result<(), String> {
    for row in DEFAULT_CA_FIXTURES {
        let old = default_ca_fixture_operand(root, row.path, row.before_sha, Some(row.before_len))
            .ok_or("default-CA fixture positive not selected")??;
        let physical = fs::read(root.join(row.path)).map_err(|e| e.to_string())?;
        let frame =
            json!({"path":row.path,"sha256":row.current_sha,"bytes":row.current_len,"mode":"0644"});
        let ca_current =
            native_search::page_prior(root, &frame, Some(&physical))?.unwrap_or(physical);
        let claim = json!({"path":row.path,"sha256":row.before_sha,
            "bytes":row.before_len,"mode":"0644"});
        need(
            read_exact(root, &claim)? == old,
            "default-CA read_exact complete caller row positive differs",
        )?;
        for mutation in [
            "missing-bytes",
            "wrong-bytes",
            "noninteger-bytes",
            "missing-mode",
            "wrong-mode",
            "nonstring-mode",
        ] {
            let mut invalid = claim.clone();
            match mutation {
                "missing-bytes" => {
                    invalid
                        .as_object_mut()
                        .ok_or("default-CA row absent")?
                        .remove("bytes");
                }
                "wrong-bytes" => {
                    invalid["bytes"] = json!(0);
                }
                "noninteger-bytes" => {
                    invalid["bytes"] = json!("959");
                }
                "missing-mode" => {
                    invalid
                        .as_object_mut()
                        .ok_or("default-CA row absent")?
                        .remove("mode");
                }
                "wrong-mode" => {
                    invalid["mode"] = json!("0755");
                }
                _ => {
                    invalid["mode"] = json!(420);
                }
            }
            need(
                read_exact(root, &invalid).is_err(),
                "default-CA read_exact incomplete caller row accepted",
            )?;
        }

        need(
            old.len() == row.before_len && digest(&old) == row.before_sha,
            "default-CA fixture parent positive differs",
        )?;
        need(
            default_ca_fixture_inverse(row, &old, "0644", row.insertions).is_err(),
            "default-CA fixture historical accepted as current",
        )?;
        for mode in ["0600", "0755", "4644"] {
            need(
                default_ca_fixture_inverse(row, &ca_current, mode, row.insertions).is_err(),
                "default-CA fixture changed mode accepted",
            )?;
        }
        let mut altered = ca_current.clone();
        altered.push(0);
        need(
            default_ca_fixture_inverse(row, &altered, "0644", row.insertions).is_err(),
            "default-CA fixture unlisted default-CA frame edit accepted",
        )?;
        // Mutate the original CA relation with its authenticated e127 frame fixed.
        // page_controls separately rejects changed physical page afterimages.
        // Omitted/duplicated/moved inverse insertions must never reconstruct parent.
        for index in 0..row.insertions.len() {
            let mut missing = row.insertions.to_vec();
            missing.remove(index);
            let mut duplicate = row.insertions.to_vec();
            duplicate.insert(index, row.insertions[index]);
            let mut moved = row.insertions.to_vec();
            moved[index].0 += 1;
            for edits in [missing, duplicate, moved] {
                need(
                    default_ca_fixture_inverse(row, &ca_current, "0644", &edits).is_err(),
                    "default-CA fixture altered inverse relation accepted",
                )?;
            }
        }
        need(
            default_ca_fixture_operand(root, row.path, "wrong", None)
                .ok_or("default-CA wrong tuple lost closed selection")?
                .is_err(),
            "default-CA fixture wrong caller accepted",
        )?;
        need(
            default_ca_fixture_operand(root, row.path, row.before_sha, Some(0))
                .ok_or("default-CA wrong size lost closed selection")?
                .is_err(),
            "default-CA fixture wrong caller length accepted",
        )?;
        need(
            default_ca_fixture_operand(
                Path::new("/missing-default-ca-fixture-control"),
                row.path,
                row.before_sha,
                None,
            )
            .ok_or("default-CA missing source not selected")?
            .is_err(),
            "default-CA fixture missing physical source accepted",
        )?;
    }
    need(
        default_ca_fixture_operand(root, "unlisted.rs", "wrong", None).is_none(),
        "default-CA fixture unlisted source changed dispatch",
    )?;
    Ok(())
}

fn read_exact(root: &Path, row: &Value) -> Result<Vec<u8>, String> {
    if let Some(prior) = default_ca_fixture_operand(
        root,
        text(row, "path")?,
        text(row, "sha256")?,
        row["bytes"].as_u64().map(|n| n as usize),
    ) {
        let prior = prior?;
        material(row, &prior, "0644")?;
        return Ok(prior);
    }
    let path = root.join(text(row, "path")?);
    let stat = fs::symlink_metadata(&path).map_err(|e| e.to_string())?;
    need(
        stat.is_file() && !stat.file_type().is_symlink(),
        "A06 source must be regular, not symlink",
    )?;
    let raw = fs::read(&path).map_err(|e| e.to_string())?;
    // W08 admits only these exact current-generation operands. Older joint and
    // Metrics routes below keep the physical buffer until their own first proof.
    let raw = if (row["path"] == "crates/ssx-application/src/lib.rs"
        && row["sha256"] == "e328c4b5a4217acbe79a186d1c85d772fbe78ff901bad81ad07aaa458e0ffa8b")
        || (row["path"] == "crates/ssx-infra/src/lib.rs"
            && row["sha256"] == "8a58685c0e79b79a12f38d44259f91adf5c5975ee664efed7d5030172d915c4a")
        || (row["path"] == "crates/xtask/src/main.rs" && row["sha256"] == w07_t02_joint::MAIN_SHA)
        || (row["path"] == "docs/rust-rewrite/a06/source-amendments/shared-successor.rs"
            && row["sha256"] == "7825c39c9d264b11170e3ab749249b266e5a530450d3cdcdf4ef076e4545a3e2")
        || (row["path"] == "docs/rust-rewrite/a06/source-amendments/w07-t02-joint.rs"
            && row["sha256"] == "2d89ad90459a297bb48196704c334a12103cfd66cc782aa98e91be32176763a6")
    {
        w08_source_successor::parent(root, text(row, "path")?, &raw)?
    } else {
        raw
    };
    // Joint normalization is permitted only for these exact historical/current tuples.

    let raw = if row["path"] == "Cargo.toml"
        && row["sha256"] == "d7383efcb512764e820ac778a5fed9b990fbf734ce75d525e077e7e905587dd3"
    {
        w07_t02_joint::cli_source_custody::legacy_prior(root, "Cargo.toml", &raw)?
    } else if (row["path"] == "crates/xtask/src/main.rs"
        && row["sha256"] == "8755129fbac7454a1ae25982dc44ce1cb92b8ead7ecf650a3901aa2f08d4a23c")
        || (row["path"] == "Cargo.lock"
            && row["sha256"] == "96e9a7760d163fc5ea2afa23033e58cfb7e59d0f2d93f66d8847ef352e0b765d")
        || (row["path"] == "crates/ssx-infra/src/search.rs"
            && row["sha256"] == "a1b310e2ef8692c96109bfd780c0e37d7e50692231e11eceb48e05860e0b5927")
        || (row["path"] == "crates/xtask/src/a13.rs"
            && row["sha256"] == "7b8c213676b77a5511da43bb1f8bb3d889d4964e3ffceb4719b21801cd5fea95")
        || (row["path"] == "docs/rust-rewrite/a13/source-amendments/validator.rs"
            && row["sha256"] == "bb1eec7b1376a2437d7069588634d9c40bf985a233b66106b1e5b49c0a473c58")
    {
        w07_t02_joint::parent(root, text(row, "path")?, &raw, w07_t02_joint::Parent::T02)?
    } else if row["path"] == "crates/xtask/src/main.rs"
        && row["sha256"] == w07_main_successor::PRIOR_SHA
    {
        w07_t02_joint::parent(root, text(row, "path")?, &raw, w07_t02_joint::Parent::W07)?
    } else {
        raw
    };
    let raw = if row["path"] == "crates/ssx-application/src/lib.rs"
        && row["sha256"] == "7d3b8126448059633426e964a1ecc9cc3c2b1fd0ba2cdc7bd75917862cf0cee4"
    {
        metrics_predecessor(root, text(row, "path")?, text(row, "sha256")?, &raw)?
    } else {
        raw
    };
    // D's original inverse remains unchanged after the physical joint proof.
    let raw = if row["path"] == "crates/xtask/src/main.rs"
        && row["sha256"] == w07_main_successor::PRIOR_SHA
    {
        w07_main_successor::reconstruct(&raw)?
    } else if row["path"] == "Cargo.lock" && row["sha256"] == w07_test_dependency::LOCK_PRIOR_SHA {
        w07_test_dependency::historical_lock(root, &raw)?
    } else {
        raw
    };
    material(row, &raw, &permissions(&path)?)?;
    Ok(raw)
}
fn edits(before: &[u8], changes: &Value) -> Result<Vec<u8>, String> {
    let rows = changes.as_array().ok_or("A06 ordered edits absent")?;
    need(!rows.is_empty(), "A06 ordered edits absent")?;
    let mut output = Vec::new();
    let mut end = 0;
    for row in rows {
        let at = row["at"]
            .as_u64()
            .and_then(|n| usize::try_from(n).ok())
            .ok_or("A06 edit offset")?;
        let remove = text(row, "remove_utf8")?.as_bytes();
        let insert = text(row, "insert_utf8")?.as_bytes();
        let next = at.checked_add(remove.len()).ok_or("A06 edit overflow")?;
        need(
            at >= end
                && next <= before.len()
                && &before[at..next] == remove
                && (!remove.is_empty() || !insert.is_empty()),
            "A06 edit preimage/order differs",
        )?;
        output.extend_from_slice(&before[end..at]);
        output.extend_from_slice(insert);
        end = next;
    }
    output.extend_from_slice(&before[end..]);
    Ok(output)
}
fn transition(
    row: &Value,
    before: &[u8],
    actual: &[u8],
    mode: &str,
    reviewed_edits_sha: &str,
) -> Result<(), String> {
    need(
        row["baseline"]["path"] == row["current"]["path"]
            && row["archive"]["source_path"] == row["baseline"]["path"]
            && row["archive"]["sha256"] == row["baseline"]["sha256"]
            && row["archive"]["bytes"] == row["baseline"]["bytes"]
            && row["archive"]["mode"] == row["baseline"]["mode"],
        "A06 predecessor linkage differs",
    )?;
    material(&row["baseline"], before, "0644")?;
    material(&row["current"], actual, mode)?;
    need(
        digest(&serde_json::to_vec(&row["edits"]).map_err(|e| e.to_string())?)
            == reviewed_edits_sha,
        "A06 reviewed edit digest differs",
    )?;
    need(
        edits(before, &row["edits"])? == actual,
        "A06 reconstruction differs from actual current",
    )
}
fn document(raw: &[u8]) -> Result<Value, String> {
    need(digest(raw) == MANIFEST_SHA, "A06 shared sidecar changed")?;
    let doc: Value = serde_json::from_slice(raw).map_err(|e| e.to_string())?;
    need(
        doc["kind"] == "PURE_A06_SHARED_SOURCE_SUCCESSOR_V2"
            && doc["baseline_revision"] == BASELINE,
        "A06 shared sidecar scope differs",
    )?;
    scope(&doc)?;
    Ok(doc)
}
fn scope(doc: &Value) -> Result<(), String> {
    use std::collections::BTreeSet;
    let paths = doc["transitions"]
        .as_array()
        .ok_or("A06 transitions absent")?
        .iter()
        .map(|r| text(&r["current"], "path"))
        .collect::<Result<Vec<_>, _>>()?;
    need(
        paths.len() == TRANSITION_PATHS.len()
            && paths.iter().copied().collect::<BTreeSet<_>>()
                == TRANSITION_PATHS.iter().copied().collect(),
        "A06 exact transition membership differs",
    )?;
    let pairs = doc["bindings"]
        .as_array()
        .ok_or("A06 bindings absent")?
        .iter()
        .map(|r| {
            Ok((
                text(&r["container"], "path")?,
                text(r, "source_path")?,
                text(r, "digest_kind")?,
            ))
        })
        .collect::<Result<Vec<_>, String>>()?;
    let expected = SHA_PAIRS
        .iter()
        .map(|(c, p)| (*c, *p, "sha256"))
        .chain(GIT_PAIRS.iter().map(|(c, p)| (*c, *p, "git_blob")))
        .collect::<BTreeSet<_>>();
    need(
        pairs.len() == expected.len() && pairs.into_iter().collect::<BTreeSet<_>>() == expected,
        "A06 exact caller membership differs",
    )
}
fn validate_v2(previous: &Value, next: &Value) -> Result<(), String> {
    let old = previous["transitions"]
        .as_array()
        .ok_or("A06 V1 transitions absent")?;
    let new = next["transitions"]
        .as_array()
        .ok_or("A06 V2 transitions absent")?;
    need(
        old.len() == 19
            && new.len() == 20
            && new[..19] == old[..]
            && new[19]["current"]["path"] == VALIDATOR,
        "A06 V2 must preserve all nineteen V1 transitions",
    )?;
    let mut restored = next.clone();
    restored["transitions"] = previous["transitions"].clone();
    restored["kind"] = previous["kind"].clone();
    need(
        restored
            .as_object_mut()
            .ok_or("A06 V2 object absent")?
            .remove("predecessor_manifest")
            .is_some(),
        "A06 V2 predecessor absent",
    )?;
    need(
        restored == *previous,
        "A06 V2 unrelated predecessor material differs",
    )
}
fn load(root: &Path) -> Result<Value, String> {
    let doc = document(&fs::read(root.join(MANIFEST)).map_err(|e| e.to_string())?)?;
    let predecessor = &doc["predecessor_manifest"];
    need(
        predecessor["path"] == PREDECESSOR
            && predecessor["sha256"]
                == "7b2846032c62d17cc4b56dd9f53f56e8fcab2140bd4166443c65ee5d2c166da0"
            && predecessor["bytes"] == 211805
            && predecessor["mode"] == "0644",
        "A06 exact V1 predecessor differs",
    )?;
    let previous: Value =
        serde_json::from_slice(&read(root, predecessor)?).map_err(|e| e.to_string())?;
    validate_v2(&previous, &doc)?;
    Ok(doc)
}
fn source<'a>(doc: &'a Value, path: &str) -> Result<&'a Value, String> {
    let rows = doc["transitions"]
        .as_array()
        .ok_or("A06 transitions absent")?;
    let matching: Vec<_> = rows
        .iter()
        .filter(|r| r["current"]["path"] == path)
        .collect();
    need(
        matching.len() == 1,
        "A06 missing/duplicate source transition",
    )?;
    Ok(matching[0])
}
fn verify_transition(root: &Path, row: &Value) -> Result<(Vec<u8>, Vec<u8>), String> {
    let before = read(root, &row["archive"])?;
    let path = text(&row["current"], "path")?;
    let selected_child = real_clock_successor::selected(path);
    let selected_layout = path == a13_header_layout_successor::SOURCE;
    let selected_page_catalog = path == "crates/ssx-infra/tests/a09_catalog_media.rs";
    let historical = if path == A13 {
        a13_json_inventory_successor::archived_prior(root, &row["current"])?
    } else if selected_layout {
        a13_header_layout_successor::archived_prior(root, &row["current"])?
    } else if selected_child {
        real_clock_successor::archived_prior(
            root,
            &row["current"],
            real_clock_successor::Role::SharedCurrent,
        )?
    } else if selected_page_catalog {
        let sealed = load(root)?;
        need(
            row == source(&sealed, path)?,
            "exact sealed A06 catalog transition caller",
        )?;
        let current = &row["current"];
        let frame = json!({"path":current["path"],"sha256":current["sha256"],"bytes":current["bytes"],"mode":current["mode"]});
        let physical = fs::read(root.join(path)).map_err(|e| e.to_string())?;
        native_search::page_prior(root, &frame, Some(&physical))?
            .ok_or("catalog Page operand absent")?
    } else {
        read(root, &row["current"])?
    };
    // Original main-to09ee proof remains exact and is completed first.
    transition(
        row,
        &before,
        &historical,
        "0644",
        text(row, "edits_sha256")?,
    )?;
    if path == A13 {
        a13_json_inventory_successor::prove_current(root, &row["current"], &historical)?;
    } else if selected_layout {
        a13_header_layout_successor::prove_current(root, &row["current"], &historical)?;
    } else if selected_child {
        real_clock_successor::prove_current(
            root,
            &row["current"],
            real_clock_successor::Role::SharedCurrent,
            &historical,
        )?;
    } else {
        material(
            &row["current"],
            &historical,
            &permissions(&root.join(path))?,
        )?;
    }
    Ok((before, historical))
}
fn frozen(root: &Path, doc: &Value) -> Result<(), String> {
    for row in doc["immutable_predecessors"]
        .as_array()
        .ok_or("A06 predecessor authority absent")?
    {
        if row["path"] == "crates/ssx-infra/tests/support/http_expired_startup.rs" {
            real_clock_successor::proven_historical(
                root,
                row,
                real_clock_successor::Role::FrozenSupport,
            )?;
        } else if row["path"] == "docs/rust-rewrite/a13/qualification-v1.json" {
            verify_current_qualification(root, row)?;
        } else {
            read(root, row)?;
        }
    }
    real_clock_http_prior(root, RealClockHttpCaller::A07)?;
    real_clock_http_prior(root, RealClockHttpCaller::A08)?;
    Ok(())
}
fn pair<'a>(
    doc: &'a Value,
    container: &str,
    path: &str,
    kind: &str,
) -> Result<Option<&'a Value>, String> {
    let rows = doc["bindings"].as_array().ok_or("A06 bindings absent")?;
    let found: Vec<_> = rows
        .iter()
        .filter(|r| {
            r["container"]["path"] == container
                && r["source_path"] == path
                && r["digest_kind"] == kind
        })
        .collect();
    need(found.len() <= 1, "A06 duplicate caller binding")?;
    Ok(found.first().copied())
}
fn caller(
    binding: &Value,
    container_sha: &str,
    raw: &[u8],
    path: &str,
    expected: &str,
    bytes: Option<usize>,
) -> Result<(), String> {
    need(
        binding["source_path"] == path
            && binding["container"]["sha256"] == container_sha
            && digest(raw) == container_sha
            && binding["expected_digest"] == expected
            && binding["expected_bytes"] == json!(bytes),
        "A06 caller typed tuple differs",
    )?;
    let doc: Value = serde_json::from_slice(raw).map_err(|e| e.to_string())?;
    let old = doc
        .pointer(text(binding, "pointer")?)
        .ok_or("A06 original frozen row absent")?;
    if binding["row_shape"] == "object" {
        need(
            old["path"] == path
                && old["sha256"] == expected
                && old
                    .get("bytes")
                    .and_then(Value::as_u64)
                    .and_then(|n| usize::try_from(n).ok())
                    == bytes,
            "A06 original frozen object differs",
        )?;
    } else {
        need(
            old == expected && bytes.is_none(),
            "A06 original frozen scalar differs",
        )?;
    }
    Ok(())
}
pub(crate) fn selected(container: &str, path: &str) -> bool {
    SHA_PAIRS.contains(&(container, path)) || page_proof_selected(container, path)
}
pub(crate) fn try_verify(
    root: &Path,
    container: &str,
    container_sha: &str,
    path: &str,
    expected: &str,
    bytes: Option<usize>,
) -> Option<Result<(), String>> {
    if let Some(result) =
        page_proof_try_verify(root, container, container_sha, path, expected, bytes)
    {
        return Some(result);
    }
    // O01's frozen row predates only W07's four Metrics forwarding methods.
    if container == "docs/rust-rewrite/o01/coverage.json"
        && path == "crates/ssx-infra/src/observability/mod.rs"
    {
        return Some((|| {
            let source = root.join(path);
            let metadata = fs::symlink_metadata(&source).map_err(|e| e.to_string())?;
            need(
                metadata.is_file() && !metadata.file_type().is_symlink(),
                "O01 Metrics source must be regular",
            )?;
            let actual = fs::read(&source).map_err(|e| e.to_string())?;
            let caller = fs::read(root.join(container)).map_err(|e| e.to_string())?;
            let prior = o01_metrics_predecessor(
                (container, container_sha, path, expected, bytes),
                &caller,
                &actual,
                &permissions(&source)?,
            )?;
            let git = std::process::Command::new("git")
                .args([
                    "cat-file",
                    "blob",
                    "e5c97a3ab52b3534a5f796ac9e89b1a0ee9fd489",
                ])
                .current_dir(root)
                .output()
                .map_err(|e| e.to_string())?;
            need(
                git.status.success() && git.stdout == prior,
                "O01 Metrics original Git blob differs",
            )?;
            Ok(())
        })());
    }
    if let Some(result) =
        post_a06_generation::try_current(root, container, container_sha, path, expected, bytes)
    {
        return Some(result);
    }
    if container == ARTIFACT_COVERAGE
        && path == "crates/ssx-infra/tests/support/default_ca_environment.rs"
    {
        return Some((|| {
            need(
                container_sha == ARTIFACT_COVERAGE_SHA && bytes.is_none(),
                "default-CA A06 frozen caller tuple differs",
            )?;
            let caller = historical_caller(root, container, container_sha)?;
            let doc: Value = serde_json::from_slice(&caller).map_err(|e| e.to_string())?;
            need(
                doc["artifacts"][187] == json!({"path":path,"sha256":expected}),
                "default-CA A06 frozen helper row differs",
            )?;
            default_ca_fixture_operand(root, path, expected, bytes)
                .ok_or("default-CA A06 helper not selected")??;
            Ok(())
        })());
    }
    if let Some(result) = try_a06_artifact(root, container, container_sha, path, expected, bytes) {
        return Some(result);
    }
    if !selected(container, path) {
        return None;
    }
    let doc = match load(root) {
        Ok(doc) => doc,
        Err(e) => return Some(Err(e)),
    };
    let binding = match pair(&doc, container, path, "sha256") {
        Ok(Some(b)) => b,
        Ok(None) => return None,
        Err(e) => return Some(Err(e)),
    };
    Some((|| {
        let raw = read(root, &binding["container"])?;
        caller(binding, container_sha, &raw, path, expected, bytes)?;
        frozen(root, &doc)?;
        let (_, actual) = verify_transition(root, source(&doc, path)?)?;
        eprintln!(
            "A06_SHARED_CURRENT_PROOF source={path} reviewed09ee_sha256={} prior_binding={expected}",
            digest(&actual)
        );
        Ok(())
    })())
}
// This entry is used by xtask A14; the same source is also compiled by the standalone A13 test crate.
#[allow(dead_code)]
pub(crate) fn try_git<F>(
    root: &Path,
    container: &str,
    container_sha: &str,
    path: &str,
    expected: &str,
    actual_blob: &str,
    mut archived_blob: F,
) -> Option<Result<(), String>>
where
    F: FnMut(&str) -> Result<String, String>,
{
    if !GIT_PAIRS.contains(&(container, path)) {
        return None;
    }
    let doc = match load(root) {
        Ok(doc) => doc,
        Err(e) => return Some(Err(e)),
    };
    let binding = match pair(&doc, container, path, "git_blob") {
        Ok(Some(b)) => b,
        Ok(None) => return None,
        Err(e) => return Some(Err(e)),
    };
    Some((|| {
        caller(
            binding,
            container_sha,
            &read(root, &binding["container"])?,
            path,
            expected,
            None,
        )?;
        frozen(root, &doc)?;
        let row = source(&doc, path)?;
        verify_transition(root, row)?;
        let accepted = &binding["accepted_archive"];
        read(root, accepted)?;
        let archived_blob = archived_blob(text(accepted, "path")?)?;
        let current = if path == A13 {
            a13_json_inventory_successor::effective_current(root, &row["current"])?
        } else if real_clock_successor::selected(path) {
            real_clock_successor::effective_current(
                root,
                &row["current"],
                real_clock_successor::Role::SharedCurrent,
            )?
        } else {
            row["current"].clone()
        };
        need(
            archived_blob.trim() == expected && current["git_blob"] == actual_blob,
            "A06 exact accepted/current Git blob differs",
        )?;
        Ok(())
    })())
}
/// Effective current rows are derived from immutable V3, never written over it.
pub(crate) fn compose_scoped(root: &Path, previous: &Value) -> Result<Value, String> {
    let doc = load(root)?;
    frozen(root, &doc)?;
    let original: Value = serde_json::from_slice(&read(root, &doc["scoped_predecessor"])?)
        .map_err(|e| e.to_string())?;
    need(original == *previous, "A06 scoped predecessor differs")?;
    let mut next = previous.clone();
    for row in next["successors"]
        .as_array_mut()
        .ok_or("A06 scoped successors absent")?
    {
        let path = text(&row["current"], "path")?.to_owned();
        if TRANSITION_PATHS.contains(&path.as_str()) {
            let replacement = source(&doc, &path)?;
            need(
                row["current"] == replacement["baseline"],
                "A06 scoped current predecessor differs",
            )?;
            let (_, actual) = verify_transition(root, replacement)?;
            let historical = read(root, &row["historical191"])?;
            need(
                edits(&historical, &replacement["scoped_edits"])? == actual,
                "A06 complete scoped reconstruction differs",
            )?;
            row["current"] = replacement["current"].clone();
            row["edits"] = json!(replacement["scoped_edits"].as_array().ok_or("A06 scoped edits absent")?.iter().map(|edit| {
                Ok(json!({"at":edit["at"],"remove":text(edit,"remove_utf8")?.as_bytes(),"insert":text(edit,"insert_utf8")?.as_bytes()}))
            }).collect::<Result<Vec<Value>, String>>()?);
            if path == "crates/ssx-infra/src/http.rs" {
                *row = real_clock_successor::scoped_http(root, row, &historical)?;
            }
            if path == a13_header_layout_successor::SOURCE {
                *row = a13_header_layout_successor::scoped(root, row, &historical)?;
            }
        }
        let historical = read(root, &row["historical191"])?;
        *row = post_a06_generation::scoped(root, row, &historical)?;
        *row = native_search::page_scoped(root, row, &historical)?;
    }
    for row in next["additional_current_materials"]
        .as_array_mut()
        .ok_or("A06 scoped current materials absent")?
    {
        let path = text(row, "path")?.to_owned();
        if TRANSITION_PATHS.contains(&path.as_str()) {
            let replacement = source(&doc, &path)?;
            need(
                *row == replacement["baseline"],
                "A06 additional predecessor differs",
            )?;
            verify_transition(root, replacement)?;
            *row = replacement["current"].clone();
            if path == A13 {
                *row = a13_json_inventory_successor::effective_current(root, row)?;
            }
            if path == VALIDATOR {
                *row = real_clock_successor::effective_current(
                    root,
                    row,
                    real_clock_successor::Role::SharedCurrent,
                )?;
                *row = post_a06_generation::effective(root, row)?;
            }
        }
    }
    for row in next["historical_chain_materials"]
        .as_array_mut()
        .ok_or("A06 scoped historical materials absent")?
    {
        if row["path"] == "crates/ssx-infra/tests/support/http_expired_startup.rs" {
            *row = real_clock_successor::scoped_history(root, row)?;
        }
    }
    Ok(next)
}
/// Explicit old accepted A13 material for predecessor relation/negative controls.
/// Actual new source is independently read and reconstructed before returning it.
pub(crate) fn a13_baseline(root: &Path, requested: &Value) -> Result<(Value, Vec<u8>), String> {
    let doc = load(root)?;
    let row = source(&doc, A13)?;
    let effective = a13_json_inventory_successor::effective_current(root, &row["current"])?;
    need(
        *requested == row["baseline"] || *requested == row["current"] || *requested == effective,
        "A06 A13 predecessor/current request differs",
    )?;
    let (before, _) = verify_transition(root, row)?;
    Ok((row["baseline"].clone(), before))
}
pub(crate) fn validator_baseline(
    root: &Path,
    requested: &Value,
) -> Result<(Value, Vec<u8>), String> {
    let doc = load(root)?;
    let row = source(&doc, VALIDATOR)?;
    let (before, _) = verify_transition(root, row)?;
    let effective = real_clock_successor::effective_current(
        root,
        &row["current"],
        real_clock_successor::Role::SharedCurrent,
    )?;
    let effective = post_a06_generation::effective(root, &effective)?;
    need(
        *requested == row["baseline"] || *requested == row["current"] || *requested == effective,
        "A06 validator predecessor/current request differs",
    )?;
    Ok((row["baseline"].clone(), before))
}
pub(crate) fn security_lock_baseline(
    root: &Path,
    supplied_current: &[u8],
) -> Result<Vec<u8>, String> {
    let doc = load(root)?;
    let row = source(&doc, "Cargo.lock")?;
    let (before, _) = verify_transition(root, row)?;
    let actual = post_a06_generation::actual(root, &row["current"])?;
    let supplied_current = w07_t02_joint::parent(
        root,
        "Cargo.lock",
        supplied_current,
        w07_t02_joint::Parent::T02,
    )?;
    need(
        supplied_current == actual,
        "A06 supplied lock differs from actual current",
    )?;
    Ok(before)
}
pub(crate) fn security_lock_successor(
    root: &Path,
    computed_tls_baseline: &[u8],
    supplied_current: &[u8],
) -> Result<(), String> {
    let before = security_lock_baseline(root, supplied_current)?;
    need(
        computed_tls_baseline == before,
        "A06 computed TLS baseline differs from sealed predecessor",
    )
}
#[cfg(test)]
pub(crate) fn legacy_control_operand(
    root: &Path,
    path: &str,
    expected: &Value,
    supplied_current: &[u8],
) -> Result<Vec<u8>, String> {
    need(
        [
            "crates/ssx-infra/src/adapters/clinicaltrials.rs",
            "crates/ssx-infra/src/adapters/mod.rs",
        ]
        .contains(&path),
        "A06 unlisted historical control operand",
    )?;
    let doc = load(root)?;
    let row = source(&doc, path)?;
    let (before, actual) = verify_transition(root, row)?;
    need(
        supplied_current == actual,
        "A06 control operand differs from actual current",
    )?;
    need(
        expected["path"] == path,
        "A06 historical control path differs",
    )?;
    material(expected, &before, "0644")?;
    Ok(before)
}
#[cfg(test)]
pub(crate) fn current_http(root: &Path, actual: &[u8]) -> Result<bool, String> {
    let doc = load(root)?;
    let row = source(&doc, "crates/ssx-infra/src/http.rs")?;
    verify_transition(root, row)?;
    let current = real_clock_successor::effective_current(
        root,
        &row["current"],
        real_clock_successor::Role::SharedCurrent,
    )?;
    Ok(read(root, &current)? == actual)
}
#[cfg(test)]
pub(crate) fn controls(root: &Path) -> Result<(), String> {
    w07_test_dependency::controls(root)?;
    default_ca_fixture_controls(root)?;
    native_search::controls(root)?;
    post_a06_generation::controls(root)?;
    a13_json_inventory_successor::controls(root)?;
    a13_qualification_successor::controls(root)?;
    real_clock_successor::controls(root)?;
    w07_evidence_successor::controls(root)?;
    let doc = load(root)?;
    frozen(root, &doc)?;
    scope(&doc)?;
    need(
        try_verify(
            Path::new("/missing-A06-control"),
            "unlisted.json",
            "wrong",
            "unlisted.rs",
            "wrong",
            None,
        )
        .is_none(),
        "A06 unlisted caller did not retain original dispatch",
    )?;
    for field in ["transitions", "bindings"] {
        let mut missing = doc.clone();
        missing[field].as_array_mut().unwrap().pop();
        need(scope(&missing).is_err(), "A06 missing scope entry accepted")?;
        let mut duplicated = doc.clone();
        let first = duplicated[field][0].clone();
        duplicated[field].as_array_mut().unwrap().push(first);
        need(
            scope(&duplicated).is_err(),
            "A06 duplicated scope entry accepted",
        )?;
    }
    for row in doc["transitions"]
        .as_array()
        .ok_or("A06 control transitions absent")?
    {
        let (before, actual) = verify_transition(root, row)?;
        let sha = text(row, "edits_sha256")?;
        need(
            transition(row, &before, &before, "0644", sha).is_err(),
            "A06 predecessor accepted as current",
        )?;
        for mode in ["0755", "0600", &raw_mode(0o104644)] {
            need(
                transition(row, &before, &actual, mode, sha).is_err(),
                "A06 changed mode accepted",
            )?;
        }
        need(raw_mode(0o104644) == "4644", "A06 special mode bits lost")?;
        need(
            material(&row["current"], &actual, &raw_mode(0o100600)).is_err(),
            "A06 raw 0600 mode accepted",
        )?;
        let mut changed = actual.clone();
        changed.push(0);
        let mut claim = row.clone();
        claim["current"]["sha256"] = json!(digest(&changed));
        claim["current"]["bytes"] = json!(changed.len());
        need(
            transition(&claim, &before, &changed, "0644", sha).unwrap_err()
                == "A06 reconstruction differs from actual current",
            "A06 rehashed unrelated current did not reach reconstruction guard",
        )?;
        for mutation in 0..4 {
            let mut bad = row.clone();
            match mutation {
                0 => {
                    bad["edits"].as_array_mut().unwrap().pop();
                }
                1 => {
                    let first = bad["edits"][0].clone();
                    bad["edits"].as_array_mut().unwrap().insert(0, first);
                }
                2 => {
                    bad["edits"][0]["at"] = json!(before.len() + 1);
                }
                _ => {
                    bad["archive"]["source_path"] = json!("wrong.rs");
                }
            }
            let structural_sha =
                digest(&serde_json::to_vec(&bad["edits"]).map_err(|e| e.to_string())?);
            need(
                transition(&bad, &before, &actual, "0644", &structural_sha).is_err(),
                "A06 causal structural mutation accepted",
            )?;
        }
    }
    for binding in doc["bindings"]
        .as_array()
        .ok_or("A06 control bindings absent")?
    {
        let path = text(binding, "source_path")?;
        let hash = text(&binding["container"], "sha256")?;
        let old = text(binding, "expected_digest")?;
        let count = binding["expected_bytes"]
            .as_u64()
            .and_then(|n| usize::try_from(n).ok());
        let raw = read(root, &binding["container"])?;
        caller(binding, hash, &raw, path, old, count)?;
        if binding["digest_kind"] == "sha256" {
            try_verify(
                root,
                text(&binding["container"], "path")?,
                hash,
                path,
                old,
                count,
            )
            .ok_or("A06 real selected route absent")??;
        }
        for (bad_hash, bad_source, bad_old, bad_count) in [
            ("wrong", path, old, count),
            (hash, "wrong.rs", old, count),
            (hash, path, "wrong", count),
            (
                hash,
                path,
                old,
                if count.is_some() { None } else { Some(0) },
            ),
        ] {
            need(
                caller(binding, bad_hash, &raw, bad_source, bad_old, bad_count).is_err(),
                "A06 caller mutation accepted",
            )?;
        }
    }
    let previous: Value = serde_json::from_slice(&read(root, &doc["predecessor_manifest"])?)
        .map_err(|e| e.to_string())?;
    validate_v2(&previous, &doc)?;
    for index in 0..19 {
        let mut wrong = doc.clone();
        wrong["transitions"][index]["current"]["sha256"] = json!("changed");
        need(
            validate_v2(&previous, &wrong).is_err(),
            "A06 V2 altered a frozen V1 transition",
        )?;
    }
    let mut dropped = doc.clone();
    dropped["transitions"].as_array_mut().unwrap().pop();
    need(
        validate_v2(&previous, &dropped).is_err(),
        "A06 V2 missing validator accepted",
    )?;
    let lock = source(&doc, "Cargo.lock")?;
    let (before, _) = verify_transition(root, lock)?;
    let expected = post_a06_generation::actual(root, &lock["current"])?;
    let actual = fs::read(root.join("Cargo.lock")).map_err(|e| e.to_string())?;
    need(
        w07_t02_joint::parent(root, "Cargo.lock", &actual, w07_t02_joint::Parent::T02)? == expected,
        "A06 physical lock parent differs from canonical current proof",
    )?;
    security_lock_successor(root, &before, &actual)?;
    let mut wrong_baseline = before.clone();
    wrong_baseline.push(0);
    need(
        security_lock_successor(root, &wrong_baseline, &actual)
            .err()
            .as_deref()
            == Some("A06 computed TLS baseline differs from sealed predecessor"),
        "A06 changed computed TLS baseline escaped the exact link",
    )?;
    let mut wrong_current = actual.clone();
    wrong_current.push(0);
    need(
        security_lock_successor(root, &before, &wrong_current)
            .err()
            .as_deref()
            == Some("W07/T02 joint source: supplied bytes are not physical joint source"),
        "A06 altered actual lock accepted",
    )?;
    for path in [
        "crates/ssx-infra/src/adapters/clinicaltrials.rs",
        "crates/ssx-infra/src/adapters/mod.rs",
    ] {
        let row = source(&doc, path)?;
        let (before, actual) = verify_transition(root, row)?;
        let accepted = legacy_control_operand(root, path, &row["baseline"], &actual)?;
        need(
            accepted == before,
            "A06 historical control baseline changed",
        )?;
        let mut wrong = actual.clone();
        wrong.push(0);
        need(
            legacy_control_operand(root, path, &row["baseline"], &wrong)
                .err()
                .as_deref()
                == Some("A06 control operand differs from actual current"),
            "A06 historical control hid changed current",
        )?;
        let mut claim = row["baseline"].clone();
        claim["sha256"] = json!("wrong");
        need(
            legacy_control_operand(root, path, &claim, &actual).is_err(),
            "A06 altered historical typed tuple accepted",
        )?;
    }
    eprintln!(
        "A06_SECURITY_LOCK_BRIDGE_CONTROLS actual_current=true tls_computed_baseline_current_mutations_and_frozen19_validator_link_rejected=true"
    );
    eprintln!(
        "A06_SHARED_TRANSITION_CONTROLS actual_current=true typed_callers_archives_order_scope_modes_and_rehashed_mutations=passed"
    );
    artifact_controls(root)?;
    Ok(())
}

// The original 20-transition/62-caller capsule above remains immutable.
const ARTIFACT_MANIFEST: &str =
    "docs/rust-rewrite/a06/source-amendments/artifact-pin-successor-v1.json";
const ARTIFACT_MANIFEST_SHA: &str =
    "dd452bd28cbfdd9b79bcd20eba7dd39322b392696450939ee592e7a44d335f24";
const ARTIFACT_MANIFEST_BYTES: usize = 94620;
const ARTIFACT_COVERAGE: &str = "docs/rust-rewrite/a06/coverage.json";
const ARTIFACT_COVERAGE_SHA: &str =
    "92686ed70979a698b0dab3414cad813198b720fe586421062db8a4cef5102d9b";
const ARTIFACT_PAIRS: &[(&str, &str)] = &[
    (
        "crates/ssx-infra/src/adapters/brave.rs",
        "1924c74ef0476f58b2d8328d3db0048e29143e87189db3c7ff8342388a17221f",
    ),
    (
        "crates/ssx-infra/src/adapters/clinicaltrials.rs",
        "95d06b3b7b2a57525cf21aef2f55121d08f4cf18f1d281b1f8a6e735062c7e7f",
    ),
    (
        "crates/ssx-infra/src/adapters/europepmc.rs",
        "6335f2ed2f40e4f8e3d126cdc5aba16adf12ef0cf690410baed723fe7482c7ad",
    ),
    (
        "crates/ssx-infra/src/adapters/mod.rs",
        "d6cecd7e52941ce0d83d2c28349e2b3473d0ab8970ed2619e65c9e9304aeeca1",
    ),
    (
        "crates/ssx-infra/src/adapters/pubchem.rs",
        "65be1a90f6fc9cfd2c50f69c6b4a0b09129916d28ba1abd7d90877ffa920dbea",
    ),
    (
        "crates/ssx-infra/src/adapters/pypi.rs",
        "eebde15c314d6af81e485e3cacd9f20bd14fedaadbe9ac34f10ba0042f6cc452",
    ),
    (
        "crates/ssx-infra/src/adapters/reddit.rs",
        "400d5e6638f9821ff81fcbc29b7843b96ed9d9e9b89c36126e242a5f18d004a3",
    ),
    (
        "crates/ssx-infra/src/adapters/source_payload.rs",
        "ebd1bdecb28d598b991529ac98dce9190f7c8a039d51473d6286250c26a16446",
    ),
    (
        "crates/ssx-infra/src/adapters/uniprot.rs",
        "1b4d19cdbd60683e8faeb96c99ca92e22b03293292a6b0965f525ffa38cba355",
    ),
    (
        "crates/ssx-infra/src/adapters/wikipedia.rs",
        "c0dcbcf98bfad239717cf9ce3e26f92600b5de5b867c090ef19f204e6ce2158d",
    ),
    (
        "crates/ssx-infra/src/http.rs",
        "700c2a4f4526a97fe54b293c9465262ef94c5666caaae1c0e92f3fd5a64554a6",
    ),
    (
        "crates/ssx-infra/tests/a08_factories.rs",
        "ddc2b6c6219a519c9faf79825b852addd9d11dc49e7c5286c25d5f5d6c44df68",
    ),
    (
        "crates/ssx-infra/tests/support/a07_resources.rs",
        "38174d6e08b54d21e1227a6ebdbf9092c4e1f0e65af14b8c9c97eeebd05b60fb",
    ),
    (
        "crates/ssx-infra/tests/a07_biomedical.rs",
        "01b40a80fe473a3ce88ffcdc8356b0e3ab637a970cfdb87c724da9b0298ae260",
    ),
    (
        "crates/ssx-infra/tests/a07_context.rs",
        "b913e5f13290c10b38ea1794b8cae976858ab4951bd0a43b478abb93109eae69",
    ),
];
const ARTIFACT_SHARED_CURRENT: &[&str] = &[
    "crates/ssx-infra/src/adapters/clinicaltrials.rs",
    "crates/ssx-infra/src/adapters/mod.rs",
    "crates/ssx-infra/src/adapters/source_payload.rs",
    "crates/ssx-infra/src/http.rs",
];
fn artifact_scope(doc: &Value) -> Result<(), String> {
    use std::collections::BTreeSet;
    need(
        doc["kind"] == "PURE_A06_ARTIFACT_PIN_SUCCESSOR_V1"
            && doc["published_A06"] == "6e4ed9e90c59f304f28206ec0bb61b3a2b365938"
            && doc["coverage"]["path"] == ARTIFACT_COVERAGE
            && doc["coverage"]["sha256"] == ARTIFACT_COVERAGE_SHA,
        "A06 artifact capsule scope differs",
    )?;
    let paths = doc["transitions"]
        .as_array()
        .ok_or("A06 artifact transitions absent")?
        .iter()
        .map(|row| text(&row["current"], "path"))
        .collect::<Result<Vec<_>, _>>()?;
    need(
        paths.len() == ARTIFACT_PAIRS.len()
            && paths.into_iter().collect::<BTreeSet<_>>()
                == ARTIFACT_PAIRS.iter().map(|(p, _)| *p).collect(),
        "A06 artifact transition membership differs",
    )?;
    let rows = doc["bindings"]
        .as_array()
        .ok_or("A06 artifact callers absent")?;
    let pairs = rows
        .iter()
        .map(|row| Ok((text(row, "source_path")?, text(row, "expected_digest")?)))
        .collect::<Result<Vec<_>, String>>()?;
    need(
        pairs.len() == ARTIFACT_PAIRS.len()
            && pairs.into_iter().collect::<BTreeSet<_>>()
                == ARTIFACT_PAIRS.iter().copied().collect()
            && rows.iter().all(|r| {
                r["container"] == doc["coverage"]
                    && r["digest_kind"] == "sha256"
                    && r["expected_bytes"].is_null()
                    && r["row_shape"] == "object"
            }),
        "A06 artifact exact caller membership differs",
    )
}
fn artifact_document(raw: &[u8]) -> Result<Value, String> {
    need(
        digest(raw) == ARTIFACT_MANIFEST_SHA,
        "A06 artifact sidecar changed",
    )?;
    let doc: Value = serde_json::from_slice(raw).map_err(|e| e.to_string())?;
    artifact_scope(&doc)?;
    Ok(doc)
}
fn artifact_load(root: &Path) -> Result<Value, String> {
    artifact_document(&read(
        root,
        &json!({"path":ARTIFACT_MANIFEST,"bytes":ARTIFACT_MANIFEST_BYTES,
        "sha256":ARTIFACT_MANIFEST_SHA,"mode":"0644"}),
    )?)
}
fn artifact_transition(
    row: &Value,
    before: &[u8],
    archive_mode: &str,
    actual: &[u8],
    current_mode: &str,
    edit_sha: &str,
) -> Result<(), String> {
    material(&row["archive"], before, archive_mode)?;
    transition(row, before, actual, current_mode, edit_sha)
}
// This one full artifact row is authenticated before its four-field projection.
// The returned bytes are the e127 operand of the unchanged artifact proof;
// the physical Page successor is authenticated separately by the common inverse.
fn biomedical_page_operand(root: &Path, row: &Value) -> Result<Vec<u8>, String> {
    const PATH: &str = "crates/ssx-infra/tests/a07_biomedical.rs";
    let sealed = artifact_load(root)?;
    need(
        row == source(&sealed, PATH)?,
        "exact sealed A06 biomedical artifact caller",
    )?;
    let current = &row["current"];
    let frame = json!({"path":current["path"],"sha256":current["sha256"],"bytes":current["bytes"],"mode":current["mode"]});
    let physical = fs::read(root.join(PATH)).map_err(|e| e.to_string())?;
    native_search::page_prior(root, &frame, Some(&physical))?
        .ok_or("biomedical Page operand absent".into())
}
fn artifact_current(root: &Path, row: &Value) -> Result<(), String> {
    let before = read(root, &row["archive"])?;
    let child_http = row["current"]["path"] == "crates/ssx-infra/src/http.rs";
    let actual = if child_http {
        real_clock_successor::archived_prior(
            root,
            &row["current"],
            real_clock_successor::Role::ArtifactCurrent,
        )?
    } else if row["current"]["path"] == "crates/ssx-infra/tests/a07_biomedical.rs" {
        biomedical_page_operand(root, row)?
    } else {
        read(root, &row["current"])?
    };
    artifact_transition(
        row,
        &before,
        &permissions(&root.join(text(&row["archive"], "path")?))?,
        &actual,
        &permissions(&root.join(text(&row["current"], "path")?))?,
        text(row, "edits_sha256")?,
    )?;
    let path = text(&row["current"], "path")?;
    if ARTIFACT_SHARED_CURRENT.contains(&path) {
        let shared = load(root)?;
        frozen(root, &shared)?;
        let (_, shared_actual) = verify_transition(root, source(&shared, path)?)?;
        need(
            shared_actual == actual,
            "A06 artifact/shared current differs",
        )?;
        if child_http {
            let artifact_child = real_clock_successor::prove_current(
                root,
                &row["current"],
                real_clock_successor::Role::ArtifactCurrent,
                &actual,
            )?;
            let shared_child = real_clock_successor::effective_current(
                root,
                &source(&shared, path)?["current"],
                real_clock_successor::Role::SharedCurrent,
            )?;
            need(
                read(root, &artifact_child)? == read(root, &shared_child)?,
                "A06 artifact/shared effective child differs",
            )?;
        }
    }
    Ok(())
}
/// Selected A06 tuples require current reconstruction even if historical bytes match.
fn try_a06_artifact(
    root: &Path,
    container: &str,
    container_sha: &str,
    path: &str,
    expected: &str,
    bytes: Option<usize>,
) -> Option<Result<(), String>> {
    if container != ARTIFACT_COVERAGE || !ARTIFACT_PAIRS.iter().any(|(p, _)| *p == path) {
        return None;
    }
    Some((|| {
        let doc = artifact_load(root)?;
        let binding =
            pair(&doc, container, path, "sha256")?.ok_or("A06 artifact selected caller absent")?;
        caller(
            binding,
            container_sha,
            &read(root, &binding["container"])?,
            path,
            expected,
            bytes,
        )?;
        artifact_current(root, source(&doc, path)?)
    })())
}
#[cfg(test)]
pub(crate) fn artifact_controls(root: &Path) -> Result<(), String> {
    let doc = artifact_load(root)?;
    for binding in doc["bindings"]
        .as_array()
        .ok_or("A06 artifact controls callers absent")?
    {
        let path = text(binding, "source_path")?;
        let old = text(binding, "expected_digest")?;
        try_a06_artifact(
            root,
            ARTIFACT_COVERAGE,
            ARTIFACT_COVERAGE_SHA,
            path,
            old,
            None,
        )
        .ok_or("A06 artifact current route absent")??;
        let row = source(&doc, path)?;
        let before = read(root, &row["archive"])?;
        let actual = if path == "crates/ssx-infra/src/http.rs" {
            real_clock_successor::proven_historical(
                root,
                &row["current"],
                real_clock_successor::Role::ArtifactCurrent,
            )?
        } else if path == "crates/ssx-infra/tests/a07_biomedical.rs" {
            biomedical_page_operand(root, row)?
        } else {
            read(root, &row["current"])?
        };
        let edit_sha = text(row, "edits_sha256")?;
        let coverage = read(root, &doc["coverage"])?;
        for (bad_container, bad_old, bad_count) in [
            ("wrong", old, None),
            (ARTIFACT_COVERAGE_SHA, "wrong", None),
            (ARTIFACT_COVERAGE_SHA, old, Some(0)),
            (ARTIFACT_COVERAGE_SHA, old, Some(before.len())),
        ] {
            need(
                caller(binding, bad_container, &coverage, path, bad_old, bad_count).is_err(),
                "A06 artifact wrong caller accepted",
            )?;
        }
        need(
            caller(
                binding,
                ARTIFACT_COVERAGE_SHA,
                &coverage,
                "wrong.rs",
                old,
                None,
            )
            .is_err(),
            "A06 artifact wrong source caller accepted",
        )?;
        let mut archive_bad = before.clone();
        archive_bad.push(0);
        need(
            artifact_transition(row, &archive_bad, "0644", &actual, "0644", edit_sha).is_err(),
            "A06 artifact altered archive accepted",
        )?;
        let mut current_bad = actual.clone();
        current_bad.push(0);
        need(
            artifact_transition(row, &before, "0644", &current_bad, "0644", edit_sha).is_err(),
            "A06 artifact altered current accepted",
        )?;
        need(
            artifact_transition(row, &before, "0644", &before, "0644", edit_sha).is_err(),
            "A06 artifact historical as current accepted",
        )?;
        for mode in ["0600".to_owned(), raw_mode(0o104644)] {
            need(mode != "0644", "A06 special mode extraction masked bits")?;
            need(
                artifact_transition(row, &before, &mode, &actual, "0644", edit_sha).is_err(),
                "A06 archive mode accepted",
            )?;
            need(
                artifact_transition(row, &before, "0644", &actual, &mode, edit_sha).is_err(),
                "A06 current mode accepted",
            )?;
        }
        let mut absent = row["archive"].clone();
        absent["path"] = json!("docs/rust-rewrite/a06/source-amendments/absent-artifact-archive");
        need(
            read(root, &absent).is_err(),
            "A06 missing archive accepted by production loader",
        )?;
        let mut bad = row.clone();
        bad["archive"]["source_path"] = json!("unlisted.rs");
        need(
            artifact_transition(&bad, &before, "0644", &actual, "0644", edit_sha).is_err(),
            "A06 archive linkage accepted",
        )?;
        let mut bad = row.clone();
        bad["edits"][0]["at"] = json!(before.len() + 1);
        let rehashed = digest(&serde_json::to_vec(&bad["edits"]).map_err(|e| e.to_string())?);
        need(
            artifact_transition(&bad, &before, "0644", &actual, "0644", &rehashed)
                .err()
                .as_deref()
                == Some("A06 edit preimage/order differs"),
            "A06 rehashed edit escaped structural guard",
        )?;
        let mut out_of_order = row.clone();
        out_of_order["edits"]
            .as_array_mut()
            .ok_or("A06 controls edits absent")?
            .insert(1, json!({"at":0,"remove_utf8":"","insert_utf8":"!"}));
        let rehashed =
            digest(&serde_json::to_vec(&out_of_order["edits"]).map_err(|e| e.to_string())?);
        need(
            artifact_transition(&out_of_order, &before, "0644", &actual, "0644", &rehashed)
                .err()
                .as_deref()
                == Some("A06 edit preimage/order differs"),
            "A06 rehashed edit order accepted",
        )?;
        let mut missing_edit = row.clone();
        missing_edit["edits"]
            .as_array_mut()
            .ok_or("A06 controls edits absent")?
            .pop();
        let rehashed =
            digest(&serde_json::to_vec(&missing_edit["edits"]).map_err(|e| e.to_string())?);
        need(
            artifact_transition(&missing_edit, &before, "0644", &actual, "0644", &rehashed)
                .is_err(),
            "A06 rehashed missing edit accepted",
        )?;
        let mut bad = row.clone();
        let insert = text(&bad["edits"][0], "insert_utf8")?.to_owned();
        bad["edits"][0]["insert_utf8"] = json!(format!("{insert}!"));
        let rehashed = digest(&serde_json::to_vec(&bad["edits"]).map_err(|e| e.to_string())?);
        need(
            artifact_transition(&bad, &before, "0644", &actual, "0644", &rehashed)
                .err()
                .as_deref()
                == Some("A06 reconstruction differs from actual current"),
            "A06 rehashed edit escaped actual reconstruction",
        )?;
    }
    for kind in ["missing", "duplicate", "extra"] {
        let mut bad = doc.clone();
        let rows = bad["transitions"]
            .as_array_mut()
            .ok_or("A06 controls transitions absent")?;
        match kind {
            "missing" => {
                rows.pop();
            }
            "duplicate" => {
                rows.push(rows[0].clone());
            }
            "extra" => {
                rows[0]["current"]["path"] = json!("unlisted.rs");
            }
            _ => unreachable!(),
        }
        need(
            artifact_scope(&bad).is_err(),
            "A06 artifact scope mutation accepted",
        )?;
        need(
            artifact_document(&serde_json::to_vec(&bad).map_err(|e| e.to_string())?).is_err(),
            "A06 rehashed capsule replaced fixed sidecar",
        )?;
    }
    need(
        try_a06_artifact(
            root,
            ARTIFACT_COVERAGE,
            ARTIFACT_COVERAGE_SHA,
            "unlisted.rs",
            "wrong",
            None,
        )
        .is_none(),
        "A06 unlisted route selected",
    )?;
    eprintln!(
        "A06_ARTIFACT_SUCCESSOR_CONTROLS historical15_actual_current_callers_archives_modes_and_rehashed_edits_rejected=true"
    );
    Ok(())
}

/// Exact live A13 additional-source caller; no unlisted historical byte fallback.
pub(crate) fn real_clock_additional_source(
    root: &Path,
    manifest: &Value,
    supplied: &[u8],
    mode: &str,
) -> Result<Vec<u8>, String> {
    real_clock_successor::a13_additional(root, manifest, supplied, mode)
}

#[derive(Clone, Copy)]
pub(crate) enum RealClockHttpCaller {
    A07,
    A08,
}
/// Only private historical-proof loaders may receive09ee after actual-child proof.
pub(crate) fn real_clock_http_prior(
    root: &Path,
    caller: RealClockHttpCaller,
) -> Result<Vec<u8>, String> {
    let doc = load(root)?;
    let (_, shared_prior) = verify_transition(root, source(&doc, "crates/ssx-infra/src/http.rs")?)?;
    let named_prior = real_clock_successor::private_http(root, caller)?;
    need(
        named_prior == shared_prior,
        "real-clock private/shared09ee prior differs",
    )?;
    Ok(named_prior)
}

// Called from the existing DeHashed source-closure selector; adds no test identity.
#[allow(dead_code)]
pub(crate) fn a13_header_layout_controls(root: &Path) -> Result<(), String> {
    native_search::page_controls(root)?;
    page_proof_controls(root)?;
    a13_header_layout_successor::controls(root)
}

pub(crate) fn a13_inventory_library_source(root: &Path, expected: &Value) -> Result<(), String> {
    a13_json_inventory_successor::library_source(root, expected)
}

pub(crate) fn verify_current_qualification(root: &Path, expected: &Value) -> Result<(), String> {
    a13_qualification_successor::verify(root, expected)
}

/// Historical caller bytes are available only after exact live generation proof.
pub(crate) fn historical_caller(
    root: &Path,
    path: &str,
    expected_sha: &str,
) -> Result<Vec<u8>, String> {
    post_a06_generation::caller_material(root, path, expected_sha)
}

#[cfg(test)]
pub(crate) fn generation_control_operand(
    root: &Path,
    path: &str,
    expected: &Value,
    supplied_current: &[u8],
) -> Result<Vec<u8>, String> {
    post_a06_generation::control_operand(root, path, expected, supplied_current)
}

// Exact Page identity pairs only. Search/DeHashed keep their generation/scoped routes.
type PageProofContext = (&'static str, &'static str, &'static [(&'static str, usize)]);
const PAGE_PROOF_CONTEXTS: &[PageProofContext] = &[
    (
        "docs/rust-rewrite/a13/dehashed-source-map-v1.json",
        "292c55e50d49bf53a0012d1c7a86ce422b2e58fe26f12bc2606b8f1621cdf18d",
        &[("crates/ssx-domain/src/records.rs", 11011)],
    ),
    (
        "docs/rust-rewrite/a13/remaining/source-map.json",
        "9f75190c18606f2e9416e4db9a99392c7cb9aa0836dc3ff41a56263f05af975f",
        &[
            ("crates/ssx-domain/src/records.rs", 11011),
            ("docs/rust-rewrite/CASE_STUDY.md", 21096),
        ],
    ),
    (
        "docs/rust-rewrite/a13/qualification-v1.json",
        "77661b6d69dc2926a2d01d277e68e22932d7fb1aae258fb272ab1192dff18c6e",
        &[
            ("crates/ssx-domain/src/records.rs", 11011),
            ("docs/rust-rewrite/CASE_STUDY.md", 21096),
            ("crates/ssx-infra/tests/a13_hibp.rs", 20344),
            ("crates/ssx-infra/tests/a13_intelx.rs", 24217),
            (
                "crates/ssx-infra/src/adapters/vulnerability_tests.rs",
                36757,
            ),
        ],
    ),
    (
        "docs/rust-rewrite/a06/coverage.json",
        "92686ed70979a698b0dab3414cad813198b720fe586421062db8a4cef5102d9b",
        &[("crates/ssx-infra/tests/support/a07_source_replay.rs", 9170)],
    ),
    (
        "docs/rust-rewrite/a07/coverage.json",
        "3356fa8d6933418a666af2642d1c95705f76d60cf1d8306024a7e7f82d0718d2",
        &[("docs/rust-rewrite/CASE_STUDY.md", 21096)],
    ),
    (
        "docs/rust-rewrite/a08/coverage.json",
        "4bd65113eded80d605b3fede32f9cb5e97d12b47c4b1756314ed28757f653856",
        &[("docs/rust-rewrite/CASE_STUDY.md", 21096)],
    ),
    (
        "docs/rust-rewrite/a09/coverage.json",
        "3dfa3fc89150ba579ee377c730fce66134125c502ab07c624dd6a8768662ef89",
        &[("docs/rust-rewrite/CASE_STUDY.md", 21096)],
    ),
    (
        "docs/rust-rewrite/a10/qualification.json",
        "abd85bf644e311c4f6ed00f1e1315e788a930d937c017ecec7f3befeb378c01a",
        &[
            ("crates/ssx-domain/src/records.rs", 11011),
            (
                "crates/ssx-infra/src/adapters/vulnerability_tests.rs",
                36757,
            ),
            ("crates/ssx-infra/tests/a10_factories.rs", 24508),
        ],
    ),
    (
        "docs/rust-rewrite/a11/qualification.json",
        "89f816f2bc3333233434dcdbf706eab80387124889923658941682d4668352f3",
        &[
            ("crates/ssx-domain/src/records.rs", 11011),
            ("crates/ssx-infra/tests/a11_factories.rs", 37305),
            ("crates/ssx-infra/tests/a11_replay.rs", 23066),
        ],
    ),
    (
        "docs/rust-rewrite/a12/qualification-v1.json",
        "f55413a595d927c7450b942bc4b545b8db8401e8a9f730e9321680d63bf5ca39",
        &[
            ("crates/ssx-domain/src/records.rs", 11011),
            ("docs/rust-rewrite/CASE_STUDY.md", 21096),
        ],
    ),
];
fn page_inventory(container: &str) -> (&'static str, bool) {
    match container {
        "docs/rust-rewrite/a06/coverage.json" => ("artifacts", false),
        "docs/rust-rewrite/a07/coverage.json"
        | "docs/rust-rewrite/a08/coverage.json"
        | "docs/rust-rewrite/a09/coverage.json" => ("source_inventory", false),
        "docs/rust-rewrite/a10/qualification.json" | "docs/rust-rewrite/a11/qualification.json" => {
            ("owned_sources", false)
        }
        "docs/rust-rewrite/a12/qualification-v1.json" => ("sources", false),
        _ => ("sources", true),
    }
}
fn page_proof_selected(container: &str, source: &str) -> bool {
    PAGE_PROOF_CONTEXTS
        .iter()
        .any(|(c, _, paths)| *c == container && paths.iter().any(|(path, _)| *path == source))
}
fn page_proof_try_verify(
    root: &Path,
    container: &str,
    container_sha: &str,
    source: &str,
    expected: &str,
    bytes: Option<usize>,
) -> Option<Result<(), String>> {
    let (_, sealed_sha, paths) = PAGE_PROOF_CONTEXTS.iter().find(|r| r.0 == container)?;
    let &(_, len) = paths.iter().find(|(path, _)| *path == source)?;
    Some((|| {
        let (field, sized) = page_inventory(container);
        need(
            container_sha == *sealed_sha && bytes == sized.then_some(len),
            "exact Page proof container/size caller",
        )?;
        let raw = historical_caller(root, container, container_sha)?;
        need(
            digest(&raw) == *sealed_sha && permissions(&root.join(container))? == "0644",
            "authenticated Page proof container",
        )?;
        let doc: Value = serde_json::from_slice(&raw).map_err(|e| e.to_string())?;
        let rows = doc[field]
            .as_array()
            .ok_or("Page proof source inventory absent")?
            .iter()
            .filter(|r| r["path"] == source)
            .collect::<Vec<_>>();
        need(
            rows.len() == 1
                && rows[0]["sha256"] == expected
                && rows[0]["bytes"].as_u64() == bytes.map(|n| n as u64),
            "exact original Page proof source tuple",
        )?;
        if !sized {
            let mut exact = json!({"path":source,"sha256":expected});
            if container == "docs/rust-rewrite/a07/coverage.json"
                || container == "docs/rust-rewrite/a08/coverage.json"
            {
                exact["git_blob"] = json!("d609376fa8a8dba26de1d941a60351915f2b1687");
            }
            need(*rows[0] == exact, "exact unsized Page proof row shape")?;
        }
        let caller = json!({"path":source,"sha256":expected,"bytes":len,"mode":"0644"});
        let prior = native_search::page_prior(root, &caller, None)?
            .ok_or("Page proof operand not selected")?;
        need(
            digest(&prior) == expected && prior.len() == len,
            "Page proof exact original material",
        )
    })())
}
pub(crate) fn page_proof_controls(root: &Path) -> Result<(), String> {
    let mut pairs = 0;
    for &(container, sha, paths) in PAGE_PROOF_CONTEXTS {
        let raw = historical_caller(root, container, sha)?;
        let doc: Value = serde_json::from_slice(&raw).map_err(|e| e.to_string())?;
        let (field, sized) = page_inventory(container);
        for &(path, len) in paths {
            pairs += 1;
            let rows = doc[field]
                .as_array()
                .ok_or("Page controls source inventory")?
                .iter()
                .filter(|r| r["path"] == path)
                .collect::<Vec<_>>();
            need(rows.len() == 1, "Page controls exact source")?;
            let expected = text(rows[0], "sha256")?;
            let size = sized.then_some(len);
            page_proof_try_verify(root, container, sha, path, expected, size)
                .ok_or("Page positive route absent")??;
            // Positive is in the caller's exact frame before each refusal family.
            for (hash, source_hash, changed_size) in [
                ("wrong", expected, size),
                (sha, "wrong", size),
                (sha, expected, if sized { None } else { Some(len) }),
                (sha, expected, Some(0)),
            ] {
                need(
                    page_proof_try_verify(root, container, hash, path, source_hash, changed_size)
                        .ok_or("Page negative route lost")?
                        .is_err(),
                    "altered Page proof caller accepted",
                )?;
            }
        }
        need(
            page_proof_try_verify(root, container, sha, "unlisted.rs", "wrong", None).is_none(),
            "unlisted Page proof source selected",
        )?;
    }
    need(pairs == 20, "closed twenty Page proof pairs")
}

/// Keep source-proof controls out of the frozen integration-test inventories.
#[cfg(test)]
#[allow(dead_code)]
pub(crate) fn snapshot_fixture_main_controls(root: &Path) -> Result<(), String> {
    w07_t02_joint::snapshot_fixture_main_controls(root)?;
    let path = "crates/xtask/src/main.rs";
    let current = fs::read(root.join(path)).map_err(|e| e.to_string())?;
    let t02 = w07_t02_joint::parent(root, path, &current, w07_t02_joint::Parent::T02)?;
    let old = json!({"path":path,"sha256":"8755129fbac7454a1ae25982dc44ce1cb92b8ead7ecf650a3901aa2f08d4a23c","bytes":39725,"mode":"0644"});
    need(
        read_exact(root, &old)? == t02,
        "literal e127 main caller proof",
    )
}
/// Only the reviewed Metrics insertion is removed for existing historical callers.
/// The full physical afterimage is required; old bytes alone are never current proof.
pub(crate) fn metrics_predecessor(
    root: &Path,
    path: &str,
    expected: &str,
    actual: &[u8],
) -> Result<Vec<u8>, String> {
    need(
        path == "crates/ssx-application/src/lib.rs"
            && expected == "7d3b8126448059633426e964a1ecc9cc3c2b1fd0ba2cdc7bd75917862cf0cee4",
        "W07 Metrics caller tuple differs",
    )?;
    let operand = w08_source_successor::parent(root, path, actual)?;
    metrics_frame_predecessor(path, expected, &operand)
}
/// Original exact Metrics-frame proof, independently exercised after the W08
/// physical proof. Its prior source remains reconstructible byte for byte.
pub(crate) fn metrics_frame_predecessor(
    path: &str,
    expected: &str,
    actual: &[u8],
) -> Result<Vec<u8>, String> {
    need(
        path == "crates/ssx-application/src/lib.rs"
            && expected == "7d3b8126448059633426e964a1ecc9cc3c2b1fd0ba2cdc7bd75917862cf0cee4",
        "W07 Metrics caller tuple differs",
    )?;

    need(
        actual.len() == 6168
            && digest(actual) == "e328c4b5a4217acbe79a186d1c85d772fbe78ff901bad81ad07aaa458e0ffa8b",
        "W07 Metrics physical afterimage differs",
    )?;
    let at = 2805;
    let insertion: &[u8] = b"    fn workflow_accepted(&self, _: &'static str, _: &'static str) {}\n    fn workflow_rejected(&self, _: &'static str, _: &'static str) {}\n    fn workflow_terminal(&self, _: &'static str, _: &'static str) {}\n    fn workflow_expired(&self, _: &'static str, _: &'static str) {}\n";
    let end = at + insertion.len();
    need(
        actual.get(at..end) == Some(insertion),
        "W07 Metrics exact insertion span differs",
    )?;
    let mut predecessor = actual[..at].to_vec();
    predecessor.extend_from_slice(&actual[end..]);
    need(
        predecessor.len() == 5893 && digest(&predecessor) == expected,
        "W07 Metrics unrelated predecessor bytes changed",
    )?;
    Ok(predecessor)
}

/// Recover only O01's exact frozen Telemetry operand from the reviewed W07 afterimage.
pub(crate) fn o01_metrics_predecessor(
    claim: (&str, &str, &str, &str, Option<usize>),
    caller: &[u8],
    actual: &[u8],
    mode: &str,
) -> Result<Vec<u8>, String> {
    let (container, container_sha, path, expected, bytes) = claim;
    need(
        container == "docs/rust-rewrite/o01/coverage.json"
            && container_sha == "32da35318cb2b2e369b10975bb985aa55469bcc1a39c1549425d1723bd5dcbce"
            && path == "crates/ssx-infra/src/observability/mod.rs"
            && expected == "a2cff0ef1848fcac8c6c7630406217e824d9a6a3b860235843fe69074c5733ae"
            && bytes.is_none(),
        "O01 Metrics exact caller tuple differs",
    )?;
    need(
        digest(caller) == container_sha,
        "O01 Metrics frozen container differs",
    )?;
    let doc: Value = serde_json::from_slice(caller).map_err(|e| e.to_string())?;
    need(
        doc["artifacts"][6] == json!({"path":path,"sha256":expected}),
        "O01 Metrics frozen row differs",
    )?;
    need(
        mode == "0644"
            && actual.len() == 21121
            && digest(actual) == "e32ec1a5d7b0fa487335a2fd55c142bfbf271634a87a2f1e8edcdafa53c4f032",
        "O01 Metrics physical afterimage differs",
    )?;
    let at = 10996;
    let insertion: &[u8] = b"    fn workflow_accepted(&self, workflow: &'static str, mode: &'static str) {\n        let _ = self.metrics.workflow_accepted(workflow, mode);\n    }\n    fn workflow_rejected(&self, workflow: &'static str, reason: &'static str) {\n        let _ = self.metrics.update(\n            \"slopsearx_workflow_rejections_total\",\n            &[(\"workflow\", workflow), (\"reason\", reason)],\n            1.,\n        );\n    }\n    fn workflow_terminal(&self, workflow: &'static str, outcome: &'static str) {\n        let _ = self.metrics.workflow_terminal(workflow, outcome, None);\n    }\n    fn workflow_expired(&self, workflow: &'static str, artifact: &'static str) {\n        let _ = self.metrics.update(\n            \"slopsearx_workflow_expired_total\",\n            &[(\"workflow\", workflow), (\"artifact\", artifact)],\n            1.,\n        );\n    }\n";
    let end = at + insertion.len();
    need(
        actual.get(at..end) == Some(insertion),
        "O01 Metrics forwarding span differs",
    )?;
    let mut prior = actual[..at].to_vec();
    prior.extend_from_slice(&actual[end..]);
    need(
        prior.len() == 20291 && digest(&prior) == expected,
        "O01 Metrics unrelated predecessor bytes differ",
    )?;
    Ok(prior)
}
