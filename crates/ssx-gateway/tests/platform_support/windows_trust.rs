//! Windows-only test fixture owner. No certificate API or alternate store operation.
use sha2::{Digest, Sha256};
use ssx_windows_owner::{
    ChildPipes, TRUST_SCRIPT_SHA256, TrustFixtureChild, TrustFixtureOperation as Operation,
    TrustFixtureScenario,
};
use std::{
    io::{self, Write},
    os::windows::ffi::{OsStrExt, OsStringExt},
    path::{Path, PathBuf},
    time::Duration,
};
use tokio::{
    io::AsyncReadExt,
    time::{Instant, timeout_at},
};

const CAP: usize = 16 * 1024;
const CLEANUP: Duration = Duration::from_secs(5);
const REGISTRATION: &str = "windows-trust-case.json";
const PREFIX: &str = "platform-tls-store-";
const FINALIZER_BUDGET: Duration = Duration::from_secs(60);
const ORPHAN_SCAN_ENTRIES: usize = 1024;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Failure {
    Admission,
    State,
    Connect,
    Resume,
    Read,
    OutputLimit,
    Observe,
    Deadline,
    Exit,
    CleanupUnknown,
    Marker,
}
#[derive(Debug)]
struct Capture {
    prefix: Vec<u8>,
    total: u64,
}
impl Default for Capture {
    fn default() -> Self {
        Self {
            prefix: Vec::with_capacity(CAP),
            total: 0,
        }
    }
}
impl Capture {
    fn take(&mut self, bytes: &[u8]) -> Result<(), Failure> {
        self.total = self
            .total
            .checked_add(bytes.len() as u64)
            .ok_or(Failure::OutputLimit)?;
        let retained = bytes.len().min(CAP.saturating_sub(self.prefix.len()));
        self.prefix.extend_from_slice(&bytes[..retained]);
        if retained != bytes.len() {
            Err(Failure::OutputLimit)
        } else {
            Ok(())
        }
    }
}
struct Report {
    failure: Option<Failure>,
    retired: bool,
    exit: Option<u32>,
    leader_before_deadline: bool,
    stdout: Capture,
    stderr: Capture,
    readers_finished: bool,
    descendant: Option<ssx_windows_owner::FixtureProcessWitness>,
    image: Option<ssx_windows_owner::FileDigest>,
}
impl Report {
    fn not_spawned(failure: Failure) -> Self {
        Self {
            failure: Some(failure),
            retired: true,
            exit: None,
            leader_before_deadline: false,
            stdout: Capture::default(),
            stderr: Capture::default(),
            readers_finished: true,
            descendant: None,
            image: None,
        }
    }
}
async fn supervise(
    mut child: TrustFixtureChild,
    mut pipes: ChildPipes,
    operation: Duration,
    cleanup: Duration,
    witness: bool,
) -> Report {
    let deadline = Instant::now() + operation;
    let image = child.image_digest();
    let mut report = Report {
        failure: None,
        retired: false,
        exit: None,
        leader_before_deadline: false,
        stdout: Capture::default(),
        stderr: Capture::default(),
        readers_finished: false,
        descendant: None,
        image: Some(image),
    };
    match timeout_at(deadline, pipes.connected()).await {
        Ok(Ok(())) => {}
        _ => report.failure = Some(Failure::Connect),
    }
    drop(pipes.stdin.take());
    if report.failure.is_none() && child.resume().is_err() {
        report.failure = Some(Failure::Resume);
    }
    let mut end = deadline;
    let mut cancelling = false;
    let mut output_done = false;
    let mut error_done = false;
    let mut obuf = [0; 1024];
    let mut ebuf = [0; 1024];
    loop {
        if Instant::now() >= end {
            if cancelling {
                let _ = pipes.cancel_io();
                report.readers_finished = output_done && error_done;
                report.failure.get_or_insert(Failure::CleanupUnknown);
                break;
            }
            report.failure.get_or_insert(Failure::Deadline);
        }
        if report.failure.is_some() && !cancelling {
            cancelling = true;
            end = Instant::now() + cleanup;
            // A cancellation request alone never grants a retirement record.
            let _ = child.cancellation().cancel();
        }
        match child.observe() {
            Ok(state) => {
                if let Some(exit) = state.leader_exit {
                    report.exit = Some(exit.code);
                    report.leader_before_deadline |= Instant::now() < deadline;
                }
                if output_done && error_done && state.leader_exit.is_some() && state.job_empty {
                    if child.finish().is_ok() {
                        report.retired = true;
                        report.readers_finished = true;
                        break;
                    }
                    report.failure.get_or_insert(Failure::Observe);
                }
            }
            Err(_) => {
                report.failure.get_or_insert(Failure::Observe);
            }
        }
        if witness && report.descendant.is_none() {
            if let Ok(text) = std::str::from_utf8(&report.stdout.prefix) {
                if let Some(line) = text
                    .rsplit_once('\n')
                    .map(|(complete, _)| complete)
                    .unwrap_or("")
                    .lines()
                    .find(|line| line.starts_with("DESCENDANT "))
                {
                    let fields: Vec<_> = line.split_whitespace().collect();
                    if fields.len() == 3 {
                        let identity = fields[1]
                            .parse::<u32>()
                            .ok()
                            .zip(fields[2].parse::<u64>().ok());
                        if let Some((pid, created)) = identity {
                            match ssx_windows_owner::FixtureProcessWitness::capture(pid, created) {
                                Ok(handle) => report.descendant = Some(handle),
                                Err(_) => {
                                    report.failure.get_or_insert(Failure::Observe);
                                }
                            }
                        }
                    }
                }
            }
        }
        if Instant::now() >= end {
            if !cancelling {
                report.failure.get_or_insert(Failure::Deadline);
                continue;
            }
            let _ = pipes.cancel_io();
            report.readers_finished = output_done && error_done;
            report.failure.get_or_insert(Failure::CleanupUnknown);
            break;
        }
        tokio::select! {
            result=pipes.stdout.read(&mut obuf), if !output_done=>{
                match result { Ok(0)=>output_done=true, Ok(n)=>{ if let Err(e)=report.stdout.take(&obuf[..n]) { report.failure.get_or_insert(e); } },Err(_)=>{output_done=true;report.failure.get_or_insert(Failure::Read);} }
            }
            result=pipes.stderr.read(&mut ebuf), if !error_done=>{
                match result { Ok(0)=>error_done=true, Ok(n)=>{if let Err(e)=report.stderr.take(&ebuf[..n]) {report.failure.get_or_insert(e);} },Err(_)=>{error_done=true;report.failure.get_or_insert(Failure::Read);} }
            }
            _=tokio::time::sleep_until(end)=>{}
            _=tokio::time::sleep(Duration::from_millis(2))=>{}
        }
    }
    if report.failure.is_none() && report.exit != Some(0) {
        report.failure = Some(Failure::Exit);
    }
    // No reader task is spawned. All read futures are scoped to select! and have
    // completed/dropped before pipe handles; ProcessCore Drop requests cancellation.
    report
}
fn workspace() -> Result<PathBuf, Failure> {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .ok_or(Failure::Admission)
}
fn base() -> Result<PathBuf, Failure> {
    std::env::var_os("SLOPSEARX_INSTALL_TEST_ROOT")
        .map(PathBuf::from)
        .ok_or(Failure::Admission)
}
fn gated() -> Result<(), Failure> {
    if std::env::var("GITHUB_ACTIONS").as_deref() == Ok("true")
        && std::env::var("SLOPSEARX_PLATFORM_TRUST_FIXTURE").as_deref() == Ok("1")
    {
        Ok(())
    } else {
        Err(Failure::Admission)
    }
}
fn ca_digest(root: &Path) -> Result<String, Failure> {
    let raw = ssx_windows_owner::read_package_file(&root.join("ca.der"), 64 * 1024)
        .map_err(|_| Failure::Admission)?;
    if raw.is_empty() {
        return Err(Failure::Admission);
    }
    Ok(Sha256::digest(raw)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect())
}
fn admit_case(root: &Path) -> Result<String, Failure> {
    ssx_windows_owner::check_private_directory(root).map_err(|_| Failure::Admission)?;
    if root
        .parent()
        .ok_or(Failure::Admission)?
        .canonicalize()
        .map_err(|_| Failure::Admission)?
        != base()?.canonicalize().map_err(|_| Failure::Admission)?
    {
        return Err(Failure::Admission);
    }
    let name = root.file_name().ok_or(Failure::Admission)?;
    if !name
        .encode_wide()
        .take(PREFIX.len())
        .eq(PREFIX.encode_utf16())
    {
        return Err(Failure::Admission);
    }
    ca_digest(root)
}
fn write_new(path: &Path, raw: &[u8]) -> Result<(), Failure> {
    let mut file = ssx_windows_owner::create_private_file(path).map_err(|_| Failure::State)?;
    file.write_all(raw)
        .and_then(|()| file.sync_all())
        .map_err(|_| Failure::State)
}
fn record(ca: &str, slot: usize) -> Vec<u8> {
    format!("ssx-windows-trust-owner-v1\n{ca}\n{TRUST_SCRIPT_SHA256}\n{slot}\n").into_bytes()
}
fn read_state(path: &Path) -> Result<Option<Vec<u8>>, Failure> {
    match ssx_windows_owner::read_confidential_file(path, 2048).map_err(|_| Failure::State)? {
        ssx_windows_owner::ConfidentialRead::Missing => Ok(None),
        ssx_windows_owner::ConfidentialRead::Bytes(b) => Ok(Some(b)),
        ssx_windows_owner::ConfidentialRead::TooLarge => Err(Failure::State),
    }
}
fn state_path(root: &Path, slot: usize, retired: bool) -> PathBuf {
    root.join(format!(
        "trust-owner-{slot}.{}",
        if retired { "retired" } else { "pending" }
    ))
}
fn quiescent(root: &Path, ca: &str) -> Result<(), Failure> {
    for slot in 0..4 {
        let pending = read_state(&state_path(root, slot, false))?;
        let retired = read_state(&state_path(root, slot, true))?;
        match (pending, retired) {
            (None, None) => {}
            (Some(a), Some(b)) if a == record(ca, slot) && b == a => {}
            _ => return Err(Failure::CleanupUnknown),
        }
    }
    Ok(())
}
fn registered(root: &Path, ca: &str, pointer: &Path) -> Result<(), Failure> {
    let raw = ssx_windows_owner::read_package_file(pointer, 2048).map_err(|_| Failure::State)?;
    let value: serde_json::Value = serde_json::from_slice(&raw).map_err(|_| Failure::State)?;
    let object = value.as_object().ok_or(Failure::State)?;
    if serde_json::to_vec(&value).map_err(|_| Failure::State)? != raw {
        return Err(Failure::State);
    }
    if object.len() != 3
        || value["schema"] != "ssx-windows-trust-case-v1"
        || value["case_utf16"]
            != serde_json::json!(
                root.file_name()
                    .ok_or(Failure::State)?
                    .encode_wide()
                    .collect::<Vec<_>>()
            )
        || value["ca_sha256"] != ca
    {
        return Err(Failure::State);
    }
    Ok(())
}
#[derive(Clone, Copy)]
enum Payload {
    Real,
    #[cfg(test)]
    Fake(TrustFixtureScenario),
}
impl Payload {
    fn create(
        self,
        root: &Path,
        op: Operation,
        ca: &str,
    ) -> io::Result<(TrustFixtureChild, ChildPipes)> {
        let mut ca_bytes = [0; 32];
        if ca.len() != 64
            || !ca
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(io::ErrorKind::InvalidInput.into());
        }
        for (i, b) in ca_bytes.iter_mut().enumerate() {
            *b = u8::from_str_radix(&ca[i * 2..i * 2 + 2], 16)
                .map_err(|_| io::ErrorKind::InvalidInput)?;
        }
        match self {
            Self::Real => TrustFixtureChild::create(
                &workspace().map_err(|_| io::ErrorKind::InvalidInput)?,
                root,
                op,
                ca_bytes,
            ),
            #[cfg(test)]
            Self::Fake(scenario) => {
                let probe = PathBuf::from(env!("CARGO_BIN_EXE_ssx-gateway-trust-probe"));
                let hash = ssx_windows_owner::build_file_digest(&probe)?.sha256;
                TrustFixtureChild::fake(&probe, hash, root, op, scenario)
            }
        }
    }
}
async fn attempt_owned(
    root: &Path,
    op: Operation,
    slot: usize,
    ca: &str,
    pointer: &Path,
    payload: Payload,
) -> Result<(), Failure> {
    quiescent(root, ca)?;
    registered(root, ca, pointer)?;
    let raw = record(ca, slot);
    write_new(&state_path(root, slot, false), &raw)?;
    let report = match payload.create(root, op, ca) {
        Ok((child, pipes)) => supervise(child, pipes, super::BUDGET, CLEANUP, false).await,
        Err(_) => Report::not_spawned(Failure::Admission),
    };
    eprintln!(
        "SSX_TRUST_OWNER operation={} primary={:?} exit={:?} retired={} readers={} stdout_bytes={} stderr_bytes={} clipped={}",
        op.argument(),
        report.failure,
        report.exit,
        report.retired,
        report.readers_finished,
        report.stdout.total,
        report.stderr.total,
        report.stdout.total > CAP as u64 || report.stderr.total > CAP as u64
    );
    if let Some(image) = report.image {
        let digest = image
            .sha256
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        eprintln!("SSX_TRUST_IMAGE sha256={digest} bytes={}", image.bytes);
    }
    // Fixed-script output contains only this public fixture/OS diagnostics. Keep
    // bounded exact prefixes in private files, not arbitrary console Display.
    write_new(
        &root.join(format!("trust-owner-{slot}.stdout")),
        &report.stdout.prefix,
    )?;
    write_new(
        &root.join(format!("trust-owner-{slot}.stderr")),
        &report.stderr.prefix,
    )?;
    if !report.retired || !report.readers_finished {
        return Err(Failure::CleanupUnknown);
    }
    if report.retired && report.readers_finished {
        write_new(&state_path(root, slot, true), &raw)?;
    }
    if let Some(failure) = report.failure {
        return Err(failure);
    }
    Ok(())
}
async fn remove_owned(
    root: &Path,
    ca: &str,
    pointer: &Path,
    payload: Payload,
) -> Result<(), Failure> {
    registered(root, ca, pointer)?;
    quiescent(root, ca)?;
    if !root
        .join("trust-active")
        .try_exists()
        .map_err(|_| Failure::Marker)?
    {
        return Ok(());
    }
    if read_state(&state_path(root, 0, false))? != Some(record(ca, 0)) {
        return Err(Failure::CleanupUnknown);
    }
    let mut slot = None;
    for candidate in 1..4 {
        if read_state(&state_path(root, candidate, false))?.is_none() {
            slot = Some(candidate);
            break;
        }
    }
    attempt_owned(
        root,
        Operation::Remove,
        slot.ok_or(Failure::State)?,
        ca,
        pointer,
        payload,
    )
    .await?;
    if root
        .join("trust-active")
        .try_exists()
        .map_err(|_| Failure::Marker)?
    {
        return Err(Failure::Marker);
    }
    Ok(())
}
pub(super) async fn install(root: &Path) -> Result<(), Failure> {
    gated()?;
    let ca = admit_case(root)?;
    if root
        .join("trust-active")
        .try_exists()
        .map_err(|_| Failure::Marker)?
    {
        return Err(Failure::Marker);
    }
    let value = serde_json::json!({"schema":"ssx-windows-trust-case-v1","case_utf16":root.file_name().ok_or(Failure::State)?.encode_wide().collect::<Vec<_>>(),"ca_sha256":ca});
    write_new(
        &base()?.join(REGISTRATION),
        &serde_json::to_vec(&value).map_err(|_| Failure::State)?,
    )?;
    attempt_owned(
        root,
        Operation::Install,
        0,
        &ca,
        &base()?.join(REGISTRATION),
        Payload::Real,
    )
    .await
}
pub(super) async fn remove(root: &Path) -> Result<(), Failure> {
    gated()?;
    let ca = admit_case(root)?;
    remove_owned(root, &ca, &base()?.join(REGISTRATION), Payload::Real).await
}
pub(super) async fn finalize() -> Result<(), Failure> {
    let deadline = Instant::now() + FINALIZER_BUDGET;
    gated()?;
    let base = base()?;
    let pointer = base.join(REGISTRATION);
    if Instant::now() >= deadline {
        return Err(Failure::CleanupUnknown);
    }
    if !pointer.try_exists().map_err(|_| Failure::State)? {
        // No mutation before registration; refuse an orphan marker instead of
        // adopting a prior unregistered or unknown source operation.
        let mut entries = std::fs::read_dir(base).map_err(|_| Failure::State)?;
        // This scan shares the entry deadline and has a finite observation
        // budget. Hitting the cap cannot certify absence, even with no marker.
        for _ in 0..ORPHAN_SCAN_ENTRIES {
            if Instant::now() >= deadline {
                return Err(Failure::CleanupUnknown);
            }
            let Some(item) = entries.next() else {
                return if Instant::now() < deadline {
                    Ok(())
                } else {
                    Err(Failure::CleanupUnknown)
                };
            };
            let item = item.map_err(|_| Failure::State)?;
            if item
                .file_name()
                .encode_wide()
                .take(PREFIX.len())
                .eq(PREFIX.encode_utf16())
                && item
                    .path()
                    .join("trust-active")
                    .try_exists()
                    .map_err(|_| Failure::State)?
            {
                return Err(Failure::CleanupUnknown);
            }
        }
        return Err(Failure::CleanupUnknown);
    }
    let raw = ssx_windows_owner::read_package_file(&pointer, 2048).map_err(|_| Failure::State)?;
    let value: serde_json::Value = serde_json::from_slice(&raw).map_err(|_| Failure::State)?;
    let units = value["case_utf16"].as_array().ok_or(Failure::State)?;
    if units.is_empty() || units.len() > 255 {
        return Err(Failure::State);
    }
    let units = units
        .iter()
        .map(|v| {
            v.as_u64()
                .and_then(|n| u16::try_from(n).ok())
                .filter(|n| *n != 0)
                .ok_or(Failure::State)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let name = std::ffi::OsString::from_wide(&units);
    if !units
        .iter()
        .copied()
        .take(PREFIX.len())
        .eq(PREFIX.encode_utf16())
        || Path::new(&name).components().count() != 1
    {
        return Err(Failure::State);
    }
    if Instant::now() >= deadline {
        return Err(Failure::CleanupUnknown);
    }
    let result = timeout_at(deadline, remove(&base.join(name)))
        .await
        .map_err(|_| Failure::CleanupUnknown)?;
    // Timeout cannot preempt synchronous admission or a marker-absent return.
    // A completed poll after the shared deadline cannot certify cleanup.
    if Instant::now() >= deadline {
        Err(Failure::CleanupUnknown)
    } else {
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn probe() -> PathBuf {
        PathBuf::from(env!("CARGO_BIN_EXE_ssx-gateway-trust-probe"))
    }
    fn fake(
        root: &Path,
        scenario: TrustFixtureScenario,
    ) -> Result<(TrustFixtureChild, ChildPipes), io::Error> {
        let p = probe();
        let digest = ssx_windows_owner::build_file_digest(&p)?;
        TrustFixtureChild::fake(&p, digest.sha256, root, Operation::Install, scenario)
    }
    fn owned(label: &str) -> PathBuf {
        super::super::root(&format!("trust-owner-{label}"))
    }
    fn limits() -> (Duration, Duration) {
        (Duration::from_secs(5), Duration::from_secs(5))
    }
    #[tokio::test(flavor = "current_thread")]
    async fn windows_trust_owner_closes_normal_fake_and_preserves_utf16_arguments() {
        let root = owned("normal with space");
        let (child, pipes) = fake(&root, TrustFixtureScenario::Success).unwrap();
        let (op, cleanup) = limits();
        let report = supervise(child, pipes, op, cleanup, false).await;
        assert_eq!(report.failure, None);
        assert_eq!(report.exit, Some(0));
        assert!(report.retired && report.readers_finished);
        assert!(report.stdout.prefix.starts_with(b"READY\n"));
        let units = [b'p' as u16, 0xd800, b' ' as u16, 0xdc00];
        let raw = root.join(std::ffi::OsString::from_wide(&units));
        ssx_windows_owner::create_private_directory(&raw).unwrap();
        let (child, pipes) = fake(&raw, TrustFixtureScenario::Success).unwrap();
        let report = supervise(child, pipes, op, cleanup, false).await;
        assert_eq!(report.failure, None);
        assert!(raw.join("entered-install").is_file());
        std::fs::remove_dir_all(&raw).unwrap();
        let mut trailing = owned("trailing separator");
        trailing.push("");
        let (child, pipes) = fake(&trailing, TrustFixtureScenario::Success).unwrap();
        let report = supervise(child, pipes, op, cleanup, false).await;
        assert_eq!(report.failure, None);
        assert!(trailing.join("entered-install").is_file());
    }
    #[tokio::test(flavor = "current_thread")]
    async fn windows_trust_owner_deadline_retires_original_quiet_child() {
        let root = owned("quiet");
        let (child, pipes) = fake(&root, TrustFixtureScenario::Quiet).unwrap();
        let now = Instant::now();
        let (op, cleanup) = limits();
        let report = supervise(child, pipes, op, cleanup, false).await;
        assert_eq!(report.failure, Some(Failure::Deadline));
        assert!(report.retired && report.readers_finished);
        assert!(!report.leader_before_deadline);
        assert!(root.join("entered-install").is_file());
        assert!(now.elapsed() < op + cleanup + Duration::from_secs(1));
    }
    #[tokio::test(flavor = "current_thread")]
    async fn windows_trust_owner_leader_exit_cannot_hide_live_descendant() {
        let root = owned("descendant");
        let (child, pipes) = fake(&root, TrustFixtureScenario::Descendant).unwrap();
        let (op, cleanup) = limits();
        let report = supervise(child, pipes, op, cleanup, true).await;
        assert_eq!(report.failure, Some(Failure::Deadline));
        assert_eq!(report.exit, Some(0));
        assert!(report.leader_before_deadline && report.retired && report.readers_finished);
        assert!(root.join("descendant-ready").is_file());
        let witness = report
            .descendant
            .expect("retained exact descendant process while READY");
        witness.exited(Duration::from_secs(1)).await.unwrap();
        assert!(!witness.running().unwrap());
    }
    #[tokio::test(flavor = "current_thread")]
    async fn windows_trust_owner_bounds_both_newline_free_output_streams() {
        for scenario in [
            TrustFixtureScenario::Flood,
            TrustFixtureScenario::FloodStdout,
            TrustFixtureScenario::FloodStderr,
        ] {
            let root = owned(scenario.argument());
            let (child, pipes) = fake(&root, scenario).unwrap();
            let (op, cleanup) = limits();
            let report = supervise(child, pipes, op, cleanup, false).await;
            assert_eq!(report.failure, Some(Failure::OutputLimit));
            assert!(report.retired && report.readers_finished);
            assert!(report.stdout.prefix.len() <= CAP && report.stderr.prefix.len() <= CAP);
            if scenario == TrustFixtureScenario::FloodStdout {
                assert!(report.stdout.total > CAP as u64);
            } else if scenario == TrustFixtureScenario::FloodStderr {
                assert!(report.stderr.total > CAP as u64);
            } else {
                assert!(report.stdout.total > 0 && report.stderr.total > 0);
            }
        }
    }
    #[tokio::test(flavor = "current_thread")]
    async fn windows_trust_owner_job_and_resume_faults_never_enter_body() {
        for scenario in [
            TrustFixtureScenario::RejectJob,
            TrustFixtureScenario::RejectResume,
        ] {
            let root = owned(scenario.argument());
            match fake(&root, scenario) {
                Err(_) => assert_eq!(scenario, TrustFixtureScenario::RejectJob),
                Ok((child, pipes)) => {
                    let (op, cleanup) = limits();
                    let report = supervise(child, pipes, op, cleanup, false).await;
                    assert_eq!(report.failure, Some(Failure::Resume));
                    assert!(report.retired);
                }
            }
            assert!(!root.join("entered-install").exists());
        }
    }
    #[test]
    fn windows_trust_owner_pending_state_never_grants_cleanup() {
        let root = owned("pending");
        let ca = "a".repeat(64);
        let raw = record(&ca, 0);
        write_new(&state_path(&root, 0, false), &raw).unwrap();
        assert_eq!(quiescent(&root, &ca), Err(Failure::CleanupUnknown));
        write_new(&state_path(&root, 0, true), &raw).unwrap();
        assert_eq!(quiescent(&root, &ca), Ok(()));
        assert_eq!(
            quiescent(&root, &"b".repeat(64)),
            Err(Failure::CleanupUnknown)
        );
        assert!(write_new(&state_path(&root, 0, false), &raw).is_err());
    }
    fn fake_registration(root: &Path, ca: &str) -> PathBuf {
        let pointer = root.join("fake-registration.json");
        let value = serde_json::json!({"schema":"ssx-windows-trust-case-v1","case_utf16":root.file_name().unwrap().encode_wide().collect::<Vec<_>>(),"ca_sha256":ca});
        write_new(&pointer, &serde_json::to_vec(&value).unwrap()).unwrap();
        pointer
    }
    #[tokio::test(flavor = "current_thread")]
    async fn windows_trust_owner_drop_leaves_pending_and_blocks_fake_remove() {
        let root = owned("drop-pending");
        let ca = "a".repeat(64);
        let pointer = fake_registration(&root, &ca);
        write_new(&state_path(&root, 0, false), &record(&ca, 0)).unwrap();
        let (child, pipes) = fake(&root, TrustFixtureScenario::Quiet).unwrap();
        let observer = child.observer();
        let task = tokio::spawn(supervise(
            child,
            pipes,
            Duration::from_secs(15),
            CLEANUP,
            false,
        ));
        timeout_at(Instant::now() + Duration::from_secs(5), async {
            while !root.join("entered-install").is_file() {
                tokio::time::sleep(Duration::from_millis(2)).await;
            }
        })
        .await
        .unwrap();
        task.abort();
        assert!(task.await.is_err());
        timeout_at(Instant::now() + Duration::from_secs(5), observer.exited())
            .await
            .unwrap()
            .unwrap();
        assert!(root.join("trust-active").is_file());
        assert_eq!(
            remove_owned(
                &root,
                &ca,
                &pointer,
                Payload::Fake(TrustFixtureScenario::Success)
            )
            .await,
            Err(Failure::CleanupUnknown)
        );
        assert!(!root.join("entered-remove").exists());
        assert!(!state_path(&root, 0, true).exists());
    }
    #[tokio::test(flavor = "current_thread")]
    async fn windows_trust_owner_failed_remove_keeps_marker_and_exact_retired_retry_succeeds() {
        let root = owned("remove-failure");
        let ca = "a".repeat(64);
        let pointer = fake_registration(&root, &ca);
        attempt_owned(
            &root,
            Operation::Install,
            0,
            &ca,
            &pointer,
            Payload::Fake(TrustFixtureScenario::Success),
        )
        .await
        .unwrap();
        assert_eq!(
            remove_owned(
                &root,
                &ca,
                &pointer,
                Payload::Fake(TrustFixtureScenario::Fail)
            )
            .await,
            Err(Failure::Exit)
        );
        assert!(root.join("trust-active").is_file());
        assert!(state_path(&root, 1, true).is_file());
        assert_eq!(
            remove_owned(
                &root,
                &ca,
                &pointer,
                Payload::Fake(TrustFixtureScenario::Quiet)
            )
            .await,
            Err(Failure::Deadline)
        );
        assert!(root.join("trust-active").is_file());
        assert!(state_path(&root, 2, true).is_file());
        remove_owned(
            &root,
            &ca,
            &pointer,
            Payload::Fake(TrustFixtureScenario::Success),
        )
        .await
        .unwrap();
        assert!(!root.join("trust-active").exists());
        assert!(state_path(&root, 3, true).is_file());
    }
    #[test]
    fn windows_trust_owner_script_ca_and_case_admission_fail_before_creation() {
        let root = owned("admission");
        let base = root.join("base");
        let case = base.join("platform-tls-store-owned");
        ssx_windows_owner::create_private_directory(&base).unwrap();
        ssx_windows_owner::create_private_directory(&case).unwrap();
        let script = root.join("crates/ssx-gateway/tests/platform_support/trust-store.ps1");
        std::fs::create_dir_all(script.parent().unwrap()).unwrap();
        let original = include_bytes!("trust-store.ps1");
        let script_hash = Sha256::digest(original)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        let crlf_pairs = original.windows(2).filter(|pair| *pair == b"\r\n").count();
        eprintln!(
            "SSX_TRUST_SCRIPT bytes={} sha256={script_hash} crlf_pairs={crlf_pairs}",
            original.len()
        );
        assert_eq!(original.len(), 1904, "fixed trust script byte count");
        assert_eq!(
            script_hash, TRUST_SCRIPT_SHA256,
            "fixed trust script digest"
        );
        assert_eq!(crlf_pairs, 0, "fixed trust script must retain LF bytes");
        std::fs::write(&script, original).unwrap();
        std::fs::write(
            case.join("ca.der"),
            b"public fake CA bytes, never parsed/imported",
        )
        .unwrap();
        let hash: [u8; 32] = Sha256::digest(b"public fake CA bytes, never parsed/imported").into();
        TrustFixtureChild::fixture_check_trust_inputs(&root, &base, &case, hash).unwrap();
        assert!(
            TrustFixtureChild::fixture_check_trust_inputs(&root, &base, &case, [0; 32]).is_err()
        );
        std::fs::write(&script, b"tampered source").unwrap();
        assert!(TrustFixtureChild::fixture_check_trust_inputs(&root, &base, &case, hash).is_err());
        std::fs::write(&script, original).unwrap();
        let sibling = root.join("platform-tls-store-foreign");
        ssx_windows_owner::create_private_directory(&sibling).unwrap();
        std::fs::write(
            sibling.join("ca.der"),
            b"public fake CA bytes, never parsed/imported",
        )
        .unwrap();
        assert!(
            TrustFixtureChild::fixture_check_trust_inputs(&root, &base, &sibling, hash).is_err()
        );
        assert!(!case.join("entered-install").exists());
        assert!(!case.join("trust-active").exists());
    }
}
