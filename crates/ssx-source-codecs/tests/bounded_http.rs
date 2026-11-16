//! Reuse original complete assertions/bytes through the charged dispatcher.
use ssx_owned_budget::Budget;
use ssx_source_codecs::{BoundedDecodeError, BoundedTextError, decode_http_bounded};

mod charged {
    pub struct Text(ssx_source_codecs::BudgetText);
    impl Text {
        pub fn codepoints(&self) -> &[u32] {
            self.0.codepoints()
        }
    }
    impl std::fmt::Debug for Text {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_tuple("ChargedText")
                .field(&self.codepoints())
                .finish()
        }
    }
    pub fn decode_implemented(body: &[u8], label: &str) -> Option<Result<Text, String>> {
        let budget = ssx_owned_budget::Budget::new(2 * 1024 * 1024).unwrap();
        let result = ssx_source_codecs::decode_http_bounded(body, label, &budget, &|| Ok(()));
        Some(match result {
            Ok(output) => {
                assert_eq!(budget.snapshot().unwrap().used, body.len() * 4);
                Ok(Text(output))
            }
            Err(ssx_source_codecs::BoundedDecodeError::Codec(message)) => {
                assert_eq!(budget.snapshot().unwrap().used, 0);
                Err(message.into())
            }
            Err(error) => panic!("unexpected bounded failure {label}/{body:?}: {error:?}"),
        })
    }
}
mod cjk {
    use crate::charged as ssx_source_codecs;
    include!("doublebyte.rs");
}
mod hz {
    use crate::charged as ssx_source_codecs;
    include!("hz.rs");
}
mod iso {
    use crate::charged as ssx_source_codecs;
    include!("iso2022.rs");
}
mod unicode {
    use crate::charged as ssx_source_codecs;
    include!("unicode.rs");
}
mod escape {
    use crate::charged as ssx_source_codecs;
    include!("unicode_escape.rs");
}
mod names {
    use crate::charged as ssx_source_codecs;
    include!("unicode_names.rs");
}
mod raw_escape {
    use crate::charged as ssx_source_codecs;
    include!("raw_unicode_escape.rs");
}
mod punycode {
    use crate::charged as ssx_source_codecs;
    include!("punycode.rs");
}
fn check_policy(body: &[u8], label: &str, text: Option<&str>, error: Option<&str>) {
    let budget = Budget::new(2 * 1024 * 1024).unwrap();
    match decode_http_bounded(body, label, &budget, &|| Ok(())) {
        Ok(output) => {
            assert_eq!(
                output.codepoints(),
                text.expect("source success")
                    .chars()
                    .map(u32::from)
                    .collect::<Vec<_>>(),
                "{label}/{body:?}"
            );
            assert_eq!(budget.snapshot().unwrap().used, body.len() * 4);
            drop(output);
        }
        Err(BoundedDecodeError::Codec(message)) => {
            assert_eq!(Some(message), error, "{label}/{body:?}")
        }
        Err(other) => panic!("unqualified family {label}/{body:?}: {other:?}"),
    }
    assert_eq!(budget.snapshot().unwrap().used, 0);
}
#[test]
fn complete117_codec_policy_aliases_and_applicability_use_charged_dispatch() {
    let source: serde_json::Value = serde_json::from_str(include_str!(
        "../../../docs/rust-rewrite/a05/source-codec-policy.json"
    ))
    .unwrap();
    let rows = source["canonical_codecs"].as_array().unwrap();
    assert_eq!(rows.len(), 117);
    let bytes = (0..=255).collect::<Vec<u8>>();
    let mut aliases = 0;
    let mut probes = 0;
    for row in rows {
        let name = row["canonical"].as_str().unwrap();
        check_policy(
            &bytes,
            name,
            row["decoded"].as_str(),
            row["error"]["message"].as_str(),
        );
        for label in row["labels"].as_array().unwrap() {
            check_policy(
                &bytes,
                label.as_str().unwrap(),
                row["decoded"].as_str(),
                row["error"]["message"].as_str(),
            );
            aliases += 1;
        }
        for probe in row["applicability_probes"].as_array().unwrap() {
            let hex = probe["body_hex"].as_str().unwrap();
            let input = (0..hex.len())
                .step_by(2)
                .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
                .collect::<Vec<_>>();
            check_policy(
                &input,
                name,
                probe["text"].as_str(),
                probe["error"]["message"].as_str(),
            );
            probes += 1;
        }
    }
    assert_eq!(aliases, 455);
    assert_eq!(probes, 342);
}
#[test]
fn source_error_empty_output_quota_and_deadline_stay_distinct() {
    let budget = Budget::new(1).unwrap();
    let output = decode_http_bounded(b"", "undefined", &budget, &|| Ok(())).unwrap();
    assert!(output.codepoints().is_empty());
    assert_eq!(budget.snapshot().unwrap().used, 0);
    drop(output);
    let budget = Budget::new(5).unwrap();
    assert_eq!(
        decode_http_bounded(&[0, 1], "utf16", &budget, &|| Ok(())).err(),
        Some(BoundedDecodeError::Codec(
            "'utf-16' codec can't decode bytes in position 0-1: Stream does not start with BOM"
        ))
    );
    assert_eq!(budget.snapshot().unwrap().used, 0);
    let budget = Budget::new(4).unwrap();
    let output = decode_http_bounded(b"a", "utf8", &budget, &|| Ok(())).unwrap();
    assert_eq!(output.codepoints(), [97]);
    assert_eq!(budget.snapshot().unwrap().used, 4);
    assert_eq!(budget.snapshot().unwrap().peak, 4);
    assert!(budget.reserve(1).is_err());
    drop(output);
    assert_eq!(budget.snapshot().unwrap().used, 0);
    let budget = Budget::new(3).unwrap();
    assert!(matches!(
        decode_http_bounded(b"a", "utf8", &budget, &|| Ok(())),
        Err(BoundedDecodeError::Output(BoundedTextError::Resource(_)))
    ));
    assert_eq!(budget.snapshot().unwrap().used, 0);
    let budget = Budget::new(4096).unwrap();
    assert_eq!(
        decode_http_bounded(b"x", "undefined", &budget, &|| Err(
            BoundedTextError::Deadline
        ))
        .err(),
        Some(BoundedDecodeError::Output(BoundedTextError::Deadline))
    );
    assert_eq!(budget.snapshot().unwrap().used, 0);
}
