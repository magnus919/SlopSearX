//! Required-feature-only lifecycle payload. Never imports or removes certificates.
#[cfg(windows)]
fn main() -> std::io::Result<()> {
    use std::{
        io::{self, Write},
        path::PathBuf,
        time::{Duration, Instant},
    };
    let mut args = std::env::args_os().skip(1);
    if args.next().as_deref() != Some(std::ffi::OsStr::new("--trust-fixture")) {
        return Err(io::ErrorKind::InvalidInput.into());
    }
    let scenario = args.next().ok_or(io::ErrorKind::InvalidInput)?;
    let mode = args.next().ok_or(io::ErrorKind::InvalidInput)?;
    let root = PathBuf::from(args.next().ok_or(io::ErrorKind::InvalidInput)?);
    if args.next().is_some() || !["install", "remove"].iter().any(|m| mode == *m) {
        return Err(io::ErrorKind::InvalidInput.into());
    }
    if !root.is_absolute() {
        return Err(io::ErrorKind::InvalidInput.into());
    }
    ssx_windows_owner::check_private_directory(&root)?;
    if scenario == "hold" {
        let (pid, created) = ssx_windows_owner::fixture_current_process_identity()?;
        std::fs::write(root.join("descendant-ready"), b"entered")?;
        println!("DESCENDANT {pid} {created}");
        io::stdout().flush()?;
        loop {
            std::thread::park_timeout(Duration::from_secs(1));
        }
    }
    std::fs::write(
        root.join(if mode == "install" {
            "entered-install"
        } else {
            "entered-remove"
        }),
        b"entered",
    )?;
    if mode == "install" {
        std::fs::write(root.join("trust-active"), b"FAKE-ONLY\n")?;
    }
    println!("READY");
    io::stdout().flush()?;
    if scenario == "success" {
        let marker = root.join("trust-active");
        if mode == "install" {
            std::fs::write(marker, b"FAKE-ONLY\n")?;
        } else if marker.try_exists()? {
            if std::fs::read(&marker)? != b"FAKE-ONLY\n" {
                return Err(io::ErrorKind::PermissionDenied.into());
            }
            std::fs::remove_file(marker)?;
        }
        return Ok(());
    }
    if scenario == "quiet" {
        loop {
            std::thread::park_timeout(Duration::from_secs(1));
        }
    }
    if scenario == "descendant" {
        let mut child = std::process::Command::new(std::env::current_exe()?)
            .arg("--trust-fixture")
            .arg("hold")
            .arg(&mode)
            .arg(&root)
            .spawn()?;
        let deadline = Instant::now() + Duration::from_secs(3);
        while !root.join("descendant-ready").try_exists()? {
            if Instant::now() >= deadline {
                let _ = child.kill();
                child.wait()?;
                return Err(io::ErrorKind::TimedOut.into());
            }
            std::thread::sleep(Duration::from_millis(2));
        }
        drop(child);
        return Ok(());
    }
    if scenario == "flood-stdout" || scenario == "flood-stderr" {
        let data = [b'F'; 1024];
        if scenario == "flood-stderr" {
            let mut stream = io::stderr().lock();
            loop {
                stream.write_all(&data)?;
                stream.flush()?;
            }
        }
        let mut stream = io::stdout().lock();
        loop {
            stream.write_all(&data)?;
            stream.flush()?;
        }
    }
    if scenario == "flood" {
        io::stderr().write_all(&[b'E'; 1024])?;
        io::stderr().flush()?;
        let writer = std::thread::spawn(|| -> io::Result<()> {
            let data = [b'E'; 1024];
            let mut out = io::stderr().lock();
            loop {
                out.write_all(&data)?;
                out.flush()?;
            }
        });
        let data = [b'O'; 1024];
        let mut out = io::stdout().lock();
        let result = loop {
            if let Err(error) = out.write_all(&data).and_then(|()| out.flush()) {
                break Err(error);
            }
        };
        let _ = writer.join();
        return result;
    }
    if scenario == "fail" {
        std::process::exit(17);
    }
    Err(io::ErrorKind::InvalidInput.into())
}
#[cfg(not(windows))]
fn main() -> std::io::Result<()> {
    Err(std::io::ErrorKind::Unsupported.into())
}
