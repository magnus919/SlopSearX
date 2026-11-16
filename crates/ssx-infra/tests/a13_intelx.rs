//! Static source-derived IntelX cases, not historical provider observations.
#[path = "a13_remaining_fixture/mod.rs"]
mod fixture;
use fixture::*;
use ssx_application::{AdmissionDecision, SearchContext};
use ssx_config::{ConfigScalar, Secret};
use ssx_domain::EngineStatus;
use std::time::{Duration, Instant};

#[tokio::test]
async fn intelx_success_two_requests_exact_delay() {
    let h = Harness::intelx(
        intelx_config(),
        vec![id_reply(), Reply::json(INTELX_BODY)],
        AdmissionDecision::Allowed,
    );
    whole(
        h.run(&query()).await,
        EngineStatus::Ok,
        vec![intelx_row()],
        None,
    );
    intelx_wire(
        &h.wire,
        "fiction@example.test",
        "10",
        Some(default_parameters()),
        false,
    );
    assert_eq!(*h.admission.names.lock().unwrap(), vec!["intelx"]);
    h.close().await;
    complete("intelx_success_two_requests_exact_delay");
}
#[tokio::test]
async fn intelx_falsey_ids_stop_before_delay() {
    for body in [
        b"{}".as_slice(),
        br#"{"id":""}"#,
        br#"{"id":null}"#,
        br#"{"id":false}"#,
        br#"{"id":0}"#,
        br#"{"id":[]}"#,
        br#"{"id":{}}"#,
    ] {
        let mut c = intelx_config();
        c.max_results = ConfigScalar::Text("invalid-slice".into());
        let h = Harness::intelx(c, vec![Reply::json(body)], AdmissionDecision::Allowed);
        whole(h.run(&query()).await, EngineStatus::Ok, vec![], None);
        intelx_wire(
            &h.wire,
            "fiction@example.test",
            "\"invalid-slice\"",
            None,
            false,
        );
        h.close().await;
    }
    complete("intelx_falsey_ids_stop_before_delay");
}
#[tokio::test]
async fn intelx_start_status_classification() {
    for status in [301, 401, 403, 404, 429, 500] {
        let h = Harness::intelx(
            intelx_config(),
            vec![Reply::status(status)],
            AdmissionDecision::Allowed,
        );
        let expected = match status {
            403 => EngineStatus::Blocked,
            429 => EngineStatus::RateLimited,
            _ => EngineStatus::Error,
        };
        let message =
            (!matches!(status, 403 | 429)).then(|| format!("IntelX returned HTTP {status}"));
        whole(h.run(&query()).await, expected, vec![], message.as_deref());
        intelx_wire(&h.wire, "fiction@example.test", "10", None, false);
        h.close().await;
    }
    complete("intelx_start_status_classification");
}
#[tokio::test]
async fn intelx_result_statuses_are_generic_errors() {
    for status in [301, 401, 403, 404, 429, 500] {
        let h = Harness::intelx(
            intelx_config(),
            vec![id_reply(), Reply::status(status)],
            AdmissionDecision::Allowed,
        );
        whole(
            h.run(&query()).await,
            EngineStatus::Error,
            vec![],
            Some(&format!("IntelX returned HTTP {status}")),
        );
        intelx_wire(
            &h.wire,
            "fiction@example.test",
            "10",
            Some(default_parameters()),
            false,
        );
        h.close().await;
    }
    complete("intelx_result_statuses_are_generic_errors");
}
#[tokio::test]
async fn intelx_start_shape_and_json_errors() {
    for body in [b"[]".as_slice(), b"null", b"17", br#""x""#] {
        let h = Harness::intelx(
            intelx_config(),
            vec![Reply::json(body)],
            AdmissionDecision::Allowed,
        );
        whole(
            h.run(&query()).await,
            EngineStatus::Error,
            vec![],
            Some("IntelX source value does not support field access"),
        );
        assert_eq!(h.wire.calls(), 1);
        h.close().await;
    }
    for body in [b"provider-secret".to_vec(), vec![b' '; 4 * 1024 * 1024 + 1]] {
        let h = Harness::intelx(
            intelx_config(),
            vec![Reply::json(&body)],
            AdmissionDecision::Allowed,
        );
        codec_error(h.run(&query()).await);
        assert_eq!(h.wire.calls(), 1);
        h.close().await;
    }
    let h = Harness::intelx(
        intelx_config(),
        vec![id_reply(), Reply::json(b"provider-secret")],
        AdmissionDecision::Allowed,
    );
    codec_error(h.run(&query()).await);
    assert_eq!(h.wire.calls(), 2);
    h.close().await;
    complete("intelx_start_shape_and_json_errors");
}
#[tokio::test]
async fn intelx_id_parameter_source_type_semantics() {
    for (id, pairs) in [
        ("true", vec![("id", "true"), ("limit", "10")]),
        ("12.5", vec![("id", "12.5"), ("limit", "10")]),
        (
            "[null,true,\"x y\",[1,false]]",
            vec![
                ("id", ""),
                ("id", "true"),
                ("id", "x y"),
                ("id", "[1, False]"),
                ("limit", "10"),
            ],
        ),
        ("{\"x\":1}", vec![("id", "{'x': 1}"), ("limit", "10")]),
    ] {
        let first = format!("{{\"id\":{id}}}");
        let h = Harness::intelx(
            intelx_config(),
            vec![Reply::json(first.as_bytes()), Reply::json(b"[]")],
            AdmissionDecision::Allowed,
        );
        whole(h.run(&query()).await, EngineStatus::Ok, vec![], None);
        intelx_wire(&h.wire, "fiction@example.test", "10", Some(pairs), false);
        h.close().await;
    }
    complete("intelx_id_parameter_source_type_semantics");
}
#[tokio::test]
async fn intelx_post_json_scalar_order_and_nonfinite_refusal() {
    for (max, json) in [
        (ConfigScalar::Boolean(true), "true"),
        (ConfigScalar::Float(2.), "2.0"),
        (ConfigScalar::Text("é\n\"".into()), "\"é\\n\\\"\""),
        (ConfigScalar::Float(1e16), "1e+16"),
    ] {
        let mut c = intelx_config();
        c.max_results = max;
        let h = Harness::intelx(c, vec![Reply::json(b"{}")], AdmissionDecision::Allowed);
        let mut q = query();
        q.query = "synthetic\n\"\\é\u{2028}".into();
        whole(h.run(&q).await, EngineStatus::Ok, vec![], None);
        intelx_wire(&h.wire, &q.query, json, None, false);
        h.close().await;
    }
    for max in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let mut c = intelx_config();
        c.max_results = ConfigScalar::Float(max);
        let h = Harness::intelx(c, vec![], AdmissionDecision::Allowed);
        whole(
            h.run(&query()).await,
            EngineStatus::Error,
            vec![],
            Some("IntelX request JSON requires finite numbers"),
        );
        assert_eq!(h.wire.calls(), 0);
        h.close().await;
    }
    complete("intelx_post_json_scalar_order_and_nonfinite_refusal");
}
#[tokio::test]
async fn intelx_configured_slices_and_invalid_indices() {
    let body = br#"[{"name":"One"},{"name":"Two"},{"name":"Three"}]"#;
    for (max, json, limit, end) in [
        (ConfigScalar::Integer(-1), "-1", "-1", 2),
        (ConfigScalar::Boolean(true), "true", "true", 1),
        (ConfigScalar::Boolean(false), "false", "false", 0),
    ] {
        let mut c = intelx_config();
        c.max_results = max;
        let h = Harness::intelx(
            c,
            vec![id_reply(), Reply::json(body)],
            AdmissionDecision::Allowed,
        );
        let names = ["One", "Two", "Three"];
        let rows = names[..end]
            .iter()
            .enumerate()
            .map(|(i, name)| row("intelx", "", name, "Type 0", i as i64 + 1, 0., None))
            .collect();
        whole(h.run(&query()).await, EngineStatus::Ok, rows, None);
        intelx_wire(
            &h.wire,
            "fiction@example.test",
            json,
            Some(vec![("id", "synthetic-search"), ("limit", limit)]),
            false,
        );
        h.close().await;
    }
    for (max, json, limit) in [
        (ConfigScalar::Text("2".into()), "\"2\"", "2"),
        (ConfigScalar::Float(2.), "2.0", "2.0"),
    ] {
        let mut c = intelx_config();
        c.max_results = max;
        let h = Harness::intelx(
            c,
            vec![id_reply(), Reply::json(b"[]")],
            AdmissionDecision::Allowed,
        );
        whole(
            h.run(&query()).await,
            EngineStatus::Error,
            vec![],
            Some("slice indices must be integers"),
        );
        intelx_wire(
            &h.wire,
            "fiction@example.test",
            json,
            Some(vec![("id", "synthetic-search"), ("limit", limit)]),
            false,
        );
        h.close().await;
    }
    complete("intelx_configured_slices_and_invalid_indices");
}
#[tokio::test]
async fn intelx_result_shape_normalization_before_slice() {
    for body in [
        b"{}".as_slice(),
        b"null",
        b"17",
        br#""unused\ud800""#,
        br#"{"records":null}"#,
        br#"{"records":{}}"#,
        br#"{"records":"not an array"}"#,
    ] {
        let h = Harness::intelx(
            intelx_config(),
            vec![id_reply(), Reply::json(body)],
            AdmissionDecision::Allowed,
        );
        whole(h.run(&query()).await, EngineStatus::Ok, vec![], None);
        assert_eq!(h.wire.calls(), 2);
        h.close().await;
    }
    let mut c = intelx_config();
    c.max_results = ConfigScalar::Text("invalid".into());
    let h = Harness::intelx(
        c,
        vec![id_reply(), Reply::json(br#"{"records":{}}"#)],
        AdmissionDecision::Allowed,
    );
    whole(
        h.run(&query()).await,
        EngineStatus::Error,
        vec![],
        Some("slice indices must be integers"),
    );
    h.close().await;
    complete("intelx_result_shape_normalization_before_slice");
}
#[tokio::test]
async fn intelx_wrapped_skipped_rows_preserve_positions() {
    let h=Harness::intelx(intelx_config(),vec![id_reply(),Reply::json(br#"[null,{"record":false},{"name":"One","type":1},7,{"record":{"systemid":"id","type":3}}]"#)],AdmissionDecision::Allowed);
    whole(
        h.run(&query()).await,
        EngineStatus::Ok,
        vec![
            row("intelx", "", "One", "Pastebin", 3, 0., None),
            row(
                "intelx",
                "https://intelx.io/?did=id",
                "id",
                "Document",
                5,
                0.,
                None,
            ),
        ],
        None,
    );
    h.close().await;
    complete("intelx_wrapped_skipped_rows_preserve_positions");
}
#[tokio::test]
async fn intelx_type_label_key_semantics_and_precedence() {
    for (value, label) in [
        ("true", "Pastebin"),
        ("1.0", "Pastebin"),
        ("false", "Type False"),
        ("null", "Type None"),
        ("\"1\"", "Type 1"),
        ("6", "Type 6"),
    ] {
        let body = format!("[{{\"name\":\"Synthetic\",\"type\":{value}}}]");
        let h = Harness::intelx(
            intelx_config(),
            vec![id_reply(), Reply::json(body.as_bytes())],
            AdmissionDecision::Allowed,
        );
        whole(
            h.run(&query()).await,
            EngineStatus::Ok,
            vec![row("intelx", "", "Synthetic", label, 1, 0., None)],
            None,
        );
        h.close().await;
    }
    for value in ["[]", "{}"] {
        let body = format!("[{{\"type\":{value},\"preview\":17}}]");
        let h = Harness::intelx(
            intelx_config(),
            vec![id_reply(), Reply::json(body.as_bytes())],
            AdmissionDecision::Allowed,
        );
        whole(
            h.run(&query()).await,
            EngineStatus::Error,
            vec![],
            Some("IntelX source label key is unhashable"),
        );
        h.close().await;
    }
    complete("intelx_type_label_key_semantics_and_precedence");
}
#[tokio::test]
async fn intelx_preview_bucket_and_title_operation_order() {
    for (body, message) in [
        (
            br#"[{"name":42,"preview":17}]"#.as_slice(),
            "IntelX source value does not support slicing",
        ),
        (
            br#"[{"systemid":{"x":1},"preview":[1],"bucket":17}]"#.as_slice(),
            "IntelX source value does not support slicing",
        ),
        (
            br#"[{"name":"Synthetic","systemid":{"x":1},"preview":[1]}]"#.as_slice(),
            "IntelX content join requires source strings",
        ),
        (
            br#"[{"name":42,"date":17}]"#.as_slice(),
            "IntelX source value has no length",
        ),
        (br#"[{"name":42}]"#.as_slice(), "expected source string"),
    ] {
        let h = Harness::intelx(
            intelx_config(),
            vec![id_reply(), Reply::json(body)],
            AdmissionDecision::Allowed,
        );
        whole(
            h.run(&query()).await,
            EngineStatus::Error,
            vec![],
            Some(message),
        );
        h.close().await;
    }
    let body = format!("[{{\"preview\":\"{}\\ud800\"}}]", "x".repeat(300));
    let h = Harness::intelx(
        intelx_config(),
        vec![id_reply(), Reply::json(body.as_bytes())],
        AdmissionDecision::Allowed,
    );
    whole(
        h.run(&query()).await,
        EngineStatus::Ok,
        vec![row(
            "intelx",
            "",
            "IntelX result 1",
            &format!("Type 0 | {}", "x".repeat(300)),
            1,
            0.,
            None,
        )],
        None,
    );
    h.close().await;
    complete("intelx_preview_bucket_and_title_operation_order");
}
#[tokio::test]
async fn intelx_id_url_fallback_and_dates() {
    let id = "s".repeat(40);
    let body = format!(
        "[{{\"systemid\":\"{id}\",\"date\":\"2024-02-03-extra\"}},{{\"name\":\"Named\",\"systemid\":42}},{{\"systemid\":[],\"date\":false}}]"
    );
    let h = Harness::intelx(
        intelx_config(),
        vec![id_reply(), Reply::json(body.as_bytes())],
        AdmissionDecision::Allowed,
    );
    whole(
        h.run(&query()).await,
        EngineStatus::Ok,
        vec![
            row(
                "intelx",
                &format!("https://intelx.io/?did={id}"),
                &"s".repeat(32),
                "Type 0",
                1,
                0.,
                Some("2024-02-03"),
            ),
            row(
                "intelx",
                "https://intelx.io/?did=42",
                "Named",
                "Type 0",
                2,
                0.,
                None,
            ),
            row("intelx", "", "IntelX result 3", "Type 0", 3, 0., None),
        ],
        None,
    );
    h.close().await;
    complete("intelx_id_url_fallback_and_dates");
}
#[tokio::test]
async fn intelx_late_failure_and_quota_atomicity() {
    // The companion private projection unit owns the tiny-quota/stored-row
    // witnesses; this integration control proves real two-request late failure.
    let h = Harness::intelx(
        intelx_config(),
        vec![
            id_reply(),
            Reply::json(br#"[{"name":"Valid"},{"preview":null}]"#),
        ],
        AdmissionDecision::Allowed,
    );
    whole(
        h.run(&query()).await,
        EngineStatus::Error,
        vec![],
        Some("IntelX source value does not support slicing"),
    );
    intelx_wire(
        &h.wire,
        "fiction@example.test",
        "10",
        Some(default_parameters()),
        false,
    );
    h.close().await;
    complete("intelx_late_failure_and_quota_atomicity");
}
#[tokio::test]
async fn intelx_header_redaction_settings_and_filters() {
    let defaults = ssx_infra::adapters::intelx::IntelXConfig::defaults();
    assert_eq!(defaults.timeout_ms, ConfigScalar::Integer(15000));
    assert!(!format!("{:?}", intelx_config()).contains(KEY));
    let raw = ssx_config::EngineConfig {
        base_url: " \t".into(),
        timeout_ms: ConfigScalar::Integer(15000),
        ..Default::default()
    };
    let resolved = ssx_infra::adapters::intelx::IntelXConfig::from_resolved(&raw);
    assert_eq!(resolved.base_url, ssx_infra::adapters::intelx::ENDPOINT);
    assert_eq!(resolved.timeout_ms, ConfigScalar::Integer(15000));
    for (decision, status, message) in [
        (
            AdmissionDecision::Allowed,
            EngineStatus::Error,
            "IntelX API key not configured (set ENGINE_INTELX_API_KEY)",
        ),
        (
            AdmissionDecision::Denied,
            EngineStatus::RateLimited,
            "rate limited",
        ),
        (
            AdmissionDecision::Unavailable,
            EngineStatus::Error,
            "intelx rate policy unavailable",
        ),
    ] {
        let mut c = intelx_config();
        c.api_key = Secret::new(None);
        let h = Harness::intelx(c, vec![], decision);
        let result = h.run(&query()).await;
        assert_eq!(result.latency_ms.get(), 0.);
        whole(result, status, vec![], Some(message));
        assert_eq!(h.wire.calls(), 0);
        h.close().await;
    }
    for key in ["κ", "bad\r\nkey"] {
        let mut c = intelx_config();
        c.api_key = Secret::new(Some(key.into()));
        let h = Harness::intelx(c, vec![], AdmissionDecision::Allowed);
        whole(
            h.run(&query()).await,
            EngineStatus::Error,
            vec![],
            Some("invalid IntelX credential configuration"),
        );
        assert_eq!(h.wire.calls(), 0);
        h.close().await;
    }
    let h = Harness::intelx(
        intelx_config(),
        vec![id_reply(), Reply::json(INTELX_BODY)],
        AdmissionDecision::Allowed,
    );
    assert!(!h.registry.descriptor("intelx").unwrap().sensitive);
    assert!(!h.registry.is_available("hibp"));
    let mut q = query();
    q.language = "fr".into();
    q.page = ssx_domain::SearchPage::from_u64(8).expect("positive fixture page");
    q.safesearch = 1;
    q.time_range = Some("week".into());
    q.media_type = Some("image".into());
    q.max_results = Some(1);
    whole(h.run(&q).await, EngineStatus::Ok, vec![intelx_row()], None);
    intelx_wire(&h.wire, &q.query, "10", Some(default_parameters()), false);
    h.close().await;
    let other = Harness::hibp(
        hibp_config(),
        vec![Reply::json(HIBP_BODY)],
        AdmissionDecision::Allowed,
    );
    whole(
        other.run(&query()).await,
        EngineStatus::Ok,
        vec![hibp_row()],
        None,
    );
    hibp_wire(&other.wire, "fiction@example.test");
    other.close().await;
    fixture::policy().await;
    complete("intelx_header_redaction_settings_and_filters");
}
#[tokio::test]
async fn intelx_one_session_cookie_continuity() {
    let mut first = id_reply();
    first.headers.insert(
        "set-cookie",
        "a13=synthetic-cookie; Path=/".parse().unwrap(),
    );
    let h = Harness::intelx(
        intelx_config(),
        vec![first, Reply::json(b"[]")],
        AdmissionDecision::Allowed,
    );
    whole(h.run(&query()).await, EngineStatus::Ok, vec![], None);
    intelx_wire(
        &h.wire,
        "fiction@example.test",
        "10",
        Some(default_parameters()),
        true,
    );
    h.close().await;
    complete("intelx_one_session_cookie_continuity");
}
#[tokio::test]
async fn intelx_fresh_phase_timeout_excludes_source_delay() {
    let mut c = intelx_config();
    c.timeout_ms = ConfigScalar::Integer(200);
    let h = Harness::intelx(
        c,
        vec![
            Reply::delayed(br#"{"id":"synthetic-search"}"#, Duration::from_millis(150)),
            Reply::delayed(b"[]", Duration::from_millis(150)),
        ],
        AdmissionDecision::Allowed,
    );
    let start = Instant::now();
    whole(h.run(&query()).await, EngineStatus::Ok, vec![], None);
    assert!(start.elapsed() >= Duration::from_millis(2300));
    intelx_wire(
        &h.wire,
        "fiction@example.test",
        "10",
        Some(default_parameters()),
        false,
    );
    h.close().await;
    complete("intelx_fresh_phase_timeout_excludes_source_delay");
}
#[tokio::test]
async fn intelx_caller_context_bounds_each_phase_and_delay() {
    let h = Harness::intelx(intelx_config(), vec![], AdmissionDecision::Allowed);
    let engine = h.registry.instantiate("intelx").unwrap();
    whole(
        engine
            .search_with_context(
                &query(),
                SearchContext {
                    deadline: Instant::now() - Duration::from_secs(1),
                },
            )
            .await,
        EngineStatus::Timeout,
        vec![],
        None,
    );
    assert!(h.admission.names.lock().unwrap().is_empty());
    h.close().await;
    for (first, second, deadline_ms, calls) in [
        (Reply::blocked(), Reply::json(b"[]"), 100, 1),
        (id_reply(), Reply::json(b"[]"), 200, 1),
        (id_reply(), Reply::blocked(), 2200, 2),
    ] {
        let h = Harness::intelx(
            intelx_config(),
            vec![first, second],
            AdmissionDecision::Allowed,
        );
        let engine = h.registry.instantiate("intelx").unwrap();
        whole(
            engine
                .search_with_context(
                    &query(),
                    SearchContext {
                        deadline: Instant::now() + Duration::from_millis(deadline_ms),
                    },
                )
                .await,
            EngineStatus::Timeout,
            vec![],
            None,
        );
        assert_eq!(h.wire.calls(), calls);
        h.close().await;
    }
    complete("intelx_caller_context_bounds_each_phase_and_delay");
}
#[tokio::test]
async fn intelx_cancel_post_delay_and_get() {
    for phase in [0, 1, 2] {
        let replies = match phase {
            0 => vec![Reply::blocked()],
            1 => vec![id_reply()],
            _ => vec![id_reply(), Reply::blocked()],
        };
        let h = Harness::intelx(intelx_config(), replies, AdmissionDecision::Allowed);
        let engine = h.registry.instantiate("intelx").unwrap();
        let q = query();
        let mut call = Box::pin(engine.search(&q));
        let expected = if phase == 2 { 2 } else { 1 };
        tokio::time::timeout(Duration::from_secs(4),async{tokio::select!{_=h.wire.entered(expected)=>{},_=&mut call=>panic!("must enter exact phase")}}).await.unwrap();
        if phase == 1 {
            tokio::select! {_=tokio::time::sleep(Duration::from_millis(100))=>{},_=&mut call=>panic!("must retain actual delay")};
            assert_eq!(h.wire.terminals(), 1);
        }
        drop(call);
        assert_eq!(h.wire.calls(), expected);
        assert_eq!(h.wire.terminals(), expected);
        if phase == 1 {
            tokio::time::sleep(Duration::from_millis(2100)).await;
            assert_eq!(h.wire.calls(), 1);
        }
        h.close().await;
    }
    complete("intelx_cancel_post_delay_and_get");
}
#[tokio::test]
async fn intelx_shutdown_entered_and_delayed_no_dispatch() {
    for phase in [0, 1, 2] {
        let replies = match phase {
            0 => vec![Reply::blocked()],
            1 => vec![id_reply()],
            _ => vec![id_reply(), Reply::blocked()],
        };
        let h = Harness::intelx(intelx_config(), replies, AdmissionDecision::Allowed);
        let engine = h.registry.instantiate("intelx").unwrap();
        let q = query();
        let mut call = Box::pin(engine.search(&q));
        let calls = if phase == 2 { 2 } else { 1 };
        tokio::time::timeout(Duration::from_secs(4),async{tokio::select!{_=h.wire.entered(calls)=>{},_=&mut call=>panic!("must reach intended phase")}}).await.unwrap();
        if phase == 1 {
            tokio::select! {_=tokio::time::sleep(Duration::from_millis(100))=>{},_=&mut call=>panic!("must retain actual delay")};
        }
        let (result, ()) = tokio::join!(&mut call, h.http.shutdown_and_wait());
        whole(
            result,
            EngineStatus::Error,
            vec![],
            Some("IntelX request failed"),
        );
        assert_eq!(h.wire.calls(), calls);
        h.close().await;
    }
    complete("intelx_shutdown_entered_and_delayed_no_dispatch");
}
#[cfg(unix)]
#[tokio::test]
async fn intelx_private_literal_two_phase_http() {
    fixture::literal("intelx", "intelx_private_literal_two_phase_http").await;
    complete("intelx_private_literal_two_phase_http");
}
#[test]
fn intelx_static_source_inventory_closed() {
    fixture::inventory("intelx");
    complete("intelx_static_source_inventory_closed");
}
