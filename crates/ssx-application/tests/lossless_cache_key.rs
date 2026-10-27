use sha2::{Digest, Sha256};
use ssx_application::cache::{CacheKeyInput, source_base_key};
use std::collections::BTreeSet;

#[test]
fn source_base_key_preserves_literal_high_page_integers() {
    // The wire bytes and hashes are independent literal expectations, not output
    // from the production identity encoder. Adjacent large integers must remain
    // distinct before the service adds its separate routing identity.
    let cases = [
        (
            "9223372036854775807",
            br#"["page literal","en",0,[],[],9223372036854775807,null,null,null,null,null]"#.as_slice(),
            "005dc1f17713d722bef8e6cdca06010311552f2acb07c8fe8dfda2fa5173a766",
        ),
        (
            "9223372036854775808",
            br#"["page literal","en",0,[],[],9223372036854775808,null,null,null,null,null]"#.as_slice(),
            "c3268e2a43cd86bcea3dcfcc1fa1020128f3001f2b4cb68795f4be18f575ff36",
        ),
        (
            "170141183460469231731687303715884105728",
            br#"["page literal","en",0,[],[],170141183460469231731687303715884105728,null,null,null,null,null]"#.as_slice(),
            "3e7b0c39429dbcf14acc03b1ee8191154ea9a15a38250915347189caaa97b968",
        ),
        (
            "170141183460469231731687303715884105729",
            br#"["page literal","en",0,[],[],170141183460469231731687303715884105729,null,null,null,null,null]"#.as_slice(),
            "616a74137356341aba1572e876076d5780fc76d145940f73ac7b11f8825c4abb",
        ),
    ];
    let mut keys = BTreeSet::new();
    for (page, wire, expected_sha256) in cases {
        assert_eq!(format!("{:x}", Sha256::digest(wire)), expected_sha256);
        let input = CacheKeyInput {
            query: "page literal".into(),
            page: ssx_json::Number::new(page).unwrap(),
            ..Default::default()
        };
        let actual = source_base_key(&input);
        assert_eq!(
            actual.as_str(),
            format!("search:v2:{expected_sha256}"),
            "page {page} must hash the unquoted, exact integer literal"
        );
        assert!(
            keys.insert(actual.as_str().to_owned()),
            "page {page} aliased"
        );
    }
    assert_eq!(keys.len(), 4);
}
