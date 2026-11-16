#[test]
fn source_ascii_substring_lowering_preimages_and_complete_controls() {
    let evidence: serde_json::Value =
        serde_json::from_str(include_str!("lower-ascii-source.json")).unwrap();
    assert_eq!(
        evidence["exhaustive_nonascii_preimages"],
        serde_json::json!([
            {"input":304,"lower_codepoints":[105,775]},
            {"input":8490,"lower_codepoints":[107]}
        ])
    );
    assert_eq!(evidence["empty_lower_preimages"], serde_json::json!([]));
    let rows = evidence["cases"].as_array().unwrap();
    assert_eq!(rows.len(), 2111);
    for row in rows {
        let points = row["points"]
            .as_array()
            .unwrap()
            .iter()
            .map(|point| u32::try_from(point.as_u64().unwrap()).unwrap())
            .collect();
        let text = ssx_source_codecs::Text::from_codepoints(points).unwrap();
        assert_eq!(
            text.contains_ascii_lower(row["needle"].as_str().unwrap()),
            row["expected"].as_bool().unwrap(),
            "{row}"
        );
        let needle = row["needle"].as_str().unwrap();
        let mut scratch = vec![0; needle.len()];
        assert_eq!(
            ssx_source_codecs::contains_ascii_lower_with_scratch(
                text.codepoints(),
                needle,
                &mut scratch,
                &|| Ok(())
            )
            .unwrap(),
            row["expected"].as_bool().unwrap(),
            "borrowed {row}"
        );
    }
}

#[test]
fn borrowed_search_rejects_short_scratch_non_ascii_and_deadline_without_ownership_escape() {
    use ssx_source_codecs::{
        BoundedTextError as Error, contains_ascii_lower_with_scratch as search,
    };
    let points = [0x130, 0x307, 0x212a, b'a' as u32];
    assert_eq!(
        search(&points, "ka", &mut [0], &|| Ok(())),
        Err(Error::OutputBound)
    );
    assert_eq!(
        search(&points, "é", &mut [0; 2], &|| Ok(())),
        Err(Error::NonAscii)
    );
    let calls = std::cell::Cell::new(0);
    let check = || {
        calls.set(calls.get() + 1);
        if calls.get() == 4 {
            Err(Error::Deadline)
        } else {
            Ok(())
        }
    };
    assert_eq!(
        search(&points, "ka", &mut [0; 2], &check),
        Err(Error::Deadline)
    );
}
