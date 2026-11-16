#[test]
fn every_source_name_alias_sequence_case_and_range_control() {
    let rows: Vec<(String, Vec<u32>)> =
        serde_json::from_str(include_str!("unicode-name-source.json")).unwrap();
    assert_eq!(rows.len(), 104357);
    for (name, expected) in rows {
        let body = format!("\\N{{{name}}}");
        assert_eq!(
            ssx_source_codecs::decode_implemented(body.as_bytes(), "unicode_escape")
                .unwrap()
                .unwrap()
                .codepoints(),
            expected,
            "name={name:?}"
        );
    }
}
