//! Explicit developer-test instrumentation; absent from default client builds.
use std::{fs::OpenOptions, io::Write};
pub enum Stage {
    ClientBuilt(bool),
    Headers { status: u16, declared: Option<u64> },
    BodyComplete(usize),
    VerificationEntered,
    Certificate { valid: bool, code: i32, depth: u32 },
    Signature { tls: u8, valid: bool },
}
fn append(line: &str) {
    let Some(path) = std::env::var_os("SSX_T08A_DIAGNOSTIC_FILE") else {
        return;
    };
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path)
        && file.metadata().is_ok_and(|m| m.len() < 65536)
    {
        let _ = writeln!(file, "{line}");
    }
}
pub fn record(stage: Stage) {
    append(&match stage {
        Stage::ClientBuilt(ok) => format!("client_built ok={ok}"),
        Stage::Headers { status, declared } => {
            format!("headers status={status} declared={declared:?}")
        }
        Stage::BodyComplete(n) => format!("body_complete bytes={n}"),
        Stage::VerificationEntered => "x509_entered".into(),
        Stage::Certificate { valid, code, depth } => {
            format!("x509 valid={valid} code={code} depth={depth}")
        }
        Stage::Signature { tls, valid } => format!("signature tls={tls} valid={valid}"),
    })
}
pub fn failure(stage: &str, error: &(dyn std::error::Error + 'static)) {
    let mut io = None;
    let mut tls = false;
    let mut current = Some(error);
    while let Some(error) = current {
        if let Some(error) = error.downcast_ref::<std::io::Error>() {
            io = Some(error.kind());
        }
        tls |= error.is::<rustls::Error>();
        current = error.source();
    }
    append(&format!("failure stage={stage} io={io:?} rustls={tls}"));
}
