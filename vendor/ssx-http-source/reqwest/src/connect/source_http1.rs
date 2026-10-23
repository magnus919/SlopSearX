//! Opt-in source HTTP/1 phase composition over the existing dial/TLS/HTTP stacks.
use super::*;
use hyper_util::client::legacy::connect::HttpInfo;
use sync_wrapper::SyncWrapper;

/// Trusted per-client source budgets; no aggregate request deadline is installed.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Profile {
    pub(crate) connect: Duration,
    pub(crate) read: Duration,
}

#[path = "source_phase.rs"]
mod source_phase;
use source_phase::phase;
pub use source_phase::{SourceHttp1ConnectPhase, SourceHttp1ConnectTimedOut};

// Upgraded is Send but not Sync. Exclusive mutable IO access through the
// already-selected SyncWrapper satisfies reqwest's sealed Conn bound without
// an unsafe assertion, a second parser, or losing Hyper's unread upgrade buffer.
trait Transport: Read + Write + Send + Unpin + 'static {}
impl<T: Read + Write + Send + Unpin + 'static> Transport for T {}
struct SourceIo {
    io: SyncWrapper<Box<dyn Transport>>,
    http_info: Option<HttpInfo>,
    tls_info: Option<crate::tls::TlsInfo>,
}
impl SourceIo {
    fn new(
        io: impl Transport,
        http_info: Option<HttpInfo>,
        tls_info: Option<crate::tls::TlsInfo>,
    ) -> Self {
        Self {
            io: SyncWrapper::new(Box::new(io)),
            http_info,
            tls_info,
        }
    }
}
impl Read for SourceIo {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: ReadBufCursor<'_>,
    ) -> Poll<io::Result<()>> {
        Pin::new(self.io.get_mut().as_mut()).poll_read(cx, buf)
    }
}
impl Write for SourceIo {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(self.io.get_mut().as_mut()).poll_write(cx, buf)
    }
    fn poll_write_vectored(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &[IoSlice<'_>],
    ) -> Poll<io::Result<usize>> {
        Pin::new(self.io.get_mut().as_mut()).poll_write_vectored(cx, bufs)
    }
    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(self.io.get_mut().as_mut()).poll_flush(cx)
    }
    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(self.io.get_mut().as_mut()).poll_shutdown(cx)
    }
}
impl Connection for SourceIo {
    fn connected(&self) -> Connected {
        // HttpConnector's TcpStream adapter produces exactly this HttpInfo extra.
        match &self.http_info {
            Some(info) => Connected::new().extra(info.clone()),
            None => Connected::new(),
        }
    }
}
impl TlsInfoFactory for SourceIo {
    fn tls_info(&self) -> Option<crate::tls::TlsInfo> {
        self.tls_info.clone()
    }
}

async fn start_tls(
    io: SourceIo,
    uri: &Uri,
    config: Arc<rustls::ClientConfig>,
    profile: Profile,
    kind: SourceHttp1ConnectPhase,
) -> Result<SourceIo, BoxError> {
    // Match HyperRustls' existing default name resolver, including IPv6 brackets.
    let host = uri.host().ok_or("source TLS URI missing host")?;
    let host = host
        .strip_prefix('[')
        .and_then(|v| v.strip_suffix(']'))
        .unwrap_or(host);
    let server_name = rustls_pki_types::ServerName::try_from(host.to_owned())?;
    let http_info = io.http_info.clone();
    let stream = phase(profile.connect, kind, async move {
        tokio_rustls::TlsConnector::from(config)
            .connect(server_name, TokioIo::new(io))
            .await
            .map_err(Into::into)
    })
    .await?;
    let session = &stream.get_ref().1;
    if session
        .alpn_protocol()
        .is_some_and(|alpn| alpn != b"http/1.1")
    {
        return Err("source connector negotiated unsupported ALPN".into());
    }
    let tls_info = crate::tls::TlsInfo {
        peer_certificate: session
            .peer_certificates()
            .and_then(|c| c.first())
            .map(|c| c.to_vec()),
        version: session
            .protocol_version()
            .and_then(crate::tls::Version::from_rustls),
    };
    Ok(SourceIo::new(
        TokioIo::new(stream),
        http_info,
        Some(tls_info),
    ))
}

// Poll public Hyper's connection and response/upgrade under this SAME future.
// Cancellation destroys both; no detached task can retain a socket or a credit.
async fn drive<T, F, D>(
    mut operation: Pin<&mut F>,
    driver: &mut Option<Pin<Box<D>>>,
) -> Result<T, BoxError>
where
    F: Future<Output = Result<T, hyper::Error>>,
    D: Future<Output = Result<(), hyper::Error>>,
{
    std::future::poll_fn(|cx| {
        if let Poll::Ready(result) = operation.as_mut().poll(cx) {
            return Poll::Ready(result.map_err(Into::into));
        }
        if let Some(connection) = driver.as_mut() {
            if let Poll::Ready(result) = connection.as_mut().poll(cx) {
                *driver = None;
                if let Err(error) = result {
                    return Poll::Ready(Err(error.into()));
                }
            }
        }
        match operation.as_mut().poll(cx) {
            Poll::Ready(result) => Poll::Ready(result.map_err(Into::into)),
            Poll::Pending if driver.is_none() => {
                Poll::Ready(Err("source CONNECT driver ended before result".into()))
            }
            Poll::Pending => Poll::Pending,
        }
    })
    .await
}

async fn tunnel(
    io: SourceIo,
    dst: &Uri,
    proxy: &Intercepted,
    profile: Profile,
) -> Result<SourceIo, BoxError> {
    let port = dst.port_u16().unwrap_or(443);
    let target = format!(
        "{}:{port}",
        dst.host().ok_or("source CONNECT missing host")?
    );
    let mut builder = hyper::client::conn::http1::Builder::new();
    builder.source_connect_response();
    builder.title_case_headers(true);
    let http_info = io.http_info.clone();
    let (mut sender, connection) = builder.handshake::<_, crate::async_impl::Body>(io).await?;
    let mut driver = Some(Box::pin(connection.with_upgrades()));
    let mut request = hyper::Request::builder()
        .method(http::Method::CONNECT)
        .uri(target.as_str())
        .header(http::header::HOST, target.as_str())
        .header(http::header::ACCEPT, "*/*")
        .body(crate::async_impl::Body::empty())?;
    if let Some(auth) = proxy.basic_auth() {
        request
            .headers_mut()
            .insert(http::header::PROXY_AUTHORIZATION, auth.clone());
    }
    if let Some(headers) = proxy.custom_headers() {
        request.headers_mut().extend(headers.clone());
    }
    request
        .extensions_mut()
        .insert(hyper::ext::SendOperationTimeout(profile.connect));
    request
        .extensions_mut()
        .insert(hyper::ext::SourceHttp1ReadTimeout(profile.read));
    let mut response_future = Box::pin(sender.send_request(request));
    let mut response = drive(response_future.as_mut(), &mut driver).await?;
    if !response.status().is_success() {
        let raw_reason = response
            .extensions()
            .get::<hyper::ext::SourceConnectResponse>()
            .map_or(&[][..], hyper::ext::SourceConnectResponse::reason);
        let reason: String = raw_reason
            .iter()
            .copied()
            .filter(u8::is_ascii)
            .map(char::from)
            .collect();
        return Err(Box::new(SourceHttp1ProxyRefused {
            status: response.status().as_u16(),
            reason,
        }));
    }
    let mut upgrade = Box::pin(hyper::upgrade::on(&mut response));
    drop(response);
    let upgraded = drive(upgrade.as_mut(), &mut driver).await?;
    // The public Upgraded itself retains unread bytes. Do not peel/drop that buffer.
    drop(driver);
    drop(sender);
    Ok(SourceIo::new(upgraded, http_info, None))
}

/// A source-compatible proxy refusal, after all response fields were validated.
/// Debug redacts peer-controlled status text; Display supplies the source error body.
#[derive(Clone)]
pub struct SourceHttp1ProxyRefused {
    status: u16,
    reason: String,
}
impl SourceHttp1ProxyRefused {
    /// The received final status (including source101 refusal).
    pub fn status(&self) -> u16 {
        self.status
    }
    /// Source ASCII-ignore reason, without untrusted logging by this connector.
    pub fn reason(&self) -> &str {
        &self.reason
    }
}
impl std::fmt::Debug for SourceHttp1ProxyRefused {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SourceHttp1ProxyRefused")
            .field("status", &self.status)
            .finish_non_exhaustive()
    }
}
impl std::fmt::Display for SourceHttp1ProxyRefused {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} {}", self.status, self.reason)
    }
}
impl std::error::Error for SourceHttp1ProxyRefused {}

pub(super) async fn connect(
    service: ConnectorService,
    dst: Uri,
    profile: Profile,
) -> Result<Conn, BoxError> {
    let (mut http, tls, tls_proxy) = match &service.inner {
        Inner::RustlsTls {
            http,
            tls,
            tls_proxy,
        } => (http.clone(), tls.clone(), tls_proxy.clone()),
        #[allow(unreachable_patterns)]
        _ => return Err("source HTTP/1 connector requires Rustls".into()),
    };
    #[cfg(feature = "ssx-http1-source-pool")]
    let proxy = match &service.source_pool_proxy {Some(selected)=>selected.clone(),None=>service.proxies.iter().find_map(|p|p.intercept(&dst))};
    #[cfg(not(feature = "ssx-http1-source-pool"))]
    let proxy = service.proxies.iter().find_map(|p| p.intercept(&dst));
    let dial_dst = proxy
        .as_ref()
        .map_or_else(|| dst.clone(), |p| p.uri().clone());
    if !matches!(dial_dst.scheme_str(), Some("http" | "https")) {
        return Err("source connector supports only HTTP/HTTPS proxy routes".into());
    }
    let io = phase(
        profile.connect,
        SourceHttp1ConnectPhase::DnsTcp,
        async move {
            http.call(dial_dst.clone())
                .await
                .map(|io| (io, dial_dst))
                .map_err(Into::into)
        },
    )
    .await?;
    let (io, dial_dst) = io;
    let mut extras = http::Extensions::new();
    io.connected().get_extras(&mut extras);
    let http_info = extras.get::<HttpInfo>().cloned();
    let mut io = SourceIo::new(io, http_info, None);
    if dial_dst.scheme_str() == Some("https") {
        let (config, kind) = if proxy.is_some() {
            (tls_proxy, SourceHttp1ConnectPhase::ProxyTls)
        } else {
            (tls.clone(), SourceHttp1ConnectPhase::OriginTls)
        };
        io = start_tls(io, &dial_dst, config, profile, kind).await?;
    }
    let mut is_proxy = proxy.is_some();
    if let Some(proxy) = proxy {
        if dst.scheme_str() == Some("https") {
            io = tunnel(io, &dst, &proxy, profile).await?;
            io = start_tls(io, &dst, tls, profile, SourceHttp1ConnectPhase::OriginTls).await?;
            is_proxy = false;
        }
    }
    Ok(Conn {
        inner: service.verbose.wrap(io),
        is_proxy,
        tls_info: service.tls_info,
    })
}
