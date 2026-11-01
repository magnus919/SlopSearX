use super::*;
use crate::{TOKEN_LENGTH, digest_valid};
use sha2::{Digest, Sha256};
use ssx_infra::http::{DEFAULT_USER_AGENT, Replay, Request, Response};
use std::sync::Arc;
fn digest(byte: char) -> String {
    format!("sha256:{}", byte.to_string().repeat(64))
}
fn inputs(counts: [usize; 3]) -> Vec<PinText> {
    PIN_PATHS
        .iter()
        .zip(counts)
        .map(|(&path, count)| PinText {
            path,
            bytes: format!(
                "unchanged prefix\r\n{}unchanged suffix\r\n",
                format!("image: {IMAGE}@{}\n", digest('a')).repeat(count)
            )
            .into_bytes(),
        })
        .collect()
}
#[test]
fn repeated_data_pins_preserve_counts_and_unrelated_bytes() {
    let sources = inputs([3, 2, 4]);
    let planned = plan(&sources, &digest('b')).unwrap();
    assert!(planned.changed());
    assert_eq!(planned.counts(), &[3, 2, 4]);
    assert_eq!(planned.current(), digest('a'));
    assert_eq!(planned.latest(), digest('b'));
    for (original, updated) in sources.iter().zip(planned.outputs()) {
        assert_eq!(
            String::from_utf8(updated.bytes.clone())
                .unwrap()
                .replace(&digest('b'), &digest('a'))
                .as_bytes(),
            original.bytes
        );
    }
    let same = plan(&sources, &digest('a')).unwrap();
    assert!(!same.changed());
    assert_eq!(same.outputs(), sources);
}
#[test]
fn all_data_sources_must_have_valid_matching_complete_pins() {
    let good = inputs([1, 1, 2]);
    for index in 0..3 {
        let mut missing = good.clone();
        missing[index].bytes = b"no pin\n".to_vec();
        assert!(plan(&missing, &digest('b')).is_err());
        let mut conflicting = good.clone();
        conflicting[index].bytes = String::from_utf8(conflicting[index].bytes.clone())
            .unwrap()
            .replace(&digest('a'), &digest('c'))
            .into_bytes();
        assert!(plan(&conflicting, &digest('b')).is_err());
    }
    assert!(plan(&inputs([1, 1, 1]), &digest('b')).is_err());
    for malformed in [
        format!("{}0", digest('a')),
        format!("sha256:{}", "A".repeat(64)),
        "sha256:short".into(),
        "sha512:bad".into(),
    ] {
        let mut sources = good.clone();
        sources[0].bytes = format!("{IMAGE}@{malformed}\n").into_bytes();
        assert!(plan(&sources, &digest('b')).is_err());
    }
    let mut embedded = good.clone();
    embedded[0].bytes = format!("other{IMAGE}@{}\n", digest('a')).into_bytes();
    assert!(plan(&embedded, &digest('b')).is_err());
    let mut reordered = good.clone();
    reordered.swap(0, 1);
    assert!(plan(&reordered, &digest('b')).is_err());
    let mut large = good.clone();
    large[0].bytes.resize(FILE_BYTES + 1, b'x');
    assert!(plan(&large, &digest('b')).is_err());
    assert!(plan(&good[..2], &digest('b')).is_err());
    assert!(plan(&good, "sha256:bad").is_err());
}
fn token_request() -> Request {
    let mut r =
        Request::get("https://ghcr.io/token?scope=repository%3Amagnus919%2Fslopsearx%3Apull")
            .unwrap();
    r.headers
        .insert("accept", "application/json".parse().unwrap());
    r.headers
        .insert("accept-encoding", "identity".parse().unwrap());
    r.headers
        .insert("user-agent", DEFAULT_USER_AGENT.parse().unwrap());
    r
}
fn manifest_request() -> Request {
    let mut r = Request::get("https://ghcr.io/v2/magnus919/slopsearx/manifests/latest").unwrap();
    r.headers.insert("accept", GHCR_ACCEPT.parse().unwrap());
    r.headers
        .insert("accept-encoding", "identity".parse().unwrap());
    r.headers
        .insert("user-agent", DEFAULT_USER_AGENT.parse().unwrap());
    r.headers
        .insert("authorization", "Bearer public-fixture".parse().unwrap());
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
fn signed() -> Response {
    let mut r = response(&manifest_request(), 200, br#"{"schemaVersion":2}"#);
    let hash = Sha256::digest(&r.body)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    r.headers.insert(
        "docker-content-digest",
        format!("sha256:{hash}").parse().unwrap(),
    );
    r
}
async fn lookup(rows: Vec<(Request, Response)>) -> (Result<String, String>, usize) {
    let transport = Arc::new(Replay::new(rows));
    let owner = AdapterHttp::default();
    owner.set_transport(transport.clone()).unwrap();
    let result = resolve_latest(&owner).await;
    owner.shutdown_and_wait().await;
    (result, transport.remaining())
}
#[tokio::test(start_paused = true)]
async fn fixed_ghcr_flow_accepts_signed_bytes_and_bounded_transient_retries() {
    let good = signed();
    let expected = good.headers["docker-content-digest"]
        .to_str()
        .unwrap()
        .to_string();
    let (result, left) = lookup(vec![
        (token_request(), response(&token_request(), 503, b"retry")),
        (token_request(), response(&token_request(), 429, b"retry")),
        (
            token_request(),
            response(&token_request(), 200, br#"{"token":"public-fixture"}"#),
        ),
        (manifest_request(), good),
    ])
    .await;
    assert_eq!(result.unwrap(), expected);
    assert_eq!(left, 0);
    assert!(digest_valid(&expected));
}
#[tokio::test(start_paused = true)]
async fn ghcr_permanent_token_and_manifest_failures_are_redacted() {
    for status in [400, 401, 403, 404] {
        let (result, left) = lookup(vec![(
            token_request(),
            response(&token_request(), status, b"secret-body"),
        )])
        .await;
        let message = result.unwrap_err();
        assert!(!message.contains("secret-body"));
        assert_eq!(left, 0);
    }
    for body in [
        br#"{"token":""}"#.to_vec(),
        br#"{"token":7}"#.to_vec(),
        format!("{{\"token\":\"{}\"}}", "s".repeat(TOKEN_LENGTH + 1)).into_bytes(),
    ] {
        let (result, left) = lookup(vec![(
            token_request(),
            response(&token_request(), 200, &body),
        )])
        .await;
        assert!(result.is_err());
        assert_eq!(left, 0);
    }
    for kind in 0..6 {
        let mut bad = signed();
        match kind {
            0 => {
                bad.headers.remove("docker-content-digest");
            }
            1 => {
                bad.headers
                    .append("docker-content-digest", digest('b').parse().unwrap());
            }
            2 => {
                bad.headers
                    .insert("docker-content-digest", "sha256:invalid".parse().unwrap());
            }
            3 => bad.body.push(b'x'),
            4 => {
                bad.headers
                    .insert("content-encoding", "gzip".parse().unwrap());
            }
            _ => bad.url = "https://ghcr.io/other".parse().unwrap(),
        }
        let (result, left) = lookup(vec![
            (
                token_request(),
                response(&token_request(), 200, br#"{"token":"public-fixture"}"#),
            ),
            (manifest_request(), bad),
        ])
        .await;
        assert!(result.is_err());
        assert_eq!(left, 0);
    }
}
#[cfg(unix)]
mod unix {
    use super::*;
    use crate::promotion::files::ApplyPoint;
    use std::{
        fs,
        os::unix::fs::{MetadataExt, PermissionsExt},
        path::PathBuf,
        sync::atomic::{AtomicU64, Ordering},
    };
    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            static NEXT: AtomicU64 = AtomicU64::new(0);
            let base = std::env::var_os("TMPDIR")
                .or_else(|| std::env::var_os("RUNNER_TEMP"))
                .map(PathBuf::from)
                .unwrap_or_else(std::env::temp_dir);
            let root = base.join(format!(
                "ssx-promotion-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&root).unwrap();
            for source in inputs([2, 1, 3]) {
                let path = root.join(source.path);
                fs::create_dir_all(path.parent().unwrap()).unwrap();
                fs::write(&path, source.bytes).unwrap();
                fs::set_permissions(&path, fs::Permissions::from_mode(0o640)).unwrap();
            }
            Self(root)
        }
        fn bytes(&self) -> Vec<Vec<u8>> {
            PIN_PATHS
                .iter()
                .map(|p| fs::read(self.0.join(p)).unwrap())
                .collect()
        }
        fn staging(&self) -> Vec<PathBuf> {
            [self.0.clone(), self.0.join("k8s")]
                .into_iter()
                .flat_map(|p| fs::read_dir(p).unwrap())
                .map(|e| e.unwrap().path())
                .filter(|p| {
                    p.file_name()
                        .unwrap()
                        .to_string_lossy()
                        .starts_with(".ssx-image-promotion-")
                })
                .collect()
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    #[test]
    fn real_owned_promotion_preserves_modes_counts_and_current_noop() {
        let f = Fixture::new();
        let before = f.bytes();
        let snapshot = PinSnapshot::read(&f.0).unwrap();
        let same = snapshot.plan(&digest('a')).unwrap();
        let inos: Vec<_> = PIN_PATHS
            .iter()
            .map(|p| fs::metadata(f.0.join(p)).unwrap().ino())
            .collect();
        assert!(snapshot.apply(&same).unwrap().is_empty());
        assert_eq!(before, f.bytes());
        assert_eq!(
            inos,
            PIN_PATHS
                .iter()
                .map(|p| fs::metadata(f.0.join(p)).unwrap().ino())
                .collect::<Vec<_>>()
        );
        let changed = snapshot.plan(&digest('b')).unwrap();
        assert_eq!(snapshot.apply(&changed).unwrap(), PIN_PATHS);
        let after = f.bytes();
        for (old, new) in before.iter().zip(&after) {
            assert_eq!(
                String::from_utf8(new.clone())
                    .unwrap()
                    .replace(&digest('b'), &digest('a'))
                    .as_bytes(),
                old
            );
        }
        for p in PIN_PATHS {
            assert_eq!(
                fs::metadata(f.0.join(p)).unwrap().permissions().mode() & 0o7777,
                0o640
            );
        }
        assert!(f.staging().is_empty());
        assert_eq!(
            PinSnapshot::read(&f.0)
                .unwrap()
                .plan(&digest('b'))
                .unwrap()
                .counts(),
            &[2, 1, 3]
        );
    }
    #[test]
    fn actual_source_validation_and_staging_failure_modify_no_pin() {
        let f = Fixture::new();
        let before = f.bytes();
        let snapshot = PinSnapshot::read(&f.0).unwrap();
        let plan = snapshot.plan(&digest('b')).unwrap();
        let error = snapshot
            .apply_with_test_hook(&plan, |point, index, parent, name| {
                if point == ApplyPoint::BeforeStage && index == 1 {
                    use nix::{
                        fcntl::{OFlag, openat},
                        sys::stat::Mode,
                    };
                    let _collision = openat(
                        parent,
                        name,
                        OFlag::O_CREAT | OFlag::O_EXCL | OFlag::O_WRONLY,
                        Mode::S_IRUSR | Mode::S_IWUSR,
                    )
                    .map_err(std::io::Error::from)?;
                }
                Ok(())
            })
            .unwrap_err();
        assert!(error.replaced.is_empty());
        assert!(error.cleanup_failed.is_empty());
        assert_eq!(before, f.bytes());
        // The deliberately preexisting collision is not owned by the promoter.
        let collision = f.staging();
        assert_eq!(collision.len(), 1);
        fs::remove_file(&collision[0]).unwrap();
        assert!(f.staging().is_empty());
        fs::write(f.0.join("spec.md"), b"missing documentation pin").unwrap();
        assert!(PinSnapshot::read(&f.0).is_err());
        assert_eq!(fs::read(f.0.join(PIN_PATHS[0])).unwrap(), before[0]);
    }
    #[test]
    fn later_real_rename_failure_reports_partial_state_and_refuses_retry() {
        let f = Fixture::new();
        let snapshot = PinSnapshot::read(&f.0).unwrap();
        let plan = snapshot.plan(&digest('b')).unwrap();
        let error = snapshot
            .apply_with_test_hook(&plan, |point, index, parent, name| {
                if point == ApplyPoint::ReadyToRename && index == 1 {
                    nix::unistd::unlinkat(parent, name, nix::unistd::UnlinkatFlags::NoRemoveDir)
                        .map_err(std::io::Error::from)?;
                }
                Ok(())
            })
            .unwrap_err();
        assert_eq!(error.replaced, vec![PIN_PATHS[0]]);
        assert!(error.cleanup_failed.is_empty());
        assert!(error.message.contains("atomic pin replacement failed"));
        assert!(f.staging().is_empty());
        assert!(PinSnapshot::read(&f.0).is_err());
        assert!(
            String::from_utf8(fs::read(f.0.join(PIN_PATHS[0])).unwrap())
                .unwrap()
                .contains(&digest('b'))
        );
        assert!(
            String::from_utf8(fs::read(f.0.join(PIN_PATHS[1])).unwrap())
                .unwrap()
                .contains(&digest('a'))
        );
    }
    #[test]
    fn ready_to_rename_rechecks_source_and_staging_identity() {
        let f = Fixture::new();
        let snapshot = PinSnapshot::read(&f.0).unwrap();
        let plan = snapshot.plan(&digest('b')).unwrap();
        let edited = b"concurrent user edit";
        let source_error = snapshot
            .apply_with_test_hook(&plan, |point, index, _, _| {
                if point == ApplyPoint::ReadyToRename && index == 0 {
                    fs::write(f.0.join(PIN_PATHS[0]), edited)?;
                }
                Ok(())
            })
            .unwrap_err();
        assert!(source_error.replaced.is_empty());
        assert!(
            source_error
                .message
                .contains("source changed at replacement boundary")
        );
        assert_eq!(fs::read(f.0.join(PIN_PATHS[0])).unwrap(), edited);

        let other = Fixture::new();
        let before = other.bytes();
        let snapshot = PinSnapshot::read(&other.0).unwrap();
        let plan = snapshot.plan(&digest('b')).unwrap();
        let mut substituted = None;
        let stage_error = snapshot
            .apply_with_test_hook(&plan, |point, index, _, name| {
                if point == ApplyPoint::ReadyToRename && index == 0 {
                    let stage = other.0.join(name);
                    let displaced = stage.with_extension("saved");
                    fs::rename(&stage, &displaced)?;
                    fs::write(&stage, b"untrusted staging replacement")?;
                    substituted = Some(stage);
                }
                Ok(())
            })
            .unwrap_err();
        assert!(stage_error.replaced.is_empty());
        assert!(
            stage_error
                .message
                .contains("staged pin output changed at replacement boundary")
        );
        assert_eq!(before, other.bytes());
        assert_eq!(stage_error.cleanup_failed, vec![PIN_PATHS[0]]);
        assert_eq!(
            fs::read(substituted.expect("hook substitutes a stage")).unwrap(),
            b"untrusted staging replacement"
        );
    }
    #[test]
    fn concurrent_source_edits_and_symlink_targets_are_preserved_and_refused() {
        let f = Fixture::new();
        let snapshot = PinSnapshot::read(&f.0).unwrap();
        let plan = snapshot.plan(&digest('b')).unwrap();
        let edited = b"concurrent user edit";
        let error = snapshot
            .apply_with_test_hook(&plan, |point, index, _, _| {
                if point == ApplyPoint::BeforeReplace && index == 0 {
                    fs::write(f.0.join(PIN_PATHS[0]), edited)?;
                }
                Ok(())
            })
            .unwrap_err();
        assert!(error.replaced.is_empty());
        assert_eq!(fs::read(f.0.join(PIN_PATHS[0])).unwrap(), edited);
        assert!(f.staging().is_empty());
        let other = Fixture::new();
        let target = other.0.join(PIN_PATHS[0]);
        fs::remove_file(&target).unwrap();
        std::os::unix::fs::symlink(other.0.join("spec.md"), &target).unwrap();
        assert!(PinSnapshot::read(&other.0).is_err());
    }
}
