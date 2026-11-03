//! Fixed local container probe. No environment, interpreter, or backend imports.
use std::process::ExitCode;
fn main() -> ExitCode {
    match ssx_healthcheck::probe() {
        Ok(()) => ExitCode::SUCCESS,
        Err(_) => ExitCode::FAILURE,
    }
}
