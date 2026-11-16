//! F10's real-peer timeout control, using the existing private owner clock.
//! This unit-only placement grants no production/config clock-selection API.
use super::{AdapterHttp, Cancellation, Error, Options, Request};
use ssx_application::health::{HealthState, OutcomeOrigin, build_engine_health};
use ssx_domain::{AdapterResponse, EngineStatus, WallClockTimestamp};
use std::{
    future::{Future, poll_fn},
    task::Poll,
    time::Duration,
};
use tokio::{io::AsyncReadExt, net::TcpListener, sync::oneshot, task::JoinHandle};

struct Peer(JoinHandle<()>);
impl Peer {
    async fn stop(
        &mut self,
        watchdog: Duration,
    ) -> Result<Result<(), tokio::task::JoinError>, tokio::time::error::Elapsed> {
        self.0.abort();
        tokio::time::timeout(watchdog, &mut self.0).await
    }
}
impl Drop for Peer {
    fn drop(&mut self) {
        self.0.abort();
    }
}

#[tokio::test]
async fn actual_peer_started_timeout_records_failure_with_null_latency() {
    // Reuse the existing real-loopback five-second watchdog. It detects a hung
    // fixture; the source operation budget remains exactly 150 logical ms.
    let watchdog = Duration::from_secs(5);
    let timeout = Duration::from_millis(150);
    let peer = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/fixture", peer.local_addr().unwrap());
    // Complete fallible setup before spawning a peer. A session-construction
    // error retires the owner before reporting it; no task is detached.
    let request = Request::get(&url).unwrap();
    let (http, driver) = AdapterHttp::clocked_real_pool_owner();
    let start = driver.now();
    let mut session = match http.session(
        Options {
            trust_env: false,
            ..Default::default()
        },
        tokio::time::Instant::from_std(start + timeout),
        Cancellation::default(),
    ) {
        Ok(session) => session,
        Err(error) => {
            let stopped = tokio::time::timeout(watchdog, http.shutdown_and_wait()).await;
            stopped.expect("bounded HTTP owner shutdown after session setup failure");
            panic!("real-peer session setup failed: {error:?}");
        }
    };
    let (tx, rx) = oneshot::channel();
    let mut peer = Peer(tokio::spawn(async move {
        let (mut io, _) = peer.accept().await.unwrap();
        let mut request = [0; 4096];
        let mut read = 0;
        while !request[..read].windows(4).any(|bytes| bytes == b"\r\n\r\n") {
            assert!(read < request.len(), "bounded real-peer request headers");
            let count = io.read(&mut request[read..]).await.unwrap();
            assert!(
                count > 0,
                "real peer closed before complete request headers"
            );
            read += count;
        }
        tx.send(request[..read].to_vec()).unwrap();
        // Keep the real socket and withhold every response byte until aborted.
        std::future::pending::<()>().await;
        drop(io);
    }));
    let observation = tokio::time::timeout(watchdog, async {
        let mut operation = Box::pin(session.execute(request));
        let headers = tokio::select! {
            result = &mut operation => return Err(format!(
                "operation completed before the full-header witness: {result:?}")),
            witness = rx => witness.map_err(|error| error.to_string())?,
        };
        // This makes an early timeout/error an explicit failure even if the
        // witness and result became ready in the same scheduler turn.
        let early = poll_fn(|cx| {
            Poll::Ready(match operation.as_mut().poll(cx) {
                Poll::Ready(result) => Some(result),
                Poll::Pending => None,
            })
        })
        .await;
        if let Some(result) = early {
            return Err(format!(
                "operation completed before clock advance: {result:?}"
            ));
        }
        let witnessed_at = driver.now();
        driver.advance_to(timeout);
        let result = operation.await;
        Ok::<_, String>((headers, witnessed_at, driver.now(), result))
    })
    .await;
    // Join the peer and stop the real HTTP owner before interpreting any
    // observation/watchdog failure. Drop remains an abort fallback for panic.
    let peer_stopped = peer.stop(watchdog).await;
    let owner_stopped = tokio::time::timeout(watchdog, http.shutdown_and_wait()).await;
    let (headers, witnessed_at, completed_at, result) = observation
        .expect("bounded real-peer fixture watchdog")
        .expect("real request must remain pending until the causal clock advance");
    assert!(
        peer_stopped
            .expect("bounded peer abort and join")
            .is_err_and(|error| error.is_cancelled())
    );
    owner_stopped.expect("bounded real HTTP owner shutdown");
    assert!(headers.starts_with(b"GET /fixture HTTP/1.1\r\n"));
    assert!(headers.windows(4).any(|bytes| bytes == b"\r\n\r\n"));
    assert_eq!(witnessed_at, start);
    assert_eq!(completed_at.duration_since(start), timeout);
    assert_eq!(result.unwrap_err(), Error::Deadline);
    {
        let state = http.state.lock().unwrap();
        assert!(state.used, "the actual caller-owned pool was exercised");
        assert!(
            state.injected.is_none(),
            "the actual network transport was retained"
        );
    }
    let mut health = HealthState::default();
    let wall = WallClockTimestamp::new(100.0).unwrap();
    health.observe(
        OutcomeOrigin::DispatchedSyntheticTimeout,
        &AdapterResponse {
            synthetic: true,
            status: EngineStatus::Timeout,
            latency_ms: ssx_domain::FiniteF64::new(150.0).unwrap(),
            ..Default::default()
        },
        wall,
    );
    let view = build_engine_health(Some(&health), None, wall, 300.0).unwrap();
    assert_eq!(view["status"], "timeout");
    assert!(view["last_observed_latency_ms"].is_null());
    assert_eq!(health.consecutive_errors, 1);
}
