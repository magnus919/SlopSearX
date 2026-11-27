//! Owned Unix stdio. A wake socket interrupts a worker waiting on either
//! endpoint, including an output pipe whose reader has stopped consuming.
use nix::{
    errno::Errno,
    fcntl::{FcntlArg, OFlag, fcntl},
    poll::{PollFd, PollFlags, PollTimeout, poll},
    unistd::{read, write},
};
use std::{
    io,
    os::{
        fd::{AsFd, OwnedFd},
        unix::net::UnixStream,
    },
    thread::{self, JoinHandle},
};
use tokio::{io::AsyncWriteExt, sync::oneshot};
use tokio_util::sync::CancellationToken;

struct Nonblocking {
    fd: OwnedFd,
    original: OFlag,
}

impl Nonblocking {
    fn new(fd: OwnedFd) -> io::Result<Self> {
        let original = OFlag::from_bits_retain(fcntl(&fd, FcntlArg::F_GETFL)?);
        fcntl(&fd, FcntlArg::F_SETFL(original | OFlag::O_NONBLOCK))?;
        Ok(Self { fd, original })
    }
}

impl Drop for Nonblocking {
    fn drop(&mut self) {
        let _ = fcntl(&self.fd, FcntlArg::F_SETFL(self.original));
    }
}

enum Direction {
    Input,
    Output,
}

pub(crate) struct Bridge {
    stream: tokio::net::UnixStream,
    wake: Option<UnixStream>,
    done: oneshot::Receiver<()>,
    worker: Option<JoinHandle<io::Result<()>>>,
    _flags: Nonblocking,
}

impl Bridge {
    pub(crate) fn stdin() -> io::Result<Self> {
        Self::attach(
            std::io::stdin().as_fd().try_clone_to_owned()?,
            Direction::Input,
        )
    }

    pub(crate) fn stdout() -> io::Result<Self> {
        Self::attach(
            std::io::stdout().as_fd().try_clone_to_owned()?,
            Direction::Output,
        )
    }

    fn attach(fd: OwnedFd, direction: Direction) -> io::Result<Self> {
        let flags = Nonblocking::new(fd)?;
        let os_fd = flags.fd.try_clone()?;
        let (local, peer) = UnixStream::pair()?;
        local.set_nonblocking(true)?;
        peer.set_nonblocking(true)?;
        let stream = tokio::net::UnixStream::from_std(local)?;
        let peer = OwnedFd::from(peer);
        let (source, destination) = match direction {
            Direction::Input => (os_fd, peer),
            Direction::Output => (peer, os_fd),
        };
        let (wake, stop) = UnixStream::pair()?;
        let (finished, done) = oneshot::channel();
        let worker = thread::Builder::new()
            .name("ssx-gateway-stdio".into())
            .spawn(move || {
                let result = pump(&source, &destination, &stop);
                drop(source);
                drop(destination);
                let _ = finished.send(());
                result
            })?;
        Ok(Self {
            stream,
            wake: Some(wake),
            done,
            worker: Some(worker),
            _flags: flags,
        })
    }

    pub(crate) fn stream(&mut self) -> &mut tokio::net::UnixStream {
        &mut self.stream
    }

    pub(crate) async fn closed(&mut self) {
        let _ = (&mut self.done).await;
    }

    pub(crate) async fn finish(&mut self, cancel: &CancellationToken) -> io::Result<()> {
        if !cancel.is_cancelled() {
            self.stream.shutdown().await?;
            tokio::select! { _ = cancel.cancelled() => (), _ = &mut self.done => () }
        }
        self.stop()
    }

    pub(crate) fn stop(&mut self) -> io::Result<()> {
        // Dropping the wake writer makes POLLHUP persistent, including while
        // the data descriptor is continuously ready or permanently full.
        drop(self.wake.take());
        self.worker.take().map_or(Ok(()), |worker| {
            worker
                .join()
                .map_err(|_| io::Error::other("stdio bridge worker panicked"))?
        })
    }
}

impl Drop for Bridge {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

fn pump(source: &OwnedFd, destination: &OwnedFd, stop: &UnixStream) -> io::Result<()> {
    let mut buffer = [0; 4096];
    let (mut offset, mut end) = (0, 0);
    loop {
        let writing = offset < end;
        let (fd, ready) = if writing {
            (destination.as_fd(), PollFlags::POLLOUT)
        } else {
            (source.as_fd(), PollFlags::POLLIN)
        };
        let mut fds = [
            PollFd::new(stop.as_fd(), PollFlags::POLLIN),
            PollFd::new(fd, ready),
        ];
        match poll(&mut fds, PollTimeout::NONE) {
            Ok(_) => {}
            Err(Errno::EINTR) => continue,
            Err(err) => return Err(err.into()),
        }
        if !fds[0].revents().unwrap_or(PollFlags::POLLERR).is_empty() {
            return Ok(());
        }
        let events = fds[1].revents().unwrap_or(PollFlags::POLLERR);
        if events.intersects(PollFlags::POLLNVAL | PollFlags::POLLERR) {
            return Err(io::Error::other("stdio bridge descriptor failed"));
        }
        if !events.intersects(ready | PollFlags::POLLHUP) {
            continue;
        }
        if writing {
            match write(destination, &buffer[offset..end]) {
                Ok(0) => return Err(io::ErrorKind::WriteZero.into()),
                Ok(count) => offset += count,
                Err(Errno::EAGAIN | Errno::EINTR) => continue,
                Err(err) => return Err(err.into()),
            }
        } else {
            match read(source, &mut buffer) {
                Ok(0) => return Ok(()),
                Ok(count) => {
                    offset = 0;
                    end = count;
                }
                Err(Errno::EAGAIN | Errno::EINTR) => continue,
                Err(err) => return Err(err.into()),
            }
        }
    }
}
