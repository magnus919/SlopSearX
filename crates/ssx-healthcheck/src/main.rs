//! Fixed local container probe. No environment, interpreter, or backend imports.
use std::{
    net::{Ipv4Addr, SocketAddr},
    process::ExitCode,
    time::Duration,
};
fn main() -> ExitCode {
    match ssx_healthcheck::probe(
        SocketAddr::from((Ipv4Addr::LOCALHOST, 8080)),
        Duration::from_secs(4),
    ) {
        Ok(()) => ExitCode::SUCCESS,
        Err(_) => ExitCode::FAILURE,
    }
}
