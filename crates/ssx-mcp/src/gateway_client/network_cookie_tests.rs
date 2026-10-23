//! Joined deterministic wire controls. IP endpoints are local fixture endpoints;
//! these tests do not replace the separate native DNS/process acceptance cases.
use super::*;
use reqwest::header::{COOKIE, HeaderMap, HeaderValue, PROXY_AUTHORIZATION};
use reqwest::source_pool::SourceHttp1PoolConfig;
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt},
    net::TcpListener,
};

struct LoopbackDns(AtomicUsize);
impl reqwest::dns::Resolve for LoopbackDns {
    fn resolve(&self, _: reqwest::dns::Name) -> reqwest::dns::Resolving {
        self.0.fetch_add(1, Ordering::SeqCst);
        Box::pin(async {
            Ok(
                Box::new(vec!["127.0.0.1:0".parse::<std::net::SocketAddr>().unwrap()].into_iter())
                    as reqwest::dns::Addrs,
            )
        })
    }
}
fn network(config: SourceHttp1PoolConfig, cookies: Arc<dyn SourceCookieStore>) -> Arc<Network> {
    network_with_dns(config, cookies, Arc::new(LoopbackDns(AtomicUsize::new(0))))
}
fn network_with_dns(
    config: SourceHttp1PoolConfig,
    cookies: Arc<dyn SourceCookieStore>,
    dns: Arc<LoopbackDns>,
) -> Arc<Network> {
    let connect = Duration::from_secs(2);
    let client = reqwest::source_pool::gateway_builder(connect, &Default::default())
        .unwrap()
        .source_http1_pool(config)
        .dns_resolver(dns)
        .build()
        .unwrap();
    network_from_client(client, connect, cookies)
}
fn network_from_client(
    client: reqwest::Client,
    connect: Duration,
    cookies: Arc<dyn SourceCookieStore>,
) -> Arc<Network> {
    let pool = client.source_http1_pool_control().unwrap();
    use sha2::Digest;
    let exe = std::env::current_exe().unwrap();
    let sha = sha2::Sha256::digest(std::fs::read(&exe).unwrap()).into();
    let helper = super::super::resolver::SourceResolverHelper::new(exe, sha).unwrap();
    Arc::new(Network {
        client,
        pool,
        connect,
        oauth: false,
        cookies,
        stop: CancellationToken::new(),
        resolver: super::super::resolver::GatewayResolver::new(helper, connect),
    })
}
fn direct(cookies: Arc<dyn SourceCookieStore>) -> Arc<Network> {
    network(
        SourceHttp1PoolConfig::new(Duration::from_secs(2), Vec::new()).unwrap(),
        cookies,
    )
}
async fn request<S: AsyncRead + Unpin>(stream: &mut S) -> Result<String> {
    let mut bytes = Vec::new();
    while !bytes.ends_with(b"\r\n\r\n") {
        let mut b = [0];
        if stream.read(&mut b).await? == 0 {
            return Err("fixture request EOF".into());
        }
        bytes.push(b[0]);
        if bytes.len() > 16384 {
            return Err("fixture header overflow".into());
        }
    }
    Ok(String::from_utf8(bytes)?.to_ascii_lowercase())
}
async fn reply<S: AsyncWrite + Unpin>(stream: &mut S, headers: &str) -> Result<()> {
    stream
        .write_all(
            format!("HTTP/1.1 {headers}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
                .as_bytes(),
        )
        .await?;
    stream.shutdown().await?;
    Ok(())
}
#[tokio::test]
async fn response_cookie_redirect_selection_replaces_caller_and_persists_deletion() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let net = direct(Arc::new(SourceCookieJar::default()));
    let server = async {
        let mut wires = Vec::new();
        for headers in [
            "302 Found\r\nLocation: /scope/next\r\nSet-Cookie: root=one; Path=/\r\nSet-Cookie: scoped=two; Path=/scope\r\nSet-Cookie: secure=hidden; Secure; Path=/",
            "200 OK\r\nSet-Cookie: root=deleted; Max-Age=0; Path=/",
            "200 OK",
        ] {
            let (mut stream, _) = listener.accept().await?;
            wires.push(request(&mut stream).await?);
            reply(&mut stream, headers).await?;
        }
        Ok::<_, Box<dyn std::error::Error + Send + Sync>>(wires)
    };
    let client = async {
        let mut headers = HeaderMap::new();
        headers.insert(COOKIE, HeaderValue::from_static("caller=first-only"));
        net.send(
            reqwest::Method::GET,
            format!("{base}/start"),
            headers,
            Vec::new(),
        )
        .await?;
        net.send_once(
            reqwest::Method::GET,
            format!("{base}/scope/last"),
            HeaderMap::new(),
            Vec::new(),
        )
        .await?;
        Ok::<_, Box<dyn std::error::Error + Send + Sync>>(())
    };
    let outcome = tokio::time::timeout(Duration::from_secs(10), async {
        tokio::try_join!(server, client)
    })
    .await;
    net.shutdown().await.unwrap();
    let (wires, ()) = outcome.unwrap().unwrap();
    assert!(wires[0].contains("cookie: caller=first-only\r\n"));
    assert!(
        !wires[1].contains("caller="),
        "redirect caller-cookie remerge mutant"
    );
    assert!(
        wires[1].contains("cookie: scoped=two; root=one\r\n"),
        "cookie extraction/path-order mutant"
    );
    assert!(
        !wires[1].contains("secure=hidden"),
        "secure cookie over plaintext mutant"
    );
    assert!(
        wires[2].contains("cookie: scoped=two\r\n"),
        "cookie deletion/persistence mutant"
    );
    assert!(!wires[2].contains("root="));
    let separate = direct(Arc::new(SourceCookieJar::default()));
    let mut headers = HeaderMap::new();
    separate
        .prepare_cookies(&format!("{base}/scope/last"), &mut headers)
        .unwrap();
    assert!(
        !headers.contains_key(COOKIE),
        "independent network jars shared state"
    );
    separate.shutdown().await.unwrap();
}
struct RedirectFailure(AtomicUsize);
impl SourceCookieStore for RedirectFailure {
    fn has_cookies(&self) -> bool {
        true
    }
    fn cookies(&self, _: &SourceCookieView) -> reqwest::Result<Option<HeaderValue>> {
        if self.0.fetch_add(1, Ordering::SeqCst) == 0 {
            Ok(None)
        } else {
            Err(reqwest::Client::new()
                .get("://invalid")
                .build()
                .unwrap_err())
        }
    }
    fn extract(&self, _: &SourceCookieView, _: &HeaderMap) -> reqwest::Result<()> {
        Ok(())
    }
}
#[tokio::test]
async fn unfollowed_redirect_prepares_cookie_before_origin_method_and_budget_gates() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let jar = Arc::new(RedirectFailure(AtomicUsize::new(0)));
    let net = direct(jar.clone());
    let server = async {
        let (mut stream, _) = listener.accept().await?;
        request(&mut stream).await?;
        reply(
            &mut stream,
            "302 Found\r\nLocation: http://other.invalid/next",
        )
        .await
    };
    let client = async {
        let result = net
            .send(reqwest::Method::POST, base, HeaderMap::new(), Vec::new())
            .await;
        if result.is_ok() {
            return Err("cookie construction was skipped by unfollowed redirect mutant".into());
        }
        Ok::<_, Box<dyn std::error::Error + Send + Sync>>(())
    };
    let outcome = tokio::time::timeout(Duration::from_secs(10), async {
        tokio::try_join!(server, client)
    })
    .await;
    net.shutdown().await.unwrap();
    outcome.unwrap().unwrap();
    assert_eq!(jar.0.load(Ordering::SeqCst), 2);
}
#[tokio::test]
async fn two_real_proxy_routes_keep_caller_precedence_and_no_proxy_has_no_ambient_credentials() {
    let first = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let second = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin_url = format!("http://{}/", origin.local_addr().unwrap());
    let config = SourceHttp1PoolConfig::new(
        Duration::from_secs(2),
        vec![
            (
                "http://one.invalid".into(),
                Some(format!("http://first:one@{}", first.local_addr().unwrap())),
            ),
            (
                "http://two.invalid".into(),
                Some(format!(
                    "http://second:two@{}",
                    second.local_addr().unwrap()
                )),
            ),
            ("all://127.0.0.1".into(), None),
        ],
    )
    .unwrap();
    let net = network(config, Arc::new(SourceCookieJar::default()));
    async fn collect(listener: TcpListener) -> Result<Vec<String>> {
        let mut wires = Vec::new();
        for _ in 0..2 {
            let (mut stream, _) = listener.accept().await?;
            wires.push(request(&mut stream).await?);
            reply(&mut stream, "200 OK").await?;
        }
        Ok(wires)
    }
    let client = async {
        for caller in [None, Some("Bearer explicit-caller")] {
            for url in ["http://one.invalid/", "http://two.invalid/", &origin_url] {
                let mut headers = HeaderMap::new();
                if let Some(value) = caller {
                    headers.insert(PROXY_AUTHORIZATION, HeaderValue::from_static(value));
                }
                net.send_once(reqwest::Method::GET, url.into(), headers, Vec::new())
                    .await?;
            }
        }
        Ok::<_, Box<dyn std::error::Error + Send + Sync>>(())
    };
    let outcome = tokio::time::timeout(Duration::from_secs(10), async {
        tokio::try_join!(collect(first), collect(second), collect(origin), client)
    })
    .await;
    net.shutdown().await.unwrap();
    let (a, b, direct, ()) = outcome.unwrap().unwrap();
    assert!(a[0].contains("proxy-authorization: basic zmlyc3q6b25l\r\n"));
    assert!(b[0].contains("proxy-authorization: basic c2vjb25konr3bw==\r\n"));
    assert!(
        !direct[0].contains("proxy-authorization:"),
        "ambient proxy credential bypass leak mutant"
    );
    for wire in [&a[1], &b[1], &direct[1]] {
        assert!(
            wire.contains("proxy-authorization: bearer explicit-caller\r\n"),
            "caller proxy removal/overwrite mutant"
        );
        assert!(!wire.contains("zmlyc3q6b25l"));
        assert!(!wire.contains("c2vjb25konr3bw=="));
    }
}
#[tokio::test]
async fn environment_no_proxy_route_bypasses_configured_credentials_on_actual_wire() {
    let proxy = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/", origin.local_addr().unwrap());
    let config = SourceHttp1PoolConfig::from_proxy_info(
        Duration::from_secs(2),
        &std::collections::BTreeMap::from([
            (
                "http".into(),
                format!("http://synthetic:secret@{}", proxy.local_addr().unwrap()),
            ),
            ("no".into(), "127.0.0.1".into()),
        ]),
    )
    .unwrap();
    let net = network(config, Arc::new(SourceCookieJar::default()));
    let server = async {
        let (mut stream, _) = origin.accept().await?;
        let wire = request(&mut stream).await?;
        reply(&mut stream, "200 OK").await?;
        Ok::<_, Box<dyn std::error::Error + Send + Sync>>(wire)
    };
    let client = async {
        net.send_once(reqwest::Method::GET, url, HeaderMap::new(), Vec::new())
            .await?;
        Ok::<_, Box<dyn std::error::Error + Send + Sync>>(())
    };
    let result = tokio::time::timeout(Duration::from_secs(10), async {
        tokio::try_join!(server, client)
    })
    .await;
    net.shutdown().await.unwrap();
    let (wire, ()) = result.unwrap().unwrap();
    assert!(
        !wire.contains("proxy-authorization:"),
        "NO_PROXY generated credential leak mutant"
    );
    std::future::poll_fn(|cx| {
        assert!(
            proxy.poll_accept(cx).is_pending(),
            "NO_PROXY contacted proxy mutant"
        );
        std::task::Poll::Ready(())
    })
    .await;
}
#[tokio::test]
async fn queued_caller_cancellation_never_resolves_and_released_credit_uses_new_caller() {
    use std::{
        future::{Future, poll_fn},
        task::Poll,
    };
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let dns = Arc::new(LoopbackDns(AtomicUsize::new(0)));
    let config = SourceHttp1PoolConfig::new(
        Duration::from_secs(5),
        vec![(
            "http://one.invalid".into(),
            Some(format!(
                "http://synthetic:secret@proxy.fixture:{}",
                listener.local_addr().unwrap().port()
            )),
        )],
    )
    .unwrap();
    let net = network_with_dns(config, Arc::new(SourceCookieJar::default()), dns.clone());
    let (full_tx, full_rx) = tokio::sync::oneshot::channel();
    let (retired_tx, retired_rx) = tokio::sync::oneshot::channel();
    let server = async {
        let mut sockets = Vec::new();
        for _ in 0..100 {
            let (mut stream, _) = listener.accept().await?;
            let wire = request(&mut stream).await?;
            if !wire.contains("proxy-authorization: basic c3ludghldgljonnly3jlda==\r\n") {
                return Err("held proxy default header differs".into());
            }
            stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 1\r\n\r\n")
                .await?;
            let first = wire
                .lines()
                .next()
                .is_some_and(|line| line.ends_with("/held/0 http/1.1"));
            sockets.push((first, stream));
        }
        full_tx.send(()).map_err(|_| "full peer gone")?;
        // Client drops exactly the first held body. Physical EOF acknowledges
        // its original stream's retirement before a replacement is requested.
        let mut one = [0];
        let retired = sockets
            .iter_mut()
            .find(|(first, _)| *first)
            .ok_or("first held request not observed")?
            .1
            .read(&mut one)
            .await?;
        if retired != 0 {
            return Err("original held body did not physically retire".into());
        }
        retired_tx.send(()).map_err(|_| "retirement peer gone")?;
        let (mut stream, _) = listener.accept().await?;
        let wire = request(&mut stream).await?;
        reply(&mut stream, "200 OK").await?;
        Ok::<_, Box<dyn std::error::Error + Send + Sync>>(wire)
    };
    let client = async {
        let futures = (0..100).map(|index| {
            net.send_once(
                reqwest::Method::GET,
                format!("http://one.invalid/held/{index}"),
                HeaderMap::new(),
                Vec::new(),
            )
        });
        let mut held = futures_util::future::try_join_all(futures).await?;
        full_rx.await?;
        if dns.0.load(Ordering::SeqCst) != 100 {
            return Err("fixture did not establish exactly 100 DNS admissions".into());
        }
        let mut headers = HeaderMap::new();
        headers.insert(
            PROXY_AUTHORIZATION,
            HeaderValue::from_static("Bearer canceled-caller"),
        );
        let mut canceled = Box::pin(net.send_once(
            reqwest::Method::GET,
            "http://one.invalid/canceled".into(),
            headers,
            Vec::new(),
        ));
        let pending = poll_fn(|cx| Poll::Ready(canceled.as_mut().poll(cx).is_pending())).await;
        if !pending {
            return Err("full mount did not queue request".into());
        }
        if dns.0.load(Ordering::SeqCst) != 100 {
            return Err("queued cancellation DNS mutant".into());
        }
        drop(canceled);
        drop(held.remove(0));
        retired_rx.await?;
        let mut headers = HeaderMap::new();
        headers.insert(
            PROXY_AUTHORIZATION,
            HeaderValue::from_static("Bearer replacement-caller"),
        );
        let response = net
            .send_once(
                reqwest::Method::GET,
                "http://one.invalid/replacement".into(),
                headers,
                Vec::new(),
            )
            .await?;
        if dns.0.load(Ordering::SeqCst) != 101 {
            return Err("canceled queued request consumed DNS/released credit".into());
        }
        drop(response);
        drop(held);
        Ok::<_, Box<dyn std::error::Error + Send + Sync>>(())
    };
    let result = tokio::time::timeout(Duration::from_secs(20), async {
        tokio::try_join!(server, client)
    })
    .await;
    net.shutdown().await.unwrap();
    let (wire, ()) = result.unwrap().unwrap();
    assert!(
        wire.contains("proxy-authorization: bearer replacement-caller\r\n"),
        "released credit caller inheritance mutant"
    );
    assert!(!wire.contains("canceled-caller"));
    assert!(!wire.contains("c3ludghldgljonnly3jlda=="));
}
#[tokio::test]
async fn connect_uses_selected_proxy_credentials_separately_from_origin_caller_header() {
    let proxy = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let config = SourceHttp1PoolConfig::new(
        Duration::from_secs(2),
        vec![(
            "https://one.invalid".into(),
            Some(format!(
                "http://synthetic:secret@{}",
                proxy.local_addr().unwrap()
            )),
        )],
    )
    .unwrap();
    let net = network(config, Arc::new(SourceCookieJar::default()));
    let server = async {
        let (mut stream, _) = proxy.accept().await?;
        let wire = request(&mut stream).await?;
        stream
            .write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n")
            .await?;
        let mut prefix = [0; 5];
        stream.read_exact(&mut prefix).await?;
        stream.shutdown().await?;
        Ok::<_, Box<dyn std::error::Error + Send + Sync>>((wire, prefix))
    };
    let client = async {
        let mut headers = HeaderMap::new();
        headers.insert(
            PROXY_AUTHORIZATION,
            HeaderValue::from_static("Bearer origin-caller"),
        );
        if net
            .send_once(
                reqwest::Method::GET,
                "https://one.invalid/".into(),
                headers,
                Vec::new(),
            )
            .await
            .is_ok()
        {
            return Err("fixture TLS unexpectedly completed".into());
        }
        Ok::<_, Box<dyn std::error::Error + Send + Sync>>(())
    };
    let result = tokio::time::timeout(Duration::from_secs(10), async {
        tokio::try_join!(server, client)
    })
    .await;
    net.shutdown().await.unwrap();
    let ((wire, prefix), ()) = result.unwrap().unwrap();
    assert!(wire.starts_with("connect one.invalid:443 http/1.1\r\n"));
    assert!(
        wire.contains("proxy-authorization: basic c3ludghldgljonnly3jlda==\r\n"),
        "CONNECT proxy credentials overwritten by origin caller mutant"
    );
    assert!(!wire.contains("origin-caller"));
    assert_eq!(prefix[0], 0x16);
    assert_eq!(prefix[1], 0x03);
    // This witnesses CONNECT selection and origin TLS start only. The peer
    // intentionally closes before certificate verification/inner-origin HTTP.
}
#[tokio::test]
async fn oauth_built_request_does_not_merge_client_cookie_but_redirect_uses_shared_jar() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let net = direct(Arc::new(SourceCookieJar::default()));
    let server = async {
        let mut wires = Vec::new();
        for headers in [
            "200 OK\r\nSet-Cookie: shared=source; Path=/",
            "307 Temporary Redirect\r\nLocation: /oauth/next",
            "200 OK",
        ] {
            let (mut stream, _) = listener.accept().await?;
            wires.push(request(&mut stream).await?);
            reply(&mut stream, headers).await?;
        }
        Ok::<_, Box<dyn std::error::Error + Send + Sync>>(wires)
    };
    let client = async {
        net.send_once(
            reqwest::Method::GET,
            format!("{base}/mcp"),
            HeaderMap::new(),
            Vec::new(),
        )
        .await?;
        net.send_built_request(
            reqwest::Method::POST,
            format!("{base}/oauth"),
            HeaderMap::new(),
            Vec::new(),
            5,
        )
        .await?;
        Ok::<_, Box<dyn std::error::Error + Send + Sync>>(())
    };
    let result = tokio::time::timeout(Duration::from_secs(10), async {
        tokio::try_join!(server, client)
    })
    .await;
    net.shutdown().await.unwrap();
    let (wires, ()) = result.unwrap().unwrap();
    assert!(
        !wires[1].contains("cookie:"),
        "OAuth bare Request merged client cookies mutant"
    );
    assert!(
        wires[2].contains("cookie: shared=source\r\n"),
        "OAuth redirect did not use shared jar mutant"
    );
}
#[tokio::test]
async fn uppercase_configured_https_returns_secure_cookie_on_next_initial_request() {
    // Reuse the workspace's fresh rcgen/rustls private-root fixture pattern.
    // Verification stays enabled: this test trusts only its explicit test root.
    let certificate = rcgen::generate_simple_self_signed(vec!["127.0.0.1".into()]).unwrap();
    let root = reqwest::Certificate::from_der(certificate.cert.der()).unwrap();
    let config = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(
            vec![certificate.cert.der().clone()],
            rustls::pki_types::PrivatePkcs8KeyDer::from(certificate.signing_key.serialize_der())
                .into(),
        )
        .unwrap();
    let acceptor = tokio_rustls::TlsAcceptor::from(Arc::new(config));
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let configured = format!("HTTPS://{}/mcp", listener.local_addr().unwrap());
    let connect = Duration::from_secs(2);
    let client = reqwest::source_pool::gateway_builder(connect, &Default::default())
        .unwrap()
        .tls_certs_only([root])
        .dns_resolver(Arc::new(LoopbackDns(AtomicUsize::new(0))))
        .build()
        .unwrap();
    let net = network_from_client(client, connect, Arc::new(SourceCookieJar::default()));
    let server = async {
        let mut wires = Vec::new();
        for headers in [
            "200 OK\r\nSet-Cookie: session=secure-source; Secure; Path=/",
            "200 OK",
        ] {
            let (socket, _) = listener.accept().await?;
            let mut stream = acceptor.accept(socket).await?;
            wires.push(request(&mut stream).await?);
            reply(&mut stream, headers).await?;
        }
        Ok::<_, Box<dyn std::error::Error + Send + Sync>>(wires)
    };
    let client = async {
        for _ in 0..2 {
            net.send(
                reqwest::Method::GET,
                configured.clone(),
                HeaderMap::new(),
                Vec::new(),
            )
            .await?;
        }
        Ok::<_, Box<dyn std::error::Error + Send + Sync>>(())
    };
    let result = tokio::time::timeout(Duration::from_secs(10), async {
        tokio::try_join!(server, client)
    })
    .await;
    net.shutdown().await.unwrap();
    let (wires, ()) = result.unwrap().unwrap();
    assert!(!wires[0].contains("cookie:"));
    assert!(
        wires[1].contains("cookie: session=secure-source\r\n"),
        "uppercase configured HTTPS omitted Secure cookie mutant"
    );
}
#[tokio::test]
async fn zero_redirect_budget_still_prepares_same_origin_get_307_cookie() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}/start", listener.local_addr().unwrap());
    let jar = Arc::new(RedirectFailure(AtomicUsize::new(0)));
    let net = direct(jar.clone());
    let server = async {
        let (mut stream, _) = listener.accept().await?;
        let wire = request(&mut stream).await?;
        reply(&mut stream, "307 Temporary Redirect\r\nLocation: /next").await?;
        Ok::<_, Box<dyn std::error::Error + Send + Sync>>(wire)
    };
    let client = async {
        let result = net
            .send_loop(
                reqwest::Method::GET,
                base,
                HeaderMap::new(),
                Vec::new(),
                0,
                true,
            )
            .await;
        if result.is_ok() {
            return Err("zero-budget cookie construction skipped mutant".into());
        }
        Ok::<_, Box<dyn std::error::Error + Send + Sync>>(())
    };
    let result = tokio::time::timeout(Duration::from_secs(10), async {
        tokio::try_join!(server, client)
    })
    .await;
    net.shutdown().await.unwrap();
    let (wire, ()) = result.unwrap().unwrap();
    assert!(wire.starts_with("get /start http/1.1\r\n"));
    assert_eq!(
        jar.0.load(Ordering::SeqCst),
        2,
        "exhausted budget bypassed redirect cookie construction"
    );
    std::future::poll_fn(|cx| {
        assert!(
            listener.poll_accept(cx).is_pending(),
            "zero budget sent a second request"
        );
        std::task::Poll::Ready(())
    })
    .await;
}
