//! Actual loopback HTTP lifetime under the existing Atom operation admission.
//! No provider or parser child is invoked by these pre-helper controls.
use ssx_infra::{
    adapters::arxiv_expat::{AtomPool, RawAdmissionError},
    http::{AdapterHttp, Cancellation, Options, Request},
};
use ssx_owned_budget::Budget;
use std::{num::NonZeroUsize, path::PathBuf, sync::Arc, time::Duration};
use tokio::{io::AsyncReadExt, net::TcpListener, sync::oneshot, time::Instant};
fn pool() -> AtomPool {
    AtomPool::new(
        PathBuf::from(env!("CARGO_BIN_EXE_ssx-atom-parser")),
        NonZeroUsize::new(1).unwrap(),
    )
    .unwrap()
}
async fn withheld_response() -> (String, oneshot::Receiver<()>, tokio::task::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let (sent, received) = oneshot::channel();
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut request = Vec::new();
        let mut bytes = [0; 1024];
        loop {
            let count = socket.read(&mut bytes).await.unwrap();
            assert_ne!(count, 0);
            request.extend_from_slice(&bytes[..count]);
            if request.windows(4).any(|w| w == b"\r\n\r\n") {
                break;
            }
            assert!(request.len() <= 8192);
        }
        sent.send(()).unwrap();
        // Dropping the in-flight actual HTTP request must close its connection.
        assert_eq!(socket.read(&mut bytes).await.unwrap(), 0);
    });
    (format!("http://{address}/atom"), received, server)
}
#[tokio::test]
async fn already_closing_pool_never_polls_pre_http_future() {
    let pool = pool();
    let operation = pool
        .begin_raw(
            Budget::new(1024).unwrap(),
            Instant::now() + Duration::from_secs(30),
        )
        .unwrap();
    let closing_pool = pool.clone();
    let shutdown = tokio::spawn(async move { closing_pool.shutdown().await });
    // Observe actual semaphore closure, rather than assuming a yield ran shutdown.
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if matches!(
                pool.begin_raw(
                    Budget::new(1).unwrap(),
                    Instant::now() + Duration::from_secs(30)
                ),
                Err(RawAdmissionError::Shutdown)
            ) {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    let response = operation
        .run_until_cancelled(async { panic!("pre-HTTP future was polled after closing") })
        .await;
    assert!(matches!(response, Err(RawAdmissionError::Shutdown)));
    drop(operation);
    shutdown.await.unwrap().unwrap();
}
#[tokio::test]
async fn actual_http_shutdown_drops_request_and_releases_pre_http_admission() {
    let (url, request_seen, server) = withheld_response().await;
    let pool = pool();
    let http = Arc::new(AdapterHttp::default());
    let operation = pool
        .begin_raw(
            Budget::new(1024).unwrap(),
            Instant::now() + Duration::from_secs(30),
        )
        .unwrap();
    let request_http = http.clone();
    let job = tokio::spawn(async move {
        let mut session = request_http
            .session(
                Options {
                    trust_env: false,
                    ..Default::default()
                },
                Instant::now() + Duration::from_secs(30),
                Cancellation::default(),
            )
            .unwrap();
        operation
            .run_until_cancelled(session.execute(Request::get(&url).unwrap()))
            .await
    });
    tokio::time::timeout(Duration::from_secs(5), request_seen)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(pool.available_operations(), 0);
    assert_eq!(pool.available_slots(), 1);
    tokio::time::timeout(Duration::from_secs(5), pool.shutdown())
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(
        job.await.unwrap(),
        Err(RawAdmissionError::Shutdown)
    ));
    tokio::time::timeout(Duration::from_secs(5), server)
        .await
        .unwrap()
        .unwrap();
    http.shutdown_and_wait().await;
}
#[tokio::test]
async fn actual_http_caller_cancellation_restores_pre_http_admission() {
    let (url, request_seen, server) = withheld_response().await;
    let pool = pool();
    let http = Arc::new(AdapterHttp::default());
    let operation = pool
        .begin_raw(
            Budget::new(1024).unwrap(),
            Instant::now() + Duration::from_secs(30),
        )
        .unwrap();
    let request_http = http.clone();
    let job = tokio::spawn(async move {
        let mut session = request_http
            .session(
                Options {
                    trust_env: false,
                    ..Default::default()
                },
                Instant::now() + Duration::from_secs(30),
                Cancellation::default(),
            )
            .unwrap();
        operation
            .run_until_cancelled(session.execute(Request::get(&url).unwrap()))
            .await
    });
    tokio::time::timeout(Duration::from_secs(5), request_seen)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(pool.available_operations(), 0);
    job.abort();
    assert!(job.await.unwrap_err().is_cancelled());
    assert_eq!(pool.available_operations(), 1);
    assert_eq!(pool.available_slots(), 1);
    tokio::time::timeout(Duration::from_secs(5), server)
        .await
        .unwrap()
        .unwrap();
    pool.shutdown().await.unwrap();
    http.shutdown_and_wait().await;
}
#[tokio::test(flavor = "current_thread")]
async fn actual_http_parent_deadline_uses_the_existing_operation_limit() {
    let (url, request_seen, server) = withheld_response().await;
    let pool = pool();
    let http = Arc::new(AdapterHttp::default());
    let deadline = Instant::now() + Duration::from_secs(3600);
    let operation = pool
        .begin_raw(Budget::new(1024).unwrap(), deadline)
        .unwrap();
    let request_http = http.clone();
    let job = tokio::spawn(async move {
        // The HTTP session's later limit cannot extend the operation deadline.
        let mut session = request_http
            .session(
                Options {
                    trust_env: false,
                    ..Default::default()
                },
                deadline + Duration::from_secs(3600),
                Cancellation::default(),
            )
            .unwrap();
        operation
            .run_until_cancelled(session.execute(Request::get(&url).unwrap()))
            .await
    });
    tokio::time::timeout(Duration::from_secs(5), request_seen)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(pool.available_operations(), 0);
    // Actual socket/request handshake precedes deterministic parent Tokio time.
    // This proves the parent deadline, not a real-clock HTTP client timer.
    tokio::time::pause();
    tokio::time::advance(Duration::from_secs(3601)).await;
    assert!(matches!(
        job.await.unwrap(),
        Err(RawAdmissionError::Deadline)
    ));
    assert_eq!(pool.available_operations(), 1);
    assert_eq!(pool.available_slots(), 1);
    tokio::time::resume();
    tokio::time::timeout(Duration::from_secs(5), server)
        .await
        .unwrap()
        .unwrap();
    pool.shutdown().await.unwrap();
    http.shutdown_and_wait().await;
}
