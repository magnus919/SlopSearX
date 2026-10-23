//! Experimental source-profile plaintext network-read operations for HTTP/1.
use std::{fmt, time::Duration};

/// A fresh timeout for every demanded plaintext IO read in an HTTP/1 response.
/// This requires `SendOperationTimeout` on the same HTTP/1 client request.
/// It is not a response-header, frame, SSE-event, body or whole-request timeout.
#[derive(Clone, Copy, Debug)]
pub struct SourceHttp1ReadTimeout(pub Duration);

/// Response phase containing the stalled plaintext read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceHttp1ReadPhase {
    /// Final or informational response headers.
    Headers,
    /// Response data, transfer framing, or trailers.
    Body,
}

/// Typed error source with no URL, header or response-body data.
#[derive(Debug)]
pub struct SourceHttp1ReadTimedOut(pub SourceHttp1ReadPhase);
impl fmt::Display for SourceHttp1ReadTimedOut {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "HTTP/1 plaintext read operation timed out: {:?}", self.0)
    }
}
impl std::error::Error for SourceHttp1ReadTimedOut {}
