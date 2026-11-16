//! Public wire DTOs. Durable backend codecs live in `ssx-domain::storage`.
use serde::{Deserialize, Serialize};
pub mod search;
pub use search::{
    HttpResultDto, HttpResultMetadata, HttpSearchDto, McpCardDto, McpCitationDto, McpMediaDto,
    McpRetrievalDto, MediaDto, PUBLIC_PAYLOAD_INLINE_BYTES, YamlResultDto, inline_payload,
    nonempty, paper_fields,
};
#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct Readiness {
    pub status: String,
    pub mode: String,
}
impl Readiness {
    pub fn foundation() -> Self {
        Self {
            status: "ok".into(),
            mode: "foundation".into(),
        }
    }
}
