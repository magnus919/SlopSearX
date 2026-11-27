//! One authorization attempt owns its listener and all accepted connections.

use axum::{
    body::Body,
    http::{Method, Request, Response, StatusCode, header},
};
use futures_util::{StreamExt, stream::FuturesUnordered};
use hyper::{body::Incoming, server::conn::http1, service::service_fn};
use hyper_util::rt::TokioIo;
use std::{
    convert::Infallible,
    fmt, io,
    net::{Ipv4Addr, SocketAddr},
    sync::{
        Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
use subtle::ConstantTimeEq;
use tokio::net::{TcpListener, TcpStream};

const MAX_CONNECTIONS: usize = 8;
const CONNECTION_TIMEOUT: Duration = Duration::from_secs(5);
const CALLBACK_PATH: &str = "/callback";

pub struct AuthorizationCode {
    code: String,
    issuer: Option<String>,
}

impl AuthorizationCode {
    pub fn into_parts(self) -> (String, Option<String>) {
        (self.code, self.issuer)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CallbackError {
    Listener,
    Timeout,
    Denied,
}

impl fmt::Display for CallbackError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Listener => "OAuth callback listener failed",
            Self::Timeout => "OAuth browser callback timed out",
            Self::Denied => "OAuth authorization was denied",
        })
    }
}

impl std::error::Error for CallbackError {}

type Outcome = Result<AuthorizationCode, CallbackError>;

pub struct LoopbackCallback {
    listener: TcpListener,
    address: SocketAddr,
}

impl LoopbackCallback {
    pub async fn bind(port: u16) -> io::Result<Self> {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, port)).await?;
        let address = listener.local_addr()?;
        Ok(Self { listener, address })
    }

    pub fn redirect_uri(&self) -> String {
        format!("http://{}{CALLBACK_PATH}", self.address)
    }

    /// Drive the callback after a caller has dispatched the authorization URL.
    /// Finishing, timing out, or dropping this future closes all owned sockets.
    pub async fn wait(self, expected_state: &str, timeout: Duration) -> Outcome {
        tokio::time::timeout(timeout, self.receive(expected_state))
            .await
            .map_err(|_| CallbackError::Timeout)?
    }

    async fn receive(self, expected_state: &str) -> Outcome {
        let host = self.address.to_string();
        let claimed = AtomicBool::new(false);
        let mut connections = FuturesUnordered::new();
        loop {
            tokio::select! {
                socket = self.listener.accept(), if connections.len() < MAX_CONNECTIONS => {
                    let (stream, _) = socket.map_err(|_| CallbackError::Listener)?;
                    connections.push(serve_one(stream, &host, expected_state, &claimed));
                }
                Some(outcome) = connections.next(), if !connections.is_empty() => {
                    if let Some(outcome) = outcome {
                        return outcome;
                    }
                }
            }
        }
    }
}

async fn serve_one(
    stream: TcpStream,
    host: &str,
    state: &str,
    claimed: &AtomicBool,
) -> Option<Outcome> {
    let outcome = Mutex::new(None);
    let service = service_fn(|request| async {
        let (status, message, callback) = handle(request, host, state);
        if callback.is_some() && claimed.swap(true, Ordering::Relaxed) {
            return Ok::<_, Infallible>(response(
                StatusCode::CONFLICT,
                "OAuth callback was already received.",
            ));
        }
        *outcome.lock().unwrap() = callback;
        Ok::<_, Infallible>(response(status, message))
    });
    let mut protocol = http1::Builder::new();
    protocol.keep_alive(false).max_buf_size(8192);
    // The connection is driven to completion before delivering the result, so
    // the browser has a chance to receive the confirmation before we close it.
    let _ = tokio::time::timeout(
        CONNECTION_TIMEOUT,
        protocol.serve_connection(TokioIo::new(stream), service),
    )
    .await;
    outcome.into_inner().unwrap()
}

fn response(status: StatusCode, message: &'static str) -> Response<Body> {
    Response::builder()
        .status(status)
        .header(header::CACHE_CONTROL, "no-store")
        .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
        .header(header::REFERRER_POLICY, "no-referrer")
        .body(Body::from(message))
        .expect("valid fixed callback headers")
}

fn handle(
    request: Request<Incoming>,
    host: &str,
    expected_state: &str,
) -> (StatusCode, &'static str, Option<Outcome>) {
    const INVALID: (StatusCode, &str, Option<Outcome>) =
        (StatusCode::BAD_REQUEST, "Invalid OAuth callback.", None);
    let mut hosts = request.headers().get_all(header::HOST).iter();
    if !hosts.next().is_some_and(|value| matches_host(value, host))
        || hosts.next().is_some()
        || request.uri().authority().is_some()
        || request.headers().contains_key(header::TRANSFER_ENCODING)
        || request
            .headers()
            .get_all(header::CONTENT_LENGTH)
            .iter()
            .any(|length| length != "0")
    {
        return INVALID;
    }
    if request.uri().path() != CALLBACK_PATH {
        return (StatusCode::NOT_FOUND, "Not found.", None);
    }
    if request.method() != Method::GET {
        return (StatusCode::METHOD_NOT_ALLOWED, "Method not allowed.", None);
    }

    let (mut state, mut code, mut error, mut issuer) = (None, None, None, None);
    for (name, value) in
        url::form_urlencoded::parse(request.uri().query().unwrap_or_default().as_bytes())
    {
        let slot = match name.as_ref() {
            "state" => &mut state,
            "code" => &mut code,
            "error" => &mut error,
            "iss" => &mut issuer,
            _ => continue,
        };
        if slot.replace(value.into_owned()).is_some() {
            return INVALID;
        }
    }
    if !state.is_some_and(|state| {
        !state.is_empty() && bool::from(state.as_bytes().ct_eq(expected_state.as_bytes()))
    }) {
        return INVALID;
    }
    match (code, error) {
        (Some(code), None) if !code.is_empty() => (
            StatusCode::OK,
            "Authorization complete. You can close this tab and return to the gateway.",
            Some(Ok(AuthorizationCode { code, issuer })),
        ),
        (None, Some(error)) if !error.is_empty() => (
            StatusCode::OK,
            "Authorization was denied. You can close this tab.",
            Some(Err(CallbackError::Denied)),
        ),
        _ => INVALID,
    }
}

fn matches_host(value: &header::HeaderValue, host: &str) -> bool {
    value == host || (host == "127.0.0.1:80" && value == "127.0.0.1")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    async fn get(address: SocketAddr, host: &str, path: &str) -> io::Result<String> {
        get_with_headers(address, host, path, "").await
    }

    async fn get_with_headers(
        address: SocketAddr,
        host: &str,
        path: &str,
        headers: &str,
    ) -> io::Result<String> {
        let mut stream = TcpStream::connect(address).await?;
        let request = format!("GET {path} HTTP/1.1\r\nHost: {host}\r\n{headers}\r\n");
        stream.write_all(request.as_bytes()).await?;
        let mut response = String::new();
        stream.read_to_string(&mut response).await?;
        Ok(response)
    }

    #[tokio::test]
    async fn validates_state_and_host_before_accepting_one_callback() -> crate::Result<()> {
        let callback = LoopbackCallback::bind(0).await?;
        let address = callback.address;
        let host = address.to_string();
        assert_eq!(callback.redirect_uri(), format!("http://{host}/callback"));
        let browser = async {
            for (authority, path) in [
                ("other.example", "/callback?code=skip&state=expected"),
                (&host, "/callback?code=skip&state=wrong"),
                (&host, "/callback?code=skip&state=expected&state=expected"),
                (
                    &host,
                    "/callback?code=skip&error=access_denied&state=expected",
                ),
            ] {
                let response = get(address, authority, path).await?;
                assert!(response.starts_with("HTTP/1.1 400"));
                assert!(!response.contains("skip"));
            }
            for headers in ["Content-Length: 4\r\n", "Transfer-Encoding: chunked\r\n"] {
                let response = get_with_headers(
                    address,
                    &host,
                    "/callback?code=skip&state=expected",
                    headers,
                )
                .await?;
                assert!(response.starts_with("HTTP/1.1 400"));
            }
            let response = get(
                address,
                &host,
                "/callback?code=authorization%2Bcode&state=expected&iss=https%3A%2F%2Fissuer.test",
            )
            .await?;
            assert!(response.starts_with("HTTP/1.1 200"));
            assert!(response.contains("cache-control: no-store"));
            assert!(!response.contains("authorization+code"));
            Ok::<(), io::Error>(())
        };
        let (outcome, response) =
            tokio::join!(callback.wait("expected", Duration::from_secs(5)), browser);
        response?;
        assert_eq!(
            outcome?.into_parts(),
            (
                "authorization+code".into(),
                Some("https://issuer.test".into())
            )
        );
        assert!(TcpStream::connect(address).await.is_err());
        Ok(())
    }

    #[test]
    fn accepts_implicit_default_port_only_on_port_80() {
        let bare = header::HeaderValue::from_static("127.0.0.1");
        assert!(matches_host(&bare, "127.0.0.1:80"));
        assert!(!matches_host(&bare, "127.0.0.1:8765"));
        let explicit = header::HeaderValue::from_static("127.0.0.1:80");
        assert!(matches_host(&explicit, "127.0.0.1:80"));
    }

    #[tokio::test]
    async fn concurrent_valid_code_and_denial_can_only_claim_once() -> crate::Result<()> {
        let callback = LoopbackCallback::bind(0).await?;
        let address = callback.address;
        let host = address.to_string();
        let server = async {
            let (first, _) = callback.listener.accept().await?;
            let (second, _) = callback.listener.accept().await?;
            let claimed = AtomicBool::new(false);
            let outcomes = tokio::join!(
                serve_one(first, &host, "expected", &claimed),
                serve_one(second, &host, "expected", &claimed),
            );
            Ok::<_, io::Error>(outcomes)
        };
        let (outcomes, code_response, denial_response) =
            tokio::time::timeout(Duration::from_secs(5), async {
                tokio::join!(
                    server,
                    get(address, &host, "/callback?code=valid&state=expected"),
                    get(
                        address,
                        &host,
                        "/callback?error=access_denied&state=expected"
                    ),
                )
            })
            .await?;
        let (first, second) = outcomes?;
        assert_ne!(first.is_some(), second.is_some());
        let responses = [code_response?, denial_response?];
        assert_eq!(
            responses
                .iter()
                .filter(|res| res.starts_with("HTTP/1.1 200"))
                .count(),
            1
        );
        assert_eq!(
            responses
                .iter()
                .filter(|res| res.starts_with("HTTP/1.1 409"))
                .count(),
            1
        );
        Ok(())
    }

    #[tokio::test]
    async fn timeout_closes_listener_and_an_incomplete_connection() -> crate::Result<()> {
        let callback = LoopbackCallback::bind(0).await?;
        let address = callback.address;
        let mut browser = TcpStream::connect(address).await?;
        browser.write_all(b"GET /callback HTTP/1.1\r\n").await?;
        assert!(matches!(
            callback.wait("expected", Duration::from_millis(100)).await,
            Err(CallbackError::Timeout)
        ));
        let mut remaining = [0; 1];
        let read =
            tokio::time::timeout(Duration::from_secs(1), browser.read(&mut remaining)).await?;
        assert!(matches!(read, Ok(0) | Err(_)));
        assert!(TcpStream::connect(address).await.is_err());
        Ok(())
    }

    #[tokio::test]
    async fn validates_state_before_recording_a_browser_denial() -> crate::Result<()> {
        let callback = LoopbackCallback::bind(0).await?;
        let address = callback.address;
        let host = address.to_string();
        let browser = async {
            let invalid = get(address, &host, "/callback?error=access_denied&state=wrong").await?;
            assert!(invalid.starts_with("HTTP/1.1 400"));
            get(
                address,
                &host,
                "/callback?error=access_denied&error_description=details&state=expected",
            )
            .await
        };
        let (outcome, response) =
            tokio::join!(callback.wait("expected", Duration::from_secs(5)), browser);
        assert!(matches!(outcome, Err(CallbackError::Denied)));
        let response = response?;
        assert!(response.starts_with("HTTP/1.1 200"));
        assert!(!response.contains("details"));
        assert!(TcpStream::connect(address).await.is_err());
        Ok(())
    }
}
