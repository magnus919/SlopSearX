//! Static source-derived HIBP cases; fabricated identities only.
#[path = "a13_remaining_fixture/mod.rs"]
mod fixture;
use fixture::*;
use ssx_application::{AdmissionDecision, SearchContext};
use ssx_config::{ConfigScalar, Secret};
use ssx_domain::EngineStatus;
use std::time::{Duration, Instant};

#[tokio::test]
async fn hibp_source_success_complete_outbound() {
    let h = Harness::hibp(
        hibp_config(),
        vec![Reply::json(HIBP_BODY)],
        AdmissionDecision::Allowed,
    );
    whole(
        h.run(&query()).await,
        EngineStatus::Ok,
        vec![hibp_row()],
        None,
    );
    hibp_wire(&h.wire, "fiction@example.test");
    assert_eq!(*h.admission.names.lock().unwrap(), vec!["hibp"]);
    h.close().await;
    complete("hibp_source_success_complete_outbound");
}
#[tokio::test]
async fn hibp_404_empty_before_json() {
    let h = Harness::hibp(
        hibp_config(),
        vec![Reply::status(404)],
        AdmissionDecision::Allowed,
    );
    whole(h.run(&query()).await, EngineStatus::Ok, vec![], None);
    hibp_wire(&h.wire, "fiction@example.test");
    h.close().await;
    complete("hibp_404_empty_before_json");
}
#[tokio::test]
async fn hibp_missing_key_and_admission_precedence() {
    for (decision, status, message) in [
        (
            AdmissionDecision::Allowed,
            EngineStatus::Error,
            "HIBP API key not configured (set ENGINE_HIBP_API_KEY)",
        ),
        (
            AdmissionDecision::Denied,
            EngineStatus::RateLimited,
            "rate limited",
        ),
        (
            AdmissionDecision::Unavailable,
            EngineStatus::Error,
            "hibp rate policy unavailable",
        ),
    ] {
        let mut c = hibp_config();
        c.api_key = Secret::new(None);
        c.base_url = "not a URL".into();
        let h = Harness::hibp(c, vec![], decision);
        let result = h.run(&query()).await;
        assert_eq!(result.latency_ms.get(), 0.);
        whole(result, status, vec![], Some(message));
        assert_eq!(h.wire.calls(), 0);
        assert_eq!(*h.admission.names.lock().unwrap(), vec!["hibp"]);
        h.close().await;
    }
    complete("hibp_missing_key_and_admission_precedence");
}
#[tokio::test]
async fn hibp_auth_redaction_and_resolved_configuration() {
    let raw = ssx_config::EngineConfig {
        base_url: " \t\n".into(),
        api_key: Secret::new(Some(KEY.into())),
        ..Default::default()
    };
    let c = ssx_infra::adapters::hibp::HibpConfig::from_resolved(
        &raw,
        ssx_infra::adapters::hibp::ENDPOINT,
    );
    assert_eq!(c.base_url, ssx_infra::adapters::hibp::ENDPOINT);
    assert!(!format!("{c:?}").contains(KEY));
    for key in ["κ", "bad\r\nkey"] {
        let mut c = hibp_config();
        c.api_key = Secret::new(Some(key.into()));
        let h = Harness::hibp(c, vec![], AdmissionDecision::Allowed);
        whole(
            h.run(&query()).await,
            EngineStatus::Error,
            vec![],
            Some("invalid HIBP credential configuration"),
        );
        assert_eq!(h.wire.calls(), 0);
        h.close().await;
    }
    let h = Harness::hibp(
        hibp_config(),
        vec![Reply::json(b"[]")],
        AdmissionDecision::Allowed,
    );
    assert!(h.registry.is_available("hibp"));
    assert!(!h.registry.is_available("dehashed"));
    assert!(!h.registry.is_available("intelx"));
    assert!(h.registry.descriptor("hibp").unwrap().sensitive);
    whole(h.run(&query()).await, EngineStatus::Ok, vec![], None);
    hibp_wire(&h.wire, "fiction@example.test");
    h.close().await;
    complete("hibp_auth_redaction_and_resolved_configuration");
}
#[tokio::test]
async fn hibp_email_first_match_word_boundaries_and_case() {
    for (q, account) in [
        (
            "Prefix First@EXAMPLE.test then second@example.test",
            "First@EXAMPLE.test",
        ),
        ("+Name@EXAMPLE.test", "Name@EXAMPLE.test"),
        ("é+Name@EXAMPLE.test", "+Name@EXAMPLE.test"),
        ("éName@EXAMPLE.test", "éname@example.test"),
        ("name@example.testé", "name@example.testé"),
        ("name@example.test\u{301}", "name@example.test"),
        ("a@x.com.more", "a@x.com.more"),
        ("a@x.com.", "a@x.com"),
    ] {
        let h = Harness::hibp(
            hibp_config(),
            vec![Reply::json(b"[]")],
            AdmissionDecision::Allowed,
        );
        let mut request = query();
        request.query = q.into();
        whole(h.run(&request).await, EngineStatus::Ok, vec![], None);
        let expected = reqwest::Url::parse(&format!(
            "http://127.0.0.1:1/injected/breachedaccount/{account}"
        ))
        .unwrap();
        {
            let requests = h.wire.requests.lock().unwrap();
            assert_eq!(requests[0].url, expected);
            headers(&requests[0], "hibp", false, false);
        }
        h.close().await;
    }
    complete("hibp_email_first_match_word_boundaries_and_case");
}
#[tokio::test]
async fn hibp_fallback_strip_unicode16_lowercase() {
    for (q, account) in [
        (" \u{1c}ΟΣ\u{1f}", "ος"),
        ("İ", "i\u{307}"),
        ("AΣ\u{345}", "aς\u{345}"),
        (" AΣ'A ", "aσ'a"),
        ("ß", "ß"),
        ("\u{1c89}", "\u{1c8a}"),
    ] {
        let h = Harness::hibp(
            hibp_config(),
            vec![Reply::json(b"[]")],
            AdmissionDecision::Allowed,
        );
        let mut request = query();
        request.query = q.into();
        whole(h.run(&request).await, EngineStatus::Ok, vec![], None);
        assert_eq!(
            h.wire.requests.lock().unwrap()[0].url,
            reqwest::Url::parse(&format!(
                "http://127.0.0.1:1/injected/breachedaccount/{account}"
            ))
            .unwrap()
        );
        h.close().await;
    }
    complete("hibp_fallback_strip_unicode16_lowercase");
}
#[tokio::test]
async fn hibp_description_prefix_and_source_type_errors() {
    let body = format!(
        "[{{\"Name\":\"Synthetic\",\"Description\":\"{}\\ud800\"}}]",
        "x".repeat(300)
    );
    let h = Harness::hibp(
        hibp_config(),
        vec![Reply::json(body.as_bytes())],
        AdmissionDecision::Allowed,
    );
    whole(
        h.run(&query()).await,
        EngineStatus::Ok,
        vec![row(
            "hibp",
            "https://haveibeenpwned.com/breaches/Synthetic",
            "Synthetic",
            &"x".repeat(300),
            1,
            0.,
            None,
        )],
        None,
    );
    h.close().await;
    for (raw, message) in [
        (
            br#"[{"Description":null,"PwnCount":"wrong"}]"#.as_slice(),
            "HIBP source value does not support slicing",
        ),
        (br#"[{"Description":[],"Name":"Synthetic"}]"#.as_slice(), ""),
        (
            br#"[{"Description":["x"],"PwnCount":0}]"#.as_slice(),
            "HIBP content join requires source strings",
        ),
    ] {
        let h = Harness::hibp(
            hibp_config(),
            vec![Reply::json(raw)],
            AdmissionDecision::Allowed,
        );
        if message.is_empty() {
            whole(
                h.run(&query()).await,
                EngineStatus::Ok,
                vec![row(
                    "hibp",
                    "https://haveibeenpwned.com/breaches/Synthetic",
                    "Synthetic",
                    "",
                    1,
                    0.,
                    None,
                )],
                None,
            );
        } else {
            whole(
                h.run(&query()).await,
                EngineStatus::Error,
                vec![],
                Some(message),
            );
        }
        h.close().await;
    }
    complete("hibp_description_prefix_and_source_type_errors");
}
#[tokio::test]
async fn hibp_grouped_count_numeric_and_error_order() {
    for (count, text, score) in [
        ("true", "1", 0.000001),
        ("1234567", "1,234,567", 1.234567),
        ("-1234", "-1,234", -0.001234),
        ("1234.5", "1,234.5", 0.0012345),
    ] {
        let body = format!("[{{\"Name\":\"Synthetic\",\"PwnCount\":{count}}}]");
        let h = Harness::hibp(
            hibp_config(),
            vec![Reply::json(body.as_bytes())],
            AdmissionDecision::Allowed,
        );
        whole(
            h.run(&query()).await,
            EngineStatus::Ok,
            vec![row(
                "hibp",
                "https://haveibeenpwned.com/breaches/Synthetic",
                "Synthetic",
                &format!("Accounts: {text}"),
                1,
                score,
                None,
            )],
            None,
        );
        h.close().await;
    }
    let h = Harness::hibp(
        hibp_config(),
        vec![Reply::json(
            br#"[{"PwnCount":"1","DataClasses":[1],"BreachDate":null}]"#,
        )],
        AdmissionDecision::Allowed,
    );
    whole(
        h.run(&query()).await,
        EngineStatus::Error,
        vec![],
        Some("HIBP account count does not support numeric grouping"),
    );
    h.close().await;
    complete("hibp_grouped_count_numeric_and_error_order");
}
#[tokio::test]
async fn hibp_data_classes_slice_and_text_join() {
    for (value, text) in [
        ("\"abcdef\"", "a, b, c, d, e"),
        ("[\"a\",\"b\",\"c\",\"d\",\"e\",17]", "a, b, c, d, e"),
    ] {
        let body = format!("[{{\"Name\":\"Synthetic\",\"DataClasses\":{value}}}]");
        let h = Harness::hibp(
            hibp_config(),
            vec![Reply::json(body.as_bytes())],
            AdmissionDecision::Allowed,
        );
        whole(
            h.run(&query()).await,
            EngineStatus::Ok,
            vec![row(
                "hibp",
                "https://haveibeenpwned.com/breaches/Synthetic",
                "Synthetic",
                &format!("Data: {text}"),
                1,
                0.,
                None,
            )],
            None,
        );
        h.close().await;
    }
    for (raw, message) in [
        (
            br#"[{"Description":[1],"DataClasses":[1]}]"#.as_slice(),
            "HIBP data class join requires source strings",
        ),
        (
            br#"[{"DataClasses":{"x":1}}]"#.as_slice(),
            "HIBP source value does not support slicing",
        ),
    ] {
        let h = Harness::hibp(
            hibp_config(),
            vec![Reply::json(raw)],
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
    complete("hibp_data_classes_slice_and_text_join");
}
#[tokio::test]
async fn hibp_source_operations_before_typed_projection() {
    for (raw, message) in [
        (
            br#"[{"Name":42,"PwnCount":"bad"}]"#.as_slice(),
            "HIBP account count does not support numeric grouping",
        ),
        (
            br#"[{"Name":42,"Description":["x"],"BreachDate":null}]"#.as_slice(),
            "HIBP content join requires source strings",
        ),
        (
            br#"[{"Name":42,"BreachDate":null}]"#.as_slice(),
            "HIBP source value has no length",
        ),
        (br#"[{"Name":42}]"#.as_slice(), "expected source string"),
    ] {
        let h = Harness::hibp(
            hibp_config(),
            vec![Reply::json(raw)],
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
    complete("hibp_source_operations_before_typed_projection");
}
#[tokio::test]
async fn hibp_dates_unsliced_and_positions() {
    let h = Harness::hibp(hibp_config(), vec![Reply::json(br#"[{"Name":"One","BreachDate":"2024-01-02-extra"},{"Name":"Two","BreachDate":"short"}]"#)], AdmissionDecision::Allowed);
    whole(
        h.run(&query()).await,
        EngineStatus::Ok,
        vec![
            row(
                "hibp",
                "https://haveibeenpwned.com/breaches/One",
                "One",
                "",
                1,
                0.,
                Some("2024-01-02-extra"),
            ),
            row(
                "hibp",
                "https://haveibeenpwned.com/breaches/Two",
                "Two",
                "",
                2,
                0.,
                None,
            ),
        ],
        None,
    );
    h.close().await;
    complete("hibp_dates_unsliced_and_positions");
}
#[tokio::test]
async fn hibp_wrong_shapes_and_late_atomicity() {
    for body in [
        b"{}".as_slice(),
        b"null",
        b"false",
        br#""unused\ud800""#,
        b"[]",
    ] {
        let h = Harness::hibp(
            hibp_config(),
            vec![Reply::json(body)],
            AdmissionDecision::Allowed,
        );
        whole(h.run(&query()).await, EngineStatus::Ok, vec![], None);
        h.close().await;
    }
    let h = Harness::hibp(
        hibp_config(),
        vec![Reply::json(br#"[{"Name":"Valid"},false]"#)],
        AdmissionDecision::Allowed,
    );
    whole(
        h.run(&query()).await,
        EngineStatus::Error,
        vec![],
        Some("HIBP response row does not support field access"),
    );
    h.close().await;
    complete("hibp_wrong_shapes_and_late_atomicity");
}
#[tokio::test]
async fn hibp_configured_max_and_request_filters_ignored() {
    let mut c = hibp_config();
    c.max_results = ConfigScalar::Text("not a slice".into());
    let h = Harness::hibp(c, vec![Reply::json(HIBP_BODY)], AdmissionDecision::Allowed);
    let mut q = query();
    q.max_results = Some(0);
    q.page = ssx_domain::SearchPage::from_u64(7).expect("positive fixture page");
    q.language = "fr".into();
    q.safesearch = 1;
    q.time_range = Some("week".into());
    q.media_type = Some("image".into());
    q.date_from = Some("2024-01-01".into());
    whole(h.run(&q).await, EngineStatus::Ok, vec![hibp_row()], None);
    hibp_wire(&h.wire, "fiction@example.test");
    h.close().await;
    complete("hibp_configured_max_and_request_filters_ignored");
}
#[tokio::test]
async fn hibp_http_transport_url_and_codec_failures() {
    for status in [301, 401, 403, 429, 500] {
        let h = Harness::hibp(
            hibp_config(),
            vec![Reply::status(status)],
            AdmissionDecision::Allowed,
        );
        let expected = match status {
            403 => EngineStatus::Blocked,
            429 => EngineStatus::RateLimited,
            _ => EngineStatus::Error,
        };
        let message =
            (!matches!(status, 403 | 429)).then(|| format!("HIBP returned HTTP {status}"));
        whole(h.run(&query()).await, expected, vec![], message.as_deref());
        h.close().await;
    }
    for (error, status, message) in [
        (
            ssx_infra::http::Error::Deadline,
            EngineStatus::Timeout,
            None,
        ),
        (
            ssx_infra::http::Error::Network,
            EngineStatus::Error,
            Some("HIBP request failed"),
        ),
    ] {
        let h = Harness::hibp(
            hibp_config(),
            vec![Reply {
                error: Some(error),
                ..Reply::json(b"[]")
            }],
            AdmissionDecision::Allowed,
        );
        whole(h.run(&query()).await, status, vec![], message);
        h.close().await;
    }
    let mut c = hibp_config();
    c.base_url = "not URL".into();
    let h = Harness::hibp(c, vec![], AdmissionDecision::Allowed);
    whole(
        h.run(&query()).await,
        EngineStatus::Error,
        vec![],
        Some("invalid HIBP request URL"),
    );
    assert_eq!(h.wire.calls(), 0);
    h.close().await;
    for body in [b"provider-secret".to_vec(), vec![b' '; 4 * 1024 * 1024 + 1]] {
        let h = Harness::hibp(
            hibp_config(),
            vec![Reply::json(&body)],
            AdmissionDecision::Allowed,
        );
        codec_error(h.run(&query()).await);
        h.close().await;
    }
    complete("hibp_http_transport_url_and_codec_failures");
}
#[tokio::test]
async fn hibp_entered_cancel_and_http_reuse() {
    let h = Harness::hibp(
        hibp_config(),
        vec![Reply::blocked(), Reply::json(b"[]")],
        AdmissionDecision::Allowed,
    );
    let engine = h.registry.instantiate("hibp").unwrap();
    let q = query();
    let mut call = Box::pin(engine.search(&q));
    tokio::time::timeout(Duration::from_secs(3), async { tokio::select! { _ = h.wire.entered(1) => {}, _ = &mut call => panic!("must enter actual transport") } }).await.unwrap();
    assert_eq!(h.wire.terminals(), 0);
    drop(call);
    assert_eq!(h.wire.terminals(), 1);
    whole(h.run(&q).await, EngineStatus::Ok, vec![], None);
    assert_eq!(h.wire.calls(), 2);
    h.close().await;
    complete("hibp_entered_cancel_and_http_reuse");
}
#[tokio::test]
async fn hibp_entered_shutdown_rejects_reuse() {
    let h = Harness::hibp(
        hibp_config(),
        vec![Reply::blocked()],
        AdmissionDecision::Allowed,
    );
    let engine = h.registry.instantiate("hibp").unwrap();
    let q = query();
    let mut call = Box::pin(engine.search(&q));
    tokio::time::timeout(Duration::from_secs(3), async { tokio::select! { _ = h.wire.entered(1) => {}, _ = &mut call => panic!("must enter actual transport") } }).await.unwrap();
    let (result, ()) = tokio::join!(&mut call, h.http.shutdown_and_wait());
    whole(
        result,
        EngineStatus::Error,
        vec![],
        Some("HIBP request failed"),
    );
    assert_eq!(h.wire.terminals(), 1);
    assert!(
        h.http
            .session(
                Default::default(),
                tokio::time::Instant::now() + Duration::from_secs(1),
                Default::default()
            )
            .is_err()
    );
    h.close().await;
    complete("hibp_entered_shutdown_rejects_reuse");
}
#[tokio::test]
async fn hibp_caller_and_configured_deadlines() {
    let h = Harness::hibp(hibp_config(), vec![], AdmissionDecision::Allowed);
    let engine = h.registry.instantiate("hibp").unwrap();
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
    for (configured, caller) in [(10000, 60), (60000, 10)] {
        let mut c = hibp_config();
        c.timeout_ms = ConfigScalar::Integer(configured);
        let h = Harness::hibp(
            c,
            vec![Reply::blocked(), Reply::delayed(b"[]", Duration::ZERO)],
            AdmissionDecision::Allowed,
        );
        let engine = h.registry.instantiate("hibp").unwrap();
        let q = query();
        let start = Instant::now();
        let mut call = Box::pin(engine.search_with_context(
            &q,
            SearchContext {
                deadline: start + Duration::from_secs(caller),
            },
        ));
        tokio::time::timeout(Duration::from_secs(3), async { tokio::select! { _ = h.wire.entered(1) => {}, _ = &mut call => panic!("deadline must reach transport") } }).await.unwrap();
        let result = tokio::time::timeout(Duration::from_secs(15), &mut call)
            .await
            .unwrap();
        assert!(start.elapsed() >= Duration::from_secs(10));
        whole(result, EngineStatus::Timeout, vec![], None);
        drop(call);
        assert_eq!(h.wire.terminals(), 1);
        whole(h.run(&q).await, EngineStatus::Ok, vec![], None);
        h.close().await;
    }
    complete("hibp_caller_and_configured_deadlines");
}
#[tokio::test]
async fn hibp_real_policy_and_mixed_intelx_atomicity() {
    fixture::policy().await;
    complete("hibp_real_policy_and_mixed_intelx_atomicity");
}
#[cfg(unix)]
#[tokio::test]
async fn hibp_private_literal_http_contract() {
    fixture::literal("hibp", "hibp_private_literal_http_contract").await;
    complete("hibp_private_literal_http_contract");
}
#[test]
fn hibp_static_source_inventory_closed() {
    fixture::inventory("hibp");
    complete("hibp_static_source_inventory_closed");
}
