//! Deterministic native-profile preservation qualifier, separate from source parity.
use ssx_infra::observability::metrics::{DEFINITIONS, Kind, Metrics};
#[test]
fn current_fixed_35_family_output_snapshot() {
    let metrics = Metrics::new(true, vec!["fixture".into()], vec!["general".into()]).unwrap();
    assert_eq!(DEFINITIONS.len(), 35);
    for definition in DEFINITIONS {
        let labels = definition
            .labels
            .iter()
            .map(|key| {
                (
                    *key,
                    match *key {
                        "engine" => "fixture",
                        "category" => "general",
                        "format" => "json",
                        "workflow" => "saved_search",
                        "mode" => "immediate",
                        "reason" => "policy",
                        "artifact" => "job",
                        "tool" => "slopsearx_search",
                        "code" => "internal_error",
                        "event_type" => "report_created",
                        "resource" => "stream",
                        "type" => {
                            if definition.name == "slopsearx_cache_hit_total" {
                                "hit"
                            } else {
                                "timeout"
                            }
                        }
                        "outcome" => {
                            if definition.name.contains("http_search") {
                                "success"
                            } else if definition.name.contains("event_reads") {
                                "delivered"
                            } else if definition.name.contains("acknowledgements") {
                                "advanced"
                            } else {
                                "succeeded"
                            }
                        }
                        other => panic!("new unbound closed label {other}"),
                    },
                )
            })
            .collect::<Vec<_>>();
        metrics
            .update(
                definition.name,
                &labels,
                match definition.kind {
                    Kind::Counter => 3.,
                    Kind::Gauge => 2.,
                    Kind::Histogram => 0.25,
                },
            )
            .unwrap();
    }
    let text = metrics.render();
    assert_eq!(
        text,
        include_str!("../../xtask/fixtures/o01/current-native35-before.txt")
    );
    assert_eq!(
        text.lines().filter(|l| l.starts_with("# HELP ")).count(),
        35
    );
    assert_eq!(
        text.lines().filter(|l| l.starts_with("# TYPE ")).count(),
        35
    );
    for definition in DEFINITIONS {
        assert!(text.contains(&format!("# HELP {} {}\n", definition.name, definition.help)));
    }
    println!(
        "O01_CURRENT35_SNAPSHOT {}",
        serde_json::to_string(&text).unwrap()
    );
}
