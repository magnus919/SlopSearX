//! Frozen-wire comparison masks only documented protocol nondeterminism.
//! Issued snapshot identity is paired at a successful search, then reused at
//! explicit cursor/result-reference paths. Adapter payloads are never traversed.
use ssx_json::{Value, json};
use std::collections::BTreeMap;
#[derive(Default)]
pub struct Comparison {
    cursors: BTreeMap<String, String>,
}
fn at(value: &mut Value, path: &[&str], f: &mut impl FnMut(&mut Value)) {
    let Some((head, tail)) = path.split_first() else {
        f(value);
        return;
    };
    if *head == "*" {
        match value {
            Value::Array(a) => {
                for v in a {
                    at(v, tail, f);
                }
            }
            Value::Object(o) => {
                for v in o.values_mut() {
                    at(v, tail, f);
                }
            }
            _ => {}
        }
    } else if let Some(value) = value.get_mut(head) {
        at(value, tail, f);
    }
}
impl Comparison {
    pub fn pair(
        &mut self,
        actual: Value,
        expected: Value,
        issued_by_search: bool,
    ) -> (Value, Value) {
        if issued_by_search {
            let a = actual["structuredContent"]["meta"]["cursor"].as_str();
            let e = expected["structuredContent"]["meta"]["cursor"].as_str();
            if let (Some(a), Some(e)) = (a, e) {
                assert!(a.starts_with("snap-") && e.starts_with("snap-"));
                if let Some(previous) = self.cursors.insert(a.into(), e.into()) {
                    assert_eq!(previous, e, "issued snapshot identity changed");
                }
            }
        }
        (self.envelope(actual, true), self.envelope(expected, false))
    }
    fn envelope(&self, mut value: Value, native: bool) -> Value {
        if let Some(payload) = value.get_mut("structuredContent") {
            self.payload(payload, native);
        }
        // Only the tool text envelope carries the same structured object. No
        // arbitrary nested `text` field or result content is interpreted.
        if let Some(contents) = value.get_mut("content").and_then(Value::as_array_mut) {
            for content in contents {
                if let Some(text) = content.get_mut("text")
                    && let Some(raw) = text.as_str()
                    && let Ok(mut payload) = ssx_json::from_str::<Value>(raw)
                {
                    self.payload(&mut payload, native);
                    *text = json!(ssx_json::to_string(&payload).unwrap());
                }
            }
        }
        // The sole plain-text clock mask is the documented status resource.
        if let Some(contents) = value.get_mut("contents").and_then(Value::as_array_mut) {
            for content in contents {
                if content["uri"].as_str() == Some("slopsearx://health/summary")
                    && let Some(raw) = content["text"].as_str()
                {
                    content["text"] = json!(
                        raw.lines()
                            .map(|line| if line.starts_with("- freshness:") {
                                "- freshness: <freshness>"
                            } else {
                                line
                            })
                            .collect::<Vec<_>>()
                            .join("\n")
                    );
                }
            }
        }
        value
    }
    fn payload(&self, value: &mut Value, native: bool) {
        for path in [
            "meta/query_id",
            "provenance/query_id",
            "snapshot/query_id",
            "retrieval/provenance/query_id",
            "error/query_id",
            "research_execution/worker_id",
            "freshness",
            "engines/*/status_at",
            "engines/*/last_known_status_at",
        ] {
            at(value, &path.split('/').collect::<Vec<_>>(), &mut |v| {
                let field = path.rsplit('/').next().unwrap();
                let valid = v.as_str().is_some_and(|s| match field {
                    "query_id" | "worker_id" => ssx_domain::QueryId::new(s).is_ok(),
                    _ => clock(s),
                });
                if valid {
                    *v = json!(format!("<{field}>"));
                }
            });
        }
        at(value, &["meta", "response_time_ms"], &mut |v| {
            if v.as_f64().is_some_and(|n| n.is_finite() && n >= 0.0) {
                *v = json!(0);
            }
        });
        if native {
            for path in [
                "meta/cursor",
                "meta/artifact/id",
                "cursor",
                "artifact/id",
                "result_id",
                "snapshot/cursor",
                "retrieval/result_id",
                "retrieval/provenance/snapshot_cursor",
                "results/*/result_id",
                "results/*/artifact/id",
                "entities/*/result_ids/*",
                "relationships/*/result_ids/*",
            ] {
                at(value, &path.split('/').collect::<Vec<_>>(), &mut |v| {
                    if let Some(raw) = v.as_str() {
                        if let Some(source) = self.cursors.get(raw) {
                            *v = json!(source);
                        } else if let Some((cursor, index)) = raw.rsplit_once(':')
                            && let Some(source) = self.cursors.get(cursor)
                        {
                            *v = json!(format!("{source}:{index}"));
                        }
                    }
                });
            }
        }
    }
}
#[test]
fn comparator_preserves_literal_adapter_fields_and_requires_issued_cursor_identity() {
    let mut comparison = Comparison::default();
    let expected = json!({"structuredContent":{"meta":{"cursor":"snap-source","query_id":"ssx-source"},"results":[{"result_id":"snap-source:0","payload":{"query_id":"literal","freshness":"literal","status_at":"literal","text":"snap-business-text"}}]}});
    let actual = json!({"structuredContent":{"meta":{"cursor":"snap-native","query_id":"ssx-native"},"results":[{"result_id":"snap-native:0","payload":{"query_id":"literal","freshness":"literal","status_at":"literal","text":"snap-business-text"}}]}});
    let (a, e) = comparison.pair(actual.clone(), expected.clone(), true);
    assert_eq!(a, e);
    for key in ["query_id", "freshness", "status_at", "text"] {
        let mut changed = actual.clone();
        changed["structuredContent"]["results"]
            .as_array_mut()
            .unwrap()[0]["payload"][key] = json!("changed");
        let (a, e) = comparison.pair(changed, expected.clone(), false);
        assert_ne!(a, e, "masked adapter payload {key}");
    }
    let (a, e) = comparison.pair(
        json!({"structuredContent":{"cursor":"snap-unissued"}}),
        json!({"structuredContent":{"cursor":"snap-source"}}),
        false,
    );
    assert_ne!(a, e);
}

fn clock(s: &str) -> bool {
    // Both source +00:00 and native Z forms carry the same ISO date/time shape.
    let b = s.as_bytes();
    b.len() >= 20
        && b[4] == b'-'
        && b[7] == b'-'
        && b[10] == b'T'
        && b[13] == b':'
        && b[16] == b':'
        && [0..4, 5..7, 8..10, 11..13, 14..16, 17..19]
            .into_iter()
            .all(|r| b[r].iter().all(u8::is_ascii_digit))
        && (s.ends_with('Z') || s.ends_with("+00:00"))
}
#[test]
fn comparator_rejects_dynamic_protocol_type_and_shape_corruption() {
    let expected = json!({"structuredContent":{"meta":{"query_id":"ssx-source","response_time_ms":1},"freshness":"2026-10-17T01:02:03+00:00","research_execution":{"worker_id":"worker-source"},"engines":{"wikipedia":{"status_at":"2026-10-17T01:02:03+00:00"}}}});
    for path in [
        vec!["meta", "query_id"],
        vec!["meta", "response_time_ms"],
        vec!["freshness"],
        vec!["research_execution", "worker_id"],
        vec!["engines", "wikipedia", "status_at"],
    ] {
        for corrupt in [json!(true), json!({}), json!([]), json!(null), json!("")] {
            let mut actual = expected.clone();
            at(&mut actual["structuredContent"], &path, &mut |v| {
                *v = corrupt.clone()
            });
            let (a, e) = Comparison::default().pair(actual, expected.clone(), false);
            assert_ne!(a, e, "masked protocol corruption {path:?}: {corrupt}");
        }
    }
}
