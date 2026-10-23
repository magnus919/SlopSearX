use super::protocol::{self, FRAME_LIMIT};
use std::io::{self, Read, Write};

#[cfg(feature = "gateway-dns-fixtures")]
use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6};

pub(crate) fn main() -> io::Result<()> {
    io::stdout().write_all(protocol::MAGIC)?;
    io::stdout().flush()?;
    let mut input = Vec::new();
    io::stdin()
        .take((FRAME_LIMIT + 1) as u64)
        .read_to_end(&mut input)?;
    let (host, decimal) = protocol::decode_request(&input)?;
    let output = lookup(host, decimal)?;
    io::stdout().write_all(&output)?;
    io::stdout().flush()
}
fn lookup(host: &str, decimal: &str) -> io::Result<Vec<u8>> {
    let hints = dns_lookup::AddrInfoHints {
        flags: 0,
        address: libc::AF_UNSPEC,
        socktype: libc::SOCK_STREAM,
        protocol: 0,
    };
    let rows = match dns_lookup::getaddrinfo(Some(host), Some(decimal), Some(hints)) {
        Ok(rows) => rows,
        Err(error) => {
            let gai = error.error_num();
            let native: io::Error = error.into();
            if gai == 0 {
                return Err(native);
            }
            let errno = if gai == libc::EAI_SYSTEM {
                native.raw_os_error().unwrap_or(0)
            } else {
                0
            };
            return Ok(protocol::error_response(gai, errno));
        }
    };
    let mut addresses = Vec::new();
    let mut record_bytes = 21usize;
    for row in rows {
        let addr = row?.sockaddr;
        // dns-lookup/socket2 retains native sin6_flowinfo bytes in SocketAddrV6.
        // Passing this value unchanged into Rust connect preserves the sockaddr;
        // CPython's printable tuple flow value requires ntohl for comparison.
        record_bytes += if addr.is_ipv4() { 7 } else { 27 };
        if record_bytes > FRAME_LIMIT {
            let mut output = protocol::MAGIC.to_vec();
            output.push(2);
            return Ok(output);
        }
        addresses.push(addr);
    }
    protocol::response(&addresses).map_err(|_| io::ErrorKind::InvalidData.into())
}

// Separate feature-gated executable: deterministic native process/IO controls.
// Production helper accepts no test commands, environment switch or mode flag.
#[cfg(feature = "gateway-dns-fixtures")]
#[allow(
    clippy::zombie_processes,
    reason = "Native fixture deliberately leaves an inherited-group descendant for retirement testing"
)]
pub(crate) fn fixture_main() -> io::Result<()> {
    // Deterministically require the supervisor's TERM grace then KILL path.
    let mut blocked = nix::sys::signal::SigSet::empty();
    blocked.add(nix::sys::signal::Signal::SIGTERM);
    nix::sys::signal::pthread_sigmask(
        nix::sys::signal::SigmaskHow::SIG_BLOCK,
        Some(&blocked),
        None,
    )
    .map_err(io::Error::from)?;
    if std::env::args_os()
        .nth(1)
        .is_some_and(|arg| arg == "--descendant")
    {
        loop {
            std::thread::park();
        }
    }
    io::stdout().write_all(protocol::MAGIC)?;
    io::stdout().flush()?;
    let mut input = Vec::new();
    io::stdin()
        .take((FRAME_LIMIT + 1) as u64)
        .read_to_end(&mut input)?;
    let (host, _) = protocol::decode_request(&input)?;
    // Fixture-only audit/control file, supplied by the private process test.
    // Production helper_main never reads this variable or accepts fixture modes.
    if let Some(path) = std::env::var_os("SLOPSEARX_DNS_FIXTURE_CONTROL") {
        use std::fs::OpenOptions;
        use std::path::PathBuf;
        let path = PathBuf::from(path);
        if !path.is_absolute() {
            return Err(io::ErrorKind::InvalidInput.into());
        }
        let mut control = OpenOptions::new().read(true).write(true).open(&path)?;
        control.lock()?;
        let mut mode = String::new();
        Read::by_ref(&mut control)
            .take(32)
            .read_to_string(&mut mode)?;
        if mode == "hold-once" {
            use std::io::{Seek, SeekFrom};
            control.set_len(0)?;
            control.seek(SeekFrom::Start(0))?;
            control.write_all(b"pass")?;
            control.sync_all()?;
        }
        drop(control);
        if mode == "hold-once" {
            // Record the actual normal-hostname hook input before publishing
            // readiness; the process test must not bypass DNS with an IP URL.
            std::fs::write(path.with_extension("host"), host)?;
            std::fs::write(
                path.with_extension("ready"),
                format!("{}\n", std::process::id()),
            )?;
            loop {
                // Parent-death fallback for a failing test: a closed stderr
                // pipe ends this fixture. Normal cancellation must still kill
                // and reap the original held child without completing lookup.
                io::stderr().write_all(b".")?;
                io::stderr().flush()?;
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
        }
        return io::stdout().write_all(
            &protocol::response(&[SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0))])
                .map_err(|_| io::ErrorKind::InvalidData)?,
        );
    }

    if host == "descendant.invalid" {
        // Test executable only: same native fixture remains in inherited group
        // and retains stdout after the immediate helper exits.
        let _descendant = std::process::Command::new(std::env::current_exe()?)
            .arg("--descendant")
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::inherit())
            .stderr(std::process::Stdio::inherit())
            .spawn()?;
    }
    if host.ends_with("%hold") || host == "hold.invalid" {
        loop {
            std::thread::park();
        }
    }
    if host == "crash.invalid" {
        std::process::exit(73);
    }
    if host == "oversize.invalid" {
        return io::stdout().write_all(&vec![0; FRAME_LIMIT + 1]);
    }
    if host == "stderr-oversize.invalid" {
        return io::stderr().write_all(&vec![0; protocol::STDERR_LIMIT + 1]);
    }
    if host == "malformed.invalid" {
        return io::stdout().write_all(b"invalid");
    }
    if host == "gai-error.invalid" {
        return io::stdout().write_all(&protocol::error_response(libc::EAI_NONAME, 0));
    }
    if host == "empty.invalid" {
        return io::stdout().write_all(&protocol::response(&[]).unwrap());
    }
    let records = vec![
        SocketAddr::V6(SocketAddrV6::new(Ipv6Addr::LOCALHOST, 7, 17, 4)),
        SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 9)),
    ];
    io::stdout().write_all(&protocol::response(&records).unwrap())?;
    if host.ends_with("%delayed") || host.ends_with("%eof-hold") {
        // Safe fixture-only pipe detachment: EOF can precede leader exit.
        // No filesystem artifact is created; /dev/null is an existing device.
        io::stdout().flush()?;
        let null = std::fs::OpenOptions::new().write(true).open("/dev/null")?;
        nix::unistd::dup2_stdout(&null).map_err(io::Error::from)?;
        nix::unistd::dup2_stderr(&null).map_err(io::Error::from)?;
        if host.ends_with("%eof-hold") {
            loop {
                std::thread::park();
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    Ok(())
}
