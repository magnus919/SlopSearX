#[test]
fn every_byte_pair_in_both_hz_states_matches_complete_source_output() {
    use std::io::Read;
    let mut decoder = flate2::read::ZlibDecoder::new(include_bytes!("hz-source.zlib").as_slice());
    let mut json = String::new();
    decoder.read_to_string(&mut json).unwrap();
    let states: Vec<Vec<Vec<u32>>> = serde_json::from_str(&json).unwrap();
    assert_eq!(states.len(), 2);
    for (state, rows) in states.iter().enumerate() {
        assert_eq!(rows.len(), 65536);
        for pair in 0..=u16::MAX {
            let mut body = if state == 0 { vec![] } else { b"~{".to_vec() };
            body.extend(pair.to_be_bytes());
            body.extend(b"~}");
            assert_eq!(
                ssx_source_codecs::decode_implemented(&body, "hz")
                    .unwrap()
                    .unwrap()
                    .codepoints(),
                rows[usize::from(pair)],
                "state{state}/{pair:04x}"
            );
        }
    }
}
