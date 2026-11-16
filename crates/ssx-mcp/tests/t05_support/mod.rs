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
        let dir = std::env::temp_dir().join(format!("ssx-t04-{id}"));
        fs::create_dir(&dir).unwrap();
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
