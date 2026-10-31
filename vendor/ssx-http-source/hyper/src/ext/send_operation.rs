//! Experimental opt-in HTTP/1 complete send-event budgets.
use std::{fmt, time::Duration};
/// A nonrenewed deadline for each complete HTTP/1 request send operation.
/// This experimental extension has no effect on requests without the extension.
#[derive(Clone, Copy, Debug)]
pub struct SendOperationTimeout(pub Duration);
/// The complete send operation which exceeded its budget.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SendOperationPhase {
    /// Serialized request headers.
    Headers,
    /// One complete body data frame, including its transfer framing.
    Data,
    /// End of message, including trailers or an empty terminal write.
    EndOfMessage,
}
/// Typed source retained in Hyper's body-write error chain.
#[derive(Debug)]
pub struct SendOperationTimedOut(pub SendOperationPhase);
impl fmt::Display for SendOperationTimedOut {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "HTTP/1 send operation timed out: {:?}", self.0)
    }
}
impl std::error::Error for SendOperationTimedOut {}
