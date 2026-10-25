//! Gateway-lifetime Windows input owner; dropping a lease never detaches a child.
#![forbid(unsafe_code)]
use crate::Result;
use ssx_windows_owner::{ChildPipes, NativeChild, TrustedImage};
use std::{
    future::Future,
    pin::Pin,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt, DuplexStream},
    task::{JoinHandle, JoinSet},
};
use tokio_util::sync::CancellationToken;
fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}
struct Child {
    native: NativeChild,
    io: JoinSet<std::result::Result<bool, String>>,
}
struct Outcome {
    error: bool,
    residual: Option<Child>,
}
#[derive(Default)]
struct State {
    closing: bool,
    stop: Option<CancellationToken>,
    task: Option<JoinHandle<Outcome>>,
    residual: Option<Child>,
    failed: bool,
}
#[derive(Default)]
struct Inner {
    state: Mutex<State>,
    closer: tokio::sync::Mutex<()>,
    ready: Arc<AtomicBool>,
    backpressure: Arc<AtomicBool>,
}
impl Drop for Inner {
    fn drop(&mut self) {
        if let Some(stop) = lock(&self.state).stop.as_ref() {
            stop.cancel();
        }
    }
}
/// RawGateway retains this owner independently of the cancellable stdio future.
#[derive(Clone, Default)]
pub(crate) struct InputRuntime {
    inner: Arc<Inner>,
}
pub(crate) struct InputOwner {
    reader: DuplexStream,
    stop: CancellationToken,
    runtime: InputRuntime,
}
impl Drop for InputOwner {
    fn drop(&mut self) {
        self.stop.cancel();
    }
}
impl InputOwner {
    pub(crate) fn reader(&mut self) -> &mut DuplexStream {
        &mut self.reader
    }
    pub(crate) async fn close(&mut self) -> Result<()> {
        self.stop.cancel();
        self.runtime.close().await
    }
}
impl InputRuntime {
    pub(crate) fn stdin(&self, image: &TrustedImage) -> Result<InputOwner> {
        self.start(image, ssx_windows_owner::current_input()?)
    }
    fn start(
        &self,
        image: &TrustedImage,
        input: std::os::windows::io::OwnedHandle,
    ) -> Result<InputOwner> {
        let runtime = tokio::runtime::Handle::try_current()?;
        let mut state = lock(&self.inner.state);
        if state.closing || state.task.is_some() || state.residual.is_some() {
            return Err("gateway input owner closed or busy".into());
        }
        let (reader, writer) = tokio::io::duplex(4096);
        let stop = CancellationToken::new();
        // Creation is synchronous, suspended and atomically assigned to its job.
        // No await exists before the independent lifetime owner stores the task.
        let (native, pipes) = NativeChild::input(image, input)?;
        state.stop = Some(stop.clone());
        state.task = Some(runtime.spawn(pump(
            native,
            pipes,
            writer,
            stop.clone(),
            self.inner.ready.clone(),
            self.inner.backpressure.clone(),
        )));
        Ok(InputOwner {
            reader,
            stop,
            runtime: self.clone(),
        })
    }
    /// Cancellation-safe: the actual JoinHandle remains in State until Ready.
    /// Dropping this close future neither takes nor aborts the pending task.
    pub(crate) async fn close(&self) -> Result<()> {
        {
            let mut state = lock(&self.inner.state);
            state.closing = true;
            if let Some(stop) = &state.stop {
                stop.cancel();
            }
        }
        let _closer = self.inner.closer.lock().await;
        let joined = std::future::poll_fn(|cx| {
            let mut state = lock(&self.inner.state);
            let Some(task) = state.task.as_mut() else {
                return std::task::Poll::Ready(None);
            };
            match Pin::new(task).poll(cx) {
                std::task::Poll::Pending => std::task::Poll::Pending,
                std::task::Poll::Ready(result) => {
                    state.task.take();
                    std::task::Poll::Ready(Some(result))
                }
            }
        })
        .await;
        if let Some(result) = joined {
            let mut state = lock(&self.inner.state);
            match result {
                Ok(outcome) => {
                    state.failed |= outcome.error;
                    state.residual = outcome.residual;
                }
                Err(_) => state.failed = true,
            }
        }
        // A retry preserves the exact original native/IO owner. No handle or PID
        // is substituted, and successful later retirement does not erase failure.
        let retry = {
            let mut state = lock(&self.inner.state);
            if let Some(mut child) = state.residual.take() {
                state.task = Some(tokio::spawn(async move {
                    let retired = retire(&mut child).await;
                    Outcome {
                        error: !retired,
                        residual: if retired { None } else { Some(child) },
                    }
                }));
                true
            } else {
                false
            }
        };
        if retry {
            let joined = std::future::poll_fn(|cx| {
                let mut state = lock(&self.inner.state);
                let Some(task) = state.task.as_mut() else {
                    return std::task::Poll::Ready(None);
                };
                match Pin::new(task).poll(cx) {
                    std::task::Poll::Pending => std::task::Poll::Pending,
                    std::task::Poll::Ready(result) => {
                        state.task.take();
                        std::task::Poll::Ready(Some(result))
                    }
                }
            })
            .await;
            if let Some(result) = joined {
                let mut state = lock(&self.inner.state);
                match result {
                    Ok(outcome) => {
                        state.failed |= outcome.error;
                        state.residual = outcome.residual;
                    }
                    Err(_) => state.failed = true,
                }
            }
        }
        let state = lock(&self.inner.state);
        if state.failed || state.residual.is_some() {
            Err("gateway Windows input retirement failed".into())
        } else {
            Ok(())
        }
    }
}
async fn drain(
    mut pipe: tokio::net::windows::named_pipe::NamedPipeServer,
) -> std::result::Result<(), String> {
    // Completes any previously pending read before dropping the OS stream. All
    // child writers have been terminated by the owned job; cap is operational.
    let mut bytes = 0usize;
    let mut block = [0u8; 4096];
    loop {
        let n = pipe
            .read(&mut block)
            .await
            .map_err(|_| "gateway input drain failed".to_owned())?;
        if n == 0 {
            return Ok(());
        }
        bytes += n;
        if bytes > 65536 {
            return Err("gateway input drain exceeded bound".into());
        }
    }
}
async fn forward(
    pipes: ChildPipes,
    mut output: DuplexStream,
    stop: CancellationToken,
    kill: ssx_windows_owner::JobCancel,
    backpressure: Arc<AtomicBool>,
) -> std::result::Result<bool, String> {
    let ChildPipes {
        stdin,
        mut stdout,
        stderr,
    } = pipes;
    drop(stdin);
    let k = kill.clone();
    let forward = async move {
        let mut bytes = [0u8; 4096];
        let mut stopped = false;
        let result=async{
            loop{
                let n=tokio::select!{biased;_=stop.cancelled()=>{stopped=true;break;},n=stdout.read(&mut bytes)=>n.map_err(|_|"gateway input read failed".to_owned())?};
                if n==0{break;}
                let mut write=Box::pin(output.write_all(&bytes[..n]));
                let observed=std::future::poll_fn(|cx|{let poll=write.as_mut().poll(cx);if poll.is_pending(){backpressure.store(true,Ordering::Release);}poll});
                tokio::select!{biased;_=stop.cancelled()=>{stopped=true;break;},r=observed=>r.map_err(|_|"gateway input receiver closed".to_owned())?};
            }
            Ok::<(),String>(())
        }.await;
        if stopped || result.is_err() {
            let _ = k.cancel();
            drain(stdout).await?;
        }
        output
            .shutdown()
            .await
            .map_err(|_| "gateway input EOF failed".to_owned())?;
        result?;
        Ok::<_, String>(stopped)
    };
    let diagnostics = async move {
        let mut raw = Vec::new();
        let result = stderr
            .take(65537)
            .read_to_end(&mut raw)
            .await
            .map_err(|_| "gateway input stderr failed".to_owned());
        if result.is_err() || raw.len() > 65536 {
            let _ = kill.cancel();
            return Err("gateway input stderr exceeded bound or failed".into());
        }
        Ok::<_, String>(())
    };
    let (data, diagnostics) = tokio::join!(forward, diagnostics);
    let stopped = data?;
    diagnostics?;
    Ok(stopped)
}
async fn retire(child: &mut Child) -> bool {
    let process = child.native.retire(Duration::from_secs(2)).await;
    let io = tokio::time::timeout(Duration::from_secs(2), async {
        while child.io.join_next().await.is_some() {}
    })
    .await;
    process.is_ok_and(|p| p.complete()) && io.is_ok()
}
async fn pump(
    native: NativeChild,
    pipes: ChildPipes,
    writer: DuplexStream,
    stop: CancellationToken,
    ready: Arc<AtomicBool>,
    backpressure: Arc<AtomicBool>,
) -> Outcome {
    let kill = native.cancellation();
    // Keep the pending connect future alive on cancellation until its actual OS
    // completion; job termination closes all inherited client endpoints.
    let setup = {
        let mut setup = Box::pin(pipes.connected());
        tokio::select! {biased;_=stop.cancelled()=>{let _=kill.cancel();setup.await},r=&mut setup=>r}
    };
    let mut child = Child {
        native,
        io: JoinSet::new(),
    };
    let setup = setup.and_then(|()| {
        if stop.is_cancelled() {
            Err(std::io::ErrorKind::Interrupted.into())
        } else {
            child.native.resume()
        }
    });
    if setup.is_err() {
        let _ = kill.cancel();
    } else {
        ready.store(true, Ordering::Release);
    }
    child
        .io
        .spawn(forward(pipes, writer, stop.clone(), kill, backpressure));
    let joined = tokio::select! {biased;
        _=stop.cancelled()=>{child.native.cancellation().cancel().ok();tokio::time::timeout(Duration::from_secs(2),child.io.join_next()).await.ok()},
        joined=child.io.join_next()=>Some(joined),
    };
    let cancelled = stop.is_cancelled();
    let io_ok = matches!(joined, Some(Some(Ok(Ok(_)))));
    let original = if cancelled {
        None
    } else {
        child.native.wait_exit(Duration::from_secs(2)).await.ok()
    };
    let retired = retire(&mut child).await;
    let error = !io_ok
        || (!cancelled && (setup.is_err() || original.is_none_or(|e| e.code != 0)))
        || !retired;
    Outcome {
        error,
        residual: if retired { None } else { Some(child) },
    }
}

#[cfg(feature = "gateway-dns-fixtures")]
#[derive(Clone, Copy)]
pub enum WindowsInputControl {
    Idle,
    Eof,
    Backpressure,
    DroppedClose,
}
#[cfg(feature = "gateway-dns-fixtures")]
pub async fn fixture_control(image: TrustedImage, case: WindowsInputControl) -> Result<()> {
    let runtime = InputRuntime::default();
    let pair = ssx_windows_owner::FixtureInput::new()?;
    let mut writer = pair.writer;
    let mut owner = runtime.start(&image, pair.input)?;
    let ready = runtime.inner.ready.clone();
    tokio::time::timeout(Duration::from_secs(3), async {
        while !ready.load(Ordering::Acquire) {
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
    })
    .await?;
    match case {
        WindowsInputControl::Idle => {
            owner.close().await?;
            drop(writer);
        }
        WindowsInputControl::Eof => {
            let bytes = b"exact native input\r\nsecond\n";
            writer.write_all(bytes).await?;
            drop(writer);
            let mut out = Vec::new();
            tokio::time::timeout(Duration::from_secs(3), owner.reader().read_to_end(&mut out))
                .await??;
            assert_eq!(out, bytes);
            owner.close().await?;
        }
        WindowsInputControl::Backpressure => {
            let bytes = vec![7u8; 1024 * 1024];
            let mut fill = Box::pin(writer.write_all(&bytes));
            let checkpoint = runtime.inner.backpressure.clone();
            tokio::time::timeout(Duration::from_secs(3),async{loop{tokio::select!{r=&mut fill=>{r?;return Err(std::io::Error::other("input completed without actual bounded output backpressure"));},_=tokio::time::sleep(Duration::from_millis(1))=>if checkpoint.load(Ordering::Acquire){return Ok(());}}}}).await??;
            owner.close().await?;
            let _completion = tokio::time::timeout(Duration::from_secs(3), &mut fill).await?;
            drop(fill);
            drop(writer);
        }
        WindowsInputControl::DroppedClose => {
            let mut close = Box::pin(owner.close());
            std::future::poll_fn(|cx| {
                assert!(close.as_mut().poll(cx).is_pending());
                std::task::Poll::Ready(())
            })
            .await;
            drop(close);
            owner.close().await?;
            drop(writer);
        }
    }
    let state = lock(&runtime.inner.state);
    assert!(state.closing && state.task.is_none() && state.residual.is_none() && !state.failed);
    Ok(())
}
