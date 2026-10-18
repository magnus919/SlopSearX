use clap::{Parser, Subcommand};
use ssx_infra::composition::Resolved;
use std::{path::PathBuf, process::ExitCode};
use tokio::sync::{oneshot, watch};
#[derive(Parser)]
#[command(about = "Native durable research worker")]
struct Args {
    #[arg(long)]
    config: Option<PathBuf>,
    #[arg(long)]
    check_config: bool,
    #[command(subcommand)]
    command: Command,
}
#[derive(Subcommand)]
enum Command {
    Research,
}
// Synchronous OS registration completes before any resource-acquiring async work.
fn signals(
    stop: watch::Sender<bool>,
    mut finished: watch::Receiver<bool>,
) -> Result<tokio::task::JoinHandle<Result<(), ()>>, ()> {
    #[cfg(unix)]
    let (mut interrupt, mut terminate) = (
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt())
            .map_err(|_| ())?,
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .map_err(|_| ())?,
    );
    #[cfg(windows)]
    let mut interrupt = tokio::signal::windows::ctrl_c().map_err(|_| ())?;
    Ok(tokio::spawn(async move {
        #[cfg(unix)]
        {
            tokio::select! {biased;_=finished.changed()=>Ok(()),signal=interrupt.recv()=>{stop.send_replace(true);eprintln!("worker_stopping role=research");signal.ok_or(())},signal=terminate.recv()=>{stop.send_replace(true);eprintln!("worker_stopping role=research");signal.ok_or(())}}
        }
        #[cfg(windows)]
        {
            tokio::select! {biased;_=finished.changed()=>Ok(()),signal=interrupt.recv()=>{stop.send_replace(true);eprintln!("worker_stopping role=research");signal.ok_or(())}}
        }
    }))
}
#[tokio::main]
async fn main() -> ExitCode {
    let args = Args::parse();
    match args.command {
        Command::Research => {}
    }
    let mut env = Vec::new();
    for (key, value) in std::env::vars_os() {
        let (Ok(key), Ok(value)) = (key.into_string(), value.into_string()) else {
            eprintln!("worker_failed stage=config field=non_unicode_environment");
            return ExitCode::FAILURE;
        };
        env.push((key, value));
    }
    let resolved = match Resolved::resolve_entries(args.config.as_deref(), &env) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("worker_failed {e}");
            return ExitCode::FAILURE;
        }
    };
    if args.check_config {
        eprintln!("worker_configuration_valid role=research");
        return ExitCode::SUCCESS;
    }
    let (stop, receiver) = watch::channel(false);
    let (finished, finish_rx) = watch::channel(false);
    let signal_owner = match signals(stop.clone(), finish_rx) {
        Ok(v) => v,
        Err(()) => {
            eprintln!("worker_failed stage=signal field=registration");
            return ExitCode::FAILURE;
        }
    };
    eprintln!("worker_starting role=research");
    // Await this constructor to completion on every path; it owns cancellation cleanup.
    let result = match resolved.open_stoppable(receiver.clone()).await {
        Err(e) if e.stage == "stopped" => {
            eprintln!("worker_startup_stopped role=research");
            Ok(())
        }
        Err(e) => Err(e),
        Ok(runtime) => {
            let (started, entered) = oneshot::channel();
            let running = runtime.run_research(receiver, started);
            tokio::pin!(running);
            tokio::select! {biased;
                result=&mut running=>result,
                entered=entered=>{
                    // Hold the watch read guard through the readiness write, so a
                    // stop already recorded by the signal owner suppresses ready.
                    if entered.is_ok() {let stopping=stop.borrow();if !*stopping {eprintln!("worker_ready role=research");}}
                    running.await
                }
            }
        }
    };
    finished.send_replace(true);
    let signal_result = signal_owner.await;
    if !matches!(signal_result, Ok(Ok(()))) {
        eprintln!("worker_failed stage=signal field=owner");
        return ExitCode::FAILURE;
    }
    match result {
        Ok(()) => {
            eprintln!("worker_stopped role=research");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("worker_failed {e}");
            ExitCode::FAILURE
        }
    }
}
