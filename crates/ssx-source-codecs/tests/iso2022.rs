use serde_json::Value;
fn bytes(hex: &str) -> Vec<u8> {
    (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
        .collect()
}
#[test]
fn all_byte_pairs_in_every_iso2022_designated_state_match_complete_source() {
    use std::io::Read;
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/iso2022");
    let manifest: Vec<Value> = serde_json::from_str(include_str!("iso2022/manifest.json")).unwrap();
    assert_eq!(manifest.len(), 39);
    for state in manifest {
        let codec = state["codec"].as_str().unwrap();
        let prefix = bytes(state["prefix_hex"].as_str().unwrap());
        assert_eq!(state["case_count"], 65536);
        let compressed = std::fs::read(root.join(state["file"].as_str().unwrap())).unwrap();
        let mut decoder = flate2::read::ZlibDecoder::new(compressed.as_slice());
        let errors: std::collections::BTreeMap<String, Value> = serde_json::from_slice(
            &std::fs::read(root.join(state["errors"].as_str().unwrap())).unwrap(),
        )
        .unwrap();
        for pair in 0..=u16::MAX {
            let mut row = [0; 21];
            decoder.read_exact(&mut row).unwrap();
            let mut body = prefix.clone();
            body.extend(pair.to_be_bytes());
            body.extend(b"\x1b(B");
            let actual = ssx_source_codecs::decode_implemented(&body, codec).unwrap();
            if row[0] == 255 {
                let error = &errors[&pair.to_string()];
                assert_eq!(error["type"], "RuntimeError");
                assert_eq!(
                    actual.unwrap_err(),
                    error["message"].as_str().unwrap(),
                    "{codec}/state{prefix:?}/{pair:04x}"
                );
            } else {
                let count = usize::from(row[0]);
                assert!(count <= 5);
                let expected = (0..count)
                    .map(|i| u32::from_be_bytes(row[1 + i * 4..5 + i * 4].try_into().unwrap()))
                    .collect::<Vec<_>>();
                assert_eq!(
                    actual.unwrap().codepoints(),
                    expected,
                    "{codec}/state{prefix:?}/{pair:04x}"
                );
            }
        }
        let mut extra = [0];
        assert_eq!(decoder.read(&mut extra).unwrap(), 0);
    }
}
#[test]
fn iso2022_escape_g2_malformed_and_full_streams_match_source() {
    let cases: Vec<Value> = serde_json::from_str(include_str!("iso2022/targeted.json")).unwrap();
    assert_eq!(cases.len(), 44627);
    for case in cases {
        let codec = case["codec"].as_str().unwrap();
        let raw = bytes(case["body_hex"].as_str().unwrap());
        let actual = ssx_source_codecs::decode_implemented(&raw, codec).unwrap();
        if let Some(error) = case.get("error") {
            assert_eq!(error["type"], "RuntimeError");
            assert_eq!(
                actual.unwrap_err(),
                error["message"].as_str().unwrap(),
                "{codec}/{raw:?}"
            );
        } else {
            let expected = case["points"]
                .as_array()
                .unwrap()
                .iter()
                .map(|p| p.as_u64().unwrap() as u32)
                .collect::<Vec<_>>();
            assert_eq!(actual.unwrap().codepoints(), expected, "{codec}/{raw:?}");
        }
    }
}
