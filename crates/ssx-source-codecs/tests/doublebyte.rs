#[test]
fn all_two_byte_inputs_and_full_streams_match_actual_source() {
    let codecs: &[(&str, &[u8], &str)] = &[
        (
            "gb18030",
            include_bytes!("cjk/gb18030.source-pairs.bin").as_slice(),
            include_str!("cjk/gb18030.source-streams.json"),
        ),
        (
            "euc_kr",
            include_bytes!("cjk/euc_kr.source-pairs.bin").as_slice(),
            include_str!("cjk/euc_kr.source-streams.json"),
        ),
        (
            "cp932",
            include_bytes!("cjk/cp932.source-pairs.bin").as_slice(),
            include_str!("cjk/cp932.source-streams.json"),
        ),
        (
            "gbk",
            include_bytes!("cjk/gbk.source-pairs.bin").as_slice(),
            include_str!("cjk/gbk.source-streams.json"),
        ),
        (
            "cp949",
            include_bytes!("cjk/cp949.source-pairs.bin").as_slice(),
            include_str!("cjk/cp949.source-streams.json"),
        ),
        (
            "cp950",
            include_bytes!("cjk/cp950.source-pairs.bin").as_slice(),
            include_str!("cjk/cp950.source-streams.json"),
        ),
        (
            "big5",
            include_bytes!("cjk/big5.source-pairs.bin").as_slice(),
            include_str!("cjk/big5.source-streams.json"),
        ),
        (
            "big5hkscs",
            include_bytes!("cjk/big5hkscs.source-pairs.bin").as_slice(),
            include_str!("cjk/big5hkscs.source-streams.json"),
        ),
        (
            "gb2312",
            include_bytes!("cjk/gb2312.source-pairs.bin").as_slice(),
            include_str!("cjk/gb2312.source-streams.json"),
        ),
        (
            "johab",
            include_bytes!("cjk/johab.source-pairs.bin").as_slice(),
            include_str!("cjk/johab.source-streams.json"),
        ),
        (
            "shift_jis",
            include_bytes!("cjk/shift_jis.source-pairs.bin").as_slice(),
            include_str!("cjk/shift_jis.source-streams.json"),
        ),
        (
            "shift_jis_2004",
            include_bytes!("cjk/shift_jis_2004.source-pairs.bin").as_slice(),
            include_str!("cjk/shift_jis_2004.source-streams.json"),
        ),
        (
            "shift_jisx0213",
            include_bytes!("cjk/shift_jisx0213.source-pairs.bin").as_slice(),
            include_str!("cjk/shift_jisx0213.source-streams.json"),
        ),
        (
            "euc_jp",
            include_bytes!("cjk/euc_jp.source-pairs.bin").as_slice(),
            include_str!("cjk/euc_jp.source-streams.json"),
        ),
        (
            "euc_jis_2004",
            include_bytes!("cjk/euc_jis_2004.source-pairs.bin").as_slice(),
            include_str!("cjk/euc_jis_2004.source-streams.json"),
        ),
        (
            "euc_jisx0213",
            include_bytes!("cjk/euc_jisx0213.source-pairs.bin").as_slice(),
            include_str!("cjk/euc_jisx0213.source-streams.json"),
        ),
    ];
    assert_eq!(codecs.len(), 16);
    for &(codec, expected, streams) in codecs {
        assert_eq!(expected.len(), 65536 * 9);
        for pair in 0..=u16::MAX {
            let expected = &expected[usize::from(pair) * 9..][..9];
            let count = usize::from(expected[0]);
            assert!((1..=2).contains(&count));
            let points = (0..count)
                .map(|i| u32::from_be_bytes(expected[1 + i * 4..5 + i * 4].try_into().unwrap()))
                .collect::<Vec<_>>();
            assert_eq!(
                ssx_source_codecs::decode_implemented(&pair.to_be_bytes(), codec)
                    .unwrap()
                    .unwrap()
                    .codepoints(),
                points,
                "{codec}/{pair:04x}"
            );
        }
        let streams: Vec<(String, Vec<u32>)> = serde_json::from_str(streams).unwrap();
        assert_eq!(streams.len(), 256);
        for (hex, points) in streams {
            let bytes = (0..hex.len())
                .step_by(2)
                .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
                .collect::<Vec<_>>();
            assert_eq!(
                ssx_source_codecs::decode_implemented(&bytes, codec)
                    .unwrap()
                    .unwrap()
                    .codepoints(),
                points,
                "{codec}/{hex}"
            );
        }
    }
}

#[test]
fn all_euc_jp_three_byte_inputs_match_actual_source() {
    let codecs: &[(&str, &[u8])] = &[
        ("euc_jp", include_bytes!("cjk/euc_jp.source-triples.bin")),
        (
            "euc_jis_2004",
            include_bytes!("cjk/euc_jis_2004.source-triples.bin"),
        ),
        (
            "euc_jisx0213",
            include_bytes!("cjk/euc_jisx0213.source-triples.bin"),
        ),
    ];
    for &(codec, expected) in codecs {
        assert_eq!(expected.len(), 65536 * 13);
        for suffix in 0..=u16::MAX {
            let expected = &expected[usize::from(suffix) * 13..][..13];
            let count = usize::from(expected[0]);
            assert!((1..=3).contains(&count));
            let points = (0..count)
                .map(|i| u32::from_be_bytes(expected[1 + i * 4..5 + i * 4].try_into().unwrap()))
                .collect::<Vec<_>>();
            let [a, b] = suffix.to_be_bytes();
            assert_eq!(
                ssx_source_codecs::decode_implemented(&[0x8f, a, b], codec)
                    .unwrap()
                    .unwrap()
                    .codepoints(),
                points,
                "{codec}/8f{suffix:04x}"
            );
        }
    }
}

#[test]
fn all_euc_kr_makeup_syllables_and_malformed_controls_match_source() {
    let rows: Vec<(String, Vec<u32>)> =
        serde_json::from_str(include_str!("euc-kr-source.json")).unwrap();
    assert_eq!(rows.len(), 13234);
    for (hex, expected) in rows {
        let bytes = (0..hex.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
            .collect::<Vec<_>>();
        assert_eq!(
            ssx_source_codecs::decode_implemented(&bytes, "euc_kr")
                .unwrap()
                .unwrap()
                .codepoints(),
            expected,
            "{hex}"
        );
    }
}

#[test]
fn every_formed_gb18030_four_byte_input_matches_complete_source_output() {
    use std::io::Read;
    let mut decoder =
        flate2::read::ZlibDecoder::new(include_bytes!("gb18030-4byte-source.zlib").as_slice());
    let mut expected = Vec::new();
    decoder.read_to_end(&mut expected).unwrap();
    assert_eq!(expected.len(), 1587600 * 17);
    let mut row = 0;
    for a in 0x81..=0xfe {
        for b in 0x30..=0x39 {
            for d in 0x81..=0xfe {
                for e in 0x30..=0x39 {
                    let expected = &expected[row * 17..][..17];
                    row += 1;
                    let count = usize::from(expected[0]);
                    assert!((1..=4).contains(&count));
                    let points = (0..count)
                        .map(|i| {
                            u32::from_be_bytes(expected[1 + i * 4..5 + i * 4].try_into().unwrap())
                        })
                        .collect::<Vec<_>>();
                    assert_eq!(
                        ssx_source_codecs::decode_implemented(&[a, b, d, e], "gb18030")
                            .unwrap()
                            .unwrap()
                            .codepoints(),
                        points,
                        "{a:02x}{b:02x}{d:02x}{e:02x}"
                    );
                }
            }
        }
    }
    assert_eq!(row, 1587600);
}
