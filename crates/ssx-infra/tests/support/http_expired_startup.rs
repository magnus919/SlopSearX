//! Issue #1032: admission uses the owner clock before any request-side work.
//! These controls add no original-provider observations or external requests.
use super::*;
use std::{future::Future, task::Poll};

#[derive(Default)]
struct Captured(Mutex<Vec<Request>>);
impl Transport for Captured {
    fn exchange(&self, request: Request) -> ExchangeFuture<'_> {
        Box::pin(async move {
            let url = request.url.clone();
            self.0.lock().unwrap().push(request);
            Ok(Response {
                status: 200,
                url,
                headers: HeaderMap::new(),
                body: b"owner-clock response".to_vec(),
            })
        })
    }
}
fn request() -> Request {
    Request::get("https://example.invalid/expired-startup-control").unwrap()
}
async fn first_poll<T>(future: impl Future<Output = T>) -> Poll<T> {
    let mut future = Box::pin(future);
    std::future::poll_fn(|cx| Poll::Ready(future.as_mut().poll(cx))).await
}
fn untouched(session: &Session, deadline: Instant) {
    assert_eq!(
        session.deadline.into_std(),
        deadline,
        "caller deadline retained"
    );
    assert_eq!(session.requests, 0, "no request budget spent");
    assert!(
        session
            .pool
            .slots
            .lock()
            .unwrap()
            .iter()
            .all(|(busy, client)| !busy && client.is_none()),
        "no checkout or retained client"
    );
}
async fn rejected(session: &mut Session, deadline: Instant, expected: Error) {
    assert!(
        matches!(first_poll(session.execute(request())).await, Poll::Ready(Err(error)) if error == expected)
    );
    untouched(session, deadline);
}

#[tokio::test]
async fn system_expired_startup_rejects_even_while_its_timer_is_pending() {
    // Freeze only Tokio scheduling, then let the production System owner cross
    // its exact 250us deadline. This deterministically exposes the admission
    // gap without relying on a particular host's timer-wheel tick alignment.
    tokio::time::pause();
    for injected in [false, true] {
        let http = AdapterHttp::default();
        assert!(!http.clock.is_manual());
        let captured = Arc::new(Captured::default());
        if injected {
            http.set_transport(captured.clone()).unwrap();
        }
        // Even a regressed real-pool control cannot reach a socket: all adapter
        // admission permits remain held until after its first execute poll.
        let permits = http
            .connections
            .clone()
            .acquire_many_owned(20)
            .await
            .unwrap();
        let deadline = http.deadline_now() + Duration::from_micros(250);
        std::thread::sleep(Duration::from_millis(1));
        assert!(http.deadline_now() >= deadline);
        assert!(
            first_poll(http.clock.sleep_until(deadline))
                .await
                .is_pending(),
            "the timer must still be pending for this causal control"
        );
        let mut session = http
            .session(Options::default(), deadline.into(), Cancellation::default())
            .unwrap();
        rejected(&mut session, deadline, Error::Deadline).await;
        assert!(
            captured.0.lock().unwrap().is_empty(),
            "no transport dispatch"
        );
        drop(session);
        drop(permits);
        http.shutdown_and_wait().await;
    }
    // The synchronous preflight must record the same owner-clock hit as
    // the timer path, even when Tokio has not observed its next timer tick.
    // A stop/cancel result must not fabricate a deadline observation.
    for (stopped, cancelled) in [(false, false), (false, true), (true, true)] {
        let (http, observer, driver) = AdapterHttp::pending_replay_owner(false);
        assert!(driver.is_none());
        let deadline = http.deadline_now() + Duration::from_micros(250);
        let cancel = Cancellation::default();
        std::thread::sleep(Duration::from_millis(1));
        assert!(http.deadline_now() >= deadline);
        let mut session = http
            .session(Options::default(), deadline.into(), cancel.clone())
            .unwrap();
        if cancelled {
            cancel.cancel();
        }
        if stopped {
            http.shutdown();
        }
        rejected(
            &mut session,
            deadline,
            if stopped {
                Error::Closed
            } else if cancelled {
                Error::Cancelled
            } else {
                Error::Deadline
            },
        )
        .await;
        assert_eq!(observer.started(), 0);
        assert_eq!(observer.dropped(), 0);
        if stopped || cancelled {
            assert!(
                observer.hit().is_none(),
                "higher-priority exit is not expiry"
            );
        } else {
            let (actual, observed) = observer.hit().expect("synchronous System expiry traced");
            assert_eq!(actual, deadline, "original caller deadline retained");
            assert!(observed >= actual);
        }
        drop(session);
        http.shutdown_and_wait().await;
    }
    // The exact boundary is causal under a private Manual owner: a dispatch
    // before the deadline records no hit; equal/past rejects without dispatch.
    for elapsed in [249, 250, 251] {
        let (http, observer, driver) = AdapterHttp::pending_replay_owner(true);
        let driver = driver.unwrap();
        let deadline = driver.now() + Duration::from_micros(250);
        let mut session = http
            .session(Options::default(), deadline.into(), Cancellation::default())
            .unwrap();
        driver.advance_to(Duration::from_micros(elapsed));
        if elapsed < 250 {
            assert!(first_poll(session.execute(request())).await.is_pending());
            assert_eq!(observer.started(), 1);
            assert_eq!(observer.dropped(), 1);
            assert!(
                observer.hit().is_none(),
                "predeadline dispatch is not expiry"
            );
        } else {
            rejected(&mut session, deadline, Error::Deadline).await;
            assert_eq!(observer.started(), 0);
            assert_eq!(observer.dropped(), 0);
            let (actual, observed) = observer.hit().expect("exact owner expiry traced");
            assert_eq!(actual, deadline);
            assert_eq!(observed, driver.now());
            assert!(observed >= deadline);
        }
        drop(session);
        http.shutdown_and_wait().await;
    }
    tokio::time::resume();
}

#[tokio::test]
async fn manual_expired_startup_includes_exact_deadline_and_past_deadline() {
    for elapsed in [250, 251] {
        let captured = Arc::new(Captured::default());
        let (http, driver) = AdapterHttp::replay_owner(captured.clone());
        let deadline = driver.now() + Duration::from_micros(250);
        let mut session = http
            .session(Options::default(), deadline.into(), Cancellation::default())
            .unwrap();
        driver.advance_to(Duration::from_micros(elapsed));
        rejected(&mut session, deadline, Error::Deadline).await;
        assert!(captured.0.lock().unwrap().is_empty());
        drop(session);
        http.shutdown_and_wait().await;
        assert_eq!(driver.owners(), 2);
    }
}

#[tokio::test]
async fn expired_startup_preserves_stop_then_cancel_then_deadline_precedence() {
    for stopped in [false, true] {
        let captured = Arc::new(Captured::default());
        let (http, driver) = AdapterHttp::replay_owner(captured.clone());
        let deadline = driver.now();
        let cancel = Cancellation::default();
        let mut session = http
            .session(Options::default(), deadline.into(), cancel.clone())
            .unwrap();
        cancel.cancel();
        if stopped {
            http.shutdown();
        }
        rejected(
            &mut session,
            deadline,
            if stopped {
                Error::Closed
            } else {
                Error::Cancelled
            },
        )
        .await;
        assert!(captured.0.lock().unwrap().is_empty());
        drop(session);
        http.shutdown_and_wait().await;
        assert_eq!(driver.owners(), 2);
    }
}

#[tokio::test]
async fn unexpired_startup_dispatches_with_system_and_manual_owner_clocks() {
    for manual in [false, true] {
        let captured = Arc::new(Captured::default());
        let (http, driver) = if manual {
            let (http, driver) = AdapterHttp::replay_owner(captured.clone());
            (http, Some(driver))
        } else {
            let http = AdapterHttp::default();
            http.set_transport(captured.clone()).unwrap();
            (http, None)
        };
        let budget = if manual {
            Duration::from_micros(250)
        } else {
            Duration::from_secs(60)
        };
        let deadline = http.deadline_now() + budget;
        if let Some(driver) = &driver {
            driver.advance_to(Duration::from_micros(249));
            std::thread::sleep(Duration::from_millis(1));
            assert!(Instant::now() >= deadline, "System time has expired");
            assert!(http.deadline_now() < deadline, "Manual owner still admits");
        }
        let mut session = http
            .session(Options::default(), deadline.into(), Cancellation::default())
            .unwrap();
        let response = tokio::time::timeout(Duration::from_secs(5), session.execute(request()))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(response.status, 200);
        assert_eq!(response.body, b"owner-clock response");
        assert_eq!(session.requests, 1);
        assert_eq!(session.deadline.into_std(), deadline);
        assert_eq!(captured.0.lock().unwrap().len(), 1);
        drop(session);
        http.shutdown_and_wait().await;
        if let Some(driver) = driver {
            assert_eq!(driver.owners(), 2);
        }
    }
}
