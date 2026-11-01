use super::*;
use ssx_infra::http::{DEFAULT_USER_AGENT, ExchangeFuture, Replay, Transport};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
fn digest(body: &[u8]) -> String {
    format!(
        "sha256:{}",
        Sha256::digest(body)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    )
}
fn base(body: &[u8]) -> Base {
    Base {
        repository: "library/python",
        tag: "3.14.7-slim-trixie".into(),
        digest: digest(body),
    }
}
fn token() -> Request {
    let mut r=Request::get("https://auth.docker.io/token?service=registry.docker.io&scope=repository%3Alibrary%2Fpython%3Apull").unwrap();
    r.headers
        .insert("accept", "application/json".parse().unwrap());
    r.headers
        .insert("accept-encoding", "identity".parse().unwrap());
    r.headers
        .insert("user-agent", DEFAULT_USER_AGENT.parse().unwrap());
    r
}
fn manifest() -> Request {
    let mut r =
        Request::get("https://registry-1.docker.io/v2/library/python/manifests/3.14.7-slim-trixie")
            .unwrap();
    r.headers.insert("accept", ACCEPT.parse().unwrap());
    r.headers
        .insert("accept-encoding", "identity".parse().unwrap());
    r.headers
        .insert("authorization", "Bearer public-fixture".parse().unwrap());
    r.headers
        .insert("user-agent", DEFAULT_USER_AGENT.parse().unwrap());
    r
}
fn response(request: &Request, status: u16, body: &[u8]) -> Response {
    Response {
        status,
        url: request.url.clone(),
        headers: Default::default(),
        body: body.to_vec(),
    }
}
fn signed(body: &[u8]) -> Response {
    let mut r = response(&manifest(), 200, body);
    r.headers
        .insert("docker-content-digest", digest(body).parse().unwrap());
    r
}
async fn replay(
    rows: Vec<(Request, Response)>,
    base: Base,
) -> (Result<Vec<Observation>, String>, usize) {
    let transport = Arc::new(Replay::new(rows));
    let owner = AdapterHttp::default();
    owner.set_transport(transport.clone()).unwrap();
    let result = verify_bases(&owner, &[base]).await;
    owner.shutdown_and_wait().await;
    (result, transport.remaining())
}
#[test]
fn actual_docker_stages_and_hostile_stage_forms() {
    let current = pinned_bases(include_str!("../../../Dockerfile")).unwrap();
    assert_eq!(current.len(), 2);
    assert_eq!(current[0].repository, "library/rust");
    assert_eq!(current[0].tag, "1.99.0-bookworm");
    assert_eq!(current[1].repository, "library/python");
    let one = format!("FROM python:3.13-slim-bookworm@sha256:{}\n", "a".repeat(64));
    assert_eq!(pinned_bases(&one).unwrap().len(), 1);
    for bad in [
        format!("{one}FROM scratch\n"),
        include_str!("../../../Dockerfile").replace(" AS healthcheck-builder", " AS arbitrary"),
        one.replace("sha256:", "sha256:A"),
        one.replace("python:3.13", "python:bad/tag"),
        one.replace("FROM ", "FROM --platform=linux/amd64 "),
        format!("{one}FROM:broken\n"),
    ] {
        assert!(pinned_bases(&bad).is_err());
    }
}
#[test]
fn exact_application_platform_set_retains_duplicate_set_semantics() {
    let good=br#"{"manifests":[{"platform":{"os":"linux","architecture":"amd64"}},{"platform":{"os":"linux","architecture":"arm64"}},{"platform":{"os":"linux","architecture":"amd64"}}]}"#;
    verify_platforms(good).unwrap();
    for bad in [br#"{"manifests":[]}"#.as_slice(),br#"{"manifests":[{"platform":{"os":"linux","architecture":"amd64"}}]}"#.as_slice(),br#"{"manifests":[{"platform":{"os":"linux","architecture":"amd64"}},{"platform":{"os":"linux","architecture":"arm64"}},{"platform":{"os":"unknown","architecture":"unknown"}}]}"#.as_slice(),br#"{"manifests":[{"platform":{"os":0,"architecture":"arm64"}}]}"#.as_slice(),b"not-json".as_slice()] {assert!(verify_platforms(bad).is_err());}
    assert!(verify_platforms(&vec![b' '; MANIFEST_BYTES + 1]).is_err());
}
#[tokio::test]
async fn real_registry_logic_accepts_signed_manifest_and_classifies_stale_pin() {
    // The base index is not restricted to the application's exact two platforms.
    let body = br#"{"manifests":[{"platform":{"os":"linux","architecture":"s390x"}}]}"#;
    let b = base(body);
    let rows = vec![
        (
            token(),
            response(&token(), 200, br#"{"token":"public-fixture"}"#),
        ),
        (manifest(), signed(body)),
    ];
    let (good, left) = replay(rows.clone(), b.clone()).await;
    assert_eq!(left, 0);
    let good = good.unwrap();
    assert_eq!(good[0].actual, b.digest);
    let mut stale = b;
    stale.digest = format!("sha256:{}", "0".repeat(64));
    let (result, left) = replay(rows, stale).await;
    assert_eq!(left, 0);
    let result = result.unwrap();
    assert_ne!(result[0].base.digest, result[0].actual);
}
#[tokio::test(start_paused = true)]
async fn transient_statuses_retry_at_most_three_then_manifest_uses_exact_token() {
    let body = b"signed manifest bytes";
    for status in [429, 500, 502, 503, 504] {
        let rows = vec![
            (token(), response(&token(), status, b"")),
            (token(), response(&token(), status, b"")),
            (
                token(),
                response(&token(), 200, br#"{"token":"public-fixture"}"#),
            ),
            (manifest(), signed(body)),
        ];
        let (result, left) = replay(rows, base(body)).await;
        assert!(result.is_ok());
        assert_eq!(left, 0);
    }
    let rows = vec![(token(), response(&token(), 503, b"")); 3];
    let (result, left) = replay(rows, base(body)).await;
    assert!(result.is_err());
    assert_eq!(left, 0);
}
#[tokio::test]
async fn permanent_redirect_token_and_size_failures_do_not_dispatch_manifest() {
    let body = b"unused";
    for status in [301, 302, 307, 308, 400, 401, 403, 404, 501] {
        let rows = vec![
            (token(), response(&token(), status, b"")),
            (manifest(), signed(body)),
        ];
        let (result, left) = replay(rows, base(body)).await;
        assert!(result.is_err());
        assert_eq!(left, 1);
    }
    for bytes in [
        b"not JSON".to_vec(),
        br#"{"token":7}"#.to_vec(),
        br#"{"token":""}"#.to_vec(),
        format!("{{\"token\":\"{}\"}}", "x".repeat(TOKEN_LENGTH + 1)).into_bytes(),
        vec![b' '; TOKEN_BYTES + 1],
    ] {
        let rows = vec![
            (token(), response(&token(), 200, &bytes)),
            (manifest(), signed(body)),
        ];
        let (result, left) = replay(rows, base(body)).await;
        assert!(result.is_err());
        assert_eq!(left, 1);
    }
}
#[tokio::test]
async fn manifest_digest_body_encoding_and_endpoint_substitutions_fail_closed() {
    let body = b"exact manifest";
    for bad in 0..7 {
        let mut r = signed(body);
        match bad {
            0 => {
                r.headers.remove("docker-content-digest");
            }
            1 => {
                r.headers
                    .insert("docker-content-digest", "sha256:bad".parse().unwrap());
            }
            2 => r.body.push(b'x'),
            3 => {
                r.headers
                    .append("docker-content-digest", digest(body).parse().unwrap());
            }
            4 => r.url = Url::parse("https://example.invalid/escape").unwrap(),
            5 => {
                r.headers
                    .insert("content-encoding", "gzip".parse().unwrap());
            }
            _ => r.body = vec![b'x'; MANIFEST_BYTES + 1],
        }
        let rows = vec![
            (
                token(),
                response(&token(), 200, br#"{"token":"public-fixture"}"#),
            ),
            (manifest(), r),
        ];
        let (result, left) = replay(rows, base(body)).await;
        assert!(result.is_err());
        assert_eq!(left, 0);
    }
}
struct Pending(AtomicUsize);
impl Transport for Pending {
    fn exchange(&self, request: Request) -> ExchangeFuture<'_> {
        assert_eq!(request.url, token().url);
        self.0.fetch_add(1, Ordering::SeqCst);
        Box::pin(std::future::pending())
    }
}
#[tokio::test(start_paused = true)]
async fn each_pending_attempt_has_a_deadline_and_retries_are_finite() {
    let transport = Arc::new(Pending(AtomicUsize::new(0)));
    let owner = AdapterHttp::default();
    owner.set_transport(transport.clone()).unwrap();
    let start = Instant::now();
    let result = verify_bases(&owner, &[base(b"unused")]).await;
    assert!(result.is_err());
    assert_eq!(transport.0.load(Ordering::SeqCst), 3);
    assert!(Instant::now().duration_since(start) >= Duration::from_secs(90));
    owner.shutdown_and_wait().await;
}
