#[test]
fn complete_punycode_source_vectors() {
    let rows: serde_json::Value =
        serde_json::from_str(include_str!("punycode-source.json")).unwrap();
    assert_eq!(rows.as_array().unwrap().len(), 2055);
    for row in rows.as_array().unwrap() {
        let hex = row["input_hex"].as_str().unwrap();
        let bytes = (0..hex.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
            .collect::<Vec<_>>();
        let expected = row["expected_codepoints"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| u32::try_from(p.as_u64().unwrap()).unwrap())
            .collect::<Vec<_>>();
        assert_eq!(
            ssx_source_codecs::decode_implemented(&bytes, "punycode")
                .unwrap()
                .unwrap()
                .codepoints(),
            expected,
            "{hex}"
        );
    }
}
