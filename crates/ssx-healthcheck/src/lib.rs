//! Status-only HTTP readiness over a direct socket, bounded by one deadline.
use std::{
    io::{self, Read, Write},
    net::{SocketAddr, TcpStream},
    time::{Duration, Instant},
};
const REQUEST: &[u8] = b"GET /health HTTP/1.1\r\nHost: 127.0.0.1:8080\r\nConnection: close\r\n\r\n";
const MAX_HEADERS: usize = 16 * 1024;
fn remaining(deadline: Instant) -> io::Result<Duration> {
    deadline
        .checked_duration_since(Instant::now())
        .filter(|v| !v.is_zero())
        .ok_or_else(|| io::Error::new(io::ErrorKind::TimedOut, "health deadline"))
}
/// The caller supplies a direct address; no proxy, redirect, TLS, or body decoding.
/// Production fixes this address to IPv4 loopback:8080 and budget to four seconds.
pub fn probe(address: SocketAddr, budget: Duration) -> io::Result<()> {
    let deadline = Instant::now()
        .checked_add(budget)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "health deadline"))?;
    let mut socket = TcpStream::connect_timeout(&address, remaining(deadline)?)?;
    socket.set_write_timeout(Some(remaining(deadline)?))?;
    socket.write_all(REQUEST)?;
    let mut headers = Vec::new();
    let mut byte = [0];
    while !headers.ends_with(b"\r\n\r\n") {
        if headers.len() == MAX_HEADERS {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "health headers too large",
            ));
        }
        socket.set_read_timeout(Some(remaining(deadline)?))?;
        socket.read_exact(&mut byte)?;
        headers.push(byte[0]);
    }
    let line = headers.split(|b| *b == b'\n').next().unwrap_or_default();
    let status = line
        .strip_prefix(b"HTTP/1.1 ")
        .or_else(|| line.strip_prefix(b"HTTP/1.0 "))
        .and_then(|line| line.strip_prefix(b"200"));
    if status.is_some_and(|tail| tail.starts_with(b" ")) {
        Ok(())
    } else {
        Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "health status is not 200",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{net::TcpListener, thread};
    fn served(response: Vec<u8>, budget: Duration) -> io::Result<()> {
        let listener = TcpListener::bind("127.0.0.1:0")?;
        let address = listener.local_addr()?;
        let worker = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let mut request = Vec::new();
            let mut byte = [0];
            while !request.ends_with(b"\r\n\r\n") {
                stream.read_exact(&mut byte).unwrap();
                request.push(byte[0]);
                assert!(request.len() <= REQUEST.len());
            }
            assert_eq!(request, REQUEST);
            let _ = stream.write_all(&response);
        });
        let result = probe(address, budget);
        worker.join().unwrap();
        result
    }
    #[test]
    fn real_socket_status_only_accepts_200_without_reading_or_decoding_body() {
        for response in [
            b"HTTP/1.1 200 OK\r\nContent-Length: 999999999\r\n\r\n".as_slice(),
            b"HTTP/1.0 200 OK\r\n\r\n\xffnot-json".as_slice(),
        ] {
            served(response.to_vec(), Duration::from_secs(2)).unwrap();
        }
    }
    #[test]
    fn real_socket_rejects_non_200_redirect_and_malformed_or_oversized_headers() {
        for status in [
            "204 No Content",
            "301 Moved",
            "302 Found",
            "307 Temporary",
            "308 Permanent",
            "503 Unavailable",
            "2000 Invalid",
        ] {
            let response =
                format!("HTTP/1.1 {status}\r\nLocation: http://example.invalid/\r\n\r\n");
            assert!(served(response.into_bytes(), Duration::from_secs(2)).is_err());
        }
        assert!(served(b"not HTTP\r\n\r\n".to_vec(), Duration::from_secs(2)).is_err());
        assert!(served(b"HTTP/1.1 200\r\n\r\n".to_vec(), Duration::from_secs(2)).is_err());
        assert!(served(b"HTTP/1.1 200 OK\r\n".to_vec(), Duration::from_secs(2)).is_err());
        assert!(served(vec![b'x'; MAX_HEADERS + 1], Duration::from_secs(2)).is_err());
    }
    #[test]
    fn real_socket_slow_headers_and_refused_connection_fail_within_budget() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let worker = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            for _ in 0..30 {
                if stream.write_all(b"x").is_err() {
                    break;
                }
                thread::sleep(Duration::from_millis(10));
            }
        });
        let start = Instant::now();
        assert!(probe(address, Duration::from_millis(80)).is_err());
        assert!(start.elapsed() < Duration::from_secs(2));
        worker.join().unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        drop(listener);
        assert!(probe(address, Duration::from_millis(100)).is_err());
    }
}
