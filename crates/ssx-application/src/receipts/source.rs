//! Existing F13 snapshot authority + pure F05 handoff projection. Receipt access
//! never invokes engine policy to acquire data and never dispatches a query.
use super::{
    Cancellation, Failure,
    service::{NON_VERIFICATION_NOTE, Source, SourceReader},
};
use crate::{
    PortFuture,
    retrieval::classify_retrieval_url,
    snapshot::{SnapshotProjectionLookup, SnapshotService},
};
use ssx_domain::{QueryId, SearchResult, SearchSnapshot, SnapshotId, TenantId};
use ssx_json::{Value, json};

#[derive(Clone)]
pub struct Snapshots(pub SnapshotService);
/// The original parses an unbounded Python integer, then compares it with the
/// snapshot length. Preserve overflow as an out-of-range index, not malformed
/// syntax. No leading zeros, signs, whitespace or Unicode decimal spelling.
pub fn result_parts(result_id: &str) -> Option<(&str, Option<usize>)> {
    let (snapshot, index) = result_id.rsplit_once(':')?;
    if index.is_empty()
        || !index.bytes().all(|c| c.is_ascii_digit())
        || (index.len() > 1 && index.starts_with('0'))
    {
        return None;
    }
    Some((snapshot, index.parse().ok()))
}
pub fn discovery(snapshot: &SearchSnapshot, index: usize, result_id: &str) -> Option<Value> {
    let result = snapshot.results().get(index)?;
    Some(discovery_result(
        snapshot.id(),
        snapshot.query_id(),
        snapshot.query(),
        result,
        result_id,
    ))
}
fn discovery_result(
    snapshot: &SnapshotId,
    query_id: &QueryId,
    query: &str,
    result: &SearchResult,
    result_id: &str,
) -> Value {
    let engines = if result.engines.is_empty() {
        vec![result.engine.clone()]
    } else {
        result.engines.iter().cloned().collect::<Vec<_>>()
    };
    let classified = classify_retrieval_url(&result.url);
    let handoff = json!({"contract":"slopsearx.retrieval_handoff","version":1,"result_id":result_id,"url":classified.url,"url_status":classified.status,"url_reason":classified.reason,"scheme":classified.scheme,"eligible":classified.status=="ok","snippet_only":result.content.chars().count()<=300,"verified":false,"verification_note":NON_VERIFICATION_NOTE,"provenance":{"snapshot_cursor":snapshot.as_str(),"query_id":query_id.as_str(),"query":query,"source_engines":engines}});
    let mut discovery = json!({"result_id":result_id,"title":result.title,"original_url":result.url,"query_id":query_id.as_str(),"snapshot_cursor":snapshot.as_str(),"query":query,"source_engines":engines,"handoff":null,"verified":false,"verification_note":NON_VERIFICATION_NOTE});
    // Move the same complete output container. It can exceed the default input
    // parser's4MiB after lawful repeated scheme/query fields are projected.
    discovery["handoff"] = handoff;
    discovery
}
impl SourceReader for Snapshots {
    fn discovery<'a>(
        &'a self,
        tenant: &'a TenantId,
        result_id: &'a str,
        cancel: &'a Cancellation,
    ) -> PortFuture<'a, Result<Source, Failure>> {
        Box::pin(async move {
            cancel.check()?;
            let Some((snapshot, index)) = result_parts(result_id) else {
                return Ok(Source::InvalidId);
            };
            let Ok(snapshot) = SnapshotId::new(snapshot) else {
                return Ok(Source::Unknown);
            };
            let found = self
                .0
                .for_tenant(tenant.clone())
                .read_projected(&snapshot)
                .await;
            cancel.check()?;
            Ok(match found {
                SnapshotProjectionLookup::Unavailable => Source::Unavailable,
                SnapshotProjectionLookup::Missing => Source::Unknown,
                SnapshotProjectionLookup::Expired { expires_at } => {
                    Source::Expired(expires_at.timestamp())
                }
                SnapshotProjectionLookup::Found(snapshot) => {
                    let Some(index) = index.filter(|i| *i < snapshot.summary().total()) else {
                        return Ok(Source::Unknown);
                    };
                    let mut selected = None;
                    let summary = snapshot.summary();
                    snapshot
                        .visit_results(&mut |position, result| {
                            if position == index {
                                selected = Some(discovery_result(
                                    summary.id(),
                                    summary.query_id(),
                                    summary.query(),
                                    result,
                                    result_id,
                                ));
                            }
                        })
                        .map_err(|failure| match failure {
                            crate::snapshot::SnapshotFailure::Closed => Failure::Closed,
                            crate::snapshot::SnapshotFailure::Unavailable
                            | crate::snapshot::SnapshotFailure::CompletionUnknown => {
                                Failure::Unavailable
                            }
                            _ => Failure::Malformed,
                        })?;
                    cancel.check()?;
                    selected.map(Source::Live).unwrap_or(Source::Unknown)
                }
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};
    use ssx_domain::{ScopeDecision, WallClockExpiry, WallClockTimestamp};
    #[test]
    fn large_retained_discovery_projection_avoids_default_parser_reentry() {
        let mut completed = 0;
        for (case, query, url, expected_input_sha) in [
            (
                "scheme",
                "q".to_owned(),
                format!("{}:resource", "a".repeat(3 * 1024 * 1024)),
                "94607bbaafe76074f9219bcbeab3aa2bd2806cb9721b115b9f882a9913776b0b",
            ),
            (
                "query",
                "q".repeat(3 * 1024 * 1024),
                "https://example.com/path".to_owned(),
                "5ace887154652d6fae580668b307b1485cab872c8b59d1543b3f569d95db63a0",
            ),
        ] {
            let result = SearchResult {
                engine: "wikipedia".into(),
                url,
                ..Default::default()
            };
            let snapshot = SearchSnapshot::new(
                SnapshotId::new("large").unwrap(),
                query,
                QueryId::new("large-source").unwrap(),
                vec![result],
                ScopeDecision {
                    selected_engines: vec!["wikipedia".into()],
                    ..Default::default()
                },
                TenantId::new("default").unwrap(),
                WallClockTimestamp::new(1000.).unwrap(),
                Some(WallClockExpiry::new(4600.).unwrap()),
            )
            .unwrap();
            let stored =
                ssx_json::to_vec(&ssx_domain::storage::snapshot::encode(&snapshot).unwrap())
                    .unwrap();
            assert!(stored.len() <= ssx_json::MAX_JSON_BYTES);
            let sha = |bytes: &[u8]| {
                Sha256::digest(bytes)
                    .iter()
                    .map(|b| format!("{b:02x}"))
                    .collect::<String>()
            };
            assert_eq!(sha(&stored), expected_input_sha);
            let value = discovery(&snapshot, 0, "large:0").unwrap();
            let keys = |v: &Value| v.as_object().unwrap().keys().cloned().collect::<Vec<_>>();
            assert_eq!(
                keys(&value),
                vec![
                    "result_id",
                    "title",
                    "original_url",
                    "query_id",
                    "snapshot_cursor",
                    "query",
                    "source_engines",
                    "handoff",
                    "verified",
                    "verification_note"
                ]
            );
            assert_eq!(value["result_id"].as_str(), Some("large:0"));
            assert_eq!(value["title"].as_str(), Some(""));
            assert_eq!(
                value["original_url"].as_str(),
                Some(snapshot.results()[0].url.as_str())
            );
            assert_eq!(value["query_id"].as_str(), Some("large-source"));
            assert_eq!(value["snapshot_cursor"].as_str(), Some("large"));
            assert_eq!(value["query"].as_str(), Some(snapshot.query()));
            assert_eq!(value["source_engines"], json!(["wikipedia"]));
            assert_eq!(value["verified"].as_bool(), Some(false));
            assert_eq!(
                value["verification_note"].as_str(),
                Some(NON_VERIFICATION_NOTE)
            );
            let handoff = &value["handoff"];
            assert_eq!(
                keys(handoff),
                vec![
                    "contract",
                    "version",
                    "result_id",
                    "url",
                    "url_status",
                    "url_reason",
                    "scheme",
                    "eligible",
                    "snippet_only",
                    "verified",
                    "verification_note",
                    "provenance"
                ]
            );
            assert_eq!(
                handoff["contract"].as_str(),
                Some("slopsearx.retrieval_handoff")
            );
            assert_eq!(handoff["version"].as_u64(), Some(1));
            assert_eq!(handoff["result_id"].as_str(), Some("large:0"));
            assert_eq!(handoff["snippet_only"].as_bool(), Some(true));
            assert_eq!(handoff["verified"].as_bool(), Some(false));
            assert_eq!(
                handoff["verification_note"].as_str(),
                Some(NON_VERIFICATION_NOTE)
            );
            let provenance = &handoff["provenance"];
            assert_eq!(
                keys(provenance),
                vec!["snapshot_cursor", "query_id", "query", "source_engines"]
            );
            assert_eq!(provenance["snapshot_cursor"], value["snapshot_cursor"]);
            assert_eq!(provenance["query_id"], value["query_id"]);
            assert_eq!(provenance["query"], value["query"]);
            assert_eq!(provenance["source_engines"], value["source_engines"]);
            let scheme = snapshot.results()[0].url.split_once(':').unwrap().0;
            assert_eq!(handoff["scheme"].as_str(), Some(scheme));
            if case == "scheme" {
                assert_eq!(handoff["url_status"].as_str(), Some("non_http"));
                assert!(handoff["url"].is_null());
                assert_eq!(handoff["eligible"].as_bool(), Some(false));
                let reason = format!("scheme '{scheme}' is not HTTP(S); not retrievable over HTTP");
                assert_eq!(handoff["url_reason"].as_str(), Some(reason.as_str()));
            } else {
                assert_eq!(handoff["url_status"].as_str(), Some("ok"));
                assert_eq!(handoff["url"], value["original_url"]);
                assert!(handoff["url_reason"].is_null());
                assert_eq!(handoff["eligible"].as_bool(), Some(true));
            }
            let output = ssx_json::to_vec(&value).unwrap();
            assert!(output.len() > ssx_json::MAX_JSON_BYTES);
            assert!(
                ssx_json::from_slice::<Value>(&output).is_err(),
                "global input budget remains unchanged"
            );
            println!(
                "W07_DISCOVERY_CONTAINER_BOUNDARY case={case} stored_bytes={} output_bytes={} output_sha256={}",
                stored.len(),
                output.len(),
                sha(&output)
            );
            completed += 1;
        }
        assert_eq!(completed, 2);
    }
}
