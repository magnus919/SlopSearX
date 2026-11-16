//! Exclusive disposable Valkey fixture. Binary identity and scratch are supplied
//! by the qualification owner; no PATH fallback or production endpoint exists.
use sha2::{Digest, Sha256};
use ssx_infra::valkey::Valkey;
use std::os::unix::{ffi::OsStrExt, fs::PermissionsExt};
use std::{
    fs,
    path::PathBuf,
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};

pub struct Server {
    child: Option<Child>,
    dir: PathBuf,
    url: String,
}
impl Server {
    pub fn start() -> Self {
        let binary = PathBuf::from(
            std::env::var_os("SLOPSEARX_TEST_VALKEY_SERVER")
                .expect("explicit qualified Valkey8.1.10 binary required"),
        );
        assert!(binary.is_absolute(), "Valkey binary must be absolute");
        let metadata = fs::symlink_metadata(&binary).unwrap();
        assert!(
            metadata.is_file() && metadata.permissions().mode() & 0o111 != 0,
            "Valkey binary must be a regular executable, not a symlink"
        );
        let expected = std::env::var("SLOPSEARX_TEST_VALKEY_SERVER_SHA256")
            .expect("qualification must pin the Valkey binary SHA256");
        assert!(
            expected.len() == 64
                && expected
                    .bytes()
                    .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase()),
            "Valkey SHA256 must be64 lowercase hex bytes"
        );
        let actual: String = Sha256::digest(fs::read(&binary).unwrap())
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        assert_eq!(actual, expected, "Valkey binary identity changed");
        let base = std::env::temp_dir();
        assert!(base.is_absolute(), "fixture TMPDIR must be absolute");
        #[cfg(target_os = "macos")]
        assert!(
            base.starts_with("/Volumes/tank01/magnus/rust-builds/slopsearx"),
            "Mac fixture scratch must be on the registered Tank root"
        );
        let mut bytes = [0; 8];
        aws_lc_rs::rand::fill(&mut bytes).unwrap();
        let id: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
        let dir = base.join(format!("w07-{id}"));
        assert!(
            dir.join("s.sock").as_os_str().as_bytes().len() < 104,
            "private fixture socket exceeds supported Unix path bound"
        );
        fs::create_dir(&dir).unwrap();
        // Own the directory before any subsequent fallible work or child spawn.
        let mut server = Self {
            child: None,
            url: format!("unix://{}", dir.join("s.sock").display()),
            dir,
        };
        fs::set_permissions(&server.dir, fs::Permissions::from_mode(0o700)).unwrap();
        server.child = Some(
            Command::new(binary)
                .args([
                    "--port",
                    "0",
                    "--bind",
                    "127.0.0.1",
                    "--save",
                    "",
                    "--appendonly",
                    "no",
                    "--daemonize",
                    "no",
                    "--unixsocketperm",
                    "700",
                    "--dir",
                ])
                .arg(&server.dir)
                .arg("--unixsocket")
                .arg(server.dir.join("s.sock"))
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .expect("disposable qualified Valkey required"),
        );
        server.wait_ready();
        server
    }
    fn wait_ready(&mut self) {
        let until = Instant::now() + Duration::from_secs(4);
        loop {
            assert!(
                self.child.as_mut().unwrap().try_wait().unwrap().is_none(),
                "Valkey startup failed"
            );
            if redis::Client::open(self.url.as_str())
                .unwrap()
                .get_connection()
                .is_ok()
            {
                break;
            }
            assert!(Instant::now() < until, "Valkey startup deadline");
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    pub fn connection(&self) -> redis::Connection {
        redis::Client::open(self.url.as_str())
            .unwrap()
            .get_connection()
            .unwrap()
    }
    pub fn store(&self) -> Valkey {
        Valkey::new(&self.url, Duration::from_millis(300)).unwrap()
    }
    pub fn url(&self) -> &str {
        &self.url
    }
    fn retire(&mut self) -> std::io::Result<()> {
        let mut first_error = None;
        if let Some(child) = self.child.as_mut() {
            // Retain this exact owner until retirement is proved, including
            // cleanup-error paths. Never signal a numeric PID after reaping.
            loop {
                let result = match child.try_wait() {
                    Ok(Some(_)) => break,
                    Ok(None) => child.kill().and_then(|()| child.wait().map(|_| ())),
                    Err(error) => Err(error),
                };
                match result {
                    Ok(()) => break,
                    Err(error) => {
                        if first_error.is_none() {
                            eprintln!("W07 fixture retains child after retirement error: {error}");
                            first_error = Some(error);
                        }
                        std::thread::sleep(Duration::from_millis(10));
                    }
                }
            }
            self.child.take();
        }
        fs::remove_dir_all(&self.dir)?;
        match first_error {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        if let Err(error) = self.retire() {
            // Never delete a live child's directory. A normal-path cleanup
            // failure fails the test; an existing panic remains a failed test.
            if std::thread::panicking() {
                eprintln!("W07 fixture retirement failed: {error}");
            } else {
                panic!("W07 fixture retirement failed: {error}");
            }
        }
    }
}
