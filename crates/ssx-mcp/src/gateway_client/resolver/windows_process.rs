//! Windows child/IO backend below the single shared resolver admission owner.
#![forbid(unsafe_code)]
use super::{
    protocol::{self, SourceResolveError, SourceResolveStage},
    runtime::locked,
};
use ssx_windows_owner::{
    ChildPipes, ExitHandle, ExitObservation, ImageRole, JobCancel, NativeChild, TrustedImage,
};
use std::{
    io,
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU32, Ordering},
    },
    time::Duration,
};
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWriteExt},
    sync::{oneshot, watch},
    task::JoinSet,
};
#[derive(Clone, Debug)]
pub struct SourceResolverHelper {
    image: TrustedImage,
    #[cfg(feature = "gateway-dns-fixtures")]
    fail_pipe_conversion: bool,
}
impl SourceResolverHelper {
    pub fn new(path: PathBuf, expected_sha256: [u8; 32]) -> io::Result<Self> {
        Ok(Self {
            image: TrustedImage::open(path, expected_sha256, ImageRole::Dns)?,
            #[cfg(feature = "gateway-dns-fixtures")]
            fail_pipe_conversion: false,
        })
    }
    #[cfg(feature = "gateway-dns-fixtures")]
    #[doc(hidden)]
    pub fn inject_pipe_conversion_failure_for_test(mut self) -> Self {
        self.fail_pipe_conversion = true;
        self
    }
}
pub(super) struct Control {
    identity: AtomicU32,
    job: Mutex<Option<JobCancel>>,
}
impl Control {
    pub(super) fn new() -> Self {
        Self {
            identity: AtomicU32::new(0),
            job: Mutex::new(None),
        }
    }
    pub(super) fn identity(&self) -> u32 {
        self.identity.load(Ordering::Acquire)
    }
    pub(super) fn clear_identity(&self) {
        self.identity.store(0, Ordering::Release);
    }
    pub(super) fn cancel_fallback(&self) {
        if let Some(job) = locked(&self.job).as_ref() {
            let _ = job.cancel();
        }
    }
    fn install(&self, child: &NativeChild) {
        *locked(&self.job) = Some(child.cancellation());
        self.identity.store(child.pid(), Ordering::Release);
    }
}
type Exchange = Result<(Vec<u8>, ExitObservation), SourceResolveError>;
pub(super) struct OwnedChild {
    native: NativeChild,
    io: JoinSet<Exchange>,
}
pub(super) struct Retirement {
    pub(super) group_absent: bool,
    exit: Option<ExitObservation>,
    io_joined: bool,
}
impl Retirement {
    pub(super) fn retired(&self) -> bool {
        self.group_absent && self.exit.is_some() && self.io_joined
    }
}
fn failure() -> SourceResolveError {
    SourceResolveError::new(SourceResolveStage::Helper)
}
async fn bounded(
    reader: impl AsyncRead + Unpin,
    limit: usize,
) -> Result<Vec<u8>, SourceResolveError> {
    let mut out = Vec::new();
    reader
        .take((limit + 1) as u64)
        .read_to_end(&mut out)
        .await
        .map_err(|_| failure())?;
    if out.len() > limit {
        return Err(SourceResolveError::new(
            SourceResolveStage::OperationalLimit,
        ));
    }
    Ok(out)
}
async fn exchange(
    mut pipes: ChildPipes,
    input: Vec<u8>,
    observer: ExitHandle,
    ready: Arc<AtomicBool>,
    cancel: JobCancel,
) -> Exchange {
    if pipes.connected().await.is_err() {
        let _ = cancel.cancel();
        return Err(failure());
    }
    let Some(mut stdin) = pipes.stdin.take() else {
        let _ = cancel.cancel();
        return Err(failure());
    };
    let (start, receive) = oneshot::channel();
    // Every future owns its pipe. join! retains all of them until completion;
    // an error requests job termination immediately instead of dropping siblings.
    let write_cancel = cancel.clone();
    let write = async move {
        let result = async {
            receive.await.map_err(|_| failure())?;
            stdin.write_all(&input).await.map_err(|_| failure())?;
            stdin.shutdown().await.map_err(|_| failure())?;
            drop(stdin);
            Ok::<_, SourceResolveError>(())
        }
        .await;
        if result.is_err() {
            let _ = write_cancel.cancel();
        }
        result
    };
    let read_cancel = cancel.clone();
    let read = async move {
        let result = async {
            let mut magic = [0; 16];
            pipes
                .stdout
                .read_exact(&mut magic)
                .await
                .map_err(|_| failure())?;
            if &magic != protocol::MAGIC {
                return Err(failure());
            }
            ready.store(true, Ordering::Release);
            start.send(()).map_err(|_| failure())?;
            bounded(pipes.stdout, protocol::FRAME_LIMIT).await
        }
        .await;
        if result.is_err() {
            let _ = read_cancel.cancel();
        }
        result
    };
    let error_cancel = cancel.clone();
    let error = async move {
        let result = bounded(pipes.stderr, protocol::STDERR_LIMIT).await;
        if result.is_err() {
            let _ = error_cancel.cancel();
        }
        result
    };
    let exit = async move {
        let result = observer.exited().await.map_err(|_| failure());
        if result.as_ref().is_err() || result.as_ref().is_ok_and(|e| e.code != 0) {
            let _ = cancel.cancel();
        }
        result
    };
    let (sent, out, err, status) = tokio::join!(write, read, error, exit);
    sent?;
    let out = out?;
    err?;
    let status = status?;
    Ok((out, status))
}
async fn cancelled(cancel: &mut watch::Receiver<bool>) {
    loop {
        if *cancel.borrow_and_update() {
            return;
        }
        if cancel.changed().await.is_err() {
            return;
        }
    }
}
pub(super) async fn retire(
    child: &mut OwnedChild,
    control: &Control,
    _completed_io: bool,
) -> Retirement {
    control.cancel_fallback();
    let observed = child.native.retire(Duration::from_secs(2)).await.ok();
    // Do not abort IO tasks: timeout retains them in the residual child. A later
    // shared close retries joining the exact original pipe futures and handles.
    let io_joined = tokio::time::timeout(Duration::from_secs(2), async {
        while child.io.join_next().await.is_some() {}
    })
    .await
    .is_ok();
    Retirement {
        group_absent: observed.as_ref().is_some_and(|r| r.job_empty),
        exit: observed.and_then(|r| r.exit),
        io_joined,
    }
}
pub(super) async fn supervise(
    helper: SourceResolverHelper,
    input: Vec<u8>,
    mut cancel: watch::Receiver<bool>,
    control: Arc<Control>,
    ready: Arc<AtomicBool>,
    budget: Duration,
) -> (ExchangeBytes, Option<OwnedChild>) {
    if *cancel.borrow() {
        return (
            Err(SourceResolveError::new(SourceResolveStage::Cancelled)),
            None,
        );
    }
    // NativeChild creation is synchronous and already assigned atomically to a
    // private job. No await precedes registered ownership/control publication.
    let (native, pipes) = match NativeChild::dns(&helper.image) {
        Ok(v) => v,
        Err(_) => return (Err(failure()), None),
    };
    control.install(&native);
    let observer = native.observer();
    let job = native.cancellation();
    let mut child = OwnedChild {
        native,
        io: JoinSet::new(),
    };
    child.io.spawn(exchange(pipes, input, observer, ready, job));
    let start = if *cancel.borrow() {
        Err(SourceResolveError::new(SourceResolveStage::Cancelled))
    } else {
        #[cfg(feature = "gateway-dns-fixtures")]
        if helper.fail_pipe_conversion {
            control.cancel_fallback();
        }
        child.native.resume().map_err(|_| failure())
    };
    let result = match start {
        Err(error) => Err(error),
        Ok(()) => tokio::select! {biased;
            _=cancelled(&mut cancel)=>Err(SourceResolveError::new(SourceResolveStage::Cancelled)),
            result=tokio::time::timeout(budget,child.io.join_next())=>match result { Ok(Some(Ok(result)))=>result,Ok(_)=>Err(failure()),Err(_)=>Err(SourceResolveError::new(SourceResolveStage::TimedOut)) },
        },
    };
    let completed = result.is_ok();
    let outcome = retire(&mut child, &control, completed).await;
    if outcome.group_absent {
        control.clear_identity();
    }
    let result = if outcome.retired() {
        result.and_then(|(bytes, original)| {
            if original.code == 0 && outcome.exit == Some(original) {
                Ok(bytes)
            } else {
                Err(failure())
            }
        })
    } else {
        Err(failure())
    };
    (result, if outcome.retired() { None } else { Some(child) })
}
type ExchangeBytes = Result<Vec<u8>, SourceResolveError>;
