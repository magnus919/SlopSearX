//! Actual IO remains owned when Hyper transfers it into an ordinary Upgraded.
use std::io::{self, IoSlice};
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};

use crate::client::legacy::connect::{Connected, Connection};
use hyper::rt::{Read, ReadBufCursor, Write};

use futures_util::task::AtomicWaker;

struct Shared<I> {
    io: Mutex<Option<I>>,
    read: AtomicWaker,
    write: AtomicWaker,
}

pub(super) type Closer = Arc<dyn Fn() + Send + Sync>;

pub(crate) struct PhysicalIo<I> {
    inner: Arc<Shared<I>>,
    terminal: Option<Box<dyn FnOnce() + Send>>,
}

impl<I: Send + 'static> PhysicalIo<I> {
    pub(super) fn new(io: I, terminal: impl FnOnce() + Send + 'static) -> (Self, Closer) {
        let inner = Arc::new(Shared {
            io: Mutex::new(Some(io)),
            read: AtomicWaker::new(),
            write: AtomicWaker::new(),
        });
        let owned = inner.clone();
        let closer: Closer = Arc::new(move || {
            // Physical transport destruction happens after releasing this IO
            // lock, and never while the pool authority lock is held.
            let io = owned
                .io
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .take();
            drop(io);
            owned.read.wake();
            owned.write.wake();
        });
        (
            Self {
                inner,
                terminal: Some(Box::new(terminal)),
            },
            closer,
        )
    }
}

impl<I> Drop for PhysicalIo<I> {
    fn drop(&mut self) {
        let io = self
            .inner
            .io
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .take();
        drop(io);
        self.inner.read.wake();
        self.inner.write.wake();
        // Explicit close may have removed the transport earlier; this guard
        // still charges the entry until the Upgraded owner itself is dropped.
        if let Some(terminal) = self.terminal.take() {
            terminal();
        }
    }
}

fn closed() -> io::Error {
    io::Error::new(
        io::ErrorKind::ConnectionAborted,
        "source pool transport closed",
    )
}

impl<I: Read + Unpin> Read for PhysicalIo<I> {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: ReadBufCursor<'_>,
    ) -> Poll<io::Result<()>> {
        self.inner.read.register(cx.waker());
        let mut inner = self
            .inner
            .io
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        match inner.as_mut() {
            Some(io) => Pin::new(io).poll_read(cx, buf),
            None => Poll::Ready(Err(closed())),
        }
    }
}

impl<I: Write + Unpin> Write for PhysicalIo<I> {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        self.inner.write.register(cx.waker());
        let mut inner = self
            .inner
            .io
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        match inner.as_mut() {
            Some(io) => Pin::new(io).poll_write(cx, buf),
            None => Poll::Ready(Err(closed())),
        }
    }
    fn poll_write_vectored(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &[IoSlice<'_>],
    ) -> Poll<io::Result<usize>> {
        self.inner.write.register(cx.waker());
        let mut inner = self
            .inner
            .io
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        match inner.as_mut() {
            Some(io) => Pin::new(io).poll_write_vectored(cx, bufs),
            None => Poll::Ready(Err(closed())),
        }
    }
    fn is_write_vectored(&self) -> bool {
        self.inner
            .io
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .as_ref()
            .is_some_and(Write::is_write_vectored)
    }
    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.inner.write.register(cx.waker());
        let mut inner = self
            .inner
            .io
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        match inner.as_mut() {
            Some(io) => Pin::new(io).poll_flush(cx),
            None => Poll::Ready(Err(closed())),
        }
    }
    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.inner.write.register(cx.waker());
        let mut inner = self
            .inner
            .io
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        match inner.as_mut() {
            Some(io) => Pin::new(io).poll_shutdown(cx),
            None => Poll::Ready(Ok(())),
        }
    }
}

impl<I: Connection> Connection for PhysicalIo<I> {
    fn connected(&self) -> Connected {
        self.inner
            .io
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .as_ref()
            .map_or_else(Connected::new, Connection::connected)
    }
}
