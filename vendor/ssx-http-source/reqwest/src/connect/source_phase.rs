//! Exact private per-phase timer, shared by production composition and its test include.
use std::{error::Error as StdError, fmt, future::Future, io, task::Poll, time::Duration};
type BoxError = Box<dyn StdError + Send + Sync>;
/// The independently timed connection phase.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceHttp1ConnectPhase {
    /// Name resolution plus all TCP attempts and socket options under one budget.
    DnsTcp,
    /// TLS to an HTTPS proxy, after its TCP connection.
    ProxyTls,
    /// TLS to the origin, direct or after successful CONNECT.
    OriginTls,
}
/// A non-secret typed payload carried by `io::ErrorKind::TimedOut`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SourceHttp1ConnectTimedOut(
    /// The exact expired phase.
    pub SourceHttp1ConnectPhase,
);
impl fmt::Display for SourceHttp1ConnectTimedOut {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self.0 {
            SourceHttp1ConnectPhase::DnsTcp => "source HTTP/1 DNS/TCP timed out",
            SourceHttp1ConnectPhase::ProxyTls => "source HTTP/1 proxy TLS timed out",
            SourceHttp1ConnectPhase::OriginTls => "source HTTP/1 origin TLS timed out",
        })
    }
}
impl StdError for SourceHttp1ConnectTimedOut {}
pub(super) async fn phase<T>(
    budget: Duration,
    kind: SourceHttp1ConnectPhase,
    future: impl Future<Output = Result<T, BoxError>>,
) -> Result<T, BoxError> {
    let mut future = Box::pin(future);
    let mut deadline = Box::pin(tokio::time::sleep(budget));
    std::future::poll_fn(|cx| {
        // A late IO wake must not rescue a phase whose own deadline passed.
        if tokio::time::Instant::now() >= deadline.deadline()
            || deadline.as_mut().poll(cx).is_ready()
        {
            return Poll::Ready(Err(Box::new(io::Error::new(
                io::ErrorKind::TimedOut,
                SourceHttp1ConnectTimedOut(kind),
            )) as BoxError));
        }
        future.as_mut().poll(cx)
    })
    .await
}
