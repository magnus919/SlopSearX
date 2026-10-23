//! Four inert evidence tools on the existing native handler/factory boundary.
//! Normalization, storage, manifest assembly and traversal stay in application.
use super::EvidenceRuntime;
use rmcp::ErrorData;
use ssx_application::{
    lineage::{self, Direction},
    receipts::{
        Cancellation, Failure,
        observation::ObservationInput,
        service::{ManifestDraft, error},
    },
};
use ssx_domain::TenantId;
use ssx_json::{Map, Value};

pub const TOOL_NAMES: [&str; 4] = [
    "slopsearx_submit_retrieval_receipt",
    "slopsearx_read_retrieval_receipts",
    "slopsearx_export_research_manifest",
    "slopsearx_get_artifact_lineage",
];
fn source() -> &'static Value {
    static SOURCE: std::sync::OnceLock<Value> = std::sync::OnceLock::new();
    SOURCE.get_or_init(|| {
        ssx_json::from_str(include_str!(
            "../../../../docs/rust-rewrite/w07/mcp-tools-source.json"
        ))
        .expect("source-projected W07 descriptors")
    })
}
pub(crate) fn descriptor(name: &str) -> Option<&'static Value> {
    source()["tools"]
        .as_array()
        .expect("W07 tools")
        .iter()
        .find(|tool| tool["name"].as_str() == Some(name))
}
pub fn descriptors() -> Vec<serde_json::Value> {
    serde_json::from_str::<serde_json::Value>(include_str!(
        "../../../../docs/rust-rewrite/w07/mcp-tools-source.json"
    ))
    .expect("W07 descriptors")["tools"]
        .as_array()
        .expect("tools")
        .clone()
}
fn semantic_error(message: &str, field: &str) -> Value {
    let mut value = error("invalid_input", message);
    value["error"]["field"] = Value::String(field.into());
    value
}
fn string(values: &Map<String, Value>, name: &str) -> String {
    values
        .get(name)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .into()
}
fn optional(values: &Map<String, Value>, name: &str) -> Option<String> {
    values.get(name).and_then(Value::as_str).map(str::to_owned)
}
fn bound(values: &Map<String, Value>, name: &str, default: usize) -> usize {
    values.get(name).map_or(default, |v| {
        v.as_i64()
            .and_then(|n| usize::try_from(n).ok())
            .unwrap_or(usize::MAX)
    })
}
/// Storage failures remain distinct from missing handles and from an error
/// that can safely claim no write happened. None of these errors trigger retry.
fn failure(failure: Failure) -> Result<Value, ErrorData> {
    Ok(match failure {
        Failure::Cancelled => return Err(ErrorData::internal_error("cancelled", None)),
        Failure::Closed | Failure::Unavailable => {
            error("store_unavailable", "receipt store is unavailable")
        }
        Failure::Malformed => error("store_corrupt", "retained receipt state is malformed"),
        Failure::InvalidInput => error("invalid_input", "receipt store rejected the input"),
        Failure::CompletionUnknown | Failure::ScriptErrorMayHaveWritten => error(
            "completion_unknown",
            "receipt persistence completion is unknown; replay the same idempotency key",
        ),
        Failure::Collision => error("store_unavailable", "receipt identity collision"),
    })
}
fn lineage_failure(failure: lineage::Failure) -> Result<Value, ErrorData> {
    match failure {
        lineage::Failure::Cancelled => Err(ErrorData::internal_error("cancelled", None)),
        lineage::Failure::Closed | lineage::Failure::Unavailable => {
            Ok(error("store_unavailable", "artifact store is unavailable"))
        }
        lineage::Failure::Malformed => Ok(error(
            "store_corrupt",
            "retained artifact state is malformed",
        )),
    }
}
/// Arguments have already passed the source-compatible function type boundary.
/// The authenticated tenant is supplied by the existing native transport owner.
pub(crate) async fn execute(
    runtime: &EvidenceRuntime,
    name: &str,
    values: &Map<String, Value>,
    tenant: &TenantId,
    cancel: &Cancellation,
) -> Result<Value, ErrorData> {
    let result_id = string(values, "result_id");
    let result = match name {
        "slopsearx_submit_retrieval_receipt" => {
            runtime
                .service
                .submit(
                    tenant,
                    &result_id,
                    ObservationInput {
                        retriever: string(values, "retriever"),
                        idempotency_key: string(values, "idempotency_key"),
                        status: string(values, "status"),
                        captured_at: optional(values, "captured_at"),
                        final_url: optional(values, "final_url"),
                        content_sha256: optional(values, "content_sha256"),
                        capture_ref: optional(values, "capture_ref"),
                        passage_refs: values.get("passage_refs").filter(|v| !v.is_null()).cloned(),
                        failure_code: optional(values, "failure_code"),
                        failure_message: optional(values, "failure_message"),
                    },
                    cancel,
                )
                .await
        }
        "slopsearx_read_retrieval_receipts" => {
            runtime
                .service
                .read(tenant, &result_id, bound(values, "limit", 20), cancel)
                .await
        }
        "slopsearx_export_research_manifest" => {
            let parse = |v: &Value| lineage::input::artifact(v).map_err(|e| e.0.to_owned());
            runtime
                .service
                .manifest_draft(
                    tenant,
                    ManifestDraft {
                        result_ids: values.get("result_ids").and_then(Value::as_array).map(|a| {
                            a.iter()
                                .map(|v| v.as_str().expect("validated result ID").into())
                                .collect()
                        }),
                        artifacts: values
                            .get("artifacts")
                            .and_then(Value::as_array)
                            .map(|a| a.iter().map(parse).collect()),
                        max_depth: bound(values, "max_depth", 2),
                        max_nodes: bound(values, "max_nodes", 100),
                        source: values.get("source").filter(|v| !v.is_null()).map(parse),
                    },
                    cancel,
                )
                .await
        }
        "slopsearx_get_artifact_lineage" => {
            let root =
                match lineage::input::artifact(values.get("artifact").expect("validated artifact"))
                {
                    Ok(root) => root,
                    Err(e) => return Ok(semantic_error(e.0, "artifact")),
                };
            let direction = match Direction::parse(
                values
                    .get("direction")
                    .and_then(Value::as_str)
                    .unwrap_or("both"),
            ) {
                Ok(direction) => direction,
                Err(e) => return Ok(semantic_error(e.0, "direction")),
            };
            let request = lineage::Request {
                root,
                direction,
                max_depth: bound(values, "max_depth", 2),
                max_nodes: bound(values, "max_nodes", 100),
            };
            if request.max_depth > 5 {
                return Ok(semantic_error(
                    "max_depth must be an integer from 0 to 5",
                    "max_depth",
                ));
            }
            if !(1..=100).contains(&request.max_nodes) {
                return Ok(semantic_error(
                    "max_nodes must be an integer from 1 to 100",
                    "max_nodes",
                ));
            }
            return match lineage::traverse(runtime.lineage.as_ref(), tenant, request, cancel).await
            {
                Ok(graph) => Ok(graph.to_value()),
                Err(e) => lineage_failure(e),
            };
        }
        _ => {
            return Err(ErrorData::invalid_params(
                format!("Unknown tool: {name}"),
                None,
            ));
        }
    };
    match result {
        Ok(value) => Ok(value),
        Err(e) => failure(e),
    }
}

#[cfg(test)]
mod tests;
