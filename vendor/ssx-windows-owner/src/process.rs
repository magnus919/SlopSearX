use crate::trust::{self, ImageRole, PrivateSecurity, TrustedImage};
use std::{
    ffi::OsStr,
    io, mem,
    os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle},
    ptr,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};
use tokio::net::windows::named_pipe::{NamedPipeServer, ServerOptions};
use windows_sys::Win32::{
    Foundation::*,
    Storage::FileSystem::*,
    System::{IO::CancelIoEx, JobObjects::*, Threading::*},
};

struct Job(OwnedHandle);
impl Drop for Job {
    fn drop(&mut self) { /* OwnedHandle closes kill-on-close job. */
    }
}
#[derive(Clone)]
pub struct JobCancel(Arc<Job>);
impl JobCancel {
    /// Scoped authority is the retained job handle, never an integer PID.
    pub fn cancel(&self) -> io::Result<()> {
        if unsafe { TerminateJobObject(trust::raw(&self.0.0), 239) } == 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    }
    fn empty(&self) -> io::Result<bool> {
        let mut accounting: JOBOBJECT_BASIC_ACCOUNTING_INFORMATION = unsafe { mem::zeroed() };
        if unsafe {
            QueryInformationJobObject(
                trust::raw(&self.0.0),
                JobObjectBasicAccountingInformation,
                (&mut accounting as *mut JOBOBJECT_BASIC_ACCOUNTING_INFORMATION).cast(),
                mem::size_of_val(&accounting) as u32,
                ptr::null_mut(),
            )
        } == 0
        {
            return Err(io::Error::last_os_error());
        }
        Ok(accounting.ActiveProcesses == 0)
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExitObservation {
    pub code: u32,
}
#[derive(Debug)]
pub struct Retirement {
    pub exit: Option<ExitObservation>,
    pub job_empty: bool,
}
impl Retirement {
    pub fn complete(&self) -> bool {
        self.exit.is_some() && self.job_empty
    }
}

/// Pipes remain owned across cancellation until all started IO futures return.
/// `cancel_io` is a request, not a completion/retirement proof.
pub struct ChildPipes {
    pub stdin: Option<NamedPipeServer>,
    pub stdout: NamedPipeServer,
    pub stderr: NamedPipeServer,
}
impl ChildPipes {
    pub async fn connected(&self) -> io::Result<()> {
        async fn connect(pipe: &NamedPipeServer) -> io::Result<()> {
            match pipe.connect().await {
                Ok(()) => Ok(()),
                Err(e) if e.raw_os_error() == Some(ERROR_PIPE_CONNECTED as i32) => Ok(()),
                Err(e) => Err(e),
            }
        }
        if let Some(pipe) = &self.stdin {
            connect(pipe).await?;
        }
        connect(&self.stdout).await?;
        connect(&self.stderr).await
    }
    pub fn cancel_io(&self) -> io::Result<()> {
        let cancel = |pipe: &NamedPipeServer| -> io::Result<()> {
            if unsafe { CancelIoEx(pipe.as_raw_handle().cast(), ptr::null()) } != 0 {
                return Ok(());
            }
            let error = io::Error::last_os_error();
            if error.raw_os_error() == Some(ERROR_NOT_FOUND as i32) {
                Ok(())
            } else {
                Err(error)
            }
        };
        if let Some(input) = &self.stdin {
            cancel(input)?;
        }
        cancel(&self.stdout)?;
        cancel(&self.stderr)
    }
}
struct Attributes {
    bytes: Vec<usize>,
    initialized: bool,
}
impl Attributes {
    fn handles(handles: &[HANDLE], jobs: &[HANDLE]) -> io::Result<Self> {
        let mut bytes = 0usize;
        unsafe {
            InitializeProcThreadAttributeList(ptr::null_mut(), 2, 0, &mut bytes);
        }
        if bytes == 0 || bytes > 65536 {
            return Err(io::ErrorKind::InvalidData.into());
        }
        let mut result = Self {
            bytes: vec![0; bytes.div_ceil(mem::size_of::<usize>())],
            initialized: false,
        };
        if unsafe { InitializeProcThreadAttributeList(result.pointer(), 2, 0, &mut bytes) } == 0 {
            return Err(io::Error::last_os_error());
        }
        result.initialized = true;
        // SAFETY: the handle slice and list remain alive and unchanged until
        // CreateProcessW returns; all referenced handles are uniquely owned.
        if unsafe {
            UpdateProcThreadAttribute(
                result.pointer(),
                0,
                PROC_THREAD_ATTRIBUTE_HANDLE_LIST as usize,
                handles.as_ptr().cast(),
                mem::size_of_val(handles),
                ptr::null_mut(),
                ptr::null(),
            )
        } == 0
        {
            return Err(io::Error::last_os_error());
        }
        if unsafe {
            UpdateProcThreadAttribute(
                result.pointer(),
                0,
                PROC_THREAD_ATTRIBUTE_JOB_LIST as usize,
                jobs.as_ptr().cast(),
                mem::size_of_val(jobs),
                ptr::null_mut(),
                ptr::null(),
            )
        } == 0
        {
            return Err(io::Error::last_os_error());
        }
        Ok(result)
    }
    fn pointer(&mut self) -> LPPROC_THREAD_ATTRIBUTE_LIST {
        self.bytes.as_mut_ptr().cast()
    }
}
impl Drop for Attributes {
    fn drop(&mut self) {
        if self.initialized {
            unsafe {
                DeleteProcThreadAttributeList(self.pointer());
            }
        }
    }
}

struct PipePair {
    parent: NamedPipeServer,
    child: OwnedHandle,
}
impl PipePair {
    fn create(parent_reads: bool) -> io::Result<Self> {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let n = NEXT
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |n| n.checked_add(1))
            .map_err(|_| io::ErrorKind::Other)?;
        let name = format!(r"\\.\pipe\slopsearx-owned-{}-{n}", std::process::id());
        let mut security = PrivateSecurity::new()?;
        let parent = unsafe {
            ServerOptions::new()
                .access_inbound(parent_reads)
                .access_outbound(!parent_reads)
                .first_pipe_instance(true)
                .reject_remote_clients(true)
                .in_buffer_size(4096)
                .out_buffer_size(4096)
                .create_with_security_attributes_raw(
                    &name,
                    (&mut security.attributes as *mut _).cast(),
                )
        }?;
        let wide = trust::wide(OsStr::new(&name))?;
        // The exact private server instance exists and has never admitted a
        // client. Opening its client endpoint is not an unbounded wait/read.
        let child = trust::handle(unsafe {
            CreateFileW(
                wide.as_ptr(),
                if parent_reads {
                    GENERIC_WRITE
                } else {
                    GENERIC_READ
                },
                0,
                &security.attributes,
                OPEN_EXISTING,
                FILE_ATTRIBUTE_NORMAL,
                ptr::null_mut(),
            )
        })?;
        let inherited = trust::duplicate(trust::raw(&child), true)?;
        drop(child);
        Ok(Self {
            parent,
            child: inherited,
        })
    }
}

/// A suspended, assigned child. Resume is explicit so the supervisor can
/// publish the scoped cancellation handle before child code is allowed to run.
pub struct NativeChild {
    process: Arc<OwnedHandle>,
    thread: Option<OwnedHandle>,
    cancel: JobCancel,
    pid: u32,
    _image: TrustedImage,
}
impl std::fmt::Debug for NativeChild {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NativeChild")
            .field("pid", &self.pid)
            .finish_non_exhaustive()
    }
}
impl Drop for NativeChild {
    fn drop(&mut self) {
        let _ = self.cancel.cancel();
    }
}
impl NativeChild {
    pub fn dns(image: &TrustedImage) -> io::Result<(Self, ChildPipes)> {
        Self::spawn(image, ImageRole::Dns, None)
    }
    pub fn input(image: &TrustedImage, input: OwnedHandle) -> io::Result<(Self, ChildPipes)> {
        Self::spawn(image, ImageRole::Input, Some(input))
    }
    fn spawn(
        image: &TrustedImage,
        role: ImageRole,
        input: Option<OwnedHandle>,
    ) -> io::Result<(Self, ChildPipes)> {
        let application = image.application(role)?;
        let output = PipePair::create(true)?;
        let error = PipePair::create(true)?;
        let incoming = if input.is_none() {
            Some(PipePair::create(false)?)
        } else {
            None
        };
        let input = match input {
            Some(handle) => Some(trust::duplicate(trust::raw(&handle), true)?),
            None => None,
        };
        let child_input = input
            .as_ref()
            .map(trust::raw)
            .or_else(|| incoming.as_ref().map(|p| trust::raw(&p.child)))
            .ok_or(io::ErrorKind::InvalidData)?;
        let handles = [
            child_input,
            trust::raw(&output.child),
            trust::raw(&error.child),
        ];
        let job = trust::handle(unsafe { CreateJobObjectW(ptr::null(), ptr::null()) })?;
        let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = unsafe { mem::zeroed() };
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        if unsafe {
            SetInformationJobObject(
                trust::raw(&job),
                JobObjectExtendedLimitInformation,
                (&limits as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
                mem::size_of_val(&limits) as u32,
            )
        } == 0
        {
            return Err(io::Error::last_os_error());
        }
        let cancel = JobCancel(Arc::new(Job(job)));
        let jobs = [trust::raw(&cancel.0.0)];
        let mut attributes = Attributes::handles(&handles, &jobs)?;
        let mut info: PROCESS_INFORMATION = unsafe { mem::zeroed() };
        let mut startup: STARTUPINFOEXW = unsafe { mem::zeroed() };
        startup.StartupInfo.cb = mem::size_of::<STARTUPINFOEXW>() as u32;
        startup.StartupInfo.dwFlags = STARTF_USESTDHANDLES;
        startup.StartupInfo.hStdInput = handles[0];
        startup.StartupInfo.hStdOutput = handles[1];
        startup.StartupInfo.hStdError = handles[2];
        startup.lpAttributeList = attributes.pointer();
        // SAFETY: mutable command storage and all STARTUPINFO/attribute/handle
        // storage remain live; application is the exact pinned no-argument role.
        let mut command = Vec::with_capacity(application.len() + 2);
        command.push(b'"' as u16);
        command.extend_from_slice(&application[..application.len() - 1]);
        command.push(b'"' as u16);
        command.push(0);
        if unsafe {
            CreateProcessW(
                application.as_ptr(),
                command.as_mut_ptr(),
                ptr::null(),
                ptr::null(),
                1,
                CREATE_SUSPENDED | EXTENDED_STARTUPINFO_PRESENT | CREATE_NO_WINDOW,
                ptr::null(),
                ptr::null(),
                &startup.StartupInfo,
                &mut info,
            )
        } == 0
        {
            return Err(io::Error::last_os_error());
        }
        // Successful CreateProcessW guarantees both returned handles. Atomic
        // JOB_LIST assignment already contains the suspended child in our job.
        // No fallible operation or await occurs after creation before ownership
        // is returned to the registered supervisor.
        let process = unsafe { OwnedHandle::from_raw_handle(info.hProcess.cast()) };
        let thread = unsafe { OwnedHandle::from_raw_handle(info.hThread.cast()) };
        let child = Self {
            process: Arc::new(process),
            thread: Some(thread),
            cancel,
            pid: info.dwProcessId,
            _image: image.clone(),
        };
        let pipes = ChildPipes {
            stdin: incoming.map(|p| p.parent),
            stdout: output.parent,
            stderr: error.parent,
        };
        Ok((child, pipes))
    }
    pub fn pid(&self) -> u32 {
        self.pid
    }
    pub fn observer(&self) -> ExitHandle {
        ExitHandle(self.process.clone())
    }
    pub fn cancellation(&self) -> JobCancel {
        self.cancel.clone()
    }
    pub fn resume(&mut self) -> io::Result<()> {
        let thread = self.thread.as_ref().ok_or(io::ErrorKind::InvalidInput)?;
        let count = unsafe { ResumeThread(trust::raw(thread)) };
        if count == u32::MAX {
            return Err(io::Error::last_os_error());
        }
        if count != 1 {
            return Err(io::ErrorKind::InvalidData.into());
        }
        drop(self.thread.take());
        Ok(())
    }
    pub fn observe(&self) -> io::Result<Option<ExitObservation>> {
        match unsafe { WaitForSingleObject(trust::raw(&self.process), 0) } {
            WAIT_TIMEOUT => Ok(None),
            WAIT_OBJECT_0 => {
                let mut code = 0;
                if unsafe { GetExitCodeProcess(trust::raw(&self.process), &mut code) } == 0 {
                    Err(io::Error::last_os_error())
                } else {
                    Ok(Some(ExitObservation { code }))
                }
            }
            _ => Err(io::Error::last_os_error()),
        }
    }
    pub async fn wait_exit(&self, budget: Duration) -> io::Result<ExitObservation> {
        tokio::time::timeout(budget, async {
            loop {
                if let Some(exit) = self.observe()? {
                    return Ok(exit);
                }
                tokio::time::sleep(Duration::from_millis(2)).await;
            }
        })
        .await
        .map_err(|_| io::Error::from(io::ErrorKind::TimedOut))?
    }
    pub async fn retire(&mut self, budget: Duration) -> io::Result<Retirement> {
        self.cancel.cancel()?;
        tokio::time::timeout(budget, async {
            loop {
                let exit = self.observe()?;
                let job_empty = self.cancel.empty()?;
                if exit.is_some() && job_empty {
                    return Ok(Retirement { exit, job_empty });
                }
                tokio::time::sleep(Duration::from_millis(2)).await;
            }
        })
        .await
        .map_err(|_| io::Error::from(io::ErrorKind::TimedOut))?
    }
}

#[derive(Clone)]
pub struct ExitHandle(Arc<OwnedHandle>);
impl ExitHandle {
    pub fn observe(&self) -> io::Result<Option<ExitObservation>> {
        match unsafe { WaitForSingleObject(trust::raw(&self.0), 0) } {
            WAIT_TIMEOUT => Ok(None),
            WAIT_OBJECT_0 => {
                let mut code = 0;
                if unsafe { GetExitCodeProcess(trust::raw(&self.0), &mut code) } == 0 {
                    Err(io::Error::last_os_error())
                } else {
                    Ok(Some(ExitObservation { code }))
                }
            }
            _ => Err(io::Error::last_os_error()),
        }
    }
    pub async fn exited(&self) -> io::Result<ExitObservation> {
        loop {
            if let Some(status) = self.observe()? {
                return Ok(status);
            }
            tokio::time::sleep(Duration::from_millis(2)).await;
        }
    }
}

#[cfg(feature = "fixtures")]
pub struct FixtureInput {
    pub writer: NamedPipeServer,
    pub input: OwnedHandle,
}
#[cfg(feature = "fixtures")]
impl FixtureInput {
    pub fn new() -> io::Result<Self> {
        let pair = PipePair::create(false)?;
        Ok(Self {
            writer: pair.parent,
            input: pair.child,
        })
    }
}
