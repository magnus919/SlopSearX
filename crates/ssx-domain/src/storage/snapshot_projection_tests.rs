//! Pure frozen-before controls; the original codec bytes are compiled only here.
use super::*;
use sha2::{Digest, Sha256};
#[path = "snapshot_projection_frozen.rs"]
mod frozen;
fn base() -> Value {
    ssx_json::json!({"snapshot_id":"snap-projection","query":"q","query_id":"query","results":[],"scope":{},"total":0,"tenant":"tenant-a","created_at":1.0,"expires_at":5.0,"lineage":[]})
}
fn compare(name: &str, input: Value) {
    let old = frozen::decode(input.clone());
    let full = snapshot::decode(input.clone());
    assert_eq!(old, full, "full decoder changed: {name}");
    match (old, snapshot::decode_rows(input)) {
        (Ok(expected), Ok(rows)) => {
            assert_eq!(
                rows.summary(),
                &snapshot::SnapshotSummary::from_snapshot(&expected),
                "header: {name}"
            );
            let mut count = 0;
            rows.visit_results(&mut |index, row| {
                assert_eq!(index, count);
                assert_eq!(row, &expected.results()[index], "row: {name}/{index}");
                let encoded = |row: &crate::SearchResult| {
                    encode_result(row)
                        .and_then(|v| ssx_json::to_vec(&v).map_err(|_| StorageError::Serialization))
                };
                assert_eq!(
                    encoded(row),
                    encoded(&expected.results()[index]),
                    "raw numeric/order bytes: {name}/{index}"
                );
                count += 1;
            })
            .unwrap();
            assert_eq!(count, expected.total());
            let full = full.unwrap();
            assert_eq!(frozen::encode(&expected), snapshot::encode(&full));
        }
        (Err(expected), Err(actual)) => assert_eq!(expected, actual, "projected error: {name}"),
        _ => panic!("projected/full outcome differs: {name}"),
    }
    eprintln!("W07_SNAPSHOT_DECODER_CASE {name}");
}
#[test]
fn frozen_snapshot_decoder_matches_all31_retained_source_rows_and199_f05_input_wrappers() {
    assert_eq!(
        Sha256::digest(include_bytes!("snapshot_projection_frozen.rs"))
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>(),
        "c6d0d79c0c40bcf0e069a5a985dcf7122ceb8c09995b71f737b454002a0a289e"
    );
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../xtask/fixtures");
    let f13: Value =
        ssx_json::from_slice(&std::fs::read(root.join("f13/source.json")).unwrap()).unwrap();
    let source = f13["cases"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|case| {
            matches!(
                case["kind"].as_str(),
                Some(
                    "snapshot-read" | "snapshot-integrity" | "snapshot-scope" | "snapshot-lineage"
                )
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(source.len(), 31);
    for case in source {
        compare(
            &format!(
                "source:{}:{}",
                case["kind"].as_str().unwrap(),
                case["id"].as_str().unwrap()
            ),
            case["input"]["payload"].clone(),
        );
    }
    let f05: Value =
        ssx_json::from_slice(&std::fs::read(root.join("f05/corpus.json")).unwrap()).unwrap();
    let cases = f05["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 199);
    let mut names = std::collections::BTreeSet::new();
    for case in cases {
        let name = case["id"].as_str().unwrap();
        assert!(names.insert(name));
        // Every original F05 input is reused as a synthetic snapshot result row.
        // This is a differential decoder control, not a claim that all source
        // operations originally consumed their inputs as snapshot results.
        let mut input = base();
        input["results"] = Value::Array(vec![case["input"].clone()]);
        input["total"] = ssx_json::json!(1);
        compare(&format!("f05-input-wrapper:{name}"), input);
    }
}
#[test]
fn frozen_snapshot_decoder_keeps_competing_invalid_precedence_and_opaque_defaults() {
    let mut cases = vec![
        ("root-scalar", ssx_json::json!(1)),
        ("root-array", ssx_json::json!([])),
    ];
    for (name, updates) in [
        (
            "scope-before-row",
            ssx_json::json!({"scope":true,"results":[1],"total":null}),
        ),
        (
            "results-shape-before-total",
            ssx_json::json!({"results":true,"total":null}),
        ),
        (
            "row-before-total",
            ssx_json::json!({"results":[1],"total":null}),
        ),
        (
            "later-row-before-total",
            ssx_json::json!({"results":[{},1],"total":null}),
        ),
        (
            "row-score-before-tier",
            ssx_json::json!({"results":[{"score":"bad","tier":"bad"}],"total":1}),
        ),
        (
            "null-total-before-dates",
            ssx_json::json!({"total":null,"created_at":"bad","expires_at":"bad"}),
        ),
        (
            "total-before-dates",
            ssx_json::json!({"total":1,"created_at":"bad"}),
        ),
        (
            "created-before-expiry",
            ssx_json::json!({"created_at":"bad","expires_at":"bad"}),
        ),
        (
            "expiry-before-jev",
            ssx_json::json!({"expires_at":"bad","scope":{"jev_scores":true}}),
        ),
        (
            "jev-before-scope",
            ssx_json::json!({"scope":{"jev_scores":true,"selected_engines":true}}),
        ),
        (
            "scope-before-id",
            ssx_json::json!({"snapshot_id":"","scope":{"selected_engines":true}}),
        ),
        (
            "id-before-query-id",
            ssx_json::json!({"snapshot_id":"","query_id":""}),
        ),
        (
            "tenant-before-backward-expiry",
            ssx_json::json!({"tenant":"","created_at":5.0,"expires_at":1.0}),
        ),
        (
            "backward-expiry-before-lineage",
            ssx_json::json!({"created_at":5.0,"expires_at":1.0,"lineage":true}),
        ),
        (
            "falsy-defaults",
            ssx_json::json!({"scope":false,"results":false,"lineage":false}),
        ),
        (
            "legacy-unicode-scope",
            ssx_json::json!({"scope":{"selected_engines":"雪a","warnings":{"first":1,"second":2}}}),
        ),
    ] {
        let mut input = base();
        for (key, value) in updates.as_object().unwrap() {
            input[key.as_str()] = value.clone();
        }
        cases.push((name, input));
    }
    let mut input = base();
    input["results"]=ssx_json::from_str(r#"[{"payload":{"n":18446744073709551617,"f":1.0000000000000000000000000000000000000000000,"$serde_json::private::Number":"literal","unicode":"雪😀"}},{"engines":"{'a', 'b'}"}]"#).unwrap();
    input["total"] = ssx_json::json!(2);
    cases.push(("opaque-defaults", input));
    assert_eq!(cases.len(), 19);
    for (name, input) in cases {
        compare(name, input);
    }
}
