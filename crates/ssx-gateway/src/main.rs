use clap::Parser;
use std::process::ExitCode;

#[cfg(unix)]
mod stdio;

#[derive(Parser)]
#[command(about = "Proxy MCP over stdio to a remote SlopSearX server")]
struct Args {
    #[arg(long, env = "MCP_REMOTE_URL", hide_env_values = true)]
    remote: String,
    #[arg(long, env = "MCP_REMOTE_TOKEN", hide_env_values = true)]
    token: Option<String>,
    #[arg(long, env = "MCP_REMOTE_OAUTH", value_parser = env_flag)]
    oauth: bool,
    #[arg(long, env = "MCP_REMOTE_OAUTH_CALLBACK_PORT", default_value_t = 8765)]
    oauth_callback_port: u16,
    #[arg(long, env = "MCP_REMOTE_OAUTH_NO_BROWSER", value_parser = env_flag)]
    oauth_no_browser: bool,
    #[arg(long, env = "MCP_REMOTE_TOKEN_FILE", hide_env_values = true)]
    oauth_token_file: Option<std::path::PathBuf>,
    #[arg(long, env = "MCP_REMOTE_CA_BUNDLE", hide_env_values = true)]
    ca_bundle: Option<std::path::PathBuf>,
}

fn env_flag(value: &str) -> Result<bool, std::convert::Infallible> {
    Ok(matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "yes"
    ))
}

#[cfg(unix)]
#[tokio::main(flavor = "current_thread")]
async fn main() -> ExitCode {
    let args = Args::parse();
    match run(args).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("{message}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(unix)]
async fn run(args: Args) -> Result<(), &'static str> {
    use ssx_mcp::{
        gateway_oauth::{AUTHORIZATION_TIMEOUT, GatewayOAuth},
        gateway_roots,
        opaque::RawGateway,
    };
    use std::sync::Arc;
    use std::time::Duration;
    use tokio::signal::unix::{SignalKind, signal};
    use tokio_util::sync::CancellationToken;

    let mut interrupt = signal(SignalKind::interrupt()).map_err(|_| "Could not watch SIGINT")?;
    let mut terminate = signal(SignalKind::terminate()).map_err(|_| "Could not watch SIGTERM")?;
    let cancel = CancellationToken::new();
    let stopping = cancel.clone();
    let signals = tokio::spawn(async move {
        tokio::select! { _ = interrupt.recv() => (), _ = terminate.recv() => () }
        stopping.cancel();
    });

    let result = async {
        // Acquire stdio before starting remote initialization so a local setup
        // error cannot leave a granted upstream session behind.
        let mut input = stdio::Bridge::stdin().map_err(|_| "Could not open gateway input")?;
        let mut output = stdio::Bridge::stdout().map_err(|_| "Could not open gateway output")?;
        let token = args.token.filter(|value| !value.is_empty());
        let ca_path = args.ca_bundle.filter(|path| !path.as_os_str().is_empty());
        if args.oauth && token.is_some() {
            return Err("--oauth and --token are mutually exclusive");
        }
        let roots = gateway_roots::load_ca_bundle(ca_path.as_deref())
            .await
            .map_err(|_| "Could not load additive gateway CA bundle")?;
        let connection = async {
            let request_timeout = Duration::from_secs(30);
            let connected = if args.oauth {
                let remote = args.remote.trim().to_owned();
                let token_file = args.oauth_token_file.filter(|path| !path.as_os_str().is_empty());
                let authorizer = GatewayOAuth::open_with_roots(
                    &remote,
                    token_file,
                    args.oauth_callback_port,
                    args.oauth_no_browser,
                    roots.clone(),
                )
                .await
                .map_err(|_| "Could not load OAuth credentials. Check the remote URL and token file permissions, or specify a fresh --oauth-token-file.")?;
                RawGateway::connect_with_authorizer_and_roots(
                    remote,
                    Arc::new(authorizer),
                    request_timeout,
                    AUTHORIZATION_TIMEOUT,
                    &cancel,
                    &roots,
                )
                .await
            } else {
                RawGateway::connect_cancellable_with_roots(
                    args.remote,
                    token,
                    request_timeout,
                    &cancel,
                    &roots,
                )
                .await
            };
            connected.map_err(|_| "Could not connect to the remote MCP server")
        };
        tokio::pin!(connection);
        let connected = tokio::select! {
            result = &mut connection => result,
            _ = input.closed() => {
                cancel.cancel();
                connection.await
            }
        };
        let gateway = match connected {
            Ok(gateway) => gateway,
            Err(_) if cancel.is_cancelled() => {
                return input.stop().map_err(|_| "Gateway input failed");
            }
            Err(message) => return Err(message),
        };
        let service = gateway
            .stdio_with_io(input.stream(), output.stream(), cancel.clone())
            .await
            .map_err(|_| "Gateway session failed");
        let input_closed = input.stop().map_err(|_| "Gateway input failed");
        let output_closed = if service.is_ok() {
            output.finish(&cancel).await
        } else {
            output.stop()
        }
        .map_err(|_| "Gateway output failed");
        service.and(input_closed).and(output_closed)
    }
    .await;
    signals.abort();
    let _ = signals.await;
    result
}

#[cfg(not(unix))]
fn main() -> ExitCode {
    let _ = Args::parse();
    eprintln!("Native gateway operation is pending on this platform");
    ExitCode::FAILURE
}
