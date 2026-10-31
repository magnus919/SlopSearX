use std::{any::Any, io::Write, process::Command as StdCommand};

use super::prelude::*;

const ROLE: &str = "PROCESS_WRAP_JOB_ACCOUNTING_ROLE";
const FIXTURE: &str = "tokio_windows::job_accounting::fixture_process";
const BOUND: Duration = Duration::from_secs(5);
const STDOUT_CAP: u64 = 4096;

#[test]
fn fixture_process() {
	match std::env::var(ROLE).as_deref() {
		Ok("exit") => {
			println!("PW_JOB_ACCOUNTING_EXIT_READY");
			std::io::stdout().flush().unwrap();
		}
		Ok("leaf") => std::thread::sleep(Duration::from_secs(30)),
		Ok("parent") => {
			let descendant = StdCommand::new(std::env::current_exe().unwrap())
				.args(["--exact", FIXTURE, "--nocapture"])
				.env(ROLE, "leaf")
				.stdin(Stdio::null())
				.stdout(Stdio::null())
				.stderr(Stdio::null())
				.spawn()
				.unwrap();
			println!("PW_JOB_ACCOUNTING_DESCENDANT_READY");
			std::io::stdout().flush().unwrap();
			// Deliberate leader exit while the original job still owns the leaf.
			drop(descendant);
		}
		_ => {}
	}
}

fn count(child: &dyn ChildWrapper) -> Result<u32> {
	let any: &dyn Any = child;
	any.downcast_ref::<JobObjectChild>()
		.ok_or_else(|| std::io::Error::other("original JobObject capability missing"))?
		.active_processes()
}

fn spawn(role: &str) -> Result<Box<dyn ChildWrapper>> {
	CommandWrap::with_new(std::env::current_exe()?, |command| {
		command
			.args(["--exact", FIXTURE, "--nocapture"])
			.env(ROLE, role)
			.stdin(Stdio::null())
			.stdout(Stdio::piped())
			.stderr(Stdio::null());
	})
	.wrap(KillOnDrop)
	.wrap(JobObject)
	.spawn()
}

async fn fixture_stdout(child: &mut dyn ChildWrapper) -> Result<String> {
	let stdout = child
		.stdout()
		.take()
		.ok_or_else(|| std::io::Error::other("fixture stdout missing"))?;
	let mut text = String::new();
	tokio::time::timeout(BOUND, async {
		let mut limited = stdout.take(STDOUT_CAP + 1);
		limited.read_to_string(&mut text).await
	})
	.await
	.map_err(std::io::Error::other)??;
	if text.len() as u64 > STDOUT_CAP {
		return Err(std::io::Error::other("fixture stdout bound"));
	}
	// A wrong exact selector must fail this control, even if the helper exits 0.
	assert!(text.contains("running 1 test"));
	assert!(text.contains("1 passed; 0 failed; 0 ignored;"));
	Ok(text)
}

async fn leader_exited(child: &mut dyn ChildWrapper) -> Result<()> {
	tokio::time::timeout(BOUND, async {
		loop {
			if let Some(status) = child.try_wait()? {
				assert!(status.success());
				return Ok(());
			}
			sleep(Duration::from_millis(5)).await;
		}
	})
	.await
	.map_err(std::io::Error::other)?
}

#[tokio::test]
async fn try_wait_before_final_wait_preserves_original_job_completion() -> Result<()> {
	let mut child = spawn("exit")?;
	let text = fixture_stdout(child.as_mut()).await?;
	assert!(text.contains("PW_JOB_ACCOUNTING_EXIT_READY"));
	leader_exited(child.as_mut()).await?;
	// Drain opportunities which consumed completion notifications in the old
	// wrapper; leader observation must not invalidate the final whole-job wait.
	for _ in 0..16 {
		assert!(child.try_wait()?.is_some());
	}
	let status = tokio::time::timeout(BOUND, child.wait())
		.await
		.map_err(std::io::Error::other)??;
	assert!(status.success());
	assert_eq!(count(child.as_ref())?, 0);
	Ok(())
}

#[tokio::test]
async fn leader_exit_does_not_cache_success_while_descendant_is_active() -> Result<()> {
	let mut child = spawn("parent")?;
	let text = fixture_stdout(child.as_mut()).await?;
	assert!(text.contains("PW_JOB_ACCOUNTING_DESCENDANT_READY"));
	leader_exited(child.as_mut()).await?;
	assert!(count(child.as_ref())? > 0);
	assert!(
		tokio::time::timeout(Duration::from_millis(100), child.wait())
			.await
			.is_err()
	);
	// Cancelling wait cannot cache leader-only success or detach a blocker.
	assert!(count(child.as_ref())? > 0);
	child.start_kill()?;
	let status = tokio::time::timeout(BOUND, child.wait())
		.await
		.map_err(std::io::Error::other)??;
	assert!(status.success());
	assert_eq!(count(child.as_ref())?, 0);
	assert!(
		tokio::time::timeout(BOUND, child.wait())
			.await
			.map_err(std::io::Error::other)??
			.success()
	);
	assert_eq!(count(child.as_ref())?, 0);
	Ok(())
}
