//! Actual native command processes, bounded local-file inputs only. No registry IO.
use std::{
    fs,
    io::Read,
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::atomic::{AtomicUsize, Ordering},
    thread,
    time::{Duration, Instant},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Case(PathBuf);
impl Case {
    fn new() -> Self {
        let p = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!(
            "image-ops-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::SeqCst)
        ));
        fs::create_dir_all(&p).unwrap();
        Self(p)
    }
}
impl Drop for Case {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
struct Owned(Child);
impl Drop for Owned {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
fn run(args: &[&str]) -> (i32, String) {
    let mut c = Owned(
        Command::new(env!("CARGO_BIN_EXE_ssx-ops"))
            .args(args)
            .env_clear()
            .env("PATH", "")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap(),
    );
    let deadline = Instant::now() + Duration::from_secs(5);
    let status = loop {
        if let Some(status) = c.0.try_wait().unwrap() {
            break status;
        }
        assert!(
            Instant::now() < deadline,
            "native static command exceeded deadline"
        );
        thread::sleep(Duration::from_millis(10));
    };
    let mut out = String::new();
    c.0.stdout
        .take()
        .unwrap()
        .take(16385)
        .read_to_string(&mut out)
        .unwrap();
    c.0.stderr
        .take()
        .unwrap()
        .take(16385)
        .read_to_string(&mut out)
        .unwrap();
    assert!(out.len() <= 16384);
    (status.code().unwrap(), out)
}
#[test]
fn native_platform_command_and_source_errors_run_without_an_interpreter() {
    let c = Case::new();
    let manifest = c.0.join("manifest.json");
    fs::write(&manifest,br#"{"manifests":[{"platform":{"os":"linux","architecture":"amd64"}},{"platform":{"os":"linux","architecture":"arm64"}}]}"#).unwrap();
    assert_eq!(
        run(&[
            "verify-image-platforms",
            "--manifest",
            manifest.to_str().unwrap()
        ])
        .0,
        0
    );
    fs::write(&manifest, b"not JSON").unwrap();
    assert_eq!(
        run(&[
            "verify-image-platforms",
            "--manifest",
            manifest.to_str().unwrap()
        ])
        .0,
        1
    );
    let docker = c.0.join("Dockerfile");
    fs::write(&docker, b"FROM scratch\n").unwrap();
    let (status, output) = run(&[
        "verify-base-digests",
        "--dockerfile",
        docker.to_str().unwrap(),
    ]);
    assert_eq!(status, 2);
    assert_eq!(output, "error: ssx-ops: base must pin tag and digest\n");
    assert_eq!(run(&["unknown-command"]).0, 2);
}
