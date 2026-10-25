//! Package-owned raw input pump. No command/data protocol or fixture modes.
#[cfg(windows)]
fn main() -> std::io::Result<()> {
    use std::io::{Read, Write};
    let mut input = std::io::stdin().lock();
    let mut output = std::io::stdout().lock();
    let mut buffer = [0u8; 4096];
    loop {
        let n = input.read(&mut buffer)?;
        if n == 0 {
            return output.flush();
        }
        output.write_all(&buffer[..n])?;
        output.flush()?;
    }
}
#[cfg(not(windows))]
fn main() -> std::io::Result<()> {
    Err(std::io::ErrorKind::Unsupported.into())
}
