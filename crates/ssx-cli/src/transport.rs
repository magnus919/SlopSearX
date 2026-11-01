//! Source-compatible owned external HTTP1 transport. No pooling or retry.
use crate::{
    Failure,
    source_io::{Bridge, HeaderPolicy, Inactivity},
};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use bytes::Bytes;
use http_body::{Body, Frame, SizeHint};
use hyper::{Request, Response, body::Incoming, client::conn::http1, ext::SourceRequestTarget};
use std::{
    collections::BTreeMap,
    convert::Infallible,
    future::poll_fn,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
    time::Duration,
};
use tokio::{
    io::{AsyncRead, AsyncWrite},
    net::TcpStream,
    task::JoinHandle,
};

trait Socket: AsyncRead + AsyncWrite + Unpin + Send {}
impl<T: AsyncRead + AsyncWrite + Unpin + Send> Socket for T {}
type Stream = Box<dyn Socket>;
#[derive(Default)]
struct Empty;
impl Body for Empty {
    type Data = Bytes;
    type Error = Infallible;
    fn poll_frame(
        self: Pin<&mut Self>,
        _: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Bytes>, Infallible>>> {
        Poll::Ready(None)
    }
    fn is_end_stream(&self) -> bool {
        true
    }
    fn size_hint(&self) -> SizeHint {
        SizeHint::with_exact(0)
    }
}
struct Drivers(Vec<JoinHandle<()>>);
impl Drivers {
    async fn retire(&mut self) {
        for driver in self.0.drain(..) {
            driver.abort();
            let _ = driver.await;
        }
    }
}
impl Drop for Drivers {
    fn drop(&mut self) {
        for driver in &self.0 {
            driver.abort();
        }
    }
}
#[derive(Clone)]
struct Endpoint {
    host: String,
    port: u16,
}
struct Destination {
    scheme: String,
    endpoint: Endpoint,
    host_header: Vec<u8>,
    authority: String,
    selector: String,
}
fn authority(value: &str) -> Result<&str, Failure> {
    value
        .split_once("://")
        .map(|(_, rest)| rest.split(['/', '?', '#']).next().unwrap_or(""))
        .ok_or(Failure::Transport)
}
fn unquote(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && i + 2 < bytes.len()
            && let (Some(a), Some(b)) = (
                (bytes[i + 1] as char).to_digit(16),
                (bytes[i + 2] as char).to_digit(16),
            )
        {
            out.push((a * 16 + b) as u8);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}
fn endpoint(authority: &str, default: u16) -> Result<Endpoint, Failure> {
    let (host, port) = if let Some(rest) = authority.strip_prefix('[') {
        let (host, tail) = rest.split_once(']').ok_or(Failure::Transport)?;
        let port = tail.strip_prefix(':').unwrap_or("");
        if !tail.is_empty() && !tail.starts_with(':') {
            return Err(Failure::Transport);
        }
        (host, port)
    } else {
        authority.rsplit_once(':').unwrap_or((authority, ""))
    };
    if host.is_empty() || host.chars().any(|c| c.is_ascii_control() || c == ' ') {
        return Err(Failure::Transport);
    }
    let port = if port.is_empty() {
        default
    } else {
        if !port.bytes().all(|b| b.is_ascii_digit()) {
            return Err(Failure::Transport);
        }
        port.parse::<u16>().map_err(|_| Failure::Transport)?
    };
    let host = if host.parse::<std::net::IpAddr>().is_ok() {
        host.to_owned()
    } else {
        ssx_source_codecs::encode_idna2003_host(host)
            .map_err(|_| Failure::Transport)?
            .ok_or(Failure::Transport)?
    };
    // Vetted URL validation remains authoritative for the selected TCP endpoint.
    let authority = if host.contains(':') {
        format!("[{host}]:{port}")
    } else {
        format!("{host}:{port}")
    };
    let checked =
        url::Url::parse(&format!("http://{authority}/")).map_err(|_| Failure::Transport)?;
    if checked.host_str().is_none() {
        return Err(Failure::Transport);
    }
    Ok(Endpoint { host, port })
}
fn validate(value: &str) -> Result<Destination, Failure> {
    let parsed = url::Url::parse(value).map_err(|_| Failure::Transport)?;
    let raw = authority(value)?;
    let decoded = unquote(raw);
    if !matches!(parsed.scheme(), "http" | "https")
        || decoded.contains('@')
        || decoded
            .chars()
            .any(|c| u32::from(c) > 255 || c.is_ascii_control() || c == ' ')
    {
        return Err(Failure::Transport);
    }
    let tail = value
        .split_once("://")
        .and_then(|(_, rest)| rest.strip_prefix(raw))
        .ok_or(Failure::Transport)?;
    let selector = tail.split('#').next().unwrap_or("");
    let selector = if selector.is_empty() {
        "/".into()
    } else if selector.starts_with('?') {
        format!("/{selector}")
    } else {
        selector.into()
    };
    SourceRequestTarget::new(selector.clone()).map_err(|_| Failure::Transport)?;
    let host_header = decoded
        .chars()
        .map(|c| u8::try_from(u32::from(c)).map_err(|_| Failure::Transport))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Destination {
        scheme: parsed.scheme().into(),
        endpoint: endpoint(&decoded, if parsed.scheme() == "https" { 443 } else { 80 })?,
        host_header,
        authority: raw.into(),
        selector,
    })
}
struct Proxy {
    endpoint: Endpoint,
    tls: bool,
    auth: Option<String>,
}
fn proxy(value: &str, destination: &Destination) -> Result<Proxy, Failure> {
    // urllib's _parse_proxy accepts scheme-less host:port and independently
    // percent-decodes credentials and authority. HTTPS uses a plain CONNECT
    // even for an https proxy URL; this is an observed source behavior.
    let (scheme, rest) = value
        .split_once("://")
        .map_or((destination.scheme.as_str(), value), |(s, r)| (s, r));
    let authority = rest
        .trim_start_matches("//")
        .split(['/', '?', '#'])
        .next()
        .unwrap_or("");
    let (userinfo, host) = authority
        .rsplit_once('@')
        .map_or((None, authority), |(u, h)| (Some(u), h));
    let auth = if let Some(userinfo) = userinfo {
        let (user, password) = userinfo.split_once(':').unwrap_or((userinfo, ""));
        if !user.is_empty() && !password.is_empty() {
            Some(format!(
                "Basic {}",
                STANDARD.encode(format!("{}:{}", unquote(user), unquote(password)).as_bytes())
            ))
        } else {
            None
        }
    } else {
        None
    };
    if destination.scheme == "http" && !matches!(scheme, "http" | "https") {
        return Err(Failure::Transport);
    }
    let tls = destination.scheme == "http" && scheme == "https";
    let default = if destination.scheme == "https" || tls {
        443
    } else {
        80
    };
    Ok(Proxy {
        endpoint: endpoint(&unquote(host), default)?,
        tls,
        auth,
    })
}
async fn connect(endpoint: &Endpoint) -> Result<Stream, Failure> {
    let socket = tokio::time::timeout(
        Duration::from_secs(15),
        TcpStream::connect((endpoint.host.as_str(), endpoint.port)),
    )
    .await
    .map_err(|_| Failure::Transport)?
    .map_err(|_| Failure::Transport)?;
    Ok(Box::new(Inactivity::new(socket)))
}
async fn encrypt(stream: Stream, endpoint: &Endpoint) -> Result<Stream, Failure> {
    let mut config = crate::tls::config()?;
    config.alpn_protocols = vec![b"http/1.1".to_vec()];
    let name = rustls::pki_types::ServerName::try_from(endpoint.host.clone())
        .map_err(|_| Failure::Transport)?;
    // CPython _ssl.c fixes a deadline for the whole SSL handshake, then for
    // each application read/write. Encrypted TCP fragment progress must not
    // restart either source operation's existing15-second budget.
    let connector = tokio_rustls::TlsConnector::from(Arc::new(config));
    let stream = tokio::time::timeout(Duration::from_secs(15), connector.connect(name, stream))
        .await
        .map_err(|_| Failure::Transport)?
        .map_err(|_error| {
            #[cfg(feature = "source-transport-test-fixtures")]
            crate::diagnostics::failure("tls", &_error);
            Failure::Transport
        })?;
    Ok(Box::new(Inactivity::new(stream)))
}
fn request(
    method: &str,
    target: String,
    host: &[u8],
    accept: Option<&str>,
    auth: Option<&str>,
) -> Result<Request<Empty>, Failure> {
    let mut builder = Request::builder()
        .method(method)
        .uri("/")
        .version(hyper::Version::HTTP_11)
        .header(hyper::header::HOST, host);
    if let Some(accept) = accept {
        builder = builder
            .header(hyper::header::ACCEPT_ENCODING, "identity")
            .header(hyper::header::USER_AGENT, "Python-urllib/3.13")
            .header(hyper::header::ACCEPT, accept)
            .header(hyper::header::CONNECTION, "close");
    }
    if let Some(auth) = auth {
        builder = builder.header(hyper::header::PROXY_AUTHORIZATION, auth);
    }
    let mut request = builder.body(Empty).map_err(|_| Failure::Transport)?;
    request
        .extensions_mut()
        .insert(SourceRequestTarget::new(target).map_err(|_| Failure::Transport)?);
    Ok(request)
}
async fn exchange(
    stream: Stream,
    mut request: Request<Empty>,
    tunnel: bool,
    drivers: &mut Drivers,
) -> Result<Response<Incoming>, Failure> {
    let mut builder = http1::Builder::new();
    builder
        .source_framing(true)
        .max_buf_size(100 * 65536 + 65536)
        .max_headers(100)
        .allow_obsolete_multiline_headers_in_responses(true);
    let (mut sender, connection) = builder
        .handshake(Bridge(HeaderPolicy::new(stream)))
        .await
        .map_err(|_| {
            #[cfg(feature = "source-transport-test-fixtures")]
            crate::diagnostics::record(crate::diagnostics::Stage::ClientBuilt(false));
            Failure::Transport
        })?;
    #[cfg(feature = "source-transport-test-fixtures")]
    crate::diagnostics::record(crate::diagnostics::Stage::ClientBuilt(true));
    drivers.0.push(tokio::spawn(async move {
        let _ = connection.with_upgrades().await;
    }));
    let (signal, mut observed) = tokio::sync::watch::channel(0u16);
    hyper::ext::on_informational(&mut request, move |info| {
        if tunnel || info.status().as_u16() != 100 {
            signal.send_replace(info.status().as_u16());
        }
    });
    let rejected = async {
        loop {
            if *observed.borrow() != 0 {
                return;
            }
            if observed.changed().await.is_err() {
                std::future::pending::<()>().await;
            }
        }
    };
    let response = tokio::select! {
        biased;
        ()=rejected=>return Err(Failure::Transport),
        response=sender.send_request(request)=>response.map_err(|_error|{
            #[cfg(feature="source-transport-test-fixtures")]
            crate::diagnostics::failure("send",&_error);
            Failure::Transport
        })?
    };
    // A fast 103 followed by 200 must not win the request-future race.
    if *observed.borrow() != 0 {
        return Err(Failure::Transport);
    }
    Ok(response)
}
async fn get(
    current: &str,
    destination: &Destination,
    route: Option<&str>,
    accept: &str,
    drivers: &mut Drivers,
) -> Result<Response<Incoming>, Failure> {
    let selected = route.map(|route| proxy(route, destination)).transpose()?;
    let remote = selected
        .as_ref()
        .map_or(&destination.endpoint, |p| &p.endpoint);
    let mut stream = connect(remote).await?;
    let mut target = destination.selector.clone();
    let mut auth = None;
    if let Some(selected) = &selected {
        if destination.scheme == "https" {
            let host = if destination.endpoint.host.contains(':') {
                format!(
                    "[{}]:{}",
                    destination.endpoint.host, destination.endpoint.port
                )
            } else {
                format!(
                    "{}:{}",
                    destination.endpoint.host, destination.endpoint.port
                )
            };
            let connect = request(
                "CONNECT",
                host.clone(),
                host.as_bytes(),
                None,
                selected.auth.as_deref(),
            )?;
            let response = exchange(stream, connect, true, drivers).await?;
            if response.status().as_u16() != 200 {
                return Err(Failure::Http(response.status().as_u16()));
            }
            stream = Box::new(Bridge(
                hyper::upgrade::on(response)
                    .await
                    .map_err(|_| Failure::Transport)?,
            ));
        } else {
            if selected.tls {
                stream = encrypt(stream, &selected.endpoint).await?;
            }
            target = current.into();
            auth = selected.auth.as_deref();
        }
    }
    if destination.scheme == "https" {
        stream = encrypt(stream, &destination.endpoint).await?;
    }
    // Credentials belong only to the selected current proxy hop. They never
    // follow a redirect to a direct origin or to another proxy.
    exchange(
        stream,
        request("GET", target, &destination.host_header, Some(accept), auth)?,
        false,
        drivers,
    )
    .await
}
async fn body(mut response: Response<Incoming>) -> Result<Vec<u8>, Failure> {
    let mut bytes = Vec::new();
    while let Some(frame) = poll_fn(|cx| Pin::new(response.body_mut()).poll_frame(cx)).await {
        let frame = frame.map_err(|_error| {
            #[cfg(feature = "source-transport-test-fixtures")]
            crate::diagnostics::failure("body", &_error);
            Failure::Transport
        })?;
        if let Ok(data) = frame.into_data() {
            bytes.extend_from_slice(&data);
        }
    }
    Ok(bytes)
}
pub async fn fetch(url: &str, accept: &str) -> Result<String, Failure> {
    let policy = crate::proxy::Policy::from_environment()?;
    let mut current = url.to_owned();
    let mut visited = BTreeMap::<String, usize>::new();
    loop {
        let destination = validate(&current)?;
        let route = policy.route(&destination.scheme, &destination.authority)?;
        let mut drivers = Drivers(Vec::new());
        let response = get(
            &current,
            &destination,
            route.as_deref(),
            accept,
            &mut drivers,
        )
        .await;
        let result = async {
            let response = response?;
            let status = response.status().as_u16();
            #[cfg(feature = "source-transport-test-fixtures")]
            crate::diagnostics::record(crate::diagnostics::Stage::Headers {
                status,
                declared: response
                    .headers()
                    .get(hyper::header::CONTENT_LENGTH)
                    .and_then(|h| h.to_str().ok())
                    .and_then(|s| s.parse().ok()),
            });
            let location = if matches!(status, 301 | 302 | 303 | 307 | 308) {
                response
                    .headers()
                    .get(hyper::header::LOCATION)
                    .or_else(|| response.headers().get("uri"))
                    .map(|h| h.as_bytes().to_vec())
            } else {
                None
            };
            if let Some(location) = location {
                let target = redirect_target(&current, &location)?;
                if visited.get(&target).copied().unwrap_or(0) >= 4 || visited.len() >= 10 {
                    return Err(Failure::Transport);
                }
                *visited.entry(target.clone()).or_default() += 1;
                body(response).await?;
                return Ok(Err(target));
            }
            if !(200..300).contains(&status) {
                return Err(Failure::Http(status));
            }
            let bytes = body(response).await?;
            #[cfg(feature = "source-transport-test-fixtures")]
            crate::diagnostics::record(crate::diagnostics::Stage::BodyComplete(bytes.len()));
            String::from_utf8(bytes).map(Ok).map_err(|_| Failure::Utf8)
        }
        .await;
        drivers.retire().await;
        match result? {
            Ok(text) => return Ok(text),
            Err(target) => current = target,
        }
    }
}

// HTTP(S) adaptation of CPython3.13.6 urllib.parse urlparse/urljoin/urlunparse.
// PSF license: docs/rust-rewrite/t08a/python-ssl-LICENSE.txt. This small local
// adapter preserves raw authority/params and source relative-path semantics;
// the vetted URL crate still validates every final outbound HTTP(S) URL.
#[derive(Default)]
struct Parts {
    scheme: String,
    authority: String,
    path: String,
    params: String,
    query: String,
    fragment: String,
}
fn split_http(value: &str, default_scheme: &str) -> Parts {
    let clean = value
        .trim_start_matches(|c: char| c.is_ascii() && c <= ' ')
        .replace(['\r', '\n', '\t'], "");
    let mut rest = clean.as_str();
    let mut scheme = default_scheme.to_owned();
    if let Some((candidate, tail)) = rest.split_once(':')
        && candidate
            .as_bytes()
            .first()
            .is_some_and(u8::is_ascii_alphabetic)
        && candidate
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'+' | b'-' | b'.'))
    {
        scheme = candidate.to_ascii_lowercase();
        rest = tail;
    }
    let mut authority = String::new();
    if let Some(tail) = rest.strip_prefix("//") {
        let end = tail.find(['/', '?', '#']).unwrap_or(tail.len());
        authority = tail[..end].to_owned();
        rest = &tail[end..];
    }
    let (rest, fragment) = rest.split_once('#').unwrap_or((rest, ""));
    let (path, query) = rest.split_once('?').unwrap_or((rest, ""));
    let segment = path.rfind('/').map_or(0, |i| i + 1);
    let (path, params) = path[segment..].find(';').map_or((path, ""), |i| {
        (&path[..segment + i], &path[segment + i + 1..])
    });
    Parts {
        scheme,
        authority,
        path: path.into(),
        params: params.into(),
        query: query.into(),
        fragment: fragment.into(),
    }
}
fn combine_http(parts: Parts) -> String {
    let mut path = parts.path;
    if !parts.params.is_empty() {
        path.push(';');
        path.push_str(&parts.params);
    }
    if !parts.authority.is_empty() {
        if !path.is_empty() && !path.starts_with('/') {
            path.insert(0, '/');
        }
        path = format!("//{}{path}", parts.authority);
    } else if path.starts_with("//")
        || (!parts.scheme.is_empty() && (path.is_empty() || path.starts_with('/')))
    {
        path = format!("//{path}");
    }
    if !parts.scheme.is_empty() {
        path = format!("{}:{path}", parts.scheme);
    }
    if !parts.query.is_empty() {
        path.push('?');
        path.push_str(&parts.query);
    }
    if !parts.fragment.is_empty() {
        path.push('#');
        path.push_str(&parts.fragment);
    }
    path
}
fn join_http(base: &str, relative: &str) -> String {
    if base.is_empty() {
        return relative.to_owned();
    }
    if relative.is_empty() {
        return base.to_owned();
    }
    let base = split_http(base, "");
    let mut next = split_http(relative, &base.scheme);
    if next.scheme != base.scheme || !matches!(next.scheme.as_str(), "http" | "https") {
        return relative.to_owned();
    }
    if !next.authority.is_empty() {
        return combine_http(next);
    }
    next.authority = base.authority;
    if next.path.is_empty() && next.params.is_empty() {
        next.path = base.path;
        next.params = base.params;
        if next.query.is_empty() {
            next.query = base.query;
        }
        return combine_http(next);
    }
    let mut segments = if next.path.starts_with('/') {
        next.path.split('/').collect::<Vec<_>>()
    } else {
        let mut parts = base.path.split('/').collect::<Vec<_>>();
        if parts.last().is_some_and(|s| !s.is_empty()) {
            parts.pop();
        }
        parts.extend(next.path.split('/'));
        let last = parts.len().saturating_sub(1);
        parts
            .into_iter()
            .enumerate()
            .filter_map(|(i, p)| (i == 0 || i == last || !p.is_empty()).then_some(p))
            .collect()
    };
    let trailing = segments.last().is_some_and(|p| matches!(*p, "." | ".."));
    let mut resolved = Vec::new();
    for segment in segments.drain(..) {
        match segment {
            ".." => {
                resolved.pop();
            }
            "." => {}
            part => resolved.push(part),
        }
    }
    if trailing {
        resolved.push("");
    }
    next.path = resolved.join("/");
    if next.path.is_empty() {
        next.path = "/".to_owned();
    }
    combine_http(next)
}

fn redirect_target(current: &str, raw: &[u8]) -> Result<String, Failure> {
    // HTTPRedirectHandler parses controls first, supplies an empty authority
    // path, then quotes recovered ISO-8859-1 bytes with string.punctuation safe.
    let source = raw.iter().map(|b| char::from(*b)).collect::<String>();
    let mut parts = split_http(&source, "");
    if !parts.authority.is_empty() && parts.path.is_empty() {
        parts.path = "/".to_owned();
    }
    let parsed = combine_http(parts);
    let mut quoted = String::new();
    for point in parsed.chars() {
        let byte = u8::try_from(u32::from(point)).map_err(|_| Failure::Transport)?;
        if (0x21..=0x7e).contains(&byte) {
            quoted.push(char::from(byte));
        } else {
            quoted.push_str(&format!("%{byte:02X}"));
        }
    }
    let result = join_http(current, &quoted);
    validate(&result)?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn source_idna_encoder_all_latin1_and_boundary_cases() {
        let source: serde_json::Value = serde_json::from_str(include_str!(
            "../../../docs/rust-rewrite/t08a/additional-host-wire-observations.json"
        ))
        .unwrap();
        let cases = source["source_idna_encoder_cases"].as_array().unwrap();
        assert_eq!(cases.len(), 138);
        for case in cases {
            let actual =
                ssx_source_codecs::encode_idna2003_host(case["host"].as_str().unwrap()).unwrap();
            assert_eq!(actual.as_deref(), case["ascii"].as_str(), "{case}");
        }
    }
    #[test]
    fn source_relative_redirect_join_preserves_raw_authority_parameters_and_dots() {
        let source: serde_json::Value = serde_json::from_str(include_str!(
            "../../../docs/rust-rewrite/t08a/additional-urljoin-observations.json"
        ))
        .unwrap();
        let cases = source["cases"].as_array().unwrap();
        assert_eq!(cases.len(), 15);
        for case in cases {
            assert_eq!(
                join_http(
                    case["base"].as_str().unwrap(),
                    case["location"].as_str().unwrap()
                ),
                case["urljoin"].as_str().unwrap(),
                "{case}"
            );
        }
    }

    #[tokio::test]
    async fn actual_builder_source_opt_in_preserves_default_connection_framing() {
        use std::io::Read;
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let mut wire = Vec::new();
        flate2::read::GzDecoder::new(
            include_bytes!("../../../docs/rust-rewrite/t08a/wire/trailers-101-100.bin.gz")
                .as_slice(),
        )
        .read_to_end(&mut wire)
        .unwrap();
        for source in [false, true] {
            let (client, mut fixture) = tokio::io::duplex(256);
            let reply = wire.clone();
            let mut server = tokio::spawn(async move {
                let mut header = Vec::new();
                let mut byte = [0];
                while !header.ends_with(b"\r\n\r\n") {
                    assert!(header.len() < 8192);
                    fixture.read_exact(&mut byte).await.unwrap();
                    header.push(byte[0]);
                }
                assert!(header.starts_with(b"GET / HTTP/1.1\r\n"));
                fixture.write_all(&reply).await.is_ok()
            });
            let mut builder = http1::Builder::new();
            // The false case makes no API call: compiled feature presence must
            // not globally relax the default Builder -> Conn -> Decoder path.
            if source {
                builder.source_framing(true);
            }
            let (mut sender, connection) = builder.handshake(Bridge(client)).await.unwrap();
            let driver = tokio::spawn(connection);
            let request = Request::builder()
                .uri("/")
                .header("host", "fixture")
                .body(Empty)
                .unwrap();
            let result = match sender.send_request(request).await {
                Ok(response) => body(response).await,
                Err(_) => Err(Failure::Transport),
            };
            driver.abort();
            let _ = driver.await;
            let completed = match tokio::time::timeout(Duration::from_secs(2), &mut server).await {
                Ok(result) => result.unwrap(),
                Err(_) => {
                    server.abort();
                    let _ = server.await;
                    panic!("owned in-memory wire fixture deadline");
                }
            };
            if source {
                assert_eq!(result.unwrap(), b"{}");
                assert!(completed, "all captured wire bytes delivered");
            } else {
                assert!(
                    result.is_err(),
                    "default connection retains upstream trailer-count limit"
                );
            }
        }
    }
    #[tokio::test(start_paused = true)]
    async fn tls_handshake_deadline_survives_observed_partial_record_progress() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt, ReadBuf};
        struct Observed(Stream, tokio::sync::watch::Sender<usize>);
        impl AsyncRead for Observed {
            fn poll_read(
                self: Pin<&mut Self>,
                cx: &mut Context<'_>,
                buffer: &mut ReadBuf<'_>,
            ) -> Poll<std::io::Result<()>> {
                let this = self.get_mut();
                let before = buffer.filled().len();
                let result = Pin::new(&mut this.0).poll_read(cx, buffer);
                let count = buffer.filled().len() - before;
                if count != 0 {
                    this.1.send_modify(|total| *total += count);
                }
                result
            }
        }
        impl AsyncWrite for Observed {
            fn poll_write(
                self: Pin<&mut Self>,
                cx: &mut Context<'_>,
                bytes: &[u8],
            ) -> Poll<std::io::Result<usize>> {
                Pin::new(&mut self.get_mut().0).poll_write(cx, bytes)
            }
            fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
                Pin::new(&mut self.get_mut().0).poll_flush(cx)
            }
            fn poll_shutdown(
                self: Pin<&mut Self>,
                cx: &mut Context<'_>,
            ) -> Poll<std::io::Result<()>> {
                Pin::new(&mut self.get_mut().0).poll_shutdown(cx)
            }
        }
        let (client, mut fixture) = tokio::io::duplex(8192);
        let (sent, mut observed) = tokio::sync::watch::channel(0usize);
        let started = tokio::time::Instant::now();
        let handshake = tokio::spawn(async move {
            encrypt(
                Box::new(Observed(Box::new(Inactivity::new(client)), sent)),
                &Endpoint {
                    host: "localhost".into(),
                    port: 0,
                },
            )
            .await
        });
        let mut header = [0; 5];
        fixture.read_exact(&mut header).await.unwrap();
        assert_eq!(header[0], 22);
        let length = usize::from(u16::from_be_bytes([header[3], header[4]]));
        assert!(length <= 8192);
        fixture.read_exact(&mut vec![0; length]).await.unwrap();
        tokio::time::advance(Duration::from_secs(14)).await;
        fixture.write_all(&[22]).await.unwrap();
        observed.wait_for(|count| *count == 1).await.unwrap(); // actual Rustls read witness
        assert!(!handshake.is_finished());
        tokio::time::advance(Duration::from_secs(1)).await;
        assert!(matches!(handshake.await.unwrap(), Err(Failure::Transport)));
        assert_eq!(
            started.elapsed(),
            Duration::from_secs(15),
            "a reset TCP timer must not substitute a later29-second TLS failure"
        );
    }
}
