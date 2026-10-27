#![allow(dead_code)] // Reused qualification helpers intentionally expose more than each target uses.
use ssx_infra::valkey::Valkey;
use std::{
    fs,
    io::Read as _,
    path::PathBuf,
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};
pub struct Server {
    child: Child,
    dir: PathBuf,
    url: String,
}
impl Server {
    pub fn start() -> Self {
        let mut bytes = [0; 16];
        fs::File::open("/dev/urandom")
            .unwrap()
            .read_exact(&mut bytes)
            .unwrap();
        bytes[6] = (bytes[6] & 15) | 64;
        bytes[8] = (bytes[8] & 63) | 128;
        let id: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
        // Qualification controller registers this path in its storage ledger
        // before execution. Defaults remain compatible with ordinary CI tests.
        let base = std::env::var_os("SLOPSEARX_TEST_TEMP_ROOT")
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir);
        let dir = base.join(format!("ssx-t06-{id}"));
        // sockaddr_un has room for 104 bytes on macOS and 108 on Linux.
        use std::os::unix::{ffi::OsStrExt, fs::PermissionsExt};
        assert!(
            dir.join("s.sock").as_os_str().as_bytes().len() < 104,
            "private fixture socket exceeds supported Unix path bound"
        );
        fs::create_dir(&dir).unwrap();
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o700)).unwrap();
        let url = format!("unix://{}", dir.join("s.sock").display());
        let mut s = Self {
            child: Self::spawn(&dir),
            dir,
            url,
        };
        s.wait_ready();
        s
    }
    fn spawn(dir: &std::path::Path) -> Child {
        Command::new(
            std::env::var("SLOPSEARX_TEST_VALKEY_SERVER")
                .unwrap_or_else(|_| "valkey-server".into()),
        )
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
        .arg(dir)
        .arg("--unixsocket")
        .arg(dir.join("s.sock"))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("disposable valkey-server required")
    }
    fn wait_ready(&mut self) {
        let until = Instant::now() + Duration::from_secs(4);
        loop {
            assert!(
                self.child.try_wait().unwrap().is_none(),
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
    pub fn stop(&mut self) {
        self.child.kill().unwrap();
        self.child.wait().unwrap();
    }
    pub fn restart(&mut self) {
        self.child = Self::spawn(&self.dir);
        self.wait_ready();
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = fs::remove_dir_all(&self.dir);
    }
}
impl Server {
    pub fn url(&self) -> &str {
        &self.url
    }
}

/// Owns suspension of this fixture's actual child. The borrow prevents stopping,
/// restarting or dropping the server before the guard resumes it.
pub struct SuspendedServer<'a> {
    server: &'a mut Server,
    resumed: bool,
}
impl Server {
    pub fn suspend(&mut self) -> SuspendedServer<'_> {
        assert!(
            self.child.try_wait().unwrap().is_none(),
            "private Valkey exited"
        );
        let guard = SuspendedServer {
            server: self,
            resumed: false,
        };
        guard.signal("-STOP").expect("suspend owned private Valkey");
        guard.assert_stopped();
        guard
    }
}
impl SuspendedServer<'_> {
    fn signal(&self, signal: &str) -> std::io::Result<()> {
        let output = Command::new("kill")
            .arg(signal)
            .arg(self.server.child.id().to_string())
            .output()?;
        if !output.status.success() {
            return Err(std::io::Error::other(format!(
                "owned Valkey {signal} failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }
        Ok(())
    }
    pub fn assert_stopped(&self) {
        let until = Instant::now() + Duration::from_secs(1);
        loop {
            let state = Command::new("ps")
                .args(["-o", "stat=", "-p"])
                .arg(self.server.child.id().to_string())
                .output()
                .expect("read owned Valkey process state");
            assert!(state.status.success(), "owned Valkey process is absent");
            if String::from_utf8_lossy(&state.stdout).contains('T') {
                return;
            }
            assert!(Instant::now() < until, "owned Valkey did not stop");
        }
    }
    pub fn resume(mut self) {
        self.signal("-CONT").expect("resume owned private Valkey");
        self.resumed = true;
    }
}
impl Drop for SuspendedServer<'_> {
    fn drop(&mut self) {
        if !self.resumed
            && let Err(error) = self.signal("-CONT")
        {
            // Preserve an earlier panic/error. Server still owns and reaps the
            // child after this borrow ends, including a failed resume.
            eprintln!("failed to resume owned private Valkey during cleanup: {error}");
        }
    }
}
