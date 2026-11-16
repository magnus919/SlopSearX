use ssx_application::{PortFuture, SearchContext, SearchEngine};
use ssx_domain::{AdapterResponse, SearchRequest};
use std::{
    sync::atomic::{AtomicUsize, Ordering},
    task::{Context, Poll, Waker},
    time::Instant,
};

struct Legacy(AtomicUsize);
impl SearchEngine for Legacy {
    fn search<'a>(&'a self, _: &'a SearchRequest) -> PortFuture<'a, AdapterResponse> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Box::pin(async { AdapterResponse::default() })
    }
}

#[test]
fn additive_context_default_invokes_legacy_search_exactly_once() {
    let engine = Legacy(AtomicUsize::new(0));
    let request = SearchRequest::default();
    let mut future = engine.search_with_context(
        &request,
        SearchContext {
            deadline: Instant::now(),
        },
    );
    assert_eq!(engine.0.load(Ordering::SeqCst), 1);
    let mut context = Context::from_waker(Waker::noop());
    assert!(matches!(future.as_mut().poll(&mut context), Poll::Ready(_)));
    assert_eq!(engine.0.load(Ordering::SeqCst), 1);
}
