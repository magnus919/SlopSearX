use super::*;
use std::{
    sync::atomic::{AtomicUsize, Ordering},
    task::Poll,
};
// The runtime clock stays live. This bound detects a stalled test; it never
// advances the Manual owner or changes the one-millisecond engine deadline.
async fn within<T>(future: impl Future<Output = T>) -> T {
    tokio::time::timeout(Duration::from_secs(5), future)
        .await
        .expect("HTTP clock fixture exceeded real watchdog")
}
#[derive(Default)]
struct Ready(AtomicUsize);
impl Transport for Ready {
    fn exchange(&self, request: Request) -> ExchangeFuture<'_> {
        Box::pin(async move {
            self.0.fetch_add(1, Ordering::SeqCst);
            Ok(Response {
                status: 200,
                url: request.url,
                headers: HeaderMap::new(),
                body: b"{}".to_vec(),
            })
        })
    }
}
#[tokio::test]
async fn elapsed_owner_deadline_rejects_before_dispatch_and_wins_ready_response_tie() {
    let transport = Arc::new(Ready::default());
    let (http, driver) = AdapterHttp::replay_owner(transport.clone());
    let mut session = http
        .session(
            Options::default(),
            tokio::time::Instant::from_std(driver.now() + Duration::from_millis(1)),
            Cancellation::default(),
        )
        .unwrap();
    driver.advance_to(Duration::from_millis(1));
    assert!(session.deadline_elapsed(driver.now()));
    assert_eq!(
        within(session.execute(Request::get("https://example.invalid").unwrap()))
            .await
            .unwrap_err(),
        Error::Deadline
    );
    assert_eq!(transport.0.load(Ordering::SeqCst), 0);
    drop(session);
    within(http.shutdown_and_wait()).await;
    assert_eq!(http.replay_workers(), (0, 0));
    assert_eq!(driver.owners(), 2);
}
#[tokio::test]
async fn close_then_cancel_then_deadline_precedence_is_preserved() {
    // Pre-entry guards stay separate from the live-select control below.
    for (close, cancel, expected) in [
        (true, true, Error::Closed),
        (false, true, Error::Cancelled),
        (false, false, Error::Deadline),
    ] {
        let transport = Arc::new(Ready::default());
        let (http, driver) = AdapterHttp::replay_owner(transport.clone());
        let cancellation = Cancellation::default();
        let mut session = http
            .session(
                Options::default(),
                tokio::time::Instant::from_std(driver.now() + Duration::from_millis(1)),
                cancellation.clone(),
            )
            .unwrap();
        driver.advance_to(Duration::from_millis(1));
        if cancel {
            cancellation.cancel();
        }
        if close {
            http.shutdown();
        }
        assert_eq!(
            within(session.execute(Request::get("https://example.invalid").unwrap()))
                .await
                .unwrap_err(),
            expected
        );
        assert_eq!(transport.0.load(Ordering::SeqCst), 0);
        drop(session);
        within(http.shutdown_and_wait()).await;
        assert_eq!(driver.owners(), 2);
    }
}
struct PendingActive<'a>(&'a AtomicUsize);
impl Drop for PendingActive<'_> {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}
#[derive(Default)]
struct PendingExchange {
    started: Notify,
    entered: AtomicUsize,
    dropped: AtomicUsize,
}
impl Transport for PendingExchange {
    fn exchange(&self, _: Request) -> ExchangeFuture<'_> {
        Box::pin(async move {
            self.entered.fetch_add(1, Ordering::SeqCst);
            let _active = PendingActive(&self.dropped);
            self.started.notify_one();
            std::future::pending().await
        })
    }
}
#[tokio::test]
async fn pending_execute_resolves_simultaneous_close_cancel_deadline_in_original_order() {
    for (close, cancel, expected) in [
        (true, true, Error::Closed),
        (false, true, Error::Cancelled),
        (false, false, Error::Deadline),
    ] {
        let transport = Arc::new(PendingExchange::default());
        let (http, driver) = AdapterHttp::replay_owner(transport.clone());
        let cancellation = Cancellation::default();
        let mut session = http
            .session(
                Options::default(),
                tokio::time::Instant::from_std(driver.now() + Duration::from_millis(1)),
                cancellation.clone(),
            )
            .unwrap();
        let mut execute =
            Box::pin(session.execute(Request::get("https://example.invalid").unwrap()));
        within(async{
            tokio::select!{
                result=&mut execute=>panic!("execute completed before pending exchange: {result:?}"),
                _=transport.started.notified()=>{}
            }
        }).await;
        assert_eq!(transport.entered.load(Ordering::SeqCst), 1);
        assert_eq!(transport.dropped.load(Ordering::SeqCst), 0);
        // Execute has passed its synchronous guards. Force a further Pending
        // poll while all three signals remain unset, retaining this future.
        let state = std::future::poll_fn(|cx| Poll::Ready(execute.as_mut().poll(cx))).await;
        assert!(
            state.is_pending(),
            "execute must be suspended in its live select"
        );
        // No await or poll between these signals: every selected signal is
        // simultaneously ready at the next poll of the retained live select.
        driver.advance_to(Duration::from_millis(1));
        if cancel {
            cancellation.cancel();
        }
        if close {
            http.shutdown();
        }
        assert_eq!(within(execute).await.unwrap_err(), expected);
        assert_eq!(transport.entered.load(Ordering::SeqCst), 1);
        assert_eq!(transport.dropped.load(Ordering::SeqCst), 1);
        drop(session);
        within(http.shutdown_and_wait()).await;
        assert_eq!(http.replay_workers(), (0, 0));
        assert_eq!(driver.owners(), 2);
    }
}
#[tokio::test]
async fn blocking_decoder_uses_same_owner_before_and_at_exact_boundary() {
    let (http, driver) = AdapterHttp::replay_owner(Arc::new(Ready::default()));
    let deadline = tokio::time::Instant::from_std(driver.now() + Duration::from_millis(1));
    driver.advance_to(Duration::from_micros(999));
    let result = within(http.decoder.decode(
        HeaderMap::new(),
        b"{}".to_vec(),
        32,
        Cancellation::default(),
        http.stop.clone(),
        deadline,
        http.clock.clone(),
    ))
    .await
    .unwrap();
    assert_eq!(result, b"{}");
    driver.advance_to(Duration::from_millis(1));
    assert_eq!(
        within(http.decoder.decode(
            HeaderMap::new(),
            b"{}".to_vec(),
            32,
            Cancellation::default(),
            http.stop.clone(),
            deadline,
            http.clock.clone()
        ))
        .await
        .unwrap_err(),
        Error::Deadline
    );
    within(http.shutdown_and_wait()).await;
    assert_eq!(http.replay_workers(), (0, 0));
    assert_eq!(driver.owners(), 2);
}
struct Gate(Arc<(Mutex<bool>, std::sync::Condvar)>);
impl Gate {
    fn release(&self) {
        let (lock, notify) = &*self.0;
        *lock.lock().unwrap() = true;
        notify.notify_all();
    }
}
impl Drop for Gate {
    fn drop(&mut self) {
        self.release();
    }
}
struct BlockingRead {
    started: Arc<Notify>,
    gate: Arc<(Mutex<bool>, std::sync::Condvar)>,
}
impl Read for BlockingRead {
    fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
        self.started.notify_one();
        let (lock, notify) = &*self.gate;
        let mut open = lock.lock().unwrap();
        while !*open {
            open = notify.wait(open).unwrap();
        }
        out[0] = b'x';
        Ok(1)
    }
}
#[tokio::test]
async fn abandoned_decoder_waiter_keeps_owner_until_shutdown_joins_real_worker() {
    let (http, driver) = AdapterHttp::replay_owner(Arc::new(Ready::default()));
    let started = Arc::new(Notify::new());
    let gate = Gate(Arc::new((Mutex::new(false), std::sync::Condvar::new())));
    let reader = BlockingRead {
        started: started.clone(),
        gate: gate.0.clone(),
    };
    let clock = http.clock.clone();
    let stop = http.stop.clone();
    let deadline = driver.now() + Duration::from_millis(1);
    let mut worker = Box::pin(http.decoder.run(move || {
        bounded_read(
            reader,
            32,
            &Cancellation::default(),
            &stop,
            deadline,
            &clock,
        )
    }));
    within(async {
        tokio::select! {
            result=&mut worker=>panic!("worker returned before gate: {result:?}"),
            _=started.notified()=>{}
        }
    })
    .await;
    assert_eq!(http.replay_workers().0, 1);
    drop(worker);
    driver.advance_to(Duration::from_millis(1));
    http.shutdown();
    assert_eq!(http.replay_workers().0, 1, "blocked worker remains owned");
    assert_eq!(
        driver.owners(),
        3,
        "HTTP, driver and non-abortable worker own the clock"
    );
    let mut shutdown = Box::pin(http.shutdown_and_wait());
    // Poll the actual drain while the worker is causally unable to finish.
    // An early-returning drain deterministically fails this assertion.
    let state = std::future::poll_fn(|cx| Poll::Ready(shutdown.as_mut().poll(cx))).await;
    assert!(
        state.is_pending(),
        "shutdown must join the still-blocked worker"
    );
    let (gate_lock, _) = &*gate.0;
    assert!(
        !*gate_lock.lock().unwrap(),
        "gate must remain held through the Pending assertion"
    );
    assert_eq!(http.replay_workers().0, 1);
    assert_eq!(driver.owners(), 3);
    gate.release();
    within(shutdown).await;
    assert_eq!(http.replay_workers(), (0, 0));
    assert_eq!(driver.owners(), 2);
}
