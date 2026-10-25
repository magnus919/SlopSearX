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
struct LoopbackScope {
    address: std::net::SocketAddr,
}
impl reqwest::dns::SourceCandidateResolver for LoopbackScope {
    fn cancel_and_wait(&self) -> reqwest::dns::SourceRetiring {
        Box::pin(async { Ok(()) })
    }
    fn resolve_candidate(
        &self,
        _: reqwest::dns::SourceName,
        _: reqwest::dns::SourceCandidate,
    ) -> reqwest::dns::SourceCandidateResolving {
        let address = self.address;
        Box::pin(async move { Ok(vec![address]) })
    }
}
impl reqwest::dns::Resolve for LoopbackDns {
    fn resolve(&self, _: reqwest::dns::Name) -> reqwest::dns::Resolving {
        panic!("Network must use the source endpoint callback, never DNS-only projection")
    }
    fn resolve_source(&self, name: reqwest::dns::SourceName) -> reqwest::dns::SourceResolving {
        self.0.fetch_add(1, Ordering::SeqCst);
        let port = name.origin_port().unwrap().parse::<u16>().unwrap();
        let owner: Arc<dyn reqwest::dns::SourceCandidateResolver> = Arc::new(LoopbackScope {
            address: std::net::SocketAddr::from(([127, 0, 0, 1], port)),
        });
        let resolver = owner.clone();
        reqwest::dns::SourceResolving {
            owner,
            future: Box::pin(async move {
                Ok(reqwest::dns::SourceResolution {
                    resolver,
                    candidates: vec![reqwest::dns::SourceCandidate {
                        family: reqwest::dns::SourceFamily::V4,
                        native_host_text: "127.0.0.1".into(),
                    }],
                })
            }),
        }
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
    let client = reqwest::source_pool::source_request_builder(connect, &Default::default())
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
        SourceHttp1PoolConfig::new_source(Duration::from_secs(2), Vec::new()).unwrap(),
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
    let config = SourceHttp1PoolConfig::new_source(
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
    let config = SourceHttp1PoolConfig::from_proxy_info_source(
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
        // Either actual route is served by this same owned future. An incorrect
        // proxy dial therefore completes HTTP and reaches the named route witness.
        let (mut stream, via_proxy) = tokio::select! {
            value = origin.accept() => (value?.0, false),
            value = proxy.accept() => (value?.0, true),
        };
        let wire = request(&mut stream).await?;
        reply(&mut stream, "200 OK").await?;
        Ok::<_, Box<dyn std::error::Error + Send + Sync>>((wire, via_proxy))
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
    let ((wire, via_proxy), ()) = result.unwrap().unwrap();
    assert!(!via_proxy, "NO_PROXY contacted proxy mutant");
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
    let config = SourceHttp1PoolConfig::new_source(
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
    let config = SourceHttp1PoolConfig::new_source(
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
    let client = reqwest::source_pool::source_request_builder(connect, &Default::default())
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

#[tokio::test]
async fn source_url_auth_flag_preserves_owner_and_explicit_auth_on_actual_wire() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let net = direct(Arc::new(SourceCookieJar::default()));
    let owner = Arc::new(
        SourceUrl::parse(&format!(
            "HTTP://us%65r:p%2Bass@{}/a/../path?#fragment",
            listener.local_addr().unwrap()
        ))
        .unwrap(),
    );
    let server = async {
        let mut wires = Vec::new();
        for _ in 0..5 {
            let (mut stream, _) = listener.accept().await?;
            wires.push(request(&mut stream).await?);
            reply(&mut stream, "200 OK").await?;
        }
        Ok::<_, Box<dyn std::error::Error + Send + Sync>>(wires)
    };
    let client = async {
        for (enabled, explicit) in [
            (false, None),
            (true, None),
            (false, Some(false)),
            (true, Some(false)),
            (false, Some(true)),
        ] {
            let mut builder = net
                .client
                .source_request(reqwest::Method::GET, owner.clone())
                .url_auth(enabled);
            if let Some(bearer) = explicit {
                builder = if bearer {
                    builder.bearer_auth("configured")
                } else {
                    builder.basic_auth("configured", Some("secret"))
                };
            }
            // try_clone must preserve suppression as well as exact owner.
            let response = builder.try_clone().unwrap().send().await?;
            assert!(Arc::ptr_eq(response.source_url(), &owner));
            assert_eq!(response.source_url().raw_userinfo(), "us%65r:p%2Bass");
        }
        Ok::<_, Box<dyn std::error::Error + Send + Sync>>(())
    };
    let outcome = tokio::time::timeout(Duration::from_secs(10), async {
        tokio::try_join!(server, client)
    })
    .await;
    net.shutdown().await.unwrap();
    let (wires, ()) = outcome.unwrap().unwrap();
    assert!(
        !wires[0].contains("authorization:"),
        "disabled URL auth leaked userinfo Basic"
    );
    assert!(wires[1].contains("authorization: basic dxnlcjpwk2fzcw==\r\n"));
    for wire in [&wires[2], &wires[3]] {
        assert!(wire.contains("authorization: basic y29uzmlndxjlzdpzzwnyzxq=\r\n"));
    }
    assert!(wires[4].contains("authorization: bearer configured\r\n"));
    for wire in wires {
        assert!(wire.starts_with("get /path? http/1.1\r\n"));
        assert!(!wire.contains("us%65r"));
    }
}
#[tokio::test]
async fn network_oauth_retains_userinfo_but_never_uses_url_basic_without_bearer() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let mut net = direct(Arc::new(SourceCookieJar::default()));
    Arc::get_mut(&mut net).unwrap().oauth = true;
    let url = format!("http://user:pass@{}/oauth", listener.local_addr().unwrap());
    let server = async {
        let (mut stream, _) = listener.accept().await?;
        let wire = request(&mut stream).await?;
        reply(&mut stream, "200 OK").await?;
        Ok::<_, Box<dyn std::error::Error + Send + Sync>>(wire)
    };
    let client = async {
        let response = net
            .send_once(reqwest::Method::GET, url, HeaderMap::new(), Vec::new())
            .await?;
        assert_eq!(response.source_url().raw_userinfo(), "user:pass");
        Ok::<_, Box<dyn std::error::Error + Send + Sync>>(())
    };
    let outcome = tokio::time::timeout(Duration::from_secs(10), async {
        tokio::try_join!(server, client)
    })
    .await;
    net.shutdown().await.unwrap();
    let (wire, ()) = outcome.unwrap().unwrap();
    assert!(
        !wire.contains("authorization:"),
        "OAuth empty bearer must suppress URL Basic without mutating owner"
    );
}

#[test]
fn httpx_redirect_origin_numeric_ipv6_scope_and_zero_are_distinct_from_transport_origin() {
    let a = SourceUrl::parse("http://[2001:0DB8::1%eth0]/").unwrap();
    let b = SourceUrl::parse("http://[2001:db8:0:0:0:0:0:1%eth0]/").unwrap();
    let c = SourceUrl::parse("http://[2001:db8::1%eth1]/").unwrap();
    assert!(source_origin(&a).unwrap() == source_origin(&b).unwrap());
    assert!(source_origin(&a).unwrap() != source_origin(&c).unwrap());
    let zero = SourceUrl::parse("http://a.test:0/").unwrap();
    let default = SourceUrl::parse("http://a.test/").unwrap();
    assert_eq!(zero.effective_origin_port(), Some("80"));
    assert!(source_origin(&zero).unwrap() != source_origin(&default).unwrap());
}
struct ObserveNormalizedCookie(std::sync::Mutex<Vec<(String, String, String)>>);
impl SourceCookieStore for ObserveNormalizedCookie {
    fn has_cookies(&self) -> bool {
        true
    }
    fn cookies(&self, view: &SourceCookieView) -> reqwest::Result<Option<HeaderValue>> {
        self.0.lock().unwrap().push((
            view.scheme().into(),
            view.request_host()?,
            view.request_path()?,
        ));
        Ok(None)
    }
    fn extract(&self, _: &SourceCookieView, _: &HeaderMap) -> reqwest::Result<()> {
        Ok(())
    }
}
#[tokio::test]
async fn network_cookie_observation_uses_normalized_owner_before_userinfo_auth() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let jar = Arc::new(ObserveNormalizedCookie(Default::default()));
    let net = direct(jar.clone());
    let configured = format!(
        "HTTP://us%65r:p%2Bass@{}/a/../path?#fragment",
        listener.local_addr().unwrap()
    );
    let server = async {
        let (mut stream, _) = listener.accept().await?;
        let wire = request(&mut stream).await?;
        reply(&mut stream, "200 OK").await?;
        Ok::<_, Box<dyn std::error::Error + Send + Sync>>(wire)
    };
    let client = async {
        let mut headers = HeaderMap::new();
        headers.insert(
            reqwest::header::AUTHORIZATION,
            HeaderValue::from_static("Bearer static-token"),
        );
        let response = net
            .send_once(reqwest::Method::GET, configured, headers, Vec::new())
            .await?;
        assert_eq!(response.source_url().raw_userinfo(), "us%65r:p%2Bass");
        Ok::<_, Box<dyn std::error::Error + Send + Sync>>(())
    };
    let outcome = tokio::time::timeout(Duration::from_secs(10), async {
        tokio::try_join!(server, client)
    })
    .await;
    net.shutdown().await.unwrap();
    let (wire, ()) = outcome.unwrap().unwrap();
    assert!(wire.starts_with("get /path? http/1.1\r\n"));
    assert!(
        wire.contains("authorization: basic dxnlcjpwk2fzcw==\r\n"),
        "configured URL Basic overrides static bearer on non-OAuth lane"
    );
    assert!(!wire.contains("static-token"));
    assert_eq!(
        *jar.0.lock().unwrap(),
        vec![(
            "http".into(),
            "us%65r:p%2bass@127.0.0.1".into(),
            "/path".into()
        )]
    );
}

#[tokio::test]
async fn cleared_redirect_userinfo_keeps_actual_sent_basic_header_lineage() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let net = direct(Arc::new(SourceCookieJar::default()));
    let server = async {
        let mut wires = Vec::new();
        for status in [
            format!("307 Temporary Redirect\r\nLocation: http://{address}/next"),
            "200 OK".into(),
        ] {
            let (mut stream, _) = listener.accept().await?;
            wires.push(request(&mut stream).await?);
            reply(&mut stream, &status).await?;
        }
        Ok::<_, Box<dyn std::error::Error + Send + Sync>>(wires)
    };
    let client = async {
        let mut headers = HeaderMap::new();
        headers.insert(
            reqwest::header::AUTHORIZATION,
            HeaderValue::from_static("Bearer static-token"),
        );
        let response = net
            .send(
                reqwest::Method::GET,
                format!("http://user:pass@{address}/start"),
                headers,
                Vec::new(),
            )
            .await?;
        assert!(response.source_url().raw_userinfo().is_empty());
        Ok::<_, Box<dyn std::error::Error + Send + Sync>>(())
    };
    let outcome = tokio::time::timeout(Duration::from_secs(10), async {
        tokio::try_join!(server, client)
    })
    .await;
    net.shutdown().await.unwrap();
    let (wires, ()) = outcome.unwrap().unwrap();
    assert_eq!(wires.len(), 2);
    for wire in wires {
        assert!(wire.contains("authorization: basic dxnlcjpwyxnz\r\n"));
        assert!(!wire.contains("static-token"));
    }
}

#[test]
fn initial_prepare_host_lazy_boundaries_and_vec_body_defaults_are_source_exact() {
    use reqwest::header::{CONTENT_LENGTH, HOST, TRANSFER_ENCODING};
    let ace = SourceUrl::parse("http://xn--.bad_label/").unwrap();
    let scope = SourceUrl::parse("http://[::1%é]/").unwrap();
    for owner in [&ace, &scope] {
        assert!(
            prepare_initial_headers(&reqwest::Method::GET, owner, &mut HeaderMap::new(), b"")
                .is_err()
        );
        let mut headers = HeaderMap::new();
        headers.insert(HOST, HeaderValue::from_static("explicit.test"));
        prepare_initial_headers(&reqwest::Method::GET, owner, &mut headers, b"").unwrap();
        assert_eq!(
            headers[HOST], "explicit.test",
            "explicit Host bypasses initial lazy display/netloc"
        );
    }
    let owner = SourceUrl::parse("http://a.test/").unwrap();
    for method in ["GET", "POST", "PUT", "PATCH", "QUERY", "DELETE"] {
        for body in [b"".as_slice(), b"abc".as_slice()] {
            let method = reqwest::Method::from_bytes(method.as_bytes()).unwrap();
            let mut headers = HeaderMap::new();
            prepare_initial_headers(&method, &owner, &mut headers, body).unwrap();
            assert_eq!(headers[HOST], "a.test");
            let expected = if !body.is_empty() {
                Some("3")
            } else if matches!(method.as_str(), "POST" | "PUT" | "PATCH" | "QUERY") {
                Some("0")
            } else {
                None
            };
            assert_eq!(
                headers.get(CONTENT_LENGTH).map(|v| v.to_str().unwrap()),
                expected
            );
        }
    }
    let mut headers = HeaderMap::new();
    headers.insert(TRANSFER_ENCODING, HeaderValue::from_static("chunked"));
    prepare_initial_headers(&reqwest::Method::POST, &owner, &mut headers, b"abc").unwrap();
    assert!(
        !headers.contains_key(CONTENT_LENGTH),
        "explicit TE suppresses auto content length"
    );
    let mut headers = HeaderMap::new();
    headers.insert(CONTENT_LENGTH, HeaderValue::from_static("7"));
    prepare_initial_headers(&reqwest::Method::POST, &owner, &mut headers, b"abc").unwrap();
    assert_eq!(
        headers[CONTENT_LENGTH], "7",
        "explicit length is not replaced"
    );
}
#[tokio::test]
async fn initial_lazy_host_failure_precedes_dns_or_socket_admission() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let dns = Arc::new(LoopbackDns(AtomicUsize::new(0)));
    let net = network_with_dns(
        SourceHttp1PoolConfig::new_source(Duration::from_secs(2), Vec::new()).unwrap(),
        Arc::new(SourceCookieJar::default()),
        dns.clone(),
    );
    for url in [
        format!("http://xn--.bad_label:{port}/"),
        format!("http://[::1%é]:{port}/"),
    ] {
        assert!(
            net.send_once(reqwest::Method::GET, url, HeaderMap::new(), Vec::new())
                .await
                .is_err()
        );
    }
    assert_eq!(
        dns.0.load(Ordering::SeqCst),
        0,
        "initial lazy failure dispatched resolver"
    );
    std::future::poll_fn(|cx| {
        assert!(
            listener.poll_accept(cx).is_pending(),
            "initial failure opened socket"
        );
        std::task::Poll::Ready(())
    })
    .await;
    net.shutdown().await.unwrap();
}
#[tokio::test]
async fn initial_recorded_host_survives_same_origin_redirect_stream_lineage() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let net = direct(Arc::new(SourceCookieJar::default()));
    let server = async {
        let mut wires = Vec::new();
        for status in ["307 Temporary Redirect\r\nLocation: /next", "200 OK"] {
            let (mut stream, _) = listener.accept().await?;
            wires.push(request(&mut stream).await?);
            reply(&mut stream, status).await?;
        }
        Ok::<_, Box<dyn std::error::Error + Send + Sync>>(wires)
    };
    let client = async {
        let mut headers = HeaderMap::new();
        headers.insert(
            reqwest::header::HOST,
            HeaderValue::from_static("explicit.test:7"),
        );
        net.send(
            reqwest::Method::GET,
            format!("http://{address}/start"),
            headers,
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
    assert_eq!(wires.len(), 2);
    for wire in wires {
        assert!(
            wire.contains("host: explicit.test:7\r\n"),
            "redirect overwrote recorded sent Host"
        );
    }
}
