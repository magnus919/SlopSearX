use ssx_owned_budget::{Budget, Charge, Error};
use std::{
    alloc::Layout,
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};
struct ChargedFuture<F> {
    inner: Pin<Box<F>>,
    _charge: Charge,
}
impl<F: Future> Future for ChargedFuture<F> {
    type Output = F::Output;
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.inner.as_mut().poll(cx)
    }
}
pub(super) fn boxed<'a, F: Future + Send + 'a>(
    future: F,
    budget: &Budget,
) -> Result<Pin<Box<dyn Future<Output = F::Output> + Send + 'a>>, Error> {
    let bytes = Layout::new::<F>()
        .size()
        .checked_add(Layout::new::<ChargedFuture<F>>().size())
        .ok_or(Error::Overflow)?;
    let charge = budget.reserve(bytes)?;
    // System allocation failure here is process-operational, not JSON syntax.
    // The supervised helper is the allocation-failure containment boundary.
    Ok(Box::pin(ChargedFuture {
        inner: Box::pin(future),
        _charge: charge,
    }))
}
