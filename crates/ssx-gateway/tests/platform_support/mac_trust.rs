//! Bounded owner for the ephemeral hosted user-domain trust fixture.
//! Fake-command controls run on Unix; only the guarded macOS entry uses security.
use nix::{
    errno::Errno,
    sys::signal::{Signal, killpg},
    unistd::Pid,
};
use sha2::{Digest, Sha256};
use std::{
    ffi::OsString,
    fs,
    os::unix::process::CommandExt,
    path::{Path, PathBuf},
    process::{ExitStatus, Stdio},
    time::Duration,
};
use tokio::{
    io::{AsyncRead, AsyncReadExt},
    process::Command,
    time::{Instant, timeout, timeout_at},
};

const OUTPUT_CAP: usize = 16 * 1024;
const INPUT_CAP: u64 = 64 * 1024;
const MARKER: &str = "trust-active";
const PREFIX: &str = "platform-tls-store-";

#[derive(Clone, Copy)]
struct Limits {
    command: Duration,
    grace: Duration,
    reap: Duration,
    drain: Duration,
    install: Duration,
    remove: Duration,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            command: Duration::from_secs(15),
            grace: Duration::from_millis(250),
            reap: Duration::from_secs(1),
            drain: Duration::from_millis(250),
            install: Duration::from_secs(90),
            remove: Duration::from_secs(55),
        }
    }
}

#[derive(Debug)]
struct Capture {
    bytes: Vec<u8>,
    clipped: bool,
}
async fn capture(mut pipe: impl AsyncRead + Unpin) -> std::io::Result<Capture> {
    let mut result = Capture {
        bytes: Vec::new(),
        clipped: false,
    };
    let mut buffer = [0; 4096];
    loop {
        let count = pipe.read(&mut buffer).await?;
        if count == 0 {
            return Ok(result);
        }
        let retained = count.min(OUTPUT_CAP.saturating_sub(result.bytes.len()));
        result.bytes.extend_from_slice(&buffer[..retained]);
        result.clipped |= retained < count;
    }
}

struct Group {
    pid: Pid,
    armed: bool,
}
impl Group {
    fn signal(&self, signal: Signal) -> Result<(), String> {
        match killpg(self.pid, signal) {
            Ok(()) | Err(Errno::ESRCH) => Ok(()),
            Err(error) => Err(format!("owned process group {}: {error}", self.pid)),
        }
    }
    async fn retire(&mut self, budget: Duration) -> Result<(), String> {
        self.signal(Signal::SIGKILL)?;
        let deadline = Instant::now() + budget;
        loop {
            match killpg(self.pid, None) {
                Err(Errno::ESRCH) => {
                    // Ownership is retired; Drop must never signal this numeric ID again.
                    self.armed = false;
                    return Ok(());
                }
                Err(error) => return Err(format!("owned process group absence query: {error}")),
                Ok(()) => {}
            }
            if Instant::now() >= deadline {
                return Err(format!(
                    "owned process group {} still exists after bounded retirement",
                    self.pid
                ));
            }
            tokio::time::sleep_until(deadline.min(Instant::now() + Duration::from_millis(10)))
                .await;
        }
    }
}
impl Drop for Group {
    fn drop(&mut self) {
        if self.armed {
            // Cancellation fallback applies only while ownership remains unretired.
            let _ = self.signal(Signal::SIGKILL);
        }
    }
}

struct Security {
    executable: PathBuf,
    limits: Limits,
}
impl Security {
    #[cfg(target_os = "macos")]
    fn hosted() -> Result<Self, String> {
        if std::env::var("GITHUB_ACTIONS").as_deref() != Ok("true")
            || std::env::var("SLOPSEARX_PLATFORM_TRUST_FIXTURE").as_deref() != Ok("1")
        {
            return Err("trust mutation requires the dedicated ephemeral Actions fixture".into());
        }
        Ok(Self {
            executable: "/usr/bin/security".into(),
            limits: Limits::default(),
        })
    }

    async fn run(
        &self,
        root: &Path,
        args: Vec<OsString>,
        overall: Instant,
    ) -> Result<Vec<u8>, String> {
        let label = args
            .first()
            .map(|value| value.to_string_lossy())
            .unwrap_or_default();
        let started = Instant::now();
        if started >= overall {
            return Err(format!("security {label}: overall budget exhausted"));
        }
        eprintln!("owned security {label}: starting");
        let mut command = Command::new(&self.executable);
        command
            .args(&args)
            .current_dir(root)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        command.as_std_mut().process_group(0);
        let mut child = command
            .spawn()
            .map_err(|error| format!("security {label}: spawn: {error}"))?;
        let mut group = Group {
            pid: Pid::from_raw(
                i32::try_from(child.id().ok_or("child has no pid")?)
                    .map_err(|error| error.to_string())?,
            ),
            armed: true,
        };
        let mut stdout = tokio::spawn(capture(child.stdout.take().ok_or("missing stdout")?));
        let mut stderr = tokio::spawn(capture(child.stderr.take().ok_or("missing stderr")?));
        let deadline = overall.min(started + self.limits.command);
        let mut errors = Vec::new();
        let status: Option<ExitStatus> = match timeout_at(deadline, child.wait()).await {
            Ok(Ok(status)) => Some(status),
            outcome => {
                errors.push(match outcome {
                    Err(_) => "deadline exceeded".to_owned(),
                    Ok(Err(error)) => format!("wait: {error}"),
                    Ok(Ok(_)) => unreachable!(),
                });
                if let Err(error) = group.signal(Signal::SIGTERM) {
                    errors.push(error);
                }
                // Retire the entire group even if the leader exits during the grace.
                let graceful = timeout(self.limits.grace, child.wait()).await;
                if let Err(error) = group.signal(Signal::SIGKILL) {
                    errors.push(error);
                }
                match graceful {
                    Ok(Ok(status)) => Some(status),
                    _ => match timeout(self.limits.reap, child.wait()).await {
                        Ok(Ok(status)) => Some(status),
                        outcome => {
                            errors.push(format!("bounded reap failed: {outcome:?}"));
                            None
                        }
                    },
                }
            }
        };
        // A successful leader is not permission to leave descendants running.
        if let Err(error) = group.retire(self.limits.reap).await {
            errors.push(error);
        }
        let drain_deadline = Instant::now() + self.limits.drain;
        let mut captures = Vec::new();
        for (name, task) in [("stdout", &mut stdout), ("stderr", &mut stderr)] {
            match timeout_at(drain_deadline, &mut *task).await {
                Ok(Ok(Ok(output))) => captures.push((name, output)),
                outcome => {
                    task.abort();
                    // Aborted async readers have no blocking work or owned children.
                    let _ = task.await;
                    errors.push(format!("{name} drain failed: {outcome:?}"));
                }
            }
        }
        let success = status.is_some_and(|status| status.success());
        if !success {
            errors.push(format!("exit: {status:?}"));
        }
        // Clipped output is a failure: list-keychains must never be partially restored.
        if captures.iter().any(|(_, output)| output.clipped) {
            errors.push("output cap exceeded".into());
        }
        eprintln!(
            "owned security {label}: finished in {:?}, status {status:?}",
            started.elapsed()
        );
        if !errors.is_empty() {
            return Err(format!(
                "security {label} after {:?}: {}; {}",
                started.elapsed(),
                errors.join("; "),
                captures
                    .iter()
                    .map(|(name, output)| format!(
                        "{name}{}={}",
                        if output.clipped { "[clipped]" } else { "" },
                        String::from_utf8_lossy(&output.bytes)
                    ))
                    .collect::<Vec<_>>()
                    .join("; ")
            ));
        }
        captures
            .into_iter()
            .find_map(|(name, output)| (name == "stdout").then_some(output.bytes))
            .ok_or_else(|| "stdout was not collected".into())
    }
}

fn regular(path: &Path, cap: u64) -> Result<Vec<u8>, String> {
    let metadata =
        fs::symlink_metadata(path).map_err(|error| format!("{}: {error}", path.display()))?;
    if !metadata.is_file() || metadata.len() > cap {
        return Err("invalid bounded fixture file".into());
    }
    use std::io::Read;
    let mut bytes = Vec::new();
    fs::File::open(path)
        .map_err(|error| error.to_string())?
        .take(cap + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    if bytes.len() as u64 > cap {
        return Err("fixture grew beyond cap".into());
    }
    Ok(bytes)
}
fn digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
fn owned_root(base: &Path, root: &Path) -> Result<PathBuf, String> {
    if !base.is_absolute()
        || !root.is_absolute()
        || !fs::symlink_metadata(base)
            .map_err(|error| error.to_string())?
            .is_dir()
        || !fs::symlink_metadata(root)
            .map_err(|error| error.to_string())?
            .is_dir()
    {
        return Err("fixture root must be an existing absolute real directory".into());
    }
    let base = base.canonicalize().map_err(|error| error.to_string())?;
    let canonical = root.canonicalize().map_err(|error| error.to_string())?;
    if canonical != root
        || canonical.parent() != Some(base.as_path())
        || !canonical
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with(PREFIX))
    {
        return Err("trust fixture is outside the exact registered job root".into());
    }
    Ok(canonical)
}
fn search_list(bytes: &[u8]) -> Result<Vec<String>, String> {
    let text = std::str::from_utf8(bytes).map_err(|error| error.to_string())?;
    let mut paths = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let path = line
            .strip_prefix('"')
            .and_then(|line| line.strip_suffix('"'))
            .ok_or("malformed saved user keychain search list")?;
        validate_search_path(path)?;
        paths.push(path.to_owned());
    }
    if paths.len() > 128 {
        return Err("too many user keychains".into());
    }
    Ok(paths)
}
fn validate_search_path(path: &str) -> Result<(), String> {
    if !Path::new(path).is_absolute() || path.contains(['\0', '\n', '\r', '"']) || path.len() > 4096
    {
        return Err("malformed saved user keychain path".into());
    }
    Ok(())
}
fn keychain(root: &Path) -> PathBuf {
    root.join("platform-fixture.keychain-db")
}
fn marker(root: &Path, original: &[String]) -> Result<serde_json::Value, String> {
    for path in original {
        validate_search_path(path)?;
    }
    Ok(serde_json::json!({
        "version": 1,
        "der_sha256": digest(&regular(&root.join("ca.der"), INPUT_CAP)?),
        "pem_sha256": digest(&regular(&root.join("ca.pem"), INPUT_CAP)?),
        "keychain": keychain(root),
        "original": original,
    }))
}
fn validate_marker(root: &Path) -> Result<Vec<String>, String> {
    let value: serde_json::Value = serde_json::from_slice(&regular(&root.join(MARKER), INPUT_CAP)?)
        .map_err(|error| error.to_string())?;
    let original: Vec<String> =
        serde_json::from_value(value["original"].clone()).map_err(|error| error.to_string())?;
    if original.len() > 128
        || value != marker(root, &original)?
        || original
            .iter()
            .any(|path| Path::new(path) == keychain(root))
    {
        return Err("trust ownership marker does not match the exact fixture".into());
    }
    if let Ok(metadata) = fs::symlink_metadata(keychain(root))
        && !metadata.is_file()
    {
        return Err("owned keychain is not a real file".into());
    }
    Ok(original)
}
fn args(values: &[&str]) -> Vec<OsString> {
    values.iter().map(OsString::from).collect()
}

async fn install_with(security: &Security, base: &Path, root: &Path) -> Result<(), String> {
    let root = owned_root(base, root)?;
    if root
        .join(MARKER)
        .try_exists()
        .map_err(|error| error.to_string())?
        || fs::symlink_metadata(keychain(&root)).is_ok()
    {
        return Err("refuse to adopt an existing trust fixture".into());
    }
    let deadline = Instant::now() + security.limits.install;
    let original = search_list(
        &security
            .run(&root, args(&["list-keychains", "-d", "user"]), deadline)
            .await?,
    )?;
    let marker = marker(&root, &original)?;
    // create_new establishes exclusive ownership before the first mutation.
    use std::io::Write;
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(root.join(MARKER))
        .map_err(|error| error.to_string())?;
    file.write_all(&serde_json::to_vec(&marker).map_err(|error| error.to_string())?)
        .map_err(|error| error.to_string())?;
    file.sync_all().map_err(|error| error.to_string())?;
    let store = keychain(&root).into_os_string();
    let mut create = args(&["create-keychain", "-p", "ephemeral-platform-fixture"]);
    create.push(store.clone());
    security.run(&root, create, deadline).await?;
    let mut unlock = args(&["unlock-keychain", "-p", "ephemeral-platform-fixture"]);
    unlock.push(store.clone());
    security.run(&root, unlock, deadline).await?;
    let mut list = args(&["list-keychains", "-d", "user", "-s"]);
    list.extend(original.into_iter().map(OsString::from));
    list.push(store.clone());
    security.run(&root, list, deadline).await?;
    let mut trust = args(&["add-trusted-cert", "-r", "trustRoot", "-k"]);
    trust.extend([store, root.join("ca.pem").into_os_string()]);
    security.run(&root, trust, deadline).await?;
    Ok(())
}
async fn remove_with(security: &Security, base: &Path, root: &Path) -> Result<(), String> {
    remove_before(
        security,
        base,
        root,
        Instant::now() + security.limits.remove,
    )
    .await
}
async fn remove_before(
    security: &Security,
    base: &Path,
    root: &Path,
    overall: Instant,
) -> Result<(), String> {
    let root = owned_root(base, root)?;
    let original = validate_marker(&root)?;
    let deadline = overall.min(Instant::now() + security.limits.remove);
    let mut errors = Vec::new();
    let mut remove = args(&["remove-trusted-cert"]);
    remove.push(root.join("ca.pem").into_os_string());
    // No stderr matching or guessed idempotence: every nonzero result is retained.
    // Exact user-domain Security.framework absence proof would be needed to waive it.
    if let Err(error) = security.run(&root, remove, deadline).await {
        errors.push(error);
    }
    let mut restore = args(&["list-keychains", "-d", "user", "-s"]);
    restore.extend(original.into_iter().map(OsString::from));
    if let Err(error) = security.run(&root, restore, deadline).await {
        errors.push(error);
    }
    if keychain(&root)
        .try_exists()
        .map_err(|error| error.to_string())?
    {
        let mut delete = args(&["delete-keychain"]);
        delete.push(keychain(&root).into_os_string());
        if let Err(error) = security.run(&root, delete, deadline).await {
            errors.push(error);
        }
        match keychain(&root).try_exists() {
            Ok(false) => {}
            Ok(true) => errors.push("owned keychain still exists after deletion".into()),
            Err(error) => errors.push(format!("owned keychain absence query: {error}")),
        }
    }
    if !errors.is_empty() {
        return Err(errors.join("\n"));
    }
    fs::remove_file(root.join(MARKER)).map_err(|error| error.to_string())
}
async fn finalize_with(security: &Security, base: &Path) -> Result<(), String> {
    let mut errors = Vec::new();
    let deadline = Instant::now() + Duration::from_secs(90);
    for (count, entry) in fs::read_dir(base)
        .map_err(|error| error.to_string())?
        .enumerate()
    {
        if count == 256 {
            errors.push("fixture root directory count exceeded".into());
            break;
        }
        let entry = entry.map_err(|error| error.to_string())?;
        let root = entry.path();
        if entry
            .file_name()
            .to_str()
            .is_some_and(|name| name.starts_with(PREFIX))
            && fs::symlink_metadata(root.join(MARKER)).is_ok()
            && let Err(error) = remove_before(security, base, &root, deadline).await
        {
            errors.push(format!("{}: {error}", root.display()));
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join("\n"))
    }
}
#[cfg(target_os = "macos")]
fn registered() -> Result<PathBuf, String> {
    std::env::var_os("SLOPSEARX_INSTALL_TEST_ROOT")
        .map(PathBuf::from)
        .ok_or_else(|| "missing registered fixture root".into())
}
#[cfg(target_os = "macos")]
pub async fn install(root: &Path) -> Result<(), String> {
    install_with(&Security::hosted()?, &registered()?, root).await
}
#[cfg(target_os = "macos")]
pub async fn remove(root: &Path) -> Result<(), String> {
    remove_with(&Security::hosted()?, &registered()?, root).await
}
#[cfg(target_os = "macos")]
pub async fn finalize() -> Result<(), String> {
    finalize_with(&Security::hosted()?, &registered()?).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        os::unix::fs::{DirBuilderExt, PermissionsExt},
        sync::atomic::{AtomicUsize, Ordering},
    };
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    struct Fake {
        base: PathBuf,
        root: PathBuf,
        security: Security,
    }
    impl Fake {
        fn new(script: &str) -> Self {
            let base = PathBuf::from(
                std::env::var_os("SLOPSEARX_INSTALL_TEST_ROOT")
                    .expect("registered fake fixture root"),
            )
            .canonicalize()
            .unwrap();
            let root = base.join(format!(
                "{PREFIX}fake-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::DirBuilder::new().mode(0o700).create(&root).unwrap();
            fs::write(root.join("ca.der"), b"fake DER; never imported").unwrap();
            fs::write(root.join("ca.pem"), b"fake PEM; never imported").unwrap();
            let executable = root.join("fake-security");
            fs::write(
                &executable,
                format!(
                    r#"#!/bin/sh
printf '%s\n' "$1" >> calls
{script}
"#
                ),
            )
            .unwrap();
            fs::set_permissions(&executable, fs::Permissions::from_mode(0o700)).unwrap();
            let security = Security {
                executable,
                limits: Limits {
                    command: Duration::from_secs(1),
                    grace: Duration::from_millis(150),
                    reap: Duration::from_secs(1),
                    drain: Duration::from_millis(150),
                    install: Duration::from_secs(12),
                    remove: Duration::from_secs(6),
                },
            };
            Self {
                base,
                root,
                security,
            }
        }
        fn owned(&self) {
            fs::write(
                self.root.join(MARKER),
                serde_json::to_vec(
                    &marker(&self.root, &["/original/login.keychain-db".into()]).unwrap(),
                )
                .unwrap(),
            )
            .unwrap();
            fs::write(keychain(&self.root), b"owned fake store").unwrap();
        }
        fn calls(&self) -> Vec<String> {
            fs::read_to_string(self.root.join("calls"))
                .unwrap_or_default()
                .lines()
                .map(str::to_owned)
                .collect()
        }
    }
    impl Drop for Fake {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.root).unwrap();
        }
    }

    #[tokio::test]
    async fn bounded_security_timeout_reaps_leader_and_retires_descendant_group() {
        let fake = Fake::new(
            r#"printf '%s' "$$" > leader
sleep 30 &
child=$!
printf '%s' "$child" > descendant
trap 'kill "$child" 2>/dev/null; wait "$child"; exit 0' TERM
wait "$child""#,
        );
        let started = Instant::now();
        let error = fake
            .security
            .run(
                &fake.root,
                args(&["hang"]),
                Instant::now() + Duration::from_secs(3),
            )
            .await
            .unwrap_err();
        assert!(error.contains("deadline exceeded"), "{error}");
        assert!(started.elapsed() < Duration::from_secs(2));
        let pid: i32 = fs::read_to_string(fake.root.join("descendant"))
            .unwrap()
            .parse()
            .unwrap();
        assert_eq!(
            nix::sys::signal::kill(Pid::from_raw(pid), None),
            Err(Errno::ESRCH)
        );
        let leader: i32 = fs::read_to_string(fake.root.join("leader"))
            .unwrap()
            .parse()
            .unwrap();
        assert_eq!(killpg(Pid::from_raw(leader), None), Err(Errno::ESRCH));
    }
    #[tokio::test]
    async fn bounded_security_successful_leader_retires_remaining_descendant_before_success() {
        let fake = Fake::new(
            r#"printf '%s' "$$" > leader
sleep 30 &
printf '%s' "$!" > descendant
exit 0"#,
        );
        fake.security
            .run(
                &fake.root,
                args(&["early-exit"]),
                Instant::now() + Duration::from_secs(3),
            )
            .await
            .unwrap();
        let descendant: i32 = fs::read_to_string(fake.root.join("descendant"))
            .unwrap()
            .parse()
            .unwrap();
        let leader: i32 = fs::read_to_string(fake.root.join("leader"))
            .unwrap()
            .parse()
            .unwrap();
        assert_eq!(
            nix::sys::signal::kill(Pid::from_raw(descendant), None),
            Err(Errno::ESRCH)
        );
        assert_eq!(killpg(Pid::from_raw(leader), None), Err(Errno::ESRCH));
    }
    #[tokio::test]
    async fn bounded_security_drains_both_streams_and_caps_diagnostics() {
        let mut fake = Fake::new(
            "i=0; while [ $i -lt 6000 ]; do printf 'abcdefghij'; printf 'klmnopqrst' >&2; i=$((i+1)); done; exit 19",
        );
        fake.security.limits.command = Duration::from_secs(2);
        let error = fake
            .security
            .run(
                &fake.root,
                args(&["flood"]),
                Instant::now() + Duration::from_secs(3),
            )
            .await
            .unwrap_err();
        assert!(error.contains("output cap exceeded"), "{error}");
        assert!(error.contains("stdout[clipped]="));
        assert!(error.contains("stderr[clipped]="));
        assert!(error.len() < 2 * OUTPUT_CAP + 2048);
    }
    #[tokio::test]
    async fn install_interruption_preserves_ownership_and_stops_later_mutations() {
        for step in 1..=5 {
            let fake = Fake::new(&format!(
                r#"count=0
if [ -f count ]; then read -r count < count; fi
count=$((count+1))
printf '%s' "$count" > count
if [ "$count" = {step} ]; then printf '%s' "$count" > entered-step; exec sleep 30; fi
if [ "$1" = list-keychains ] && [ "$#" = 3 ]; then printf '    "/original/login.keychain-db"\n'; fi
exit 0"#
            ));
            let error = install_with(&fake.security, &fake.base, &fake.root)
                .await
                .unwrap_err();
            assert!(error.contains("deadline exceeded"), "{error}");
            // The intended failure step must actually enter, rather than an
            // earlier control being killed during process startup.
            assert_eq!(
                fs::read_to_string(fake.root.join("entered-step")).unwrap(),
                step.to_string()
            );
            assert_eq!(fake.calls().len(), step);
            assert_eq!(fake.root.join(MARKER).exists(), step > 1);
        }
    }
    #[tokio::test]
    async fn cleanup_continues_after_unknown_missing_item_and_timeout_errors() {
        for (failure, diagnostic) in [
            ("echo unknown >&2; exit 71", "unknown"),
            (
                "echo 'The specified item could not be found in the keychain' >&2; exit 44",
                "The specified item could not be found",
            ),
            ("exec sleep 30", "deadline exceeded"),
        ] {
            let fake = Fake::new(&format!(
                r#"if [ "$1" = remove-trusted-cert ]; then {failure}; fi
if [ "$1" = delete-keychain ]; then rm -- "$2"; fi
exit 0"#
            ));
            fake.owned();
            let error = remove_with(&fake.security, &fake.base, &fake.root)
                .await
                .unwrap_err();
            assert!(error.contains(diagnostic), "{error}");
            assert!(!error.contains("owned keychain still exists"), "{error}");
            assert_eq!(
                fake.calls(),
                ["remove-trusted-cert", "list-keychains", "delete-keychain"]
            );
            assert!(!keychain(&fake.root).exists());
            assert!(fake.root.join(MARKER).exists());
        }
    }
    #[tokio::test]
    async fn cleanup_success_requires_exact_store_absence_and_clears_ownership() {
        let fake = Fake::new(
            r#"if [ "$1" = delete-keychain ]; then rm -- "$2"; fi
exit 0"#,
        );
        fake.owned();
        remove_with(&fake.security, &fake.base, &fake.root)
            .await
            .unwrap();
        assert!(!fake.root.join(MARKER).exists());
        assert!(!keychain(&fake.root).exists());
        assert_eq!(
            fake.calls(),
            ["remove-trusted-cert", "list-keychains", "delete-keychain"]
        );
    }
    #[tokio::test]
    async fn cleanup_overall_deadline_bounds_later_stages_and_preserves_marker() {
        // Budget exhaustion before dispatch is a distinct valid outcome: no
        // child may start, but all three cleanup stages must retain failure.
        let expired = Fake::new("exit 0");
        expired.owned();
        let error = remove_before(
            &expired.security,
            &expired.base,
            &expired.root,
            Instant::now(),
        )
        .await
        .unwrap_err();
        assert_eq!(error.matches("overall budget exhausted").count(), 3);
        assert!(expired.calls().is_empty());
        assert!(expired.root.join(MARKER).exists());

        // Prove an entered command spends the shared budget and prevents later
        // mutations. The readiness handshake must arrive before acceptance.
        let mut fake = Fake::new("printf entered > entered-removal\nexec sleep 30");
        fake.security.limits.command = Duration::from_secs(5);
        fake.owned();
        let started = Instant::now();
        let ready = async {
            timeout(Duration::from_secs(3), async {
                while !fake.root.join("entered-removal").exists() {
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
            })
            .await
            .is_ok()
        };
        let (result, entered) = tokio::join!(
            remove_before(
                &fake.security,
                &fake.base,
                &fake.root,
                started + Duration::from_secs(2)
            ),
            ready,
        );
        let error = result.unwrap_err();
        assert!(
            entered,
            "shared-budget witness never entered its target command: {error}"
        );
        assert!(started.elapsed() < Duration::from_secs(4));
        assert!(error.contains("deadline exceeded"));
        assert_eq!(error.matches("overall budget exhausted").count(), 2);
        assert_eq!(fake.calls(), ["remove-trusted-cert"]);
        assert!(fake.root.join(MARKER).exists());
    }
    #[tokio::test]
    async fn ownership_rejection_never_invokes_a_command() {
        for corrupt in ["fingerprint", "keychain", "saved-list", "absent", "outside"] {
            let fake = Fake::new("exit 0");
            fake.owned();
            let path = fake.root.join(MARKER);
            let mut value: serde_json::Value =
                serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
            match corrupt {
                "fingerprint" => value["der_sha256"] = "bad".into(),
                "keychain" => value["keychain"] = "/unowned/store".into(),
                "saved-list" => value["original"] = serde_json::json!(["relative/path"]),
                "absent" => {
                    fs::remove_file(&path).unwrap();
                }
                "outside" => {}
                _ => unreachable!(),
            }
            if corrupt != "absent" {
                fs::write(path, serde_json::to_vec(&value).unwrap()).unwrap();
            }
            let base = if corrupt == "outside" {
                &fake.root
            } else {
                &fake.base
            };
            assert!(remove_with(&fake.security, base, &fake.root).await.is_err());
            assert!(fake.calls().is_empty());
            assert!(keychain(&fake.root).exists());
        }
    }
    #[tokio::test]
    async fn finalizer_continues_all_owned_roots_and_preserves_failed_marker() {
        let fake = Fake::new("exit 0");
        // Use an isolated registered base so concurrent controls are never adopted.
        let base = fake.root.join("isolated");
        fs::DirBuilder::new().mode(0o700).create(&base).unwrap();
        for name in [
            "platform-tls-store-good",
            "platform-tls-store-bad",
            "unrelated",
        ] {
            let root = base.join(name);
            fs::DirBuilder::new().mode(0o700).create(&root).unwrap();
            fs::write(root.join("ca.der"), b"fake").unwrap();
            fs::write(root.join("ca.pem"), b"fake").unwrap();
            fs::write(
                root.join(MARKER),
                serde_json::to_vec(&marker(&root, &[]).unwrap()).unwrap(),
            )
            .unwrap();
        }
        let executable = fake.root.join("finalizer-security");
        fs::write(
            &executable,
            r#"#!/bin/sh
printf '%s\n' "$1" >> calls
case "$PWD:$1" in *-bad:remove-trusted-cert) exit 72;; esac
exit 0
"#,
        )
        .unwrap();
        fs::set_permissions(&executable, fs::Permissions::from_mode(0o700)).unwrap();
        let security = Security {
            executable,
            limits: fake.security.limits,
        };
        assert!(finalize_with(&security, &base).await.is_err());
        assert!(!base.join("platform-tls-store-good").join(MARKER).exists());
        assert!(base.join("platform-tls-store-bad").join(MARKER).exists());
        assert!(!base.join("unrelated/calls").exists());
        assert_eq!(
            fs::read_to_string(base.join("platform-tls-store-bad/calls"))
                .unwrap()
                .lines()
                .count(),
            2
        );
    }
}
