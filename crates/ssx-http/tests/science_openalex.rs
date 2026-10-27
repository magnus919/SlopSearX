//! Public HTTP uses the same actual adapter/service/store as the MCP cases.
#[path = "../../ssx-infra/tests/support/science_openalex.rs"]
mod common;
use common::owned_loopback_fixture::OwnedLoopback;
use common::{FixtureCore as Fixture, *};
use serde_json::{Value, json};
use ssx_domain::{PublicationDateProof, storage};
use ssx_http::{HttpState, html::PublicHtml, native_router};
use ssx_infra::observability::{Options as TelemetryOptions, Telemetry};
use std::{
    sync::{Arc, atomic::Ordering},
    time::Duration,
};

struct Native {
    address: std::net::SocketAddr,
    stop: tokio::sync::oneshot::Sender<()>,
    owner: OwnedLoopback<std::io::Result<()>>,
    telemetry: Arc<Telemetry>,
    client: reqwest::Client,
}
impl Native {
    async fn start(fixture: &Fixture) -> Self {
        let telemetry = Telemetry::new(
            TelemetryOptions {
                audit: false,
                ..Default::default()
            },
            None,
            vec!["openalex".into(), "brave".into()],
            vec!["science".into(), "general".into()],
        )
        .unwrap();
        let router = native_router(HttpState {
            search: fixture.service.clone(),
            policy: fixture.policy.clone(),
            telemetry: telemetry.clone(),
            formats: ["json", "yaml", "html", "csv", "rss"]
                .into_iter()
                .map(str::to_owned)
                .collect(),
            discovery: ssx_json::json!({"categories":["science","general"]}),
            health_contexts: Default::default(),
            valkey_connected: Arc::new(|| true),
            valkey_fail_closed: false,
            valkey_quota_configured: false,
            html: Arc::new(PublicHtml::default()),
        });
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let (stop, stopping) = tokio::sync::oneshot::channel();
        let owner = OwnedLoopback::spawn(async move {
            axum::serve(
                listener,
                router.into_make_service_with_connect_info::<std::net::SocketAddr>(),
            )
            .with_graceful_shutdown(async {
                let _ = stopping.await;
            })
            .await
        });
        let client = reqwest::Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(Duration::from_secs(10))
            .build()
            .unwrap();
        Self {
            address,
            stop,
            owner,
            telemetry,
            client,
        }
    }
    async fn request(&self, path: &str, method: &str, params: &[(&str, &str)]) -> (u16, String) {
        let encoded = url::form_urlencoded::Serializer::new(String::new())
            .extend_pairs(params.iter().copied())
            .finish();
        let base = format!("http://{}", self.address);
        let request = if method == "GET" {
            self.client.get(format!("{base}{path}?{encoded}"))
        } else {
            self.client
                .post(format!("{base}{path}"))
                .header("content-type", "application/x-www-form-urlencoded")
                .body(encoded)
        };
        let response = request.send().await.unwrap();
        (response.status().as_u16(), response.text().await.unwrap())
    }
    async fn close(self) {
        self.stop.send(()).unwrap();
        self.owner.finish().await.unwrap();
        self.telemetry.shutdown().await;
    }
}
fn decode(body: &str, format: &str) -> Value {
    if format == "yaml" {
        serde_saphyr::from_str(body.split_once("---\n").unwrap().0).unwrap()
    } else {
        serde_json::from_str(body).unwrap()
    }
}
fn assert_http(value: &Value, format: &str, days: &[&str], expected: &Value, cached: bool) {
    assert_eq!(value["meta"]["date_filter"], *expected);
    assert_eq!(value["meta"]["cached"], cached);
    assert_eq!(value["meta"]["engine_status"].as_object().unwrap().len(), 1);
    assert_eq!(value["meta"]["engine_status"]["openalex"]["status"], "ok");
    let mut actual: Vec<_> = value["results"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| {
            assert_eq!(r["engine"], "openalex");
            assert_eq!(r["engines"], json!(["openalex"]));
            r[if format == "yaml" {
                "published"
            } else {
                "publishedDate"
            }]
            .as_str()
            .unwrap()
        })
        .collect();
    actual.sort_unstable();
    assert_eq!(actual, days);
}

#[tokio::test]
async fn native_http_real_openalex_routes_bounds_formats_and_cache_reacquisition() {
    // Every URI/method pair acquires each bound through actual OpenAlex. YAML
    // then reads the canonical JSON acquisition; two corrupt proofs reacquire.
    let fixture = Fixture::new(Mode::Audited, 200, equal_body(), Some(19)).await;
    let native = Native::start(&fixture).await;
    let mut equal_key = None;
    let mut equal_wire = String::new();
    let mut acquired = 0;
    for (index, (suffix, from, to, days)) in [
        (
            "lower_leap",
            Some("2024-02-29"),
            None,
            vec!["2024-02-29", "2024-03-01"],
        ),
        (
            "upper_leap",
            None,
            Some("2024-02-29"),
            vec!["2024-02-28", "2024-02-29"],
        ),
        (
            "equal_leap",
            Some("2024-02-29"),
            Some("2024-02-29"),
            vec!["2024-02-29"],
        ),
        (
            "both_span",
            Some("2024-02-28"),
            Some("2024-03-01"),
            vec!["2024-02-28", "2024-02-29", "2024-03-01"],
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let c = corpus();
        let row = c["records"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["id"].as_str().unwrap().ends_with(suffix))
            .unwrap();
        let wire = format!(
            "GET {} HTTP/1.1",
            row["requests"][0]["url"]
                .as_str()
                .unwrap()
                .strip_prefix("https://api.openalex.org")
                .unwrap()
        );
        let mut bound_key: Option<String> = None;
        for path in ["/", "/search"] {
            for method in ["GET", "POST"] {
                if let Some(key) = &bound_key {
                    let mut conn = fixture.connection().await;
                    let removed: u64 = redis::cmd("DEL")
                        .arg(key)
                        .query_async(&mut conn)
                        .await
                        .unwrap();
                    assert_eq!(
                        removed, 1,
                        "evict only this private fixture's known bound key"
                    );
                }
                let mut first = true;
                for format in ["json", "yaml"] {
                    let mut params = vec![
                        ("q", "date control / +"),
                        ("engines", "openalex"),
                        ("format", format),
                    ];
                    if let Some(v) = from {
                        params.push(("date_from", v));
                    }
                    if let Some(v) = to {
                        params.push(("date_to", v));
                    }
                    let before = fixture.keys("search:*").await;
                    let quota_before = fixture.counts.client.load(Ordering::SeqCst);
                    let (status, body) = native.request(path, method, &params).await;
                    assert_eq!(status, 200, "{path} {method} {format}: {body}");
                    assert_eq!(
                        fixture.counts.client.load(Ordering::SeqCst),
                        quota_before + 1,
                        "real HTTP peer must exercise client admission even on cache hits"
                    );
                    let value = decode(&body, format);
                    assert_http(&value, format, &days, &proof(from, to), !first);
                    // Bind projection to the pinned adapter fixture's actual URL/day pairs.
                    let date_field = if format == "yaml" {
                        "published"
                    } else {
                        "publishedDate"
                    };
                    let mut actual: Vec<_> = value["results"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|r| (r["url"].as_str().unwrap(), r[date_field].as_str().unwrap()))
                        .collect();
                    let mut source: Vec<_> = row["output"]["results"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|r| {
                            (
                                r["url"].as_str().unwrap(),
                                r["published_date"].as_str().unwrap(),
                            )
                        })
                        .collect();
                    actual.sort_unstable();
                    source.sort_unstable();
                    assert_eq!(actual, source);
                    let after = fixture.keys("search:*").await;
                    assert_eq!(after.len(), index + 1, "bound-specific canonical identity");
                    if first {
                        acquired += 1;
                        bound_key =
                            Some(after.iter().find(|k| !before.contains(k)).unwrap().clone());
                        assert_eq!(after.iter().filter(|k| !before.contains(k)).count(), 1);
                        if suffix == "equal_leap" {
                            equal_key =
                                Some(after.iter().find(|k| !before.contains(k)).unwrap().clone());
                            equal_wire = wire.clone();
                        }
                    } else {
                        assert_eq!(before, after);
                    }
                    assert_eq!(fixture.upstream.lines().len(), acquired);
                    assert_eq!(fixture.upstream.lines()[acquired - 1], wire);
                    first = false;
                }
            }
        }
    }
    let key = equal_key.unwrap();
    let mut conn = fixture.connection().await;
    let original: String = redis::cmd("GET")
        .arg(&key)
        .query_async(&mut conn)
        .await
        .unwrap();
    let original: ssx_json::Value = ssx_json::from_str(&original).unwrap();
    for (index, mutation) in ["missing", "mismatched"].into_iter().enumerate() {
        let mut corrupted = original.clone();
        if mutation == "missing" {
            corrupted
                .as_object_mut()
                .unwrap()
                .shift_remove("date_filter");
        } else {
            corrupted["date_filter"] = PublicationDateProof::openalex(
                Some("2024-02-28".into()),
                Some("2024-02-28".into()),
            )
            .unwrap()
            .to_value();
        }
        let _: () = redis::cmd("SET")
            .arg(&key)
            .arg(ssx_json::to_string(&corrupted).unwrap())
            .query_async(&mut conn)
            .await
            .unwrap();
        let params = [
            ("q", "date control / +"),
            ("engines", "openalex"),
            ("date_from", "2024-02-29"),
            ("date_to", "2024-02-29"),
            ("format", "json"),
        ];
        let (status, body) = native.request("/search", "GET", &params).await;
        assert_eq!(status, 200);
        assert_http(
            &decode(&body, "json"),
            "json",
            &["2024-02-29"],
            &proof(Some("2024-02-29"), Some("2024-02-29")),
            false,
        );
        assert_eq!(fixture.upstream.lines().len(), index + 17);
        assert_eq!(fixture.upstream.lines()[index + 16], equal_wire);
        let repaired: String = redis::cmd("GET")
            .arg(&key)
            .query_async(&mut conn)
            .await
            .unwrap();
        let repaired = storage::decode_response(ssx_json::from_str(&repaired).unwrap()).unwrap();
        assert_eq!(
            repaired.date_filter.unwrap().to_value(),
            ssx_json::Value::from(proof(Some("2024-02-29"), Some("2024-02-29")))
        );
        assert_eq!(repaired.scope.selected_engines, ["openalex"]);
        assert_eq!(
            repaired.results[0].published_date.as_deref(),
            Some("2024-02-29")
        );
        assert_eq!(fixture.keys("search:*").await.len(), 4);
    }
    // The same actual adapter still supports ordinary unbounded machine views.
    for format in ["json", "yaml", "csv", "rss"] {
        let (status, body) = native
            .request(
                "/search",
                "GET",
                &[
                    ("q", "date control / +"),
                    ("engines", "openalex"),
                    ("format", format),
                ],
            )
            .await;
        assert_eq!(status, 200, "{format}: {body}");
        if matches!(format, "json" | "yaml") {
            let value = decode(&body, format);
            assert!(value["meta"].get("date_filter").is_none());
            assert_eq!(value["results"].as_array().unwrap().len(), 7);
        } else {
            assert!(body.contains("2024-02-29"));
        }
    }
    assert_eq!(fixture.upstream.lines().len(), 19);
    assert_eq!(
        fixture.upstream.lines()[18],
        "GET /works?search=date%20control%20/%20%2B&per_page=10 HTTP/1.1"
    );
    assert_eq!(fixture.counts.adapter.load(Ordering::SeqCst), 19);
    drop(conn);
    native.close().await;
    fixture.close().await;
}

#[tokio::test]
async fn native_http_date_denials_have_zero_real_acquisition() {
    for (mode, label, changes, field) in [
        (
            Mode::Audited,
            "malformed",
            vec![("date_from", "2024-2-29")],
            "date_from",
        ),
        (
            Mode::Audited,
            "invalid_leap",
            vec![("date_from", "1900-02-29")],
            "date_from",
        ),
        (Mode::Audited, "empty", vec![("date_from", "")], "date_from"),
        (
            Mode::Audited,
            "reversed",
            vec![("date_from", "2024-03-01")],
            "date_from",
        ),
        (
            Mode::Audited,
            "relative",
            vec![("time_range", "week")],
            "time_range",
        ),
        (Mode::Audited, "implicit", vec![("engines", "")], "engines"),
        (
            Mode::Audited,
            "category",
            vec![("engines", ""), ("categories", "science")],
            "engines",
        ),
        (
            Mode::BroadAvailable,
            "mixed",
            vec![("engines", "openalex,brave")],
            "engines",
        ),
        (
            Mode::BroadAvailable,
            "unsupported",
            vec![("engines", "brave")],
            "engines",
        ),
        (Mode::Unaudited, "unaudited", vec![], "engines"),
        (Mode::Unavailable, "unavailable", vec![], "engines"),
        (
            Mode::Audited,
            "unknown",
            vec![("date_start", "2024-02-29")],
            "date_start",
        ),
        (
            Mode::Audited,
            "duplicate",
            vec![("date_from", "2024-02-29")],
            "date_from",
        ),
        (Mode::Audited, "csv", vec![("format", "csv")], "format"),
        (Mode::Audited, "rss", vec![("format", "rss")], "format"),
    ] {
        let fixture = Fixture::new(mode, 200, equal_body(), None).await;
        let native = Native::start(&fixture).await;
        for path in ["/", "/search"] {
            for method in ["GET", "POST"] {
                let mut params = vec![
                    ("q", "date control / +"),
                    ("engines", "openalex"),
                    ("date_from", "2024-02-29"),
                    ("date_to", "2024-02-29"),
                    ("format", "json"),
                ];
                for (key, value) in &changes {
                    if label != "duplicate" {
                        params.retain(|(k, _)| k != key);
                    }
                    params.push((*key, *value));
                }
                let (status, body) = native.request(path, method, &params).await;
                assert_eq!(status, 400, "{label} {path} {method}: {body}");
                if matches!(label, "csv" | "rss") {
                    assert!(
                        body.contains(
                            "Bounded publication-date proof requires json, yaml, or html format."
                        ),
                        "{label}: {body}"
                    );
                    if label == "csv" {
                        assert!(body.contains("invalid_filter"));
                    } else {
                        assert!(body.starts_with("<?xml"));
                    }
                } else {
                    let value: Value = serde_json::from_str(&body).unwrap();
                    assert_eq!(value["error"], "invalid_filter", "{label}: {body}");
                    assert_eq!(value["field"], field, "{label}: {body}");
                }
                assert_eq!(fixture.counts.observations(), [0; 4], "{label}");
                assert!(fixture.upstream.lines().is_empty(), "{label}");
                assert!(fixture.keys("*").await.is_empty(), "{label}");
            }
        }
        // URI plus form duplicates are one combined control stream.
        if label == "duplicate" {
            let (status, body) = native
                .request(
                    "/search?date_from=2024-02-29",
                    "POST",
                    &[
                        ("q", "date control / +"),
                        ("engines", "openalex"),
                        ("date_from", "2024-02-29"),
                        ("format", "json"),
                    ],
                )
                .await;
            assert_eq!(status, 400, "{body}");
            assert_eq!(fixture.counts.observations(), [0; 4]);
        }
        native.close().await;
        fixture.close().await;
    }
}

#[tokio::test]
async fn native_http_real_openalex_failure_and_empty_have_distinct_status_outcomes() {
    for (upstream, status, outcome) in [(200, 200, "ok"), (500, 503, "error")] {
        for format in ["json", "yaml"] {
            let fixture = Fixture::new(
                Mode::Audited,
                upstream,
                br#"{"results":[]}"#.to_vec(),
                Some(1),
            )
            .await;
            let native = Native::start(&fixture).await;
            let (actual, body) = native
                .request(
                    "/search",
                    "GET",
                    &[
                        ("q", "date control / +"),
                        ("engines", "openalex"),
                        ("date_from", "2024-02-29"),
                        ("date_to", "2024-02-29"),
                        ("format", format),
                    ],
                )
                .await;
            assert_eq!(actual, status, "{body}");
            let value = decode(&body, format);
            assert_eq!(
                value["meta"]["engine_status"]["openalex"]["status"],
                outcome
            );
            assert_eq!(
                value["meta"]["date_filter"],
                proof(Some("2024-02-29"), Some("2024-02-29"))
            );
            assert!(value["results"].as_array().unwrap().is_empty());
            assert_eq!(fixture.upstream.lines().len(), 1);
            assert_eq!(
                fixture.upstream.lines()[0],
                "GET /works?search=date%20control%20/%20%2B&per_page=10&filter=from_publication_date%3A2024-02-29%2Cto_publication_date%3A2024-02-29 HTTP/1.1"
            );
            native.close().await;
            fixture.close().await;
        }
    }
}
