//! Complete retained source codec policy; unfinished families fail closed.
#[test]
fn complete_source_codec_policy_and_applicability() {
    let policy: serde_json::Value = serde_json::from_str(include_str!(
        "../../../docs/rust-rewrite/a05/source-codec-policy.json"
    ))
    .unwrap();
    let rows = policy["canonical_codecs"].as_array().unwrap();
    assert_eq!(rows.len(), 117);
    let mut failures = vec![];
    let bytes = (0..=255).collect::<Vec<u8>>();
    for row in rows {
        let name = row["canonical"].as_str().unwrap();
        let mut probes = vec![(
            "bytes0..255".to_owned(),
            bytes.clone(),
            row.get("decoded").cloned(),
            row.get("error").cloned(),
        )];
        for probe in row["applicability_probes"].as_array().unwrap() {
            let hex = probe["body_hex"].as_str().unwrap();
            let bytes = (0..hex.len())
                .step_by(2)
                .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
                .collect();
            probes.push((
                probe["input"].as_str().unwrap().to_owned(),
                bytes,
                probe.get("text").cloned(),
                probe.get("error").cloned(),
            ));
        }
        for alias in row["labels"].as_array().unwrap() {
            let alias = alias.as_str().unwrap();
            assert_eq!(
                ssx_source_codecs::canonical(alias),
                Some(name),
                "alias {alias}"
            );
            let result = source_decode(&bytes, alias);
            if let Some(expected) = row["decoded"].as_str() {
                assert_eq!(
                    result.unwrap().codepoints(),
                    expected.chars().map(u32::from).collect::<Vec<_>>(),
                    "alias {alias}"
                );
            } else {
                assert_eq!(
                    result.unwrap_err(),
                    row["error"]["message"].as_str().unwrap(),
                    "alias {alias}"
                );
            }
            println!("A05_CODEC_ALIAS {alias}");
        }
        for (id, bytes, text, error) in probes {
            let result = source_decode(&bytes, name);
            let matches = match result {
                Ok(text_value) => text
                    .as_ref()
                    .and_then(|v| v.as_str())
                    .is_some_and(|expected| {
                        text_value.codepoints()
                            == expected.chars().map(u32::from).collect::<Vec<_>>()
                    }),
                Err(message) => {
                    error.as_ref().and_then(|e| e["message"].as_str()) == Some(message.as_str())
                }
            };
            if matches {
                println!("A05_CODEC {name}/{id}");
            } else {
                failures.push(format!("{name}/{id}"));
            }
        }
    }
    assert!(
        failures.is_empty(),
        "native codec source mismatches or unfinished families: {failures:?}"
    );
}

fn source_decode(body: &[u8], label: &str) -> Result<ssx_source_codecs::Text, String> {
    match ssx_source_codecs::decode_http(body, label) {
        Ok(text) => Ok(text),
        Err(ssx_source_codecs::DecodeError::Codec(message)) => Err(message),
        Err(ssx_source_codecs::DecodeError::RegistryInvariant) => {
            panic!("native decoder omitted source codec {label}")
        }
    }
}
