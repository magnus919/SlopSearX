//! Closed test-only launch roles; never an installed-package or arbitrary-command API.
use crate::{ChildPipes, ExitHandle, JobCancel, Retirement, process::ProcessCore, trust};
use sha2::{Digest, Sha256};
use std::{
    ffi::{OsStr, OsString},
    fs::{File, OpenOptions},
    io::{self, Read},
    os::windows::{
        ffi::{OsStrExt, OsStringExt},
        fs::OpenOptionsExt,
    },
    path::{Path, PathBuf},
    time::Duration,
};
use windows_sys::Win32::{Storage::FileSystem::*, System::SystemInformation::GetSystemDirectoryW};

pub const TRUST_SCRIPT_SHA256: &str =
    "ec7ba51fef8c033441661de38383f2ac2dfe7c5497fbb4c09eb2e307b03ff1b3";
const SCRIPT: &str = "crates/ssx-gateway/tests/platform_support/trust-store.ps1";
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TrustFixtureOperation {
    Install,
    Remove,
}
impl TrustFixtureOperation {
    pub fn argument(self) -> &'static str {
        match self {
            Self::Install => "install",
            Self::Remove => "remove",
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TrustFixtureScenario {
    Success,
    Quiet,
    Descendant,
    Hold,
    Flood,
    FloodStdout,
    FloodStderr,
    Fail,
    RejectJob,
    RejectResume,
}
impl TrustFixtureScenario {
    pub fn argument(self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::Quiet => "quiet",
            Self::Descendant => "descendant",
            Self::Hold => "hold",
            Self::Flood => "flood",
            Self::FloodStdout => "flood-stdout",
            Self::FloodStderr => "flood-stderr",
            Self::Fail => "fail",
            Self::RejectJob => "reject-job",
            Self::RejectResume => "reject-resume",
        }
    }
}
#[derive(Debug)]
pub struct TrustFixtureObservation {
    pub leader_exit: Option<crate::ExitObservation>,
    pub job_empty: bool,
}

struct HeldFile {
    _file: File,
    _parents: Vec<File>,
    digest: [u8; 32],
    bytes: u64,
}
// Source content has a compiled digest rather than the installed-package owner policy.
// Retain no-delete parent fences and deny file writers/deletion through child retirement.
// System PowerShell is a fixed OS input, never an environment-selected image.
fn hold(path: &Path, expected: Option<[u8; 32]>, limit: u64) -> io::Result<HeldFile> {
    if !path.is_absolute() {
        return Err(io::ErrorKind::InvalidInput.into());
    }
    let mut parents = Vec::new();
    for parent in path
        .parent()
        .ok_or(io::ErrorKind::InvalidInput)?
        .ancestors()
    {
        let file = OpenOptions::new()
            .read(true)
            .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
            .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_BACKUP_SEMANTICS)
            .open(parent)?;
        trust::identity(&file, true)?;
        parents.push(file);
    }
    // Reopen each name after all no-delete fences are held. A concurrent
    // ancestor rename during admission cannot leave a fence on an old object.
    for (name, held) in path
        .parent()
        .ok_or(io::ErrorKind::InvalidInput)?
        .ancestors()
        .zip(&parents)
    {
        let named = OpenOptions::new()
            .read(true)
            .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
            .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_BACKUP_SEMANTICS)
            .open(name)?;
        if trust::identity(&named, true)? != trust::identity(held, true)? {
            return Err(io::ErrorKind::InvalidData.into());
        }
    }
    let mut file = OpenOptions::new()
        .read(true)
        .share_mode(FILE_SHARE_READ)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
        .open(path)?;
    let before = trust::fixture_system_identity(&file)?;
    let mut hash = Sha256::new();
    let mut buffer = [0; 4096];
    let mut bytes = 0u64;
    loop {
        let n = file.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        bytes = bytes
            .checked_add(n as u64)
            .ok_or(io::ErrorKind::InvalidData)?;
        if bytes > limit {
            return Err(io::ErrorKind::InvalidData.into());
        }
        hash.update(&buffer[..n]);
    }
    let digest: [u8; 32] = hash.finalize().into();
    if bytes == 0
        || expected.is_some_and(|value| value != digest)
        || trust::fixture_system_identity(&file)? != before
    {
        return Err(io::ErrorKind::InvalidData.into());
    }
    Ok(HeldFile {
        _file: file,
        _parents: parents,
        digest,
        bytes,
    })
}
fn script_digest() -> [u8; 32] {
    let mut output = [0; 32];
    for (i, b) in output.iter_mut().enumerate() {
        *b = u8::from_str_radix(&TRUST_SCRIPT_SHA256[i * 2..i * 2 + 2], 16)
            .expect("compiled script digest");
    }
    output
}
fn system_directory() -> io::Result<PathBuf> {
    let mut wide = vec![0; 32768];
    // SAFETY: initialized writable UTF16 buffer, exact capacity and checked length.
    let count = unsafe { GetSystemDirectoryW(wide.as_mut_ptr(), wide.len() as u32) } as usize;
    if count == 0 || count >= wide.len() {
        return Err(io::Error::last_os_error());
    }
    wide.truncate(count);
    Ok(PathBuf::from(OsString::from_wide(&wide)))
}
// Implements Microsoft's documented C/Windows argv backslash/quote rules for a
// finite argument vector; never command text or cmd.exe/PowerShell expressions.
// https://learn.microsoft.com/en-us/cpp/c-language/parsing-c-command-line-arguments
fn command_line(application: &Path, arguments: &[OsString]) -> io::Result<Vec<u16>> {
    let image: Vec<u16> = application.as_os_str().encode_wide().collect();
    if image.contains(&0) || image.contains(&(b'"' as u16)) {
        return Err(io::ErrorKind::InvalidInput.into());
    }
    let mut result = vec![b'"' as u16];
    result.extend(image);
    result.push(b'"' as u16);
    for argument in arguments {
        let units: Vec<u16> = argument.encode_wide().collect();
        if units.contains(&0) {
            return Err(io::ErrorKind::InvalidInput.into());
        }
        result.extend([b' ' as u16, b'"' as u16]);
        let mut slashes = 0;
        for unit in units {
            if unit == b'\\' as u16 {
                slashes += 1;
                continue;
            }
            if unit == b'"' as u16 {
                result.extend(std::iter::repeat_n(b'\\' as u16, slashes * 2 + 1));
            } else {
                result.extend(std::iter::repeat_n(b'\\' as u16, slashes));
            }
            slashes = 0;
            result.push(unit);
        }
        result.extend(std::iter::repeat_n(b'\\' as u16, slashes * 2));
        result.push(b'"' as u16);
    }
    result.push(0);
    if result.len() > 32767 {
        return Err(io::ErrorKind::InvalidInput.into());
    }
    Ok(result)
}
fn environment(system: &Path, root: &Path, real: bool) -> io::Result<Vec<u16>> {
    let windows = system.parent().ok_or(io::ErrorKind::InvalidData)?;
    let mut values = vec![
        ("SystemRoot", windows.as_os_str().to_owned()),
        ("WINDIR", windows.as_os_str().to_owned()),
        ("TEMP", root.as_os_str().to_owned()),
        ("TMP", root.as_os_str().to_owned()),
    ];
    if real {
        values.extend([
            ("GITHUB_ACTIONS", OsString::from("true")),
            ("SLOPSEARX_PLATFORM_TRUST_FIXTURE", OsString::from("1")),
        ]);
    }
    values.sort_by_key(|(name, _)| name.to_ascii_uppercase());
    let mut result = Vec::new();
    for (name, value) in values {
        let mut row = OsString::from(name);
        row.push("=");
        row.push(value);
        let wide = trust::wide(&row)?;
        result.extend(wide);
    }
    result.push(0);
    Ok(result)
}
fn admitted_case(root: &Path, base: &Path) -> io::Result<File> {
    let _base = trust::opened_confidential_parent(base)?;
    if root
        .parent()
        .ok_or(io::ErrorKind::InvalidInput)?
        .canonicalize()?
        != base.canonicalize()?
        || !root.file_name().is_some_and(|name| {
            name.encode_wide()
                .take("platform-tls-store-".len())
                .eq("platform-tls-store-".encode_utf16())
        })
    {
        return Err(io::ErrorKind::InvalidInput.into());
    }
    trust::opened_confidential_parent(root)
}
struct AdmittedTrust {
    case: File,
    system: PathBuf,
    application: PathBuf,
    script: PathBuf,
    held: Vec<HeldFile>,
}
fn admit_trust(
    workspace: &Path,
    base: &Path,
    root: &Path,
    expected_ca: [u8; 32],
) -> io::Result<AdmittedTrust> {
    let case = admitted_case(root, base)?;
    let system = system_directory()?;
    let application = system.join("WindowsPowerShell/v1.0/powershell.exe");
    let script = workspace.join(SCRIPT);
    let held = vec![
        hold(&application, None, 64 * 1024 * 1024)?,
        hold(&script, Some(script_digest()), 2923)?,
        hold(&root.join("ca.der"), Some(expected_ca), 64 * 1024)?,
    ];
    Ok(AdmittedTrust {
        case,
        system,
        application,
        script,
        held,
    })
}

pub struct TrustFixtureChild {
    core: ProcessCore,
    _held: Vec<HeldFile>,
    _case: File,
    reject_resume: bool,
}
impl TrustFixtureChild {
    /// Only the fixed compiled-script/OS-image pair and closed operation are admitted.
    pub fn create(
        workspace: &Path,
        root: &Path,
        operation: TrustFixtureOperation,
        expected_ca: [u8; 32],
    ) -> io::Result<(Self, ChildPipes)> {
        if std::env::var("GITHUB_ACTIONS").as_deref() != Ok("true")
            || std::env::var("SLOPSEARX_PLATFORM_TRUST_FIXTURE").as_deref() != Ok("1")
        {
            return Err(io::ErrorKind::PermissionDenied.into());
        }
        let base = PathBuf::from(
            std::env::var_os("SLOPSEARX_INSTALL_TEST_ROOT").ok_or(io::ErrorKind::InvalidInput)?,
        );
        let AdmittedTrust {
            case,
            system,
            application,
            script,
            held,
        } = admit_trust(workspace, &base, root, expected_ca)?;
        let args: Vec<OsString> = [
            "-NoLogo",
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-File",
        ]
        .into_iter()
        .map(Into::into)
        .chain([
            script.into_os_string(),
            operation.argument().into(),
            root.as_os_str().to_owned(),
        ])
        .collect();
        let command = command_line(&application, &args)?;
        let env = environment(&system, root, true)?;
        let wide = trust::wide(application.as_os_str())?;
        let (core, pipes) = ProcessCore::spawn(&wide, command, None, Some(&env), None)?;
        Ok((
            Self {
                core,
                _held: held,
                _case: case,
                reject_resume: false,
            },
            pipes,
        ))
    }
    /// Read-only fixture control of the exact real admission. Returns no child,
    /// admitted capability or way to bypass the hosted flags in create().
    pub fn fixture_check_trust_inputs(
        workspace: &Path,
        base: &Path,
        root: &Path,
        expected_ca: [u8; 32],
    ) -> io::Result<()> {
        admit_trust(workspace, base, root, expected_ca).map(drop)
    }

    /// Qualification-only sealed payload; cannot select an arbitrary program or argv.
    pub fn fake(
        probe: &Path,
        expected: [u8; 32],
        root: &Path,
        operation: TrustFixtureOperation,
        scenario: TrustFixtureScenario,
    ) -> io::Result<(Self, ChildPipes)> {
        if probe.file_name() != Some(OsStr::new("ssx-gateway-trust-probe.exe")) {
            return Err(io::ErrorKind::InvalidInput.into());
        }
        let case = trust::opened_confidential_parent(root)?;
        let held = vec![hold(probe, Some(expected), 512 * 1024 * 1024)?];
        let args = vec![
            OsString::from("--trust-fixture"),
            scenario.argument().into(),
            operation.argument().into(),
            root.as_os_str().to_owned(),
        ];
        let command = command_line(probe, &args)?;
        let system = system_directory()?;
        let env = environment(&system, root, false)?;
        let wide = trust::wide(probe.as_os_str())?;
        let mut rejected = None;
        let wrong = (scenario == TrustFixtureScenario::RejectJob).then_some(&mut rejected);
        let (core, pipes) = ProcessCore::spawn(&wide, command, None, Some(&env), wrong)?;
        Ok((
            Self {
                core,
                _held: held,
                _case: case,
                reject_resume: scenario == TrustFixtureScenario::RejectResume,
            },
            pipes,
        ))
    }
    pub fn image_digest(&self) -> crate::FileDigest {
        // Both finite constructors retain the exact executable as the first file.
        crate::FileDigest {
            bytes: self._held[0].bytes,
            sha256: self._held[0].digest,
        }
    }
    pub fn pid(&self) -> u32 {
        self.core.pid()
    }
    pub fn observer(&self) -> ExitHandle {
        self.core.observer()
    }
    pub fn cancellation(&self) -> JobCancel {
        self.core.cancellation()
    }
    pub fn resume(&mut self) -> io::Result<()> {
        if self.reject_resume {
            return Err(io::ErrorKind::Other.into());
        }
        self.core.resume()
    }
    pub fn observe(&self) -> io::Result<TrustFixtureObservation> {
        Ok(TrustFixtureObservation {
            leader_exit: self.core.observe()?,
            job_empty: self.core.job_empty()?,
        })
    }
    pub async fn retire(&mut self, budget: Duration) -> io::Result<Retirement> {
        self.core.retire(budget).await
    }
    pub fn finish(&mut self) -> io::Result<Retirement> {
        self.core.finish()
    }
}
