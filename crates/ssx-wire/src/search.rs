//! Versioned public result projections. These DTOs intentionally exclude the
//! backend-only `work_group`; surface adapters may add allowlisted Paper fields.
use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use ssx_json::Value;

pub const PUBLIC_PAYLOAD_INLINE_BYTES: usize = 512;

/// Formatter-derived metadata supplied by the HTTP boundary. URL parsing and
/// date interpretation are owned by the formatter contract (T02), not this
/// backend-independent DTO projection.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct HttpResultMetadata {
    pub pubdate: Option<i64>,
    pub parsed_url: Option<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MediaDto {
    pub media_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thumbnail: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub height: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration: Option<ssx_json::Number>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HttpResultDto {
    pub url: String,
    pub title: String,
    pub content: String,
    pub engine: String,
    pub engines: Vec<String>,
    pub score: ssx_json::Number,
    pub positions: Vec<i64>,
    pub category: String,
    #[serde(rename = "publishedDate")]
    pub published_date: Option<String>,
    pub pubdate: Option<i64>,
    pub tier: i64,
    pub length: Option<i64>,
    pub thumbnail: Option<String>,
    pub img_src: Option<String>,
    pub media: Option<MediaDto>,
    pub payload: Option<Value>,
    pub iframe_src: Option<String>,
    pub audio_src: Option<String>,
    pub views: Option<Value>,
    pub author: Option<String>,
    pub metadata: Option<Value>,
    pub template: String,
    pub parsed_url: Option<Vec<String>>,
    pub open_group: bool,
    pub close_group: bool,
    pub priority: String,
    #[serde(flatten)]
    pub paper: BTreeMap<String, Value>,
}

/// Fields used by the source YAML+Markdown projection, which intentionally has
/// a smaller result schema than HTTP JSON.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct YamlResultDto {
    pub url: String,
    pub title: String,
    pub engine: String,
    pub engines: Vec<String>,
    pub content: Option<String>,
    pub score: ssx_json::Number,
    pub position: i64,
    pub published: Option<String>,
    pub tier: i64,
    pub media: Option<MediaDto>,
    pub payload: Option<Value>,
    #[serde(flatten)]
    pub paper: BTreeMap<String, Value>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HttpSearchDto {
    pub query: String,
    pub results: Vec<HttpResultDto>,
    pub number_of_results: usize,
    pub answers: Vec<Value>,
    pub corrections: Vec<String>,
    pub infoboxes: Vec<Value>,
    pub suggestions: Vec<String>,
    pub unresponsive_engines: Vec<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub meta: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct McpCitationDto {
    pub label: String,
    pub url: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct McpRetrievalDto {
    pub contract: String,
    pub version: u8,
    pub eligible: bool,
    pub url_status: String,
    pub url_reason: Option<String>,
    pub scheme: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct McpCardDto {
    pub title: String,
    pub url: String,
    pub snippet: String,
    pub source_engines: Vec<String>,
    pub source_count: usize,
    pub primary_engine: String,
    pub category: String,
    pub published_at: Option<String>,
    pub score: ssx_json::Number,
    pub position: i64,
    pub tier: i64,
    pub citation: McpCitationDto,
    pub retrieval: McpRetrievalDto,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub media: Option<McpMediaDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artifact: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payload: Option<Value>,
    #[serde(flatten)]
    pub paper: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct McpMediaDto {
    pub media_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thumbnail: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub height: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration: Option<ssx_json::Number>,
}
pub use ssx_publication::{normalize_doi, paper_fields};

pub fn inline_payload(payload: Option<&Value>) -> Option<Value> {
    let payload = payload.filter(|value| value.is_object())?;
    let size = ssx_json::to_vec(payload).ok()?.len();
    (size <= PUBLIC_PAYLOAD_INLINE_BYTES).then(|| payload.clone())
}
pub fn nonempty(value: Option<&String>) -> Option<String> {
    value.filter(|text| !text.is_empty()).cloned()
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn doi_unicode_boundaries_never_slice_codepoints() {
        assert_eq!(normalize_doi("aééé"), None);
        assert_eq!(
            normalize_doi("DOİ: 10.١٢٣٤/Straße"),
            Some("10.١٢٣٤/strasse".into())
        );
        assert_eq!(normalize_doi("10.1234/a\u{1f}b"), None);
        assert_eq!(normalize_doi("https://doi.org/doi:10.1234/x"), None);
    }
    #[test]
    fn public_dto_codec_preserves_opaque_huge_numbers_and_reserved_tag_objects() {
        let raw = r#"{"url":"https://example.test/","title":"title","content":"content","engine":"engine","engines":["engine"],"score":1.0,"positions":[],"category":"general","publishedDate":null,"pubdate":null,"tier":1,"length":null,"thumbnail":null,"img_src":null,"media":null,"payload":{"$serde_json::private::Number":"123","nested":{"n":18446744073709551617}},"iframe_src":null,"audio_src":null,"views":null,"author":null,"metadata":null,"template":"default.html","parsed_url":null,"open_group":false,"close_group":false,"priority":""}"#;
        let dto: HttpResultDto = ssx_json::from_str(raw).unwrap();
        let encoded = ssx_json::to_string(&dto).unwrap();
        let expected: Value = ssx_json::from_str(raw).unwrap();
        let actual: Value = ssx_json::from_str(&encoded).unwrap();
        assert_eq!(actual, expected);
        assert_eq!(
            actual["payload"]["nested"]["n"].to_string(),
            "18446744073709551617"
        );
        assert_eq!(actual["payload"]["$serde_json::private::Number"], "123");
    }
}
