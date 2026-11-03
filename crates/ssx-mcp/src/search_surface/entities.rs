//! MCP paging and artifact envelope over the shared pure entity projection.
use super::artifact;
pub(crate) use ssx_application::entity_projection::Collector;
use ssx_domain::SearchSnapshot;
use ssx_json::{Value, json};
pub(super) fn read(s: &SearchSnapshot, page: i64, start: usize, size: usize, version: u8) -> Value {
    let mut collector = Collector::new(s.id().as_str(), version);
    for (i, r) in s.results().iter().enumerate() {
        collector.feed(i, r);
    }
    let (groups, edges) = collector.finish();
    let unresolved = groups.iter().filter(|v| v["entity_id"].is_null()).count();
    let total = groups.len();
    let selected: Vec<_> = groups
        .into_iter()
        .skip(start)
        .take(size)
        .map(|mut v| {
            v["artifact"] = v["entity_id"]
                .as_str()
                .map(|id| {
                    artifact(
                        "entity_group",
                        &ssx_domain::canonical::composite_artifact_id(&[s.id().as_str(), id])
                            .expect("composite artifact"),
                    )
                })
                .unwrap_or(Value::Null);
            v
        })
        .collect();
    let mut out = json!({"contract":"slopsearx.entity_groups","version":version,"cursor":s.id().as_str(),"query":s.query(),"page":page,"entities":selected,"meta":{"total_entities":total,"total_results":s.total(),"unresolved_entities":unresolved,"has_more":start.saturating_add(size)<total,"query_id":s.query_id().as_str(),"note":"Entity identity is source-reported, not independent corroboration or verification."}});
    if version == 2 {
        out["relationships"] = json!(edges.into_values().collect::<Vec<_>>());
    }
    out
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn frozen_source_entity_grammar_conserves_indices_facts_edges_and_unicode16() {
        assert_eq!(unicode_normalization::UNICODE_VERSION, (16, 0, 0));
        let corpus: Value = ssx_json::from_str(include_str!(
            "../../../../docs/rust-rewrite/t06/entities.json"
        ))
        .unwrap();
        assert_eq!(corpus["unicode_version"], "16.0.0");
        let mut failures = Vec::new();
        for row in corpus["cases"].as_array().unwrap() {
            let results: Vec<_> = row["results"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| ssx_domain::storage::decode_result(v.clone()).unwrap())
                .collect();
            let snapshot = SearchSnapshot::new(
                ssx_domain::SnapshotId::new("snap-test").unwrap(),
                "q".into(),
                ssx_domain::QueryId::new("q1").unwrap(),
                results,
                Default::default(),
                ssx_domain::TenantId::new("default").unwrap(),
                ssx_domain::WallClockTimestamp::new(1.0).unwrap(),
                None,
            )
            .unwrap();
            let version = row["version"].as_u64().unwrap() as u8;
            let mut actual = read(&snapshot, 1, 0, 10000, version);
            for group in actual["entities"].as_array_mut().unwrap() {
                group.as_object_mut().unwrap().shift_remove("artifact");
            }
            if actual["entities"] != row["entities"] {
                failures.push(format!(
                    "{} v{} actual={} expected={}",
                    row["case"], version, actual["entities"], row["entities"]
                ));
            }
            if version == 2 && actual["relationships"] != row["relationships"] {
                failures.push(format!(
                    "{} relationships actual={} expected={}",
                    row["case"], actual["relationships"], row["relationships"]
                ));
            }
        }
        assert!(
            failures.is_empty(),
            "{} entity source differences\n{}",
            failures.len(),
            failures
                .iter()
                .take(15)
                .cloned()
                .collect::<Vec<_>>()
                .join("\n")
        );
    }
}
