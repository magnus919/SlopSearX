#![allow(dead_code)]
use serde_json::{Value, json};
use std::{
    path::{Path, PathBuf},
    process::Stdio,
    time::Duration,
};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader, Lines},
    process::{Child, ChildStdin, ChildStdout},
};
pub const BUDGET: Duration = Duration::from_secs(15);
pub fn root(label: &str) -> PathBuf {
    let root = PathBuf::from(
        std::env::var_os("SLOPSEARX_INSTALL_TEST_ROOT")
            .expect("registered private installation root"),
    );
    assert!(root.is_absolute());
    let path = root.join(format!("platform-{label}-{}", std::process::id()));
    ssx_mcp::gateway_client::resolver::install::prepare_output_root(&path).unwrap();
    path
}
pub fn stage(root: &Path, fixture: bool) -> PathBuf {
    let dns = if fixture {
        env!("CARGO_BIN_EXE_ssx-gateway-dns-fixture")
    } else {
        env!("CARGO_BIN_EXE_ssx-gateway-dns")
    };
    ssx_mcp::gateway_client::resolver::install::stage(
        Path::new(env!("CARGO_BIN_EXE_ssx-gateway")),
        Path::new(dns),
        #[cfg(windows)]
        Path::new(env!("CARGO_BIN_EXE_ssx-gateway-input")),
        root,
    )
    .unwrap()
}
pub struct Process {
    pub child: Child,
    input: Option<ChildStdin>,
    output: Lines<BufReader<ChildStdout>>,
}
impl Process {
    pub fn spawn(executable: &Path, url: &str, env: &[(&str, &str)]) -> Self {
        let mut command = tokio::process::Command::new(executable);
        command
            .env_clear()
            .env("no_proxy", "*")
            .env("MCP_REMOTE_TOKEN", "platform-sentinel")
            .arg("--remote")
            .arg(url)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true);
        // Windows platform verification/process creation may require the OS directory.
        #[cfg(windows)]
        if let Some(root) = std::env::var_os("SystemRoot") {
            command.env("SystemRoot", root);
        }
        for (name, value) in env {
            command.env(name, value);
        }
        let mut child = command.spawn().unwrap();
        Self {
            input: child.stdin.take(),
            output: BufReader::new(child.stdout.take().unwrap()).lines(),
            child,
        }
    }
    pub async fn initialize(&mut self) -> Option<Value> {
        let input = self.input.as_mut().unwrap();
        let message = json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"platform-install-witness","version":"1"}}});
        if input
            .write_all(format!("{message}\n").as_bytes())
            .await
            .is_err()
        {
            return None;
        }
        let line = tokio::time::timeout(BUDGET, self.output.next_line())
            .await
            .expect("finite installed invocation")
            .unwrap();
        line.map(|line| serde_json::from_str(&line).unwrap())
    }
    pub async fn close(mut self, success: bool) {
        drop(self.input.take());
        let status = tokio::time::timeout(BUDGET, self.child.wait())
            .await
            .expect("original installed gateway retires")
            .unwrap();
        assert_eq!(status.success(), success);
    }
}
pub fn rpc(value: &Value) -> Option<Value> {
    let result = match value["method"].as_str().unwrap() {
        "initialize" => {
            json!({"protocolVersion":"2025-11-25","capabilities":{"tools":{}},"serverInfo":{"name":"loopback-platform","version":"1"}})
        }
        "tools/list" => json!({"tools":[]}),
        "resources/list" => json!({"resources":[]}),
        "prompts/list" => json!({"prompts":[]}),
        "notifications/initialized" => return None,
        method => panic!("unexpected private fixture RPC {method}"),
    };
    Some(json!({"jsonrpc":"2.0","id":value["id"],"result":result}))
}
