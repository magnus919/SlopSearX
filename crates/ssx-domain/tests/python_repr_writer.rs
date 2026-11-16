use sha2::{Digest, Sha256};
use ssx_domain::storage::{python_value_string, python_value_string_len, write_python_value};
use ssx_json::Value;

#[test]
fn counting_and_writing_match_across_preserved_source_records() {
    let corpora = [
        include_str!("../../../docs/rust-rewrite/a06/source-initial.json"),
        include_str!("../../../docs/rust-rewrite/a06/source-regressions.json"),
        include_str!("../../../docs/rust-rewrite/a06/source-shared.json"),
        include_str!("../../../docs/rust-rewrite/a06/source-xml.json"),
    ];
    let oracle: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/python-repr-oracle.json")).unwrap();
    let mut records = 0;
    for (corpus_index, corpus) in corpora.into_iter().enumerate() {
        let expected = &oracle["corpora"][corpus_index];
        assert_eq!(
            hex_digest(corpus.as_bytes()),
            expected["input_sha256"].as_str().unwrap()
        );
        let root: Value = ssx_json::from_str(corpus).unwrap();
        let source_records = root["records"].as_array().unwrap();
        assert_eq!(
            source_records.len(),
            expected["repr_sha256"].as_array().unwrap().len()
        );
        for (index, value) in source_records.iter().enumerate() {
            let mut written = String::new();
            write_python_value(value, &mut written).unwrap();
            assert_eq!(python_value_string_len(value), Some(written.len()));
            assert_eq!(python_value_string(value), written);
            assert_eq!(
                hex_digest(written.as_bytes()),
                expected["repr_sha256"][index].as_str().unwrap(),
                "corpus {corpus_index} record {index}"
            );
            records += 1;
        }
    }
    assert_eq!(records, 139);
}

#[test]
fn sink_failures_propagate_at_each_nested_key_and_escape_boundary() {
    struct FailAfter(usize);
    impl std::fmt::Write for FailAfter {
        fn write_str(&mut self, text: &str) -> std::fmt::Result {
            self.0 = self.0.checked_sub(text.len()).ok_or(std::fmt::Error)?;
            Ok(())
        }
    }
    let value: Value =
        ssx_json::from_str(r#"{"a'b\n\u200b":[true,null,1.5,{"x\"y":"\u0000\t\\界"}]}"#).unwrap();
    let expected = python_value_string(&value);
    for limit in 0..expected.len() {
        assert!(
            write_python_value(&value, &mut FailAfter(limit)).is_err(),
            "{limit}"
        );
    }
    assert!(write_python_value(&value, &mut FailAfter(expected.len())).is_ok());
    assert_eq!(python_value_string_len(&value), Some(expected.len()));
}

#[test]
fn direct_values_match_independent_cpython_str_outcomes() {
    let oracle: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/python-repr-oracle.json")).unwrap();
    for control in oracle["controls"].as_array().unwrap() {
        let value: Value = ssx_json::from_str(control["json"].as_str().unwrap()).unwrap();
        let expected = control["python_str"].as_str().unwrap();
        let mut written = String::new();
        write_python_value(&value, &mut written).unwrap();
        assert_eq!(written, expected, "{}", control["json"]);
        assert_eq!(python_value_string_len(&value), Some(expected.len()));
    }
}

fn hex_digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
