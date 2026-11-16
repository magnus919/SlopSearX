#[test]
fn typed_http_facade_retains_source_empty_unknown_rejection_and_surrogate_semantics() {
    use ssx_source_codecs::{DecodeError, decode_http};
    // These source behaviors are retained in the mandatory codec policy and
    // whole-adapter surrogate corpus; the facade must not change precedence.
    assert!(
        decode_http(b"", "undefined")
            .unwrap()
            .codepoints()
            .is_empty()
    );
    assert_eq!(
        decode_http(&[0xff], "not-a-registered-codec")
            .unwrap()
            .codepoints(),
        &[0xfffd]
    );
    assert_eq!(
        decode_http(b"x", "undefined"),
        Err(DecodeError::Codec("undefined encoding".into()))
    );
    let text = decode_http(br"\ud800\udc00", "raw_unicode_escape").unwrap();
    let address = text.codepoints().as_ptr();
    let points = text.into_codepoints();
    assert_eq!(points, [0xd800, 0xdc00]);
    assert_eq!(
        points.as_ptr(),
        address,
        "ownership move must not copy/normalize"
    );
}
