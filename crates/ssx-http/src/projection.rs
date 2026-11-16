//! Pure HTTP boundary mapping from internal domain records to public DTOs.
use ssx_domain::{SearchResponse, SearchResult};
use ssx_json::Value;
use ssx_wire::{
    HttpResultDto, HttpResultMetadata, HttpSearchDto, MediaDto, YamlResultDto, inline_payload,
    nonempty, paper_fields,
};

pub fn result_json(result: &SearchResult, metadata: &HttpResultMetadata) -> HttpResultDto {
    let paper = paper_fields(result.work_group.as_ref());
    HttpResultDto {
        url: result.url.clone(),
        title: result.title.clone(),
        content: result.content.clone(),
        engine: result.engine.clone(),
        engines: sorted_engines(result),
        score: result.score.as_number().clone(),
        positions: if result.position == 0 {
            vec![]
        } else {
            vec![result.position]
        },
        category: result.category.clone(),
        published_date: result.published_date.clone(),
        pubdate: metadata.pubdate,
        tier: result.tier,
        length: None,
        thumbnail: result.thumbnail.clone(),
        img_src: result.img_src.clone(),
        media: result.media.as_ref().map(|media| MediaDto {
            media_type: media.media_type.clone(),
            url: nonempty(media.url.as_ref()),
            thumbnail: nonempty(media.thumbnail.as_ref()),
            source: nonempty(media.source.as_ref()),
            width: media.width,
            height: media.height,
            duration: media
                .duration
                .as_ref()
                .map(|number| number.as_number().clone()),
        }),
        payload: inline_payload(result.payload.as_ref()),
        iframe_src: None,
        audio_src: None,
        views: None,
        author: None,
        metadata: None,
        template: if paper.is_empty() {
            "default.html"
        } else {
            "paper.html"
        }
        .into(),
        parsed_url: metadata.parsed_url.clone(),
        open_group: false,
        close_group: false,
        priority: String::new(),
        paper,
    }
}

pub fn result_yaml(result: &SearchResult) -> YamlResultDto {
    YamlResultDto {
        url: result.url.clone(),
        title: result.title.clone(),
        engine: result.engine.clone(),
        engines: sorted_engines(result),
        content: nonempty(Some(&result.content)),
        score: result.score.as_number().clone(),
        position: result.position,
        published: result.published_date.clone(),
        tier: result.tier,
        media: result.media.as_ref().map(|media| MediaDto {
            media_type: media.media_type.clone(),
            url: nonempty(media.url.as_ref()),
            thumbnail: nonempty(media.thumbnail.as_ref()),
            source: nonempty(media.source.as_ref()),
            width: media.width,
            height: media.height,
            duration: media
                .duration
                .as_ref()
                .map(|number| number.as_number().clone()),
        }),
        payload: inline_payload(result.payload.as_ref()),
        paper: paper_fields(result.work_group.as_ref()),
    }
}

/// Public HTTP JSON envelope. The caller supplies the already-decided metadata
/// and engine health view; this projection never makes routing or policy choices.
pub fn search_json(
    response: &SearchResponse,
    meta: Option<Value>,
    result_metadata: &[HttpResultMetadata],
) -> Result<HttpSearchDto, &'static str> {
    if response.results.len() != result_metadata.len() {
        return Err("HTTP result metadata count must match results");
    }
    let results = response
        .results
        .iter()
        .zip(result_metadata)
        .map(|(result, metadata)| result_json(result, metadata))
        .collect::<Vec<_>>();
    Ok(HttpSearchDto {
        query: response.query.clone(),
        number_of_results: results.len(),
        results,
        answers: response.answers.clone(),
        corrections: response.corrections.clone(),
        infoboxes: response.infoboxes.clone(),
        suggestions: response.suggestions.clone(),
        unresponsive_engines: response
            .engine_outcomes
            .iter()
            .filter(|outcome| outcome.status != "ok")
            .map(|outcome| {
                vec![
                    outcome.engine.clone(),
                    outcome
                        .message
                        .clone()
                        .unwrap_or_else(|| outcome.status.clone()),
                ]
            })
            .collect(),
        meta,
    })
}

fn sorted_engines(result: &SearchResult) -> Vec<String> {
    if result.engines.is_empty() {
        vec![result.engine.clone()]
    } else {
        result.engines.iter().cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use ssx_domain::SearchResult;
    use ssx_json::json as ssxjson;

    #[test]
    fn public_json_has_sorted_engines_null_fields_and_no_internal_work_group() {
        let result = SearchResult {
            engine: "primary".into(),
            engines: ["z", "a"].into_iter().map(str::to_owned).collect(),
            work_group: Some(ssxjson!({"private":true,"paper":{"doi":"DOI: 10.1234/ABC"}})),
            ..Default::default()
        };
        let value =
            serde_json::to_value(result_json(&result, &HttpResultMetadata::default())).unwrap();
        assert_eq!(value["engines"], json!(["a", "z"]));
        assert_eq!(value["doi"], "10.1234/abc");
        assert!(value.get("work_group").is_none());
        assert!(value["publishedDate"].is_null());
    }

    #[test]
    fn payload_inline_threshold_and_zero_position_match_projection_contract() {
        let result = SearchResult {
            payload: Some(ssxjson!({"data":"x".repeat(ssx_wire::PUBLIC_PAYLOAD_INLINE_BYTES)})),
            ..Default::default()
        };
        let value =
            serde_json::to_value(result_json(&result, &HttpResultMetadata::default())).unwrap();
        assert!(value["payload"].is_null());
        assert_eq!(value["positions"], json!([]));
        let yaml = serde_json::to_value(result_yaml(&result)).unwrap();
        assert!(yaml["content"].is_null());
    }

    #[test]
    fn public_projection_preserves_nested_lossless_payload_numbers() {
        let result = SearchResult {
            payload: Some(
                ssx_json::from_str(
                    r#"{"$serde_json::private::Number":"123","nested":{"n":18446744073709551617}}"#,
                )
                .unwrap(),
            ),
            ..Default::default()
        };
        let bytes =
            serde_json::to_vec(&result_json(&result, &HttpResultMetadata::default())).unwrap();
        let value: Value = ssx_json::from_slice(&bytes).unwrap();
        assert_eq!(value["payload"]["$serde_json::private::Number"], "123");
        assert_eq!(
            value["payload"]["nested"]["n"].to_string(),
            "18446744073709551617"
        );
    }

    #[test]
    fn http_projection_uses_caller_owned_formatter_metadata() {
        let result = SearchResult {
            url: "https://example.test/a".into(),
            published_date: Some("2026-1-2".into()),
            ..Default::default()
        };
        let metadata = HttpResultMetadata {
            pubdate: Some(1_767_312_000),
            parsed_url: Some(vec![
                "https".into(),
                "example.test".into(),
                "/a".into(),
                "".into(),
                "".into(),
                "".into(),
            ]),
        };
        let projected = result_json(&result, &metadata);
        assert_eq!(projected.pubdate, metadata.pubdate);
        assert_eq!(projected.parsed_url, metadata.parsed_url);
        let response = SearchResponse {
            results: vec![result],
            ..Default::default()
        };
        assert!(search_json(&response, None, &[]).is_err());
        assert!(search_json(&response, None, &[metadata]).is_ok());
    }
}
