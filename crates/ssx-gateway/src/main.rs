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
    use ssx_mcp::opaque::RawGateway;
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
        let connection =
            RawGateway::connect_cancellable(args.remote, token, Duration::from_secs(30), &cancel);
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
            Err(_) => return Err("Could not connect to the remote MCP server"),
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
