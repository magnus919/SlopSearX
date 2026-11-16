//! Pure MCP result-card projection helpers. Tool routing and snapshot storage
//! remain outside this module.
use ssx_domain::{ArtifactKind, ArtifactRef, ResultId, SearchResult};
use ssx_wire::{
    McpCardDto, McpCitationDto, McpMediaDto, McpRetrievalDto, inline_payload, nonempty,
    paper_fields,
};

pub fn result_card(
    result: &SearchResult,
    result_id: Option<&ResultId>,
    retrieval: McpRetrievalDto,
) -> McpCardDto {
    let media = result.media.as_ref().map(|media| McpMediaDto {
        media_type: media.media_type.clone(),
        thumbnail: nonempty(media.thumbnail.as_ref()),
        width: media.width,
        height: media.height,
        duration: media
            .duration
            .as_ref()
            .map(|number| number.as_number().clone()),
    });
    let id = result_id.map(ToString::to_string);
    let artifact = id
        .as_deref()
        .and_then(|value| ArtifactRef::new(ArtifactKind::Result, value).ok())
        .map(|value| value.to_value());
    McpCardDto {
        title: result.title.clone(),
        url: result.url.clone(),
        snippet: result.content.chars().take(300).collect(),
        source_engines: sorted_engines(result),
        source_count: if result.engines.is_empty() {
            1
        } else {
            result.engines.len()
        },
        primary_engine: result.engine.clone(),
        category: result.category.clone(),
        published_at: result.published_date.clone(),
        score: result.score.as_number().clone(),
        position: result.position,
        tier: result.tier,
        citation: McpCitationDto {
            label: result.title.clone(),
            url: result.url.clone(),
        },
        retrieval,
        media,
        result_id: id,
        artifact,
        payload: inline_payload(result.payload.as_ref()),
        paper: paper_fields(result.work_group.as_ref()),
    }
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
    fn card_sorts_provenance_hides_work_group_and_has_no_content_field() {
        let result = SearchResult {
            url: "https://example.test/item".into(),
            content: "x".repeat(301),
            engine: "primary".into(),
            engines: ["z", "a"].into_iter().map(str::to_owned).collect(),
            work_group: Some(ssxjson!({"paper":{"doi":"10.1234/ABC"},"private":true})),
            ..Default::default()
        };
        let retrieval = McpRetrievalDto {
            contract: "slopsearx.retrieval_handoff".into(),
            version: 1,
            eligible: true,
            url_status: "ok".into(),
            url_reason: None,
            scheme: Some("https".into()),
        };
        let value = serde_json::to_value(result_card(&result, None, retrieval)).unwrap();
        assert_eq!(value["source_engines"], json!(["a", "z"]));
        assert_eq!(value["source_count"], 2);
        assert_eq!(value["snippet"].as_str().unwrap().chars().count(), 300);
        assert_eq!(value["doi"], "10.1234/abc");
        assert!(value.get("work_group").is_none());
        assert!(value.get("content").is_none());
        assert_eq!(value["retrieval"]["url_status"], "ok");
    }
}
