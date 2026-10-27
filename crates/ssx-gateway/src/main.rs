//! Native remote client: this executable never constructs a search backend.
use ssx_mcp::gateway_client::{runtime, settings::Settings};
use std::{collections::BTreeMap, process::ExitCode};
use tokio_util::sync::CancellationToken;
const HELP: &str = "ssx-gateway --remote URL [--token TOKEN | --oauth] [--oauth-callback-port PORT] [--oauth-no-browser] [--oauth-token-file PATH]\nEnvironment: MCP_REMOTE_URL, MCP_REMOTE_TOKEN, MCP_REMOTE_OAUTH, MCP_REMOTE_OAUTH_CALLBACK_PORT, MCP_REMOTE_OAUTH_NO_BROWSER, MCP_REMOTE_TOKEN_FILE, MCP_REMOTE_CA_BUNDLE\nRemote mode uses stdio. Credentials and URLs are never included in error diagnostics.";
#[tokio::main]
async fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        println!("{HELP}");
        return ExitCode::SUCCESS;
    }
    if args.as_slice() == ["--version"] {
        println!("ssx-gateway {}", env!("CARGO_PKG_VERSION"));
        return ExitCode::SUCCESS;
    }
    let env: BTreeMap<String, String> = [
        "MCP_REMOTE_URL",
        "MCP_REMOTE_TOKEN",
        "MCP_REMOTE_OAUTH",
        "MCP_REMOTE_OAUTH_CALLBACK_PORT",
        "MCP_REMOTE_OAUTH_NO_BROWSER",
        "MCP_REMOTE_TOKEN_FILE",
    ]
    .into_iter()
    .filter_map(|key| std::env::var(key).ok().map(|value| (key.to_owned(), value)))
    .collect();
    let settings = match Settings::parse(&args, &env) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("gateway configuration failed: {error:?}\n{HELP}");
            return ExitCode::FAILURE;
        }
    };
    let cancel = CancellationToken::new();
    let signal = cancel.clone();
    let task = tokio::spawn(async move {
        #[cfg(unix)]
        {
            if let Ok(mut terminate) =
                tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            {
                tokio::select! {_=tokio::signal::ctrl_c()=>{},_=terminate.recv()=>{}}
            } else {
                let _ = tokio::signal::ctrl_c().await;
            }
        }
        #[cfg(not(unix))]
        {
            let _ = tokio::signal::ctrl_c().await;
        }
        signal.cancel()
    });
    let result = runtime::run(settings, cancel).await;
    task.abort();
    let _ = task.await;
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(_) => {
            eprintln!("gateway connection or cleanup failed");
            ExitCode::FAILURE
        }
    }
}
