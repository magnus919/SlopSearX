use ssx_domain::{
    canonical::{JsonProfile, canonical_json},
    staged::*,
};
use ssx_json::{Value, json};
fn fixture() -> Value {
    ssx_json::from_str(include_str!("../../xtask/fixtures/w11/corpus.json")).unwrap()
}
#[test]
fn source_identity_and_schema_array_normalization() {
    let f = fixture();
    let r = Record::from_value(f["record"].clone()).unwrap();
    assert_eq!(
        String::from_utf8(canonical_json(&identity(r.value()), JsonProfile::AsciiCompact).unwrap())
            .unwrap(),
        f["canonical_identity"].as_str().unwrap()
    );
    assert_eq!(r.value()["plan_digest"], f["digest"]);
    for row in f["normalizations"].as_array().unwrap() {
        let result = Record::from_value(row["input"].clone());
        assert_eq!(result.is_ok(), row["native_accept"].as_bool().unwrap());
        if let Ok(r) = result {
            assert_eq!(r.value(), &row["source"]);
        }
    }
}
#[test]
fn malformed_records_never_panic_or_invent_budget() {
    let f = fixture();
    for field in [
        "tenant",
        "operation_id",
        "version",
        "plan",
        "objectives",
        "budget",
        "stages",
        "next_stage",
        "retry_keys",
        "plan_digest",
    ] {
        for bad in [
            json!(null),
            json!(false),
            json!(42),
            json!([]),
            json!({"bad":1}),
        ] {
            let mut v = f["record"].clone();
            if field == "retry_keys" && bad == json!([]) {
                continue;
            }
            v[field] = bad;
            assert!(Record::from_value(v).is_err(), "{field}");
        }
    }
    let mut v = f["record"].clone();
    v["stages"].as_array_mut().unwrap()[0]["attempts"] = json!([42]);
    assert!(Record::from_value(v).is_err());
    let mut v = f["record"].clone();
    v["budget"]["reserved"] = json!(1);
    assert!(Record::from_value(v).is_err());
    let controls: Value = ssx_json::from_str(include_str!(
        "../../xtask/fixtures/w11/source-controls.json"
    ))
    .unwrap();
    assert_eq!(controls["source_read"][1]["record_found"], json!(true));
    assert_eq!(controls["source_read"][1]["expired"], json!(false));
    assert_eq!(controls["source_read"][2]["expired"], json!(true));
    assert_eq!(
        controls["source_real_valkey_retry_at_equality"]["status"],
        json!("expired")
    );
}
