use ssx_application::{PortFuture, health::*, rate::*, retry_after::retry_after};
use ssx_domain::{
    AdapterResponse, EngineStatus, FiniteF64, SearchResult, ValkeyQuotaTime, WallClockTimestamp,
    rate::*,
};
use ssx_json::{Value, json};
use std::{
    collections::VecDeque,
    future::Future,
    sync::{Arc, Mutex},
    task::{Context, Poll, Waker},
};
fn corpus() -> Value {
    ssx_json::from_str(include_str!("../../xtask/fixtures/f10/corpus.json")).unwrap()
}
fn now(t: f64) -> WallClockTimestamp {
    WallClockTimestamp::new(t).unwrap()
}
fn ready<F: Future>(future: F) -> F::Output {
    let mut p = std::pin::pin!(future);
    match p.as_mut().poll(&mut Context::from_waker(Waker::noop())) {
        Poll::Ready(v) => v,
        Poll::Pending => panic!("pure fixture future unexpectedly pending"),
    }
}
#[derive(Default)]
struct Cell {
    allowed: VecDeque<bool>,
    shared: f64,
    calls: u64,
    failure: Option<RateFailure>,
}
#[derive(Default)]
struct Mock(Mutex<Cell>);
impl RateStore for Mock {
    fn acquire<'a>(
        &'a self,
        _: &'a EngineKey,
        _: u64,
    ) -> PortFuture<'a, Result<QuotaReceipt, RateFailure>> {
        Box::pin(async {
            let mut c = self.0.lock().unwrap();
            c.calls += 1;
            if let Some(f) = &c.failure {
                return Err(f.clone());
            }
            Ok(QuotaReceipt {
                allowed: c.allowed.pop_front().unwrap(),
                count: 0,
                bucket_key: "fixture".into(),
                server_time: ValkeyQuotaTime::from_milliseconds(0),
                remaining_ttl_ms: 1,
            })
        })
    }
    fn cooldown<'a>(
        &'a self,
        _: &'a EngineKey,
    ) -> PortFuture<'a, Result<SharedCooldown, RateFailure>> {
        Box::pin(async {
            let c = self.0.lock().unwrap();
            if let Some(f) = &c.failure {
                return Err(f.clone());
            }
            Ok(if c.shared > 0.0 {
                SharedCooldown::Active {
                    seconds: c.shared as u64,
                }
            } else {
                SharedCooldown::Missing
            })
        })
    }
    fn publish_cooldown<'a>(
        &'a self,
        _: &'a EngineKey,
        s: f64,
    ) -> PortFuture<'a, Result<u64, RateFailure>> {
        Box::pin(async move {
            self.0.lock().unwrap().shared = s;
            Ok(s.ceil() as u64)
        })
    }
    fn warmup(&self) -> PortFuture<'_, Result<(), RateFailure>> {
        Box::pin(async { Ok(()) })
    }
    fn shutdown(&self) -> PortFuture<'_, ()> {
        Box::pin(async {})
    }
}
#[test]
fn source_local_bucket_backpressure_and_registry_contracts() {
    let data = corpus();
    for name in data["registry"].as_array().unwrap() {
        EngineKey::new(name.as_str().unwrap()).unwrap();
    }
    assert_eq!(data["registry"].as_array().unwrap().len(), 51);
    for case in data["local"].as_array().unwrap() {
        let mut b = LocalTokenBucket::new(
            case["rate"].as_f64().unwrap(),
            case["burst"].as_f64().unwrap(),
        )
        .unwrap();
        for (op, expected) in case["operations"]
            .as_array()
            .unwrap()
            .iter()
            .zip(case["expected"].as_array().unwrap())
        {
            assert_eq!(
                b.acquire(
                    &EngineKey::new(op[1].as_str().unwrap()).unwrap(),
                    op[2].as_u64().unwrap(),
                    op[0].as_f64().unwrap()
                )
                .unwrap(),
                expected.as_bool().unwrap()
            );
        }
    }
    let engine = EngineKey::new("fakeeng").unwrap();
    for case in data["sequences"].as_array().unwrap() {
        let mock = Arc::new(Mock::default());
        mock.0.lock().unwrap().allowed = case["allowed"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_bool().unwrap())
            .collect();
        let mut limiter = RateLimiter::new(mock.clone(), false);
        for (op, expected) in case["operations"]
            .as_array()
            .unwrap()
            .iter()
            .zip(case["outputs"].as_array().unwrap())
        {
            let kind = op.as_str().unwrap_or_else(|| op[0].as_str().unwrap());
            let value = match kind {
                "acquire" => json!(ready(limiter.acquire(&engine, 1, 100.0)).unwrap().allowed),
                "publish" => json!(
                    ready(limiter.set_upstream_cooldown(&engine, op[1].as_f64().unwrap(), 100.0))
                        .unwrap()
                        .0
                ),
                "remaining" => json!(
                    ready(limiter.upstream_cooldown_remaining(&engine, 100.0))
                        .unwrap()
                        .remaining_seconds
                ),
                "reactivate" => {
                    limiter.reactivate(&engine);
                    Value::Null
                }
                "shared" => {
                    mock.0.lock().unwrap().shared = op[1].as_f64().unwrap();
                    Value::Null
                }
                _ => panic!("unowned source op"),
            };
            assert_eq!(value, expected["value"], "{}", case["name"]);
            let state=limiter.state(&engine).map(|s|json!({"consecutive_failures":s.consecutive_denials,"cooldown_until":s.cooldown_until,"upstream_cooldown_until":s.upstream_cooldown_until,"deactivated":s.deactivated}));
            assert_eq!(ssx_json::to_value(state).unwrap(), expected["state"]);
            assert_eq!(
                mock.0.lock().unwrap().calls,
                expected["strategy_calls"].as_u64().unwrap()
            );
            assert_eq!(
                limiter.deactivated_engines().len(),
                expected["deactivated"].as_array().unwrap().len()
            );
        }
    }
}
fn response(status: &str, latency: f64, count: u64, synthetic: bool) -> AdapterResponse {
    AdapterResponse {
        status: ssx_json::from_value(json!(status)).unwrap(),
        latency_ms: FiniteF64::new(latency).unwrap(),
        results: (0..count).map(|_| SearchResult::default()).collect(),
        synthetic,
        ..Default::default()
    }
}
#[test]
fn source_actual_service_health_and_freshness_views() {
    let data = corpus();
    for case in data["health"].as_array().unwrap() {
        let mut state = HealthState::default();
        for _ in 0..case["initial_errors"].as_u64().unwrap() {
            state.record_failure(now(case["now"].as_f64().unwrap()));
        }
        let origin = match case["origin"].as_str().unwrap() {
            "adapter" => OutcomeOrigin::AdapterReturned,
            "synthetic" => OutcomeOrigin::DispatchedSyntheticTimeout,
            "caller_cancelled" => OutcomeOrigin::CallerCancelled,
            "undispatched" => OutcomeOrigin::Undispatched,
            _ => panic!(),
        };
        state.observe(
            origin,
            &response(
                case["status"].as_str().unwrap(),
                case["latency"].as_f64().unwrap(),
                case["result_count"].as_u64().unwrap(),
                case["synthetic"].as_bool().unwrap(),
            ),
            now(case["now"].as_f64().unwrap()),
        );
        assert_eq!(
            build_engine_health(
                Some(&state),
                None,
                now(case["now"].as_f64().unwrap()),
                300.0
            )
            .unwrap(),
            case["expected"],
            "{}",
            case["name"]
        );
    }
    for case in data["views"].as_array().unwrap() {
        let mut s = HealthState::default();
        s.observe(
            OutcomeOrigin::AdapterReturned,
            &response("ok", 3.25, 2, false),
            now(case["at"].as_f64().unwrap()),
        );
        let c = &case["context"];
        let context = HealthContext {
            configured: c[0].as_bool().unwrap(),
            auth_class: c[1].as_str().map(str::to_owned),
            auth_configured: c[2].as_bool(),
        };
        assert_eq!(
            build_engine_health(
                Some(&s),
                Some(&context),
                now(case["now"].as_f64().unwrap()),
                300.0
            )
            .unwrap(),
            case["expected"]
        );
    }
    let empty = build_engine_health(None, None, now(0.0), 300.0).unwrap();
    assert_eq!(empty["status"], "unknown");
    assert_eq!(empty["configured"], false);
    assert!(empty["last_observed_latency_ms"].is_null());
    assert!(empty["auth_configured"].is_null());
}
#[test]
fn source_circuit_boundary_and_retry_after_codec() {
    let data = corpus();
    for case in data["circuit"].as_array().unwrap() {
        let mut state = HealthState::new(
            case["threshold"].as_i64().unwrap(),
            case["timeout"].as_i64().unwrap(),
        );
        for op in case["outputs"].as_array().unwrap() {
            let t = now(op["now"].as_f64().unwrap());
            match op["operation"].as_str().unwrap() {
                "failure" => state.record_failure(t),
                "success" => state.record_success(),
                _ => {}
            }
            assert_eq!(state.consecutive_errors, op["errors"].as_u64().unwrap());
            assert_eq!(state.circuit_open_until, op["open_until"].as_f64().unwrap());
            assert_eq!(state.circuit_allowed(t), op["allowed"].as_bool().unwrap());
            assert_eq!(state.circuit_open(t), op["open"].as_bool().unwrap());
        }
    }
    for case in data["retry_after"].as_array().unwrap() {
        assert_eq!(
            retry_after(case["input"].as_str(), case["now"].as_f64().unwrap()),
            case["expected"].as_f64().unwrap(),
            "{:?}",
            case["input"]
        );
    }
    for case in data["bounds"].as_array().unwrap() {
        assert_eq!(
            bound_provider_cooldown(case["input"].as_f64().unwrap()),
            case["expected"].as_f64().unwrap()
        );
    }
}
#[test]
fn invalid_input_and_unavailable_are_explicit_and_never_healthy() {
    for name in ["", "a:b", "../x", "a b", "a\n", "é", "upstream-cooldown"] {
        assert!(EngineKey::new(name).is_err());
    }
    for (rate, window) in [
        (-1.0, 1.0),
        (f64::NAN, 1.0),
        (f64::INFINITY, 1.0),
        (1.0, 0.0),
        (1.0, -1.0),
        (1.0, f64::NAN),
        (1.0, 0.0001),
    ] {
        assert!(QuotaConfig::new(rate, window).is_err());
    }
    let engine = EngineKey::new("fixture").unwrap();
    let mut local = LocalTokenBucket::new(1.0, 2.0).unwrap();
    local.acquire(&engine, 1, 100.0).unwrap();
    assert!(local.acquire(&engine, 1, 99.0).is_err());
    for closed in [false, true] {
        let store = Arc::new(Mock::default());
        store.0.lock().unwrap().failure = Some(RateFailure::Unavailable);
        let mut limiter = RateLimiter::new(store, closed);
        let decision = ready(limiter.acquire(&engine, 1, 100.0)).unwrap();
        assert_eq!(decision.allowed, !closed);
        assert_eq!(decision.reason, RateReason::QuotaUnavailable);
        assert_eq!(decision.authority_failure, Some(RateFailure::Unavailable));
        assert_eq!(decision.cooldown_failure, Some(RateFailure::Unavailable));
        let observation = ready(limiter.upstream_cooldown_remaining(&engine, 100.0)).unwrap();
        assert!(observation.shared.is_err());
    }
    let mut s = HealthState::default();
    let resp = response("error", 9999.0, 0, true);
    for origin in [OutcomeOrigin::Undispatched, OutcomeOrigin::CallerCancelled] {
        s.observe(origin, &resp, now(100.0));
        assert_eq!(s.consecutive_errors, 0);
        assert_eq!(
            build_engine_health(Some(&s), None, now(100.0), 300.0).unwrap()["status"],
            "unknown"
        );
    }
    assert_eq!(stale_seconds(Some("inf")), 300.0);
    assert_eq!(circuit_settings(Some("２"), Some("-2")), (2, -2));
    assert_eq!(resolve_valkey_url("", Some("fixture")), "fixture");
    assert_eq!(resolve_valkey_url("explicit", Some("other")), "explicit");
    assert_eq!(EngineStatus::default(), EngineStatus::Ok);
}

#[test]
fn source_raw_configuration_and_documented_fractional_expiry_hardening() {
    let data = corpus();
    for case in data["settings"].as_array().unwrap() {
        let actual = circuit_settings(case["threshold"].as_str(), case["timeout"].as_str());
        assert_eq!(json!([actual.0, actual.1]), case["expected"]);
    }
    for case in data["freshness"].as_array().unwrap() {
        let actual = stale_seconds(case["input"].as_str());
        if case["expected"] == "infinite" {
            assert_eq!(actual, 300.0)
        } else {
            assert_eq!(actual, case["expected"].as_f64().unwrap());
        }
    }
    for case in data["ttl_law"].as_array().unwrap() {
        let window = case["window"].as_f64().unwrap();
        assert_eq!(
            case["source_expiry_seconds"].as_u64().unwrap(),
            (window * 2.0) as u64
        );
        assert_eq!(
            QuotaConfig::new(10.0, window).unwrap().retention_ms(),
            (window * 2000.0) as u64
        );
    }
    for input in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert_eq!(bound_provider_cooldown(input), 30.0);
        assert_eq!(bound_strategy_cooldown(input), 30.0)
    }
    assert_eq!(bound_strategy_cooldown(-1.0), 1.0);
    assert_eq!(bound_provider_cooldown(-1.0), 30.0);
}

#[test]
fn decimal_millisecond_configuration_does_not_reject_binary_roundoff() {
    let config = QuotaConfig::new(1.0, 1.001).unwrap();
    assert_eq!(config.window_ms(), 1001);
    assert_eq!(config.retention_ms(), 2002);
    assert!(QuotaConfig::new(1.0, 1.0011).is_err());
}

#[test]
fn source_url_fail_settings_and_timestamp_limits() {
    let data = corpus();
    for case in data["url_resolution"].as_array().unwrap() {
        assert_eq!(
            resolve_valkey_url(
                case["explicit"].as_str().unwrap(),
                case["environment"].as_str()
            ),
            case["expected"].as_str().unwrap()
        );
    }
    for case in data["fail_settings"].as_array().unwrap() {
        let mock = Arc::new(Mock::default());
        mock.0.lock().unwrap().failure = Some(RateFailure::Unavailable);
        let mut limiter = RateLimiter::new(mock, case["fail_closed"].as_bool().unwrap());
        let name = EngineKey::new("fakeeng").unwrap();
        for expected in case["wrapped_allowed"].as_array().unwrap() {
            assert_eq!(
                ready(limiter.acquire(&name, 1, 100.0)).unwrap().allowed,
                expected.as_bool().unwrap()
            );
        }
    }
    for case in data["timestamp_limits"].as_array().unwrap() {
        let at = case["at"].as_f64().unwrap();
        let mut state = HealthState::default();
        state.observe(
            OutcomeOrigin::AdapterReturned,
            &response("ok", 1.0, 0, false),
            now(at),
        );
        let view = build_engine_health(Some(&state), None, now(at), 300.0);
        if case["expected"].is_null() {
            assert!(view.is_err(), "{at:?}")
        } else {
            assert_eq!(view.unwrap()["status_at"], case["expected"], "{at:?}")
        }
    }
}
