#[test]
fn every_captured_single_byte_and_alias_matches_complete_source_output() {
    let rows: serde_json::Value =
        serde_json::from_str(include_str!("singlebyte-source.json")).unwrap();
    assert_eq!(rows.as_array().unwrap().len(), 69);
    let bytes = (0..=255).collect::<Vec<u8>>();
    for row in rows.as_array().unwrap() {
        let canonical = row["canonical"].as_str().unwrap();
        let expected = row["expected"].as_str().unwrap();
        assert_eq!(
            ssx_source_codecs::decode_single_byte(&bytes, canonical).as_deref(),
            Some(expected),
            "{canonical}"
        );
        for alias in row["aliases"].as_array().unwrap() {
            let alias = alias.as_str().unwrap();
            assert_eq!(
                ssx_source_codecs::canonical(alias),
                Some(canonical),
                "{alias}"
            );
            assert_eq!(
                ssx_source_codecs::decode_single_byte(&bytes, alias).as_deref(),
                Some(expected),
                "{alias}"
            );
        }
    }
}
