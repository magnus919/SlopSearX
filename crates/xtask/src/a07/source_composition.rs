//! Finite A07 source composition; frozen evidence remains historical and immutable.
use super::{digest, exact_test_present, require};
use crate::Result;
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::ErrorKind,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};
pub(super) const MATRIX_TEST: &str =
    "actual_source_matrix_preserves_requests_and_full_normalized_results";
const CAPSULE: &str = "docs/rust-rewrite/a07/source-amendments/pure-a06-composition-v1.json";
const CAPSULE_SHA: &str = "58cfde8aa2635d2150792a386c2f0b4570ca544d869967c6c2f80bf9e1c3a0ec";
const ORIGINALS: &[(&str, &str, u64)] = &[
    (
        "crates/ssx-infra/src/http.rs",
        "c5116d9a0081a0497a12643440053f001b6d68b48e9b4bf8c5feabf81eed5d10",
        54774,
    ),
    (
        "crates/ssx-infra/src/http/monotonic.rs",
        "1a58034e2ca03360d8bb4cdf419353e3677be0528704962a66ecb4add388a24f",
        2433,
    ),
    (
        "crates/ssx-infra/src/adapters/clinicaltrials.rs",
        "2baaac73ee64f63192ec9c35ad44577d42cf4c314d1b6d3c57c7447086319485",
        42996,
    ),
    (
        "crates/ssx-infra/tests/a07_biomedical.rs",
        "6197dc61021719257105ce255b981ccbb95a2ed5fe60cbff67e891f3b2d0cf9a",
        30369,
    ),
];
#[derive(Clone)]
struct Material {
    bytes: Vec<u8>,
    mode: u32,
}
fn string<'a>(v: &'a Value, key: &str) -> Result<&'a str> {
    v[key]
        .as_str()
        .ok_or_else(|| format!("A07 composition missing {key}"))
}
fn array<'a>(v: &'a Value, key: &str) -> Result<&'a Vec<Value>> {
    v[key]
        .as_array()
        .ok_or_else(|| format!("A07 composition missing {key}"))
}
#[cfg(any(unix, test))]
fn permission_bits(raw: u32) -> u32 {
    raw & 0o7777
}
fn load(root: &Path, path: &str) -> Result<Material> {
    let p = root.join(path);
    let metadata = fs::symlink_metadata(&p).map_err(|e| e.to_string())?;
    require(
        metadata.file_type().is_file(),
        "A07 composition requires regular source",
    )?;
    #[cfg(unix)]
    let mode = {
        use std::os::unix::fs::PermissionsExt;
        permission_bits(metadata.permissions().mode())
    };
    #[cfg(not(unix))]
    let mode = 0o644;
    Ok(Material {
        bytes: fs::read(p).map_err(|e| e.to_string())?,
        mode,
    })
}
/// Named09ee operand only at this private proof boundary, after actual-child proof.
fn composition_material(root: &Path, path: &str) -> Result<Material> {
    if path == "crates/ssx-infra/src/http.rs" {
        Ok(Material {
            bytes: crate::a13::a06_source_successor::real_clock_http_prior(
                root,
                crate::a13::a06_source_successor::RealClockHttpCaller::A07,
            )?,
            mode: 0o644,
        })
    } else if path == "crates/ssx-infra/tests/support/default_ca_environment.rs" {
        Ok(Material {
            bytes: crate::a13::a06_source_successor::default_ca_fixture_operand(
                root,
                path,
                "00aee0178f3f111449e5a6e99f495538a1eafa29ec95ced7d8343a3a8ee1108c",
                Some(959),
            )
            .ok_or("A07 default-CA fixture tuple not selected")??,
            mode: 0o644,
        })
    } else {
        load(root, path)
    }
}

fn check_material(row: &Value, material: &Material) -> Result<()> {
    require(
        row["bytes"].as_u64() == Some(material.bytes.len() as u64)
            && row["sha256"].as_str() == Some(digest(&material.bytes).as_str())
            && row["mode"].as_u64() == Some(u64::from(material.mode))
            && material.mode == 0o644,
        "A07 composition material identity differs",
    )
}
fn reconstruct(old: &[u8], edits: &[Value]) -> Result<Vec<u8>> {
    require(!edits.is_empty(), "A07 composition empty transition")?;
    let mut result = Vec::new();
    let mut cursor = 0usize;
    let mut previous = None;
    for edit in edits {
        require(
            edit.as_object().is_some_and(|o| o.len() == 3),
            "A07 composition edit fields",
        )?;
        let at = usize::try_from(edit["at"].as_u64().ok_or("A07 composition edit offset")?)
            .map_err(|_| "A07 composition offset overflow")?;
        require(
            previous.is_none_or(|p| at > p) && at >= cursor && at <= old.len(),
            "A07 composition edit order",
        )?;
        let removed = string(edit, "remove_utf8")?.as_bytes();
        let inserted = string(edit, "insert_utf8")?.as_bytes();
        require(removed != inserted, "A07 composition no-op edit")?;
        let end = at
            .checked_add(removed.len())
            .ok_or("A07 composition edit overflow")?;
        require(
            old.get(at..end) == Some(removed),
            "A07 composition edit preimage",
        )?;
        result.extend_from_slice(&old[cursor..at]);
        result.extend_from_slice(inserted);
        cursor = end;
        previous = Some(at);
    }
    result.extend_from_slice(&old[cursor..]);
    Ok(result)
}
fn verify_payload(
    doc: &Value,
    predecessor: &Value,
    loader: &impl Fn(&str) -> Result<Material>,
) -> Result<()> {
    require(
        doc["schema"] == "slopsearx.a07.pure-a06-composition.v1",
        "A07 composition schema",
    )?;
    require(
        doc["predecessor_candidate_sources"] == predecessor["candidate_sources"],
        "A07 composition predecessor candidates",
    )?;
    let transitions = array(doc, "transitions")?;
    require(
        transitions.len() == ORIGINALS.len(),
        "A07 composition transition scope",
    )?;
    for (transition, (path, sha, bytes)) in transitions.iter().zip(ORIGINALS) {
        let original = &transition["before"];
        require(
            original == &json!({"path":path,"sha256":sha,"bytes":bytes,"mode":420}),
            "A07 composition predecessor tuple",
        )?;
        let archive = &transition["archive"];
        require(
            archive
                == &json!({"path":format!("docs/rust-rewrite/a07/source-amendments/pure-a06-v1/d561/{path}.frozen"),"sha256":sha,"bytes":bytes,"mode":420}),
            "A07 composition archive tuple",
        )?;
        let old = loader(string(archive, "path")?)?;
        check_material(archive, &old)?;
        let current = &transition["after"];
        require(
            current["path"].as_str() == Some(*path),
            "A07 composition current path",
        )?;
        let actual = loader(path)?;
        check_material(current, &actual)?;
        require(
            reconstruct(&old.bytes, array(transition, "edits")?)? == actual.bytes,
            "A07 composition reconstruction differs",
        )?;
    }
    // The production capsule digest closes the exact 16-source list and its metadata.
    let fixed = array(doc, "fixed_sources")?;
    require(fixed.len() == 16, "A07 composition fixed scope")?;
    let mut seen = BTreeSet::new();
    for row in fixed {
        let path = string(row, "path")?;
        require(seen.insert(path), "A07 composition duplicate fixed source")?;
        check_material(row, &loader(path)?)?;
    }
    Ok(())
}
fn parse(raw: &[u8]) -> Result<Value> {
    require(
        digest(raw) == CAPSULE_SHA,
        "A07 composition capsule changed",
    )?;
    serde_json::from_slice(raw).map_err(|e| e.to_string())
}
pub(super) fn verify_sources(
    root: &Path,
    predecessor: &Value,
    records: &mut Vec<Value>,
) -> Result<Value> {
    let capsule = load(root, CAPSULE)?;
    require(capsule.mode == 0o644, "A07 composition capsule mode")?;
    let doc = parse(&capsule.bytes)?;
    verify_payload(&doc, predecessor, &|p| composition_material(root, p))?;
    records.push(json!({"A07_source_composition":{"capsule":CAPSULE,"sha256":CAPSULE_SHA,"transitions":4,"fixed_sources":16,"frozen_evidence":"unchanged; exact historical Git inventory checks retained"}}));
    Ok(doc)
}
fn moved() -> Vec<Value> {
    ["clinicaltrials","europepmc","pubchem","uniprot"].into_iter().map(|name| json!({"id":format!("{name}-config-timeout_ms-bool-True"),"package":"ssx-infra","target":"lib","qualified_name":format!("adapters::clinicaltrials::owner_clock_tests::{name}_bool_true_full_source_replay")})).collect()
}
pub(super) fn verify_partition(
    doc: &Value,
    ids: &BTreeSet<String>,
    available: &BTreeMap<(String, String), BTreeSet<String>>,
) -> Result<()> {
    let expected = json!({"total":172,"integration_count":168,"integration":{"package":"ssx-infra","target":"test:a07_biomedical","qualified_name":MATRIX_TEST},"moved":moved()});
    require(
        doc["execution_partition"] == expected,
        "A07 composition execution partition changed",
    )?;
    require(ids.len() == 172, "A07 composition primary count")?;
    exact_test_present(&expected["integration"], available)?;
    for binding in moved() {
        require(
            ids.contains(string(&binding, "id")?),
            "A07 composition moved ID absent",
        )?;
        exact_test_present(&binding, available)?;
    }
    Ok(())
}
fn expected_ids(ids: &BTreeSet<String>, package: &str, target: &str) -> BTreeSet<String> {
    let moved: BTreeSet<String> = moved()
        .iter()
        .map(|r| r["id"].as_str().unwrap().to_owned())
        .collect();
    if package == "ssx-infra" && target == "lib" {
        moved
    } else if package == "ssx-infra" && target == "test:a07_biomedical" {
        ids.difference(&moved).cloned().collect()
    } else {
        BTreeSet::new()
    }
}
fn verify_receipts(raw: &str, expected: &BTreeSet<String>) -> Result<()> {
    require(
        raw.is_empty() || raw.ends_with('\n'),
        "A07 composition partial execution receipt",
    )?;
    let mut found = BTreeSet::new();
    for line in raw.lines() {
        let row: Value = serde_json::from_str(line).map_err(|e| e.to_string())?;
        let id = string(&row, "id")?;
        require(
            row == json!({"id":id,"kind":"full_public_SearchEngine","compared":"original_config_status_requests_complete_output"}),
            "A07 composition malformed execution receipt",
        )?;
        require(
            expected.contains(id) && found.insert(id.to_owned()),
            "A07 composition unexpected or duplicate execution receipt",
        )?;
    }
    require(
        found == *expected,
        "A07 composition missing full-source execution receipts",
    )
}
struct Receipt(PathBuf);
impl Drop for Receipt {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}
fn receipt() -> Result<Receipt> {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let base = std::env::var_os("SLOPSEARX_TEST_TEMP_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_nanos();
    for _ in 0..64 {
        let path = base.join(format!(
            "a07-replay-{}-{now}-{}.jsonl",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(_) => return Ok(Receipt(path)),
            Err(e) if e.kind() == ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e.to_string()),
        }
    }
    Err("A07 composition receipt allocation exhausted".into())
}
pub(super) fn execute(
    root: &Path,
    package: &str,
    target: &str,
    args: &[&str],
    ids: &BTreeSet<String>,
    records: &mut Vec<Value>,
) -> Result<String> {
    let receipt = receipt()?;
    let assignment = format!(
        "A07_COMPLETED_CASES={}",
        receipt.0.to_str().ok_or("A07 receipt path is not UTF8")?
    );
    let mut child = vec![assignment.as_str(), "cargo"];
    child.extend_from_slice(args);
    let output = crate::command(root, "env", &child, records);
    let metadata = fs::symlink_metadata(&receipt.0).map_err(|e| e.to_string())?;
    require(
        metadata.file_type().is_file() && metadata.len() <= 1024 * 1024,
        "A07 composition execution receipt type/size",
    )?;
    let raw = fs::read_to_string(&receipt.0).map_err(|e| e.to_string())?;
    let expected = expected_ids(ids, package, target);
    records.push(json!({"A07_full_source_execution":{"package":package,"target":target,"expected":expected.len(),"receipt_sha256":digest(raw.as_bytes()),"raw_jsonl":raw}}));
    let output = output?;
    verify_receipts(&raw, &expected)?;
    Ok(output)
}
#[cfg(test)]
pub(super) fn controls(root: &Path) {
    assert_eq!(permission_bits(0o100644), 0o644);
    assert_eq!(permission_bits(0o104644), 0o4644);
    let raw = fs::read(root.join(CAPSULE)).unwrap();
    let doc = parse(&raw).unwrap();
    let predecessor: Value = serde_json::from_slice(&fs::read(root.join("docs/rust-rewrite/a07/source-amendments/pubchem-independent-budget-owner-clock-v1.json")).unwrap()).unwrap();
    let mut materials = BTreeMap::new();
    for t in doc["transitions"].as_array().unwrap() {
        for row in [&t["archive"], &t["after"]] {
            let p = row["path"].as_str().unwrap();
            materials.insert(p.to_owned(), composition_material(root, p).unwrap());
        }
    }
    for row in doc["fixed_sources"].as_array().unwrap() {
        let p = row["path"].as_str().unwrap();
        materials.insert(p.to_owned(), composition_material(root, p).unwrap());
    }
    let verify = |d: &Value, p: &Value, m: &BTreeMap<String, Material>| {
        verify_payload(d, p, &|path| {
            m.get(path)
                .cloned()
                .ok_or("A07 control missing material".into())
        })
    };
    verify(&doc, &predecessor, &materials).unwrap();
    assert!(parse(&[raw.as_slice(), b"\n"].concat()).is_err());
    for row in &doc["transitions"].as_array().unwrap()[..] {
        for key in ["archive", "after"] {
            let path = row[key]["path"].as_str().unwrap();
            let mut missing = materials.clone();
            missing.remove(path);
            assert!(verify(&doc, &predecessor, &missing).is_err());
            let mut changed = materials.clone();
            changed.get_mut(path).unwrap().bytes.push(b'!');
            assert!(verify(&doc, &predecessor, &changed).is_err());
            for mode in [0o600, 0o4644] {
                let mut wrong_mode = materials.clone();
                wrong_mode.get_mut(path).unwrap().mode = permission_bits(0o100000 | mode);
                assert!(verify(&doc, &predecessor, &wrong_mode).is_err());
            }
        }
    }
    for row in doc["fixed_sources"].as_array().unwrap() {
        let path = row["path"].as_str().unwrap();
        let mut missing = materials.clone();
        missing.remove(path);
        assert!(verify(&doc, &predecessor, &missing).is_err());
        let mut changed = materials.clone();
        changed.get_mut(path).unwrap().bytes.push(b'!');
        assert!(verify(&doc, &predecessor, &changed).is_err());
        for mode in [0o600, 0o4644] {
            let mut wrong_mode = materials.clone();
            wrong_mode.get_mut(path).unwrap().mode = permission_bits(0o100000 | mode);
            assert!(verify(&doc, &predecessor, &wrong_mode).is_err());
        }
    }
    let mut wrong_schema = doc.clone();
    wrong_schema["schema"] = json!("another-source-container");
    assert_eq!(
        verify(&wrong_schema, &predecessor, &materials).unwrap_err(),
        "A07 composition schema"
    );
    for index in 0..ORIGINALS.len() {
        let path = ORIGINALS[index].0;
        let archive = doc["transitions"][index]["archive"]["path"]
            .as_str()
            .unwrap();
        let mut rollback = materials.clone();
        rollback.insert(path.into(), materials[archive].clone());
        assert!(verify(&doc, &predecessor, &rollback).is_err());
        let mut rollback_pins = doc.clone();
        rollback_pins["transitions"][index]["after"]["sha256"] =
            doc["transitions"][index]["before"]["sha256"].clone();
        rollback_pins["transitions"][index]["after"]["bytes"] =
            doc["transitions"][index]["before"]["bytes"].clone();
        assert_eq!(
            verify(&rollback_pins, &predecessor, &rollback).unwrap_err(),
            "A07 composition reconstruction differs"
        );
        // Rehashing an unrelated current edit passes material identity but must fail reconstruction.
        let mut changed = materials.clone();
        changed.get_mut(path).unwrap().bytes.push(b'!');
        let mut amended = doc.clone();
        let bytes = &changed[path].bytes;
        amended["transitions"][index]["after"]["sha256"] = json!(digest(bytes));
        amended["transitions"][index]["after"]["bytes"] = json!(bytes.len());
        assert_eq!(
            verify(&amended, &predecessor, &changed).unwrap_err(),
            "A07 composition reconstruction differs"
        );
    }
    let mut wrong_predecessor = predecessor.clone();
    wrong_predecessor["candidate_sources"]
        .as_array_mut()
        .unwrap()
        .pop();
    assert!(verify(&doc, &wrong_predecessor, &materials).is_err());
    let mut wrong = doc.clone();
    wrong["transitions"][0]["before"]["sha256"] = json!("rehash");
    assert_eq!(
        verify(&wrong, &predecessor, &materials).unwrap_err(),
        "A07 composition predecessor tuple"
    );
    let mut wrong = doc.clone();
    wrong["transitions"].as_array_mut().unwrap().pop();
    assert!(verify(&wrong, &predecessor, &materials).is_err());
    let mut wrong = doc.clone();
    wrong["transitions"][0]["after"]["path"] = json!("unlisted.rs");
    assert_eq!(
        verify(&wrong, &predecessor, &materials).unwrap_err(),
        "A07 composition current path"
    );
    for kind in ["missing", "duplicate", "moved", "preimage", "insert"] {
        let mut altered = doc.clone();
        let edits = altered["transitions"][0]["edits"].as_array_mut().unwrap();
        match kind {
            "missing" => {
                edits.remove(0);
            }
            "duplicate" => {
                let first = edits[0].clone();
                edits.insert(1, first);
            }
            "moved" => {
                edits.swap(0, 1);
            }
            "preimage" => {
                edits[0]["remove_utf8"] = json!("wrong preimage");
            }
            _ => {
                edits[0]["insert_utf8"] = json!("unlisted change");
            }
        }
        let error = verify(&altered, &predecessor, &materials).unwrap_err();
        assert_eq!(
            error,
            match kind {
                "duplicate" | "moved" => "A07 composition edit order",
                "preimage" => "A07 composition edit preimage",
                _ => "A07 composition reconstruction differs",
            }
        );
    }
    let mut identities: BTreeSet<String> = predecessor["original_integration19"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().into())
        .collect();
    identities.insert(MATRIX_TEST.into());
    let mut resources: BTreeSet<String> = predecessor["original_resource17"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().into())
        .collect();
    resources.insert(
        predecessor["private_control"]["qualified_name"]
            .as_str()
            .unwrap()
            .into(),
    );
    let baseline = BTreeMap::from([
        (
            ("ssx-infra".into(), "test:a07_biomedical".into()),
            identities.clone(),
        ),
        (("ssx-infra".into(), "lib".into()), resources.clone()),
    ]);
    super::pubchem_clock_binding(&predecessor, &baseline).unwrap();
    for name in identities {
        let mut missing = baseline.clone();
        missing
            .get_mut(&("ssx-infra".into(), "test:a07_biomedical".into()))
            .unwrap()
            .remove(&name);
        assert!(super::pubchem_clock_binding(&predecessor, &missing).is_err());
    }
    for name in resources {
        let mut missing = baseline.clone();
        missing
            .get_mut(&("ssx-infra".into(), "lib".into()))
            .unwrap()
            .remove(&name);
        assert!(super::pubchem_clock_binding(&predecessor, &missing).is_err());
    }
    let mut extra = baseline.clone();
    extra
        .get_mut(&("ssx-infra".into(), "test:a07_biomedical".into()))
        .unwrap()
        .insert("unlisted".into());
    assert!(super::pubchem_clock_binding(&predecessor, &extra).is_err());
    let mut aliased = predecessor.clone();
    aliased["private_control"]["target"] = json!("test:a07_biomedical");
    assert!(super::pubchem_clock_binding(&aliased, &baseline).is_err());
    let ids = super::primary_ids(root).unwrap();
    let partition = &doc["execution_partition"];
    let mut available = BTreeMap::new();
    available.insert(
        ("ssx-infra".into(), "test:a07_biomedical".into()),
        BTreeSet::from([MATRIX_TEST.into()]),
    );
    available.insert(
        ("ssx-infra".into(), "lib".into()),
        moved()
            .iter()
            .map(|v| v["qualified_name"].as_str().unwrap().into())
            .collect(),
    );
    verify_partition(&doc, &ids, &available).unwrap();
    for key in ["total", "integration_count"] {
        let mut wrong = doc.clone();
        wrong["execution_partition"][key] = json!(0);
        assert!(verify_partition(&wrong, &ids, &available).is_err());
    }
    for index in 0..4 {
        let mut wrong = doc.clone();
        wrong["execution_partition"]["moved"][index]["target"] = json!("test:a07_biomedical");
        assert!(verify_partition(&wrong, &ids, &available).is_err());
    }
    assert_eq!(partition["moved"].as_array().unwrap().len(), 4);
    for (package, target, count) in [
        ("ssx-infra", "lib", 4),
        ("ssx-infra", "test:a07_biomedical", 168),
        ("ssx-mcp", "test:a07_consumers", 0),
    ] {
        let expected = expected_ids(&ids, package, target);
        assert_eq!(expected.len(), count);
        let raw=expected.iter().map(|id|format!("{}\n",json!({"id":id,"kind":"full_public_SearchEngine","compared":"original_config_status_requests_complete_output"}))).collect::<String>();
        verify_receipts(&raw, &expected).unwrap();
        if count > 0 {
            assert!(verify_receipts("", &expected).is_err());
            assert!(verify_receipts(&format!("{raw}{raw}"), &expected).is_err());
            assert!(verify_receipts(raw.trim_end_matches('\n'), &expected).is_err());
            assert!(
                verify_receipts(
                    &raw.replacen("full_public_SearchEngine", "partial_transport_only", 1),
                    &expected
                )
                .is_err()
            );
            let missing_one = raw
                .lines()
                .skip(1)
                .map(|line| format!("{line}\n"))
                .collect::<String>();
            assert!(verify_receipts(&missing_one, &expected).is_err());
            let other = expected_ids(
                &ids,
                "ssx-infra",
                if target == "lib" {
                    "test:a07_biomedical"
                } else {
                    "lib"
                },
            );
            assert!(verify_receipts(&raw, &other).is_err());
        }
        assert!(verify_receipts(&format!("{raw}{{\"id\":\"unlisted\"}}\n"), &expected).is_err());
        assert!(verify_receipts(&format!("{raw}malformed\n"), &expected).is_err());
    }
}
