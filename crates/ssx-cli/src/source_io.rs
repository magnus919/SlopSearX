//! Safe copying bridge between vetted Hyper/Tokio IO and source socket policy.
//! Semantics follow hyper-util0.1.21 TokioIo (MIT); fixed initialized scratch
//! avoids its unsafe uninitialized-buffer bridge. No stream/body size ceiling.
use std::{
    future::Future,
    io,
    pin::Pin,
    task::{Context, Poll},
    time::Duration,
};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::time::Sleep;

pub struct Bridge<T>(pub T);
impl<T: AsyncRead + Unpin> hyper::rt::Read for Bridge<T> {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        mut target: hyper::rt::ReadBufCursor<'_>,
    ) -> Poll<io::Result<()>> {
        let mut bytes = [0u8; 8192];
        let count = target.remaining().min(bytes.len());
        if count == 0 {
            return Poll::Ready(Ok(()));
        }
        let mut buffer = ReadBuf::new(&mut bytes[..count]);
        match Pin::new(&mut self.get_mut().0).poll_read(cx, &mut buffer) {
            Poll::Ready(Ok(())) => {
                target.put_slice(buffer.filled());
                Poll::Ready(Ok(()))
            }
            Poll::Ready(Err(error)) => Poll::Ready(Err(error)),
            Poll::Pending => Poll::Pending,
        }
    }
}
impl<T: AsyncWrite + Unpin> hyper::rt::Write for Bridge<T> {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bytes: &[u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.get_mut().0).poll_write(cx, bytes)
    }
    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.get_mut().0).poll_flush(cx)
    }
    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.get_mut().0).poll_shutdown(cx)
    }
}
impl<T: hyper::rt::Read + Unpin> AsyncRead for Bridge<T> {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        target: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let mut bytes = [0u8; 8192];
        let count = target.remaining().min(bytes.len());
        if count == 0 {
            return Poll::Ready(Ok(()));
        }
        let mut buffer = hyper::rt::ReadBuf::new(&mut bytes[..count]);
        match Pin::new(&mut self.get_mut().0).poll_read(cx, buffer.unfilled()) {
            Poll::Ready(Ok(())) => {
                target.put_slice(buffer.filled());
                Poll::Ready(Ok(()))
            }
            Poll::Ready(Err(error)) => Poll::Ready(Err(error)),
            Poll::Pending => Poll::Pending,
        }
    }
}
impl<T: hyper::rt::Write + Unpin> AsyncWrite for Bridge<T> {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bytes: &[u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.get_mut().0).poll_write(cx, bytes)
    }
    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.get_mut().0).poll_flush(cx)
    }
    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.get_mut().0).poll_shutdown(cx)
    }
}

pub struct Inactivity<T> {
    inner: T,
    read: Option<Pin<Box<Sleep>>>,
    write: Option<Pin<Box<Sleep>>>,
}
impl<T> Inactivity<T> {
    pub fn new(inner: T) -> Self {
        Self {
            inner,
            read: None,
            write: None,
        }
    }
}
fn pending(timer: &mut Option<Pin<Box<Sleep>>>, cx: &mut Context<'_>) -> bool {
    timer
        .get_or_insert_with(|| Box::pin(tokio::time::sleep(Duration::from_secs(15))))
        .as_mut()
        .poll(cx)
        .is_ready()
}
fn timeout() -> io::Error {
    io::Error::new(io::ErrorKind::TimedOut, "source socket inactivity deadline")
}
impl<T: AsyncRead + Unpin> AsyncRead for Inactivity<T> {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buffer: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let this = self.get_mut();
        if this
            .read
            .as_mut()
            .is_some_and(|t| t.as_mut().poll(cx).is_ready())
        {
            return Poll::Ready(Err(timeout()));
        }
        match Pin::new(&mut this.inner).poll_read(cx, buffer) {
            Poll::Pending if pending(&mut this.read, cx) => Poll::Ready(Err(timeout())),
            Poll::Pending => Poll::Pending,
            ready => {
                this.read = None;
                ready
            }
        }
    }
}
impl<T: AsyncWrite + Unpin> AsyncWrite for Inactivity<T> {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buffer: &[u8],
    ) -> Poll<io::Result<usize>> {
        let this = self.get_mut();
        if this
            .write
            .as_mut()
            .is_some_and(|t| t.as_mut().poll(cx).is_ready())
        {
            return Poll::Ready(Err(timeout()));
        }
        match Pin::new(&mut this.inner).poll_write(cx, buffer) {
            Poll::Pending if pending(&mut this.write, cx) => Poll::Ready(Err(timeout())),
            Poll::Pending => Poll::Pending,
            ready => {
                this.write = None;
                ready
            }
        }
    }
    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        let this = self.get_mut();
        if this
            .write
            .as_mut()
            .is_some_and(|t| t.as_mut().poll(cx).is_ready())
        {
            return Poll::Ready(Err(timeout()));
        }
        match Pin::new(&mut this.inner).poll_flush(cx) {
            Poll::Pending if pending(&mut this.write, cx) => Poll::Ready(Err(timeout())),
            Poll::Pending => Poll::Pending,
            ready => {
                this.write = None;
                ready
            }
        }
    }
    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        let this = self.get_mut();
        if this
            .write
            .as_mut()
            .is_some_and(|t| t.as_mut().poll(cx).is_ready())
        {
            return Poll::Ready(Err(timeout()));
        }
        match Pin::new(&mut this.inner).poll_shutdown(cx) {
            Poll::Pending if pending(&mut this.write, cx) => Poll::Ready(Err(timeout())),
            Poll::Pending => Poll::Pending,
            ready => {
                this.write = None;
                ready
            }
        }
    }
}

/// Source limits physical status/header lines, not logical parsed header names.
/// The HTTP parser remains Hyper; this observer only enforces source budgets.
pub struct HeaderPolicy<T> {
    inner: T,
    active: bool,
    status: bool,
    continued: bool,
    line: usize,
    lines: usize,
    prefix: [u8; 16],
    prefix_len: usize,
    previous_cr: bool,
}
impl<T> HeaderPolicy<T> {
    pub fn new(inner: T) -> Self {
        Self {
            inner,
            active: true,
            status: true,
            continued: false,
            line: 0,
            lines: 0,
            prefix: [0; 16],
            prefix_len: 0,
            previous_cr: false,
        }
    }
    fn observe(&mut self, bytes: &[u8]) -> io::Result<()> {
        for &byte in bytes {
            if !self.active {
                break;
            }
            self.line = self
                .line
                .checked_add(1)
                .filter(|n| *n <= 65536)
                .ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        "source response line exceeds 65536 bytes",
                    )
                })?;
            if self.status && self.prefix_len < self.prefix.len() {
                self.prefix[self.prefix_len] = byte;
                self.prefix_len += 1;
            }
            if byte == b'\n' {
                if self.status {
                    self.continued = self.prefix[..self.prefix_len]
                        .split(|b| b.is_ascii_whitespace())
                        .filter(|s| !s.is_empty())
                        .nth(1)
                        == Some(b"100".as_slice());
                    self.status = false;
                } else {
                    self.lines += 1;
                    if self.lines > 100 {
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidData,
                            "source response exceeds 100 physical header lines",
                        ));
                    }
                    if self.line == 1 || (self.line == 2 && self.previous_cr) {
                        if self.continued {
                            self.status = true;
                            self.lines = 0;
                            self.prefix_len = 0;
                        } else {
                            self.active = false;
                        }
                    }
                }
                self.line = 0;
            }
            self.previous_cr = byte == b'\r';
        }
        Ok(())
    }
}
impl<T: AsyncRead + Unpin> AsyncRead for HeaderPolicy<T> {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buffer: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let this = self.get_mut();
        let before = buffer.filled().len();
        match Pin::new(&mut this.inner).poll_read(cx, buffer) {
            Poll::Ready(Ok(())) => Poll::Ready(this.observe(&buffer.filled()[before..])),
            other => other,
        }
    }
}
impl<T: AsyncWrite + Unpin> AsyncWrite for HeaderPolicy<T> {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bytes: &[u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.get_mut().inner).poll_write(cx, bytes)
    }
    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.get_mut().inner).poll_flush(cx)
    }
    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.get_mut().inner).poll_shutdown(cx)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::future::poll_fn;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    #[test]
    fn physical_header_source_budgets_survive_byte_splits_and_continue_reset() {
        let mut exact = HeaderPolicy::new(());
        for byte in b"HTTP/1.1 100 \xff\r\nX: a\r\n\r\nHTTP/1.1 200 OK\r\n" {
            exact.observe(&[*byte]).unwrap();
        }
        for _ in 0..99 {
            for byte in b"X: a\r\n" {
                exact.observe(&[*byte]).unwrap();
            }
        }
        exact.observe(b"\r\n").unwrap();
        assert!(!exact.active);
        exact.observe(&vec![b'x'; 65537]).unwrap(); // body has no header budget
        let mut over = HeaderPolicy::new(());
        over.observe(b"HTTP/1.1 200 OK\r\n").unwrap();
        for _ in 0..100 {
            over.observe(b"X: a\r\n").unwrap();
        }
        assert!(over.observe(b"\r\n").is_err());
        let mut line = HeaderPolicy::new(());
        line.observe(b"HTTP/1.1 200 OK\r\n").unwrap();
        line.observe(&vec![b'x'; 65534]).unwrap();
        line.observe(b"\r\n").unwrap();
        assert_eq!(line.line, 0);
        line.observe(&vec![b'x'; 65536]).unwrap();
        assert!(line.observe(b"\n").is_err());
    }
    #[tokio::test(start_paused = true)]
    async fn socket_clock_expires_at_fifteen_and_resets_after_progress() {
        let (read, mut write) = tokio::io::duplex(16);
        let mut read = Inactivity::new(read);
        let mut byte = [0];
        for (delay, value, expires) in [
            (14999, b'a', false),
            (14000, b'b', false),
            (15000, b'c', true),
        ] {
            // Establish actual Pending before moving the clock. The block ends
            // the future's mutable borrow before inspecting/reusing the buffer.
            let result = {
                let future = read.read_exact(&mut byte);
                tokio::pin!(future);
                poll_fn(|cx| {
                    assert!(future.as_mut().poll(cx).is_pending());
                    Poll::Ready(())
                })
                .await;
                tokio::time::advance(Duration::from_millis(delay)).await;
                write.write_all(&[value]).await.unwrap();
                future.await
            };
            if expires {
                assert_eq!(result.unwrap_err().kind(), io::ErrorKind::TimedOut);
            } else {
                result.unwrap();
                assert_eq!(byte, [value]);
            }
        }
    }
}
