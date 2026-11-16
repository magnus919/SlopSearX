#[test]
fn complete_httpx_unicode_malformed_bmp_and_truncation_source_vectors() {
    use serde_json::Value;
    use std::io::Read;
    let manifest: Vec<Value> = serde_json::from_str(include_str!("unicode/manifest.json")).unwrap();
    assert_eq!(manifest.len(), 9);
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/unicode");
    for group in manifest {
        let codec = group["codec"].as_str().unwrap();
        let data = std::fs::read(root.join(group["file"].as_str().unwrap())).unwrap();
        let mut decoder = flate2::read::ZlibDecoder::new(data.as_slice());
        let mut json = String::new();
        decoder.read_to_string(&mut json).unwrap();
        let cases: Vec<Value> = serde_json::from_str(&json).unwrap();
        assert_eq!(cases.len() as u64, group["cases"].as_u64().unwrap());
        for case in cases {
            let hex = case["body_hex"].as_str().unwrap();
            let bytes = (0..hex.len())
                .step_by(2)
                .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
                .collect::<Vec<_>>();
            let actual = ssx_source_codecs::decode_implemented(&bytes, codec).unwrap();
            if let Some(error) = case.get("error") {
                assert_eq!(error["type"], "UnicodeDecodeError");
                assert_eq!(
                    actual.unwrap_err(),
                    error["message"].as_str().unwrap(),
                    "{codec}/{hex}"
                );
            } else {
                let points = case["points"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|p| p.as_u64().unwrap() as u32)
                    .collect::<Vec<_>>();
                assert_eq!(actual.unwrap().codepoints(), points, "{codec}/{hex}");
            }
        }
    }
}
