//! Diagnostic-only copy of the T07 fixture CA recipe at 3947261d.
use rcgen::{BasicConstraints, CertificateParams, DnType, IsCa, KeyPair, KeyUsagePurpose};
use std::env;
use std::fs::OpenOptions;
use std::io::{self, Write};
use std::path::PathBuf;
use std::process;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = env::args_os().skip(1);
    let output = args
        .next()
        .map(PathBuf::from)
        .ok_or("missing CA output path")?;
    if !output.is_absolute() || args.next().is_some() {
        return Err("expected exactly one absolute CA output path".into());
    }

    let key = KeyPair::generate()?;
    let mut params = CertificateParams::new(Vec::<String>::new())?;
    params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
    params.key_usages = vec![
        KeyUsagePurpose::KeyCertSign,
        KeyUsagePurpose::DigitalSignature,
        KeyUsagePurpose::CrlSign,
    ];
    params.distinguished_name.push(
        DnType::CommonName,
        format!("SSX ephemeral platform CA {}", process::id()),
    );
    let certificate = params.self_signed(&key)?;
    let der = certificate.der();
    if der.is_empty() || der.len() > 64 * 1024 {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "invalid fixture size").into());
    }
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)?
        .write_all(der)?;
    Ok(())
}
