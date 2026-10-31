//! Feature-gated separate executable only; production helper has no test modes.
use super::protocol;
use std::io::{self, Read, Write};
#[allow(
    clippy::zombie_processes,
    reason = "fixture descendant must be retired by the original helper job owner"
)]
pub(super) fn main() -> io::Result<()> {
    if std::env::args_os()
        .nth(1)
        .is_some_and(|v| v == "--descendant")
    {
        loop {
            std::thread::park();
        }
    }
    io::stdout().write_all(protocol::MAGIC)?;
    io::stdout().flush()?;
    let mut raw = Vec::new();
    io::stdin()
        .take((protocol::FRAME_LIMIT + 1) as u64)
        .read_to_end(&mut raw)?;
    let request = protocol::decode_request(&raw)?;
    let host = request.host;
    let kind = request.kind;
    if host == "descendant.invalid" {
        // The fixture inherits its already assigned Windows job; no breakaway
        // flag is requested. A leader exit alone must not establish retirement.
        let _child = std::process::Command::new(std::env::current_exe()?)
            .arg("--descendant")
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::inherit())
            .stderr(std::process::Stdio::inherit())
            .spawn()?;
    }
    if host == "hold.invalid" || host.ends_with("%hold") {
        loop {
            std::thread::park();
        }
    }
    if host == "crash.invalid" {
        std::process::exit(73);
    }
    if host == "oversize.invalid" {
        return io::stdout().write_all(&vec![0u8; protocol::FRAME_LIMIT + 1]);
    }
    if host == "stderr-oversize.invalid" {
        return io::stderr().write_all(&vec![0u8; protocol::STDERR_LIMIT + 1]);
    }
    if host == "malformed.invalid" {
        return io::stdout().write_all(b"invalid");
    }
    if host == "gai-error.invalid" {
        return io::stdout().write_all(&protocol::error_response(kind, 11001, 0));
    }
    if host == "staged-source.invalid" {
        if kind != protocol::RequestKind::AnyIoCandidates || request.decimal != Some("65537") {
            return Err(io::ErrorKind::InvalidData.into());
        }
        let rows = [std::net::SocketAddr::V6(std::net::SocketAddrV6::new(
            "::2".parse().unwrap(),
            3,
            77,
            19,
        ))];
        return io::stdout().write_all(&protocol::response(kind, &rows).unwrap());
    }
    if host == "::2%19" {
        if kind != protocol::RequestKind::AsyncIoCandidate || request.decimal != Some("65537") {
            return Err(io::ErrorKind::InvalidData.into());
        }
        let rows = [std::net::SocketAddr::V6(std::net::SocketAddrV6::new(
            "::2".parse().unwrap(),
            37,
            88,
            23,
        ))];
        return io::stdout().write_all(&protocol::response(kind, &rows).unwrap());
    }
    if host == "empty.invalid" {
        return io::stdout().write_all(&protocol::response(kind, &[]).unwrap());
    }
    let rows = [
        std::net::SocketAddr::V4(std::net::SocketAddrV4::new(
            std::net::Ipv4Addr::LOCALHOST,
            7,
        )),
        std::net::SocketAddr::V6(std::net::SocketAddrV6::new(
            std::net::Ipv6Addr::LOCALHOST,
            9,
            17,
            4,
        )),
    ];
    io::stdout()
        .write_all(&protocol::response(kind, &rows).map_err(|_| io::ErrorKind::InvalidData)?)?;
    io::stdout().flush()
}
