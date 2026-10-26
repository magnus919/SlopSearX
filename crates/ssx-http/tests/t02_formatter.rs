use ssx_http::formatter_metadata::{iso_to_epoch, parsed_url};
use ssx_json::Value;
#[test]
fn original_pure_formatter_controls_are_not_http_acceptance() {
    let controls: Value =
        ssx_json::from_str(include_str!("formatter_source_controls.json")).unwrap();
    let mut completed = 0;
    for case in controls["records"].as_array().unwrap() {
        let result = if case["id"].as_str().unwrap().starts_with("epoch:") {
            ssx_json::to_value(iso_to_epoch(case["input"].as_str())).unwrap()
        } else {
            ssx_json::to_value(parsed_url(case["input"].as_str().unwrap_or(""))).unwrap()
        };
        assert_eq!(result, case["output"], "{}", case["id"]);
        println!(
            "T02_BINDING {}",
            ssx_json::json!({"kind":"formatter","id":case["id"],"input":case["input"],"output":result})
        );
        completed += 1;
    }
    assert_eq!(completed, 51);
}

#[test]
fn reviewed_malformed_offsets_match_original_http_null_pubdates() {
    use sha2::{Digest, Sha256};
    let source =
        include_str!("../../../docs/rust-rewrite/t02/oracle/source_http_review_records.jsonl");
    let mut completed = 0;
    for line in source.lines() {
        let case: Value = ssx_json::from_str(line).unwrap();
        let Some(control) = case["request"].get("formatter") else {
            continue;
        };
        let expected: Value = ssx_json::from_str(case["body"].as_str().unwrap()).unwrap();
        assert!(expected["results"][0]["pubdate"].is_null());
        let result = ssx_json::to_value(iso_to_epoch(control["input"].as_str())).unwrap();
        assert_eq!(result, expected["results"][0]["pubdate"], "{}", case["id"]);
        let sha = Sha256::digest(line.as_bytes())
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        println!(
            "T02_BINDING {}",
            ssx_json::json!({"kind":"formatter-review","id":control["id"],"source_record_sha256":sha,"input":control["input"],"output":result})
        );
        completed += 1;
    }
    assert_eq!(completed, 2);
}

#[test]
fn python_repr_nonprintable_literals_use_existing_unicode16_contract() {
    // Literal source controls, not new backend captures or derived expectations.
    let cases = [
        ("\u{00ad}", "'\\xad'"),
        ("\u{200b}", "'\\u200b'"),
        ("\u{2028}\u{2029}", "'\\u2028\\u2029'"),
        ("\u{1ffff}", "'\\U0001ffff'"),
        ("é😀", "'é😀'"),
        ("a'b\u{2028}", "\"a'b\\u2028\""),
        ("a'\"b", "'a\\'\"b'"),
        ("\\\n\r\t\0", "'\\\\\\n\\r\\t\\x00'"),
    ];
    for (input, expected) in cases {
        assert_eq!(ssx_http::formats::repr(input), expected, "{input:?}");
    }
}
