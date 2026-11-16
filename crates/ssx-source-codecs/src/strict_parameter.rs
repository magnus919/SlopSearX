//! Nonincremental strict mode used only by the RFC2231 ASCII selector.
use crate::{StrictAsciiOutcome, Text};
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParameterError {
    LookupFailure,
    ValueError(&'static str),
    RuntimeError(String),
    OverflowError(&'static str),
    RegistryInvariant,
}
fn text(points: Vec<u32>) -> Text {
    Text::from_codepoints(points).expect("strict decoder checked Unicode range")
}
fn unicode(bytes: &[u8], name: &str) -> Option<Option<Text>> {
    if name == "utf-7" {
        return Some(crate::utf7::decode_strict(bytes).map(text));
    }
    let mut points = Vec::new();
    crate::bounded_parameter::unicode_into(bytes, name, &mut points).map(|result| {
        result
            .expect("legacy Vec output is infallible")
            .then(|| text(points))
    })
}
pub(crate) fn decode(bytes: &[u8], label: &str) -> Result<Option<Text>, ParameterError> {
    if label.contains('\0') {
        return Err(ParameterError::ValueError("embedded null character"));
    }
    // str(b"", encoding, "strict") checks an embedded NUL in the label,
    // then returns empty text without codec lookup (even unknown/nontext).
    if bytes.is_empty() {
        return Ok(Some(Text::from("")));
    }
    let name = crate::lookup_http_encoding(label)?.ok_or(ParameterError::LookupFailure)?;
    if crate::strict_validity::valid(name, 0).is_some() {
        if bytes
            .iter()
            .any(|&b| crate::strict_validity::valid(name, b) != Some(true))
        {
            return Ok(None);
        }
        return crate::decode_single_byte_points(bytes, name)
            .map(Some)
            .ok_or(ParameterError::RegistryInvariant);
    }
    if let Some(result) = unicode(bytes, name) {
        return Ok(result);
    }
    if let Some(result) = crate::doublebyte::decode_strict(bytes, name) {
        return Ok(result.ok());
    }
    if let Some(result) = crate::iso2022::decode_strict(bytes, name) {
        return match result {
            Ok(text) => Ok(Some(text)),
            Err(crate::iso2022::ModeError::Unicode) => Ok(None),
            Err(crate::iso2022::ModeError::Runtime(message)) => {
                Err(ParameterError::RuntimeError(message))
            }
        };
    }
    Ok(match name {
        "ascii" => bytes
            .is_ascii()
            .then(|| text(bytes.iter().copied().map(u32::from).collect())),
        "charmap" | "iso8859-1" => Some(text(bytes.iter().copied().map(u32::from).collect())),
        "punycode" => match crate::punycode::decode_strict(bytes) {
            Ok(text) => Some(text),
            Err(crate::punycode::StrictError::Unicode { .. }) => None,
            Err(crate::punycode::StrictError::Overflow) => {
                return Err(ParameterError::OverflowError(
                    "Python int too large to convert to C ssize_t",
                ));
            }
        },
        "unicode-escape" => crate::unicode_escape::decode_strict(bytes),
        "raw-unicode-escape" => crate::raw_unicode_escape::decode_strict(bytes),
        "undefined" => None,
        "hz" => crate::hz::decode_strict(bytes),
        "base64" | "bz2" | "hex" | "quopri" | "rot-13" | "uu" | "zlib" => {
            return Err(ParameterError::LookupFailure);
        }
        // IDNA is handled by the narrower all-label validator without materializing
        // a non-ASCII string. Any other registry omission fails closed.
        _ => return Err(ParameterError::RegistryInvariant),
    })
}
/// Strict nonincremental parameter mode. Never substitute body replacement mode.
pub fn ascii(bytes: &[u8], label: &str) -> Result<StrictAsciiOutcome, ParameterError> {
    if label.contains('\0') {
        return Err(ParameterError::ValueError("embedded null character"));
    }
    if crate::canonical(label) == Some("idna") {
        return crate::strict_idna_parameter(bytes);
    }
    match decode(bytes, label)? {
        None => Ok(StrictAsciiOutcome::UnicodeFailure),
        Some(text) if text.codepoints().iter().all(|&c| c < 128) => Ok(StrictAsciiOutcome::Ascii(
            text.codepoints()
                .iter()
                .map(|&c| char::from(c as u8))
                .collect(),
        )),
        Some(_) => Ok(StrictAsciiOutcome::NonAscii),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn unhex(s: &str) -> Vec<u8> {
        s.as_bytes()
            .as_chunks::<2>()
            .0
            .iter()
            .map(|b| u8::from_str_radix(std::str::from_utf8(b).unwrap(), 16).unwrap())
            .collect()
    }
    #[test]
    fn all_non_idna_strict_modes_match_complete_source_text_and_error_outcomes() {
        let source: serde_json::Value =
            serde_json::from_str(include_str!("../tests/strict-parameter-source.json")).unwrap();
        let mut codecs = std::collections::BTreeSet::new();
        let mut count = 0;
        for row in source["cases"].as_array().unwrap() {
            let name = row["label"].as_str().unwrap();
            if name == "idna" {
                continue;
            }
            codecs.insert(name);
            count += 1;
            let input = unhex(row["body_hex"].as_str().unwrap());
            let expected = if let Some(points) = row["decoded_codepoints"].as_array() {
                Ok(Some(text(
                    points
                        .iter()
                        .map(|p| u32::try_from(p.as_u64().unwrap()).unwrap())
                        .collect(),
                )))
            } else {
                let errors = row["exception_metadata"].as_array().unwrap();
                assert_eq!(errors.len(), 1);
                match errors[0]["class"].as_str().unwrap() {
                    "UnicodeError" | "UnicodeDecodeError" => Ok(None),
                    "LookupError" => Err(ParameterError::LookupFailure),
                    "RuntimeError" => Err(ParameterError::RuntimeError(
                        errors[0]["message"].as_str().unwrap().into(),
                    )),
                    other => panic!("unexpected source error {other}"),
                }
            };
            assert_eq!(decode(&input, name), expected, "{}", row["id"]);
        }
        assert_eq!(codecs.len(), 116);
        assert_eq!(count, 2552);
    }
    #[test]
    fn all_singlebyte_strict_validity_bits_match_complete_actual_source() {
        let source: serde_json::Value =
            serde_json::from_str(include_str!("../tests/singlebyte-strict-validity.json")).unwrap();
        let rows = source["cases"].as_array().unwrap();
        assert_eq!(rows.len(), 69);
        for row in rows {
            let name = row["canonical"].as_str().unwrap();
            let bits = unhex(row["validity_bits_hex"].as_str().unwrap());
            assert_eq!(bits.len(), 32);
            for b in 0..=u8::MAX {
                assert_eq!(
                    crate::strict_validity::valid(name, b),
                    Some(bits[usize::from(b) / 8] & (1 << (b % 8)) != 0),
                    "{name}/{b}"
                );
            }
        }
    }
    fn inflate(bytes: &[u8]) -> Vec<u8> {
        use std::io::Read;
        let mut out = Vec::new();
        flate2::read::ZlibDecoder::new(bytes)
            .read_to_end(&mut out)
            .unwrap();
        out
    }
    fn check_mode(
        body: &[u8],
        label: &str,
        actual: Result<Option<Text>, ParameterError>,
        mode: u8,
        points: Option<Vec<u32>>,
        context: &str,
    ) {
        use crate::{BoundedParameterError, BoundedStrictAsciiOutcome};
        let budget = ssx_owned_budget::Budget::new(1024 * 1024).unwrap();
        let charged = crate::strict_parameter_ascii_bounded(body, label, &budget, &|| Ok(()));
        match mode {
            0 => assert!(
                matches!(charged, Ok(BoundedStrictAsciiOutcome::UnicodeFailure)),
                "charged {context}"
            ),
            1 => {
                let expected = points.as_ref().expect("valid source points");
                if expected.iter().all(|&p| p < 128) {
                    let Ok(BoundedStrictAsciiOutcome::Ascii(output)) = charged else {
                        panic!("charged ASCII {context}");
                    };
                    let expected: Vec<u8> = expected.iter().map(|&p| p as u8).collect();
                    assert_eq!(output.as_str().as_bytes(), expected, "charged {context}");
                } else {
                    assert!(
                        matches!(charged, Ok(BoundedStrictAsciiOutcome::NonAscii)),
                        "charged NonAscii {context}"
                    );
                }
            }
            2 => assert!(
                matches!(
                    charged,
                    Err(BoundedParameterError::RuntimeError("internal codec error"))
                ),
                "charged {context}"
            ),
            _ => panic!("invalid source mode"),
        }
        assert_eq!(budget.snapshot().unwrap().used, 0, "charged {context}");
        match mode {
            0 => assert_eq!(actual, Ok(None), "{context}"),
            1 => assert_eq!(
                actual,
                Ok(Some(text(points.expect("valid source points")))),
                "{context}"
            ),
            2 => assert_eq!(
                actual,
                Err(ParameterError::RuntimeError("internal codec error".into())),
                "{context}"
            ),
            _ => panic!("invalid source strict mode"),
        }
    }
    #[test]
    fn all_iso2022_strict_states_and_targeted_sequences_preserve_complete_outcome_and_precedence() {
        use std::io::Read;
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests");
        let source: serde_json::Value =
            serde_json::from_str(include_str!("../tests/stateful-strict/manifest.json")).unwrap();
        let states = source["states"].as_array().unwrap();
        assert_eq!(states.len(), 41);
        let mut executed = 0;
        for state in states.iter().filter(|s| s["codec"] != "hz") {
            let codec = state["codec"].as_str().unwrap();
            let prefix = unhex(state["prefix_hex"].as_str().unwrap());
            let modes = inflate(
                &std::fs::read(
                    root.join("stateful-strict")
                        .join(state["file"].as_str().unwrap()),
                )
                .unwrap(),
            );
            assert_eq!(modes.len(), 65536);
            let original = std::fs::read(
                root.join("iso2022")
                    .join(state["source_file"].as_str().unwrap()),
            )
            .unwrap();
            let mut original = flate2::read::ZlibDecoder::new(original.as_slice());
            for pair in 0..=u16::MAX {
                let mut row = [0u8; 21];
                original.read_exact(&mut row).unwrap();
                let mode = modes[usize::from(pair)];
                let points = if mode == 1 {
                    assert!(row[0] <= 5);
                    Some(
                        (0..usize::from(row[0]))
                            .map(|i| {
                                u32::from_be_bytes(row[1 + i * 4..5 + i * 4].try_into().unwrap())
                            })
                            .collect(),
                    )
                } else {
                    None
                };
                let mut body = prefix.clone();
                body.extend(pair.to_be_bytes());
                body.extend(b"\x1b(B");
                check_mode(
                    &body,
                    codec,
                    decode(&body, codec),
                    mode,
                    points,
                    &format!("{codec}/{prefix:?}/{pair:04x}"),
                );
                executed += 1;
            }
            assert_eq!(original.read(&mut [0]).unwrap(), 0);
        }
        assert_eq!(executed, 39 * 65536);
        let targeted: Vec<serde_json::Value> =
            serde_json::from_str(include_str!("../tests/iso2022/targeted.json")).unwrap();
        let modes = inflate(include_bytes!(
            "../tests/stateful-strict/iso-targeted.modes.zlib"
        ));
        assert_eq!(targeted.len(), 44627);
        assert_eq!(modes.len(), targeted.len());
        for (row, &mode) in targeted.iter().zip(&modes) {
            let points = if mode == 1 {
                Some(
                    row["points"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|p| u32::try_from(p.as_u64().unwrap()).unwrap())
                        .collect(),
                )
            } else {
                None
            };
            let body = unhex(row["body_hex"].as_str().unwrap());
            check_mode(
                &body,
                row["codec"].as_str().unwrap(),
                decode(&body, row["codec"].as_str().unwrap()),
                mode,
                points,
                &format!("{}", row["body_hex"]),
            );
        }
    }
    #[test]
    fn both_hz_strict_states_match_all_source_pairs_and_complete_valid_text() {
        let rows: Vec<Vec<Vec<u32>>> =
            serde_json::from_slice(&inflate(include_bytes!("../tests/hz-source.zlib"))).unwrap();
        assert_eq!(rows.len(), 2);
        let masks = [
            include_bytes!("../tests/stateful-strict/hz-0.modes.zlib").as_slice(),
            include_bytes!("../tests/stateful-strict/hz-1.modes.zlib").as_slice(),
        ];
        let mut count = 0;
        for (i, compressed) in masks.iter().enumerate() {
            let modes = inflate(compressed);
            assert_eq!(modes.len(), 65536);
            assert_eq!(rows[i].len(), 65536);
            for pair in 0..=u16::MAX {
                let mode = modes[usize::from(pair)];
                let mut body = if i == 0 { vec![] } else { b"~{".to_vec() };
                body.extend(pair.to_be_bytes());
                body.extend(b"~}");
                check_mode(
                    &body,
                    "hz",
                    decode(&body, "hz"),
                    mode,
                    (mode == 1).then(|| rows[i][usize::from(pair)].clone()),
                    &format!("hz/{i}/{pair:04x}"),
                );
                count += 1;
            }
        }
        assert_eq!(count, 131072);
    }
    #[test]
    fn all_registered_parameter_codecs_match_complete_source_selector_outcomes() {
        let source: serde_json::Value =
            serde_json::from_str(include_str!("../tests/strict-parameter-source.json")).unwrap();
        let rows = source["cases"].as_array().unwrap();
        assert_eq!(rows.len(), 2574);
        let mut ids = std::collections::BTreeSet::new();
        let mut codecs = std::collections::BTreeSet::new();
        for row in rows {
            let label = row["label"].as_str().unwrap();
            codecs.insert(label);
            assert!(ids.insert(row["id"].as_str().unwrap()));
            let expected = if let Some(points) = row["decoded_codepoints"].as_array() {
                if points.iter().all(|p| p.as_u64().unwrap() < 128) {
                    Ok(StrictAsciiOutcome::Ascii(
                        points
                            .iter()
                            .map(|p| char::from(p.as_u64().unwrap() as u8))
                            .collect(),
                    ))
                } else {
                    Ok(StrictAsciiOutcome::NonAscii)
                }
            } else {
                let e = &row["exception_metadata"][0];
                match e["class"].as_str().unwrap() {
                    "UnicodeError" | "UnicodeDecodeError" => Ok(StrictAsciiOutcome::UnicodeFailure),
                    "LookupError" => Err(ParameterError::LookupFailure),
                    "RuntimeError" => Err(ParameterError::RuntimeError(
                        e["message"].as_str().unwrap().into(),
                    )),
                    other => panic!("unexpected source exception {other}"),
                }
            };
            assert_eq!(
                ascii(&unhex(row["body_hex"].as_str().unwrap()), label),
                expected,
                "{}",
                row["id"]
            );
        }
        assert_eq!(codecs.len(), 117);
        assert_eq!(ids.len(), 2574);
    }
    #[test]
    fn strict_punycode_and_idna_preserve_overflow_offsets_caps_and_first_error_precedence() {
        let source: serde_json::Value =
            serde_json::from_str(include_str!("../tests/strict-overflow-source.json")).unwrap();
        let rows = source["cases"].as_array().unwrap();
        assert_eq!(rows.len(), 2255);
        let mut ids = std::collections::BTreeSet::new();
        let mut overflow = 0;
        for row in rows {
            assert!(ids.insert(row["id"].as_str().unwrap()));
            let body = unhex(row["body_hex"].as_str().unwrap());
            let label = row["label"].as_str().unwrap();
            let expected = if let Some(points) = row["decoded_codepoints"].as_array() {
                if label == "punycode" {
                    assert_eq!(
                        decode(&body, label).unwrap().unwrap().codepoints(),
                        points
                            .iter()
                            .map(|p| u32::try_from(p.as_u64().unwrap()).unwrap())
                            .collect::<Vec<_>>(),
                        "complete punycode {}",
                        row["id"]
                    );
                }
                if points.iter().all(|p| p.as_u64().unwrap() < 128) {
                    Ok(StrictAsciiOutcome::Ascii(
                        points
                            .iter()
                            .map(|p| char::from(p.as_u64().unwrap() as u8))
                            .collect(),
                    ))
                } else {
                    Ok(StrictAsciiOutcome::NonAscii)
                }
            } else {
                let e = &row["exception_metadata"][0];
                match e["class"].as_str().unwrap() {
                    "UnicodeDecodeError" | "UnicodeError" => Ok(StrictAsciiOutcome::UnicodeFailure),
                    "OverflowError" => {
                        overflow += 1;
                        assert_eq!(e["message"], "Python int too large to convert to C ssize_t");
                        Err(ParameterError::OverflowError(
                            "Python int too large to convert to C ssize_t",
                        ))
                    }
                    other => panic!("unexpected source error {other}"),
                }
            };
            assert_eq!(ascii(&body, label), expected, "{}", row["id"]);
        }
        assert_eq!(ids.len(), 2255);
        assert_eq!(overflow, 90);
    }
    #[test]
    fn strict_utf7_escape_and_name_vectors_match_complete_source_mode_and_valid_text() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests");
        let manifest: serde_json::Value =
            serde_json::from_str(include_str!("../tests/algorithm-strict/manifest.json")).unwrap();
        let groups = manifest["groups"].as_array().unwrap();
        assert_eq!(groups.len(), 4);
        let mut executed = 0;
        for group in groups {
            let name = group["source"].as_str().unwrap();
            let codec = group["codec"].as_str().unwrap();
            let raw = std::fs::read(root.join(name)).unwrap();
            let source: Vec<serde_json::Value> =
                serde_json::from_slice(&if name.ends_with(".zlib") {
                    inflate(&raw)
                } else {
                    raw
                })
                .unwrap();
            let modes = inflate(
                &std::fs::read(
                    root.join("algorithm-strict")
                        .join(group["file"].as_str().unwrap()),
                )
                .unwrap(),
            );
            assert_eq!(source.len(), group["cases"].as_u64().unwrap() as usize);
            assert_eq!(modes.len(), source.len());
            for (row, &mode) in source.iter().zip(&modes) {
                let (body, points) = if name == "unicode-name-source.json" {
                    (
                        format!("\\N{{{}}}", row[0].as_str().unwrap()).into_bytes(),
                        &row[1],
                    )
                } else {
                    (
                        unhex(
                            row["body_hex"]
                                .as_str()
                                .or_else(|| row["input_hex"].as_str())
                                .unwrap(),
                        ),
                        row.get("points").unwrap_or(&row["expected_codepoints"]),
                    )
                };
                let expected = if mode == 1 {
                    Some(
                        points
                            .as_array()
                            .unwrap()
                            .iter()
                            .map(|p| u32::try_from(p.as_u64().unwrap()).unwrap())
                            .collect(),
                    )
                } else {
                    None
                };
                check_mode(
                    &body,
                    codec,
                    decode(&body, codec),
                    mode,
                    expected,
                    &format!("{name}/{body:?}"),
                );
                executed += 1;
            }
        }
        assert_eq!(executed, 237121);
    }
}
