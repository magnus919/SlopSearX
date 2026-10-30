//! Developer-gate child owner. Fixed-size drains, finite retention and tree cleanup.
use process_wrap::tokio::{ChildWrapper, CommandWrap, KillOnDrop};
use serde_json::{Value, json};
use std::{
    io::Write,
    process::{Command, Stdio},
    time::{Duration, Instant},
};
use tokio::io::{AsyncRead, AsyncReadExt};

pub struct Captured {
    pub pid: u32,
    pub status: Option<i32>,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub error: Option<String>,
    pub cleanup: Value,
}
struct Owner {
    child: Box<dyn ChildWrapper>,
    armed: bool,
}
impl Drop for Owner {
    fn drop(&mut self) {
        // Drop only signals. Normal/error paths below own bounded wait/drain.
        if self.armed {
            let _ = self.child.start_kill();
        }
    }
}
async fn drain(
    mut pipe: impl AsyncRead + Unpin,
    bytes: &mut Vec<u8>,
    cap: usize,
    stream: &str,
    eof: &mut bool,
) -> Result<(), String> {
    let mut chunk = [0u8; 1024];
    loop {
        let n = pipe
            .read(&mut chunk)
            .await
            .map_err(|e| format!("{stream} read: {e}"))?;
        if n == 0 {
            *eof = true;
            return Ok(());
        }
        let retain = n.min(cap.saturating_sub(bytes.len()));
        bytes.extend_from_slice(&chunk[..retain]);
        if retain != n {
            return Err(format!("{stream} output exceeds {cap} bytes"));
        }
    }
}
async fn parent_exit(
    child: &mut dyn ChildWrapper,
    observed: &mut Option<std::process::ExitStatus>,
    observed_ms: &mut Option<u128>,
    started: Instant,
) -> Result<std::process::ExitStatus, String> {
    loop {
        if let Some(status) = child.try_wait().map_err(|e| format!("child wait: {e}"))? {
            *observed = Some(status);
            *observed_ms = Some(started.elapsed().as_millis());
            return Ok(status);
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
}
#[cfg(unix)]
async fn tree_absent(pid: u32) -> bool {
    use nix::{errno::Errno, sys::signal::killpg, unistd::Pid};
    let Ok(pid) = i32::try_from(pid) else {
        return false;
    };
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    let mut absent = 0;
    while tokio::time::Instant::now() < deadline {
        match killpg(Pid::from_raw(pid), None) {
            Err(Errno::ESRCH) => {
                absent += 1;
                if absent == 2 {
                    return true;
                }
            }
            _ => absent = 0,
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    false
}
pub fn capture(
    command: Command,
    budget: Duration,
    stdout_cap: usize,
    stderr_cap: usize,
) -> Result<Captured, String> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| e.to_string())?;
    let result = runtime.block_on(async {
        let mut command = tokio::process::Command::from(command);
        command.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
        let mut command = CommandWrap::from(command);
        command.wrap(KillOnDrop);
        #[cfg(unix)] { command.wrap(process_wrap::tokio::ProcessGroup::leader()); }
        #[cfg(windows)] { command.wrap(process_wrap::tokio::JobObject); }
        let mut owner = Owner { child: command.spawn().map_err(|e|format!("child spawn: {e}"))?, armed: true };
        let pid = owner.child.id();
        let started = Instant::now();
        let mut observed = None;
        let mut observed_ms = None;
        let mut stdout_eof = false;
        let mut stderr_eof = false;
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let pipes = (owner.child.stdout().take(), owner.child.stderr().take());
        let run = if pid.is_none() {
            drop(pipes);
            Err("spawned child PID absent".to_owned())
        } else if let (Some(out), Some(err)) = pipes {
            // No detached reader tasks: cancellation drops both pipes and ends
            // every read future before cleanup starts. The buffers remain owned.
            let work = async {
                tokio::try_join!(parent_exit(owner.child.as_mut(), &mut observed, &mut observed_ms, started), drain(out, &mut stdout, stdout_cap, "stdout", &mut stdout_eof), drain(err, &mut stderr, stderr_cap, "stderr", &mut stderr_eof))
            };
            match tokio::time::timeout(budget, work).await {
                Ok(Ok((status, (), ()))) => Ok(status),
                Ok(Err(error)) => Err(error),
                Err(_) => Err("child/output EOF deadline".to_owned()),
            }
        } else { Err("spawned child pipe absent".to_owned()) };
        // Tree kill also runs after parent exit: a descendant retaining a pipe
        // or silently running without one must not escape. Only now call wait,
        // after termination, so Windows job completion cannot block run budget.
        let killed = owner.child.start_kill();
        let kill_error = killed.as_ref().err().map(ToString::to_string);
        #[cfg(windows)]
        let may_wait = killed.is_ok();
        #[cfg(unix)]
        let may_wait = true;
        let waited = if may_wait {
            match tokio::time::timeout(Duration::from_secs(5), owner.child.wait()).await {
                Ok(Ok(_)) => true,
                _ => false,
            }
        } else { false };
        #[cfg(unix)]
        let retired = waited && if let Some(pid) = pid { tree_absent(pid).await } else { false };
        #[cfg(windows)]
        let retired = waited && killed.is_ok(); // JobObject wait observes zero active processes.
        if retired { owner.armed = false; }
        let status = observed.and_then(|s|s.code());
        let mut error = run.err();
        if !retired { error = Some(format!("{}; child tree retirement unproven",error.unwrap_or_default())); }
        let cleanup = json!({"kill_attempted":true,"kill_error":kill_error,"waited":waited,"tree_retired":retired,"read_futures_dropped_before_cleanup":true,"stdout_retained":stdout.len(),"stderr_retained":stderr.len(),"stdout_cap":stdout_cap,"stderr_cap":stderr_cap,"cleanup_bounds":{"wait_seconds":5,"unix_absence_seconds":if cfg!(unix){5}else{0},"runtime_shutdown_seconds":1,"total_seconds":if cfg!(unix){11}else{6}},"parent_exit_observed_before_cleanup":observed.is_some(),"parent_exit_observed_ms":observed_ms,"stdout_eof_before_cleanup":stdout_eof,"stderr_eof_before_cleanup":stderr_eof,"containment":if cfg!(windows){"Windows JobObject"}else{"Unix process group, two absent observations"}});
        Ok(Captured{pid:pid.unwrap_or(0),status,stdout,stderr,error,cleanup})
    });
    // process-wrap's Windows final job wait may use spawn_blocking. A failed
    // OS retirement never turns runtime Drop into an unbounded hidden wait.
    runtime.shutdown_timeout(Duration::from_secs(1));
    result
}
pub fn fixture_entry() -> Option<i32> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if args.first().map(String::as_str) != Some("__t08a-child-fixture") {
        return None;
    }
    let mode = args.get(1).map(String::as_str).unwrap_or("");
    match mode {
        "flood" => {
            let block = [b'x'; 1024];
            for _ in 0..4096 {
                if std::io::stdout().write_all(&block).is_err() {
                    break;
                }
            }
            Some(0)
        }
        "timeout" | "hold-pipes" => {
            println!("T08A_READY {}", std::process::id());
            std::io::stdout().flush().unwrap();
            std::thread::sleep(Duration::from_secs(30));
            Some(0)
        }
        "eof-descendant" => {
            let image = std::env::current_exe().unwrap();
            let mut child = Command::new(image)
                .args(["__t08a-child-fixture", "hold-pipes"])
                .spawn()
                .unwrap();
            // This fixture deliberately exits without waiting. The owner under
            // test must contain and terminate the inherited-pipe descendant.
            let _ = child.try_wait();
            Some(0)
        }
        _ => Some(2),
    }
}
pub fn controls(image: &std::path::Path, records: &mut Vec<Value>) -> Result<(), String> {
    for (mode, expected) in [
        ("flood", "stdout output exceeds 1024 bytes"),
        ("timeout", "child/output EOF deadline"),
        ("eof-descendant", "child/output EOF deadline"),
    ] {
        let mut command = Command::new(image);
        command
            .args(["__t08a-child-fixture", mode])
            .env_clear()
            .env("PATH", "/nonexistent");
        windows_system_environment(&mut command);
        let started = Instant::now();
        let actual = capture(command, Duration::from_secs(2), 1024, 1024)?;
        let passed = actual.error.as_deref() == Some(expected)
            && actual.cleanup["tree_retired"] == true
            && actual.stdout.len() <= 1024
            && actual.stderr.len() <= 1024
            && started.elapsed() <= Duration::from_secs(15)
            && (mode == "flood" || String::from_utf8_lossy(&actual.stdout).contains("T08A_READY"))
            && (mode != "timeout"
                || actual.cleanup["parent_exit_observed_before_cleanup"] == false)
            && (mode != "eof-descendant"
                || (actual.status == Some(0)
                    && actual.cleanup["parent_exit_observed_before_cleanup"] == true
                    && actual.cleanup["stdout_eof_before_cleanup"] == false));
        records.push(json!({"control":mode,"expected":expected,"error":actual.error,"pid":actual.pid,"status":actual.status,"stdout":String::from_utf8_lossy(&actual.stdout),"stderr":String::from_utf8_lossy(&actual.stderr),"elapsed_ms":started.elapsed().as_millis(),"cleanup":actual.cleanup,"passed":passed}));
        if !passed {
            return Err(format!("T08A owning child control failed: {mode}"));
        }
    }
    Ok(())
}
pub fn windows_system_environment(command: &mut Command) {
    #[cfg(windows)]
    for name in ["SystemRoot", "WINDIR"] {
        if let Some(value) = std::env::var_os(name) {
            command.env(name, value);
        }
    }
    #[cfg(not(windows))]
    let _ = command;
}
