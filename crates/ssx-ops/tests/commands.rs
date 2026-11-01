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
        #[cfg(unix)]
        {
            let _ = nix::sys::signal::killpg(
                nix::unistd::Pid::from_raw(self.0.id() as i32),
                nix::sys::signal::Signal::SIGKILL,
            );
        }
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
fn run(args: &[&str]) -> (i32, String) {
    let mut command = Command::new(env!("CARGO_BIN_EXE_ssx-ops"));
    command.args(args).env_clear().env("PATH", "");
    run_prepared(command)
}
fn run_prepared(mut command: Command) -> (i32, String) {
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    let mut c = Owned(
        command
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

#[test]
fn native_promotion_explicit_modes_and_source_failures_never_require_an_interpreter() {
    for args in [
        vec!["promote-image-digest"],
        vec!["promote-image-digest", "--check", "--write"],
        vec!["promote-image-digest", "--write", "--write"],
        vec!["promote-image-digest", "--check", "--root"],
        vec!["promote-image-digest", "--unrecognized"],
    ] {
        let (code, output) = run(&args);
        assert_eq!(code, 2);
        assert!(output.starts_with("error: ssx-ops:"));
    }
    let case = Case::new();
    for mode in ["--check", "--write"] {
        let (code, output) = run(&[
            "promote-image-digest",
            mode,
            "--root",
            case.0.to_str().unwrap(),
        ]);
        assert_eq!(code, 2);
        assert!(output.starts_with("error: ssx-ops:"));
        assert!(!output.contains("registry lookup"));
    }
    assert!(fs::read_dir(&case.0).unwrap().next().is_none());
}
#[cfg(unix)]
#[test]
fn actual_promotion_workflow_captures_zero_ten_and_error_under_errexit() {
    use std::os::unix::fs::PermissionsExt;
    let source = fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../.github/workflows/image-promotion.yml"),
    )
    .unwrap();
    let body = source
        .split("        id: promote\n        run: |\n")
        .nth(1)
        .unwrap()
        .split("\n      - name: Open or update promotion PR")
        .next()
        .unwrap()
        .lines()
        .map(|line| line.strip_prefix("          ").unwrap_or(line))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(body.len() < 8192);
    for (stub_code, expected_code, expected_output) in [
        (0, 0, Some("changed=0\n")),
        (10, 0, Some("changed=1\n")),
        (2, 1, None),
    ] {
        let case = Case::new();
        let stub = case.0.join("ssx-ops-stub");
        let output = case.0.join("github-output");
        fs::write(&stub, format!("#!/bin/sh\nexit {stub_code}\n")).unwrap();
        fs::set_permissions(&stub, fs::Permissions::from_mode(0o700)).unwrap();
        let mut command = Command::new("/bin/bash");
        command
            .args(["-e", "-o", "pipefail", "-c", &body])
            .env_clear()
            .env("PATH", "")
            .env("SSX_IMAGE_OPS_BIN", &stub)
            .env("GITHUB_OUTPUT", &output);
        let (code, _) = run_prepared(command);
        assert_eq!(code, expected_code);
        match expected_output {
            Some(value) => assert_eq!(fs::read_to_string(&output).unwrap(), value),
            None => assert!(!output.exists()),
        }
    }
}
