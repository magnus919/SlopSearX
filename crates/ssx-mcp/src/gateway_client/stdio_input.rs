//! Gateway-only Unix input owner. No uncancellable Tokio stdin blocking read.
//! A pollable wake socket interrupts idle input and output backpressure. The
//! worker owns one 4KiB staging buffer plus a bounded kernel socket channel.
use crate::Result;
use nix::{
    errno::Errno,
    fcntl::{FcntlArg, OFlag, fcntl},
    poll::{PollFd, PollFlags, PollTimeout, poll},
};
use std::{
    io::Write,
    os::{
        fd::{AsFd, OwnedFd},
        unix::net::UnixStream,
    },
    thread::JoinHandle,
};

struct InputFd {
    fd: OwnedFd,
    original: OFlag,
}
impl InputFd {
    fn new(fd: OwnedFd) -> Result<Self> {
        let original = OFlag::from_bits_retain(fcntl(&fd, FcntlArg::F_GETFL)?);
        let input = Self { fd, original };
        fcntl(&input.fd, FcntlArg::F_SETFL(original | OFlag::O_NONBLOCK))?;
        Ok(input)
    }
    fn restore(&self) -> Result<()> {
        fcntl(&self.fd, FcntlArg::F_SETFL(self.original))?;
        Ok(())
    }
}
impl Drop for InputFd {
    fn drop(&mut self) {
        // Also restore when thread creation fails or the worker unwinds. The
        // normal joined path propagates restoration errors explicitly.
        let _ = self.restore();
    }
}

pub(crate) struct InputOwner {
    reader: Option<tokio::net::UnixStream>,
    wake: Option<UnixStream>,
    #[cfg(test)]
    backpressured: std::sync::Arc<std::sync::atomic::AtomicBool>,
    worker: Option<JoinHandle<Result<()>>>,
}
impl InputOwner {
    pub(crate) fn stdin() -> Result<Self> {
        Self::start(std::io::stdin().as_fd().try_clone_to_owned()?)
    }
    fn start(fd: OwnedFd) -> Result<Self> {
        let input = InputFd::new(fd)?;
        let (reader, output) = UnixStream::pair()?;
        reader.set_nonblocking(true)?;
        output.set_nonblocking(true)?;
        let reader = tokio::net::UnixStream::from_std(reader)?;
        let (wake, stop) = UnixStream::pair()?;
        wake.set_nonblocking(true)?;
        stop.set_nonblocking(true)?;
        #[cfg(test)]
        let backpressured = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        #[cfg(test)]
        let checkpoint = backpressured.clone();
        let worker = std::thread::Builder::new()
            .name("ssx-gateway-input".into())
            .spawn(move || {
                let result = pump(
                    &input,
                    output,
                    stop,
                    #[cfg(test)]
                    checkpoint,
                );
                let restored = input.restore();
                result.and(restored)
            })?;
        Ok(Self {
            reader: Some(reader),
            wake: Some(wake),
            worker: Some(worker),
            #[cfg(test)]
            backpressured,
        })
    }
    pub(crate) fn reader(&mut self) -> &mut tokio::net::UnixStream {
        self.reader
            .as_mut()
            .expect("gateway input reader owned until join")
    }
    pub(crate) async fn close(&mut self) -> Result<()> {
        self.shutdown()
    }
    /// Synchronously joins the actual poll worker. It performs no blocking OS
    /// read/write: its wake descriptor remains ready through join completion.
    pub(crate) fn shutdown(&mut self) -> Result<()> {
        // Shutting down the wake writer makes POLLHUP persistent: no full
        // wake buffer or interrupted one-byte write can strand the join.
        let wake = self.wake.take().map_or(Ok(()), |wake| {
            let result = wake.shutdown(std::net::Shutdown::Write);
            drop(wake); // Peer HUP is guaranteed even if shutdown failed.
            result
        });
        let joined = match self.worker.take() {
            Some(worker) => worker.join().map_err(|_| "gateway input worker panicked")?,
            None => Ok(()),
        };
        if let Err(error) = wake
            && error.kind() != std::io::ErrorKind::NotConnected
        {
            return Err(error.into());
        }
        joined
    }
}
impl Drop for InputOwner {
    fn drop(&mut self) {
        // An error path or dropped caller cannot detach the actual worker.
        let _ = self.shutdown();
    }
}

fn pump(
    input: &InputFd,
    mut output: UnixStream,
    stop: UnixStream,
    #[cfg(test)] backpressured: std::sync::Arc<std::sync::atomic::AtomicBool>,
) -> Result<()> {
    let mut bytes = [0u8; 4096];
    let (mut at, mut end) = (0, 0);
    loop {
        let pending = at < end;
        if pending {
            // Attempt the nonblocking write before waiting. A WouldBlock is
            // actual output backpressure, independent of worker scheduling.
            match output.write(&bytes[at..end]) {
                Ok(0) => return Err("gateway input channel closed".into()),
                Ok(n) => {
                    at += n;
                    continue;
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    #[cfg(test)]
                    backpressured.store(true, std::sync::atomic::Ordering::SeqCst);
                }
                Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(error) => return Err(error.into()),
            }
        }
        let mut fds = [
            PollFd::new(stop.as_fd(), PollFlags::POLLIN),
            if pending {
                PollFd::new(output.as_fd(), PollFlags::POLLOUT)
            } else {
                PollFd::new(input.fd.as_fd(), PollFlags::POLLIN)
            },
        ];
        match poll(&mut fds, PollTimeout::NONE) {
            Ok(_) => {}
            Err(Errno::EINTR) => continue,
            Err(error) => return Err(error.into()),
        }
        let ready = fds.map(|fd| fd.revents().unwrap_or(PollFlags::empty()));
        // Wake has priority even when the data descriptor is also ready.
        if !ready[0].is_empty() {
            return Ok(());
        }
        if ready[1].intersects(PollFlags::POLLERR | PollFlags::POLLNVAL) {
            return Err("gateway input descriptor failed".into());
        }
        if pending && ready[1].intersects(PollFlags::POLLOUT | PollFlags::POLLHUP) {
            match output.write(&bytes[at..end]) {
                Ok(0) => return Err("gateway input channel closed".into()),
                Ok(n) => at += n,
                Err(error)
                    if matches!(
                        error.kind(),
                        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::Interrupted
                    ) => {}
                Err(error) => return Err(error.into()),
            }
        } else if !pending && ready[1].intersects(PollFlags::POLLIN | PollFlags::POLLHUP) {
            match nix::unistd::read(&input.fd, &mut bytes) {
                Ok(0) => return Ok(()),
                Ok(n) => {
                    at = 0;
                    end = n;
                }
                Err(Errno::EAGAIN | Errno::EINTR) => {}
                Err(error) => return Err(error.into()),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    fn input() -> Result<(InputOwner, UnixStream)> {
        let (input, writer) = UnixStream::pair()?;
        Ok((InputOwner::start(input.into())?, writer))
    }
    #[tokio::test]
    async fn cancellation_joins_idle_worker_without_input_eof() -> Result<()> {
        let (mut owner, _held_writer) = input()?;
        owner.shutdown()?;
        assert!(owner.worker.is_none());
        Ok(())
    }
    #[tokio::test]
    async fn eof_preserves_all_bytes_and_joins_worker() -> Result<()> {
        let (mut owner, mut writer) = input()?;
        writer.write_all(b"owned frame\n")?;
        writer.shutdown(std::net::Shutdown::Write)?;
        let mut bytes = Vec::new();
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(3),
            owner.reader().read_to_end(&mut bytes),
        )
        .await;
        owner.shutdown()?;
        result??;
        assert_eq!(bytes, b"owned frame\n");
        Ok(())
    }
    #[tokio::test]
    async fn cancellation_joins_worker_under_output_backpressure() -> Result<()> {
        let (mut owner, writer) = input()?;
        writer.set_nonblocking(true)?;
        let mut writer = tokio::net::UnixStream::from_std(writer)?;
        let payload = vec![7u8; 1024 * 1024];
        let mut fill = Box::pin(writer.write_all(&payload));
        // Drive the finite writer until the worker has actually attempted a
        // nonblocking output write and observed WouldBlock. Elapsed time alone
        // cannot establish this checkpoint.
        let checkpoint = owner.backpressured.clone();
        let observed = tokio::time::timeout(std::time::Duration::from_secs(3), async {
            loop {
                tokio::select! {
                    result = &mut fill => { result?; return Err(std::io::Error::other("fixture input completed without backpressure")); },
                    _ = tokio::time::sleep(std::time::Duration::from_millis(1)) => {
                        if checkpoint.load(std::sync::atomic::Ordering::SeqCst) { return Ok(()); }
                    }
                }
            }
        }).await;
        owner.shutdown()?;
        drop(fill);
        observed??;
        assert!(owner.worker.is_none());
        Ok(())
    }
}
