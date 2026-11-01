use ssx_ops::{bounded_file, pinned_bases, verify_bases, verify_platforms};
use std::{path::Path, process::ExitCode};
#[tokio::main]
async fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result=match args.as_slice(){
        [command] if command=="verify-base-digests"=>bases("Dockerfile").await,
        [command,flag,path] if command=="verify-base-digests" && flag=="--dockerfile"=>bases(path).await,
        [command,flag,path] if command=="verify-image-platforms" && flag=="--manifest"=>match bounded_file(Path::new(path),1024*1024).map_err(|_|"cannot read bounded manifest".to_owned()).and_then(|v|verify_platforms(&v)){Ok(())=>Ok(0),Err(e)=>{eprintln!("error: ssx-ops: {e}");Ok(1)}},
        _=>Err("usage: ssx-ops verify-base-digests [--dockerfile PATH] | verify-image-platforms --manifest PATH".into()),
    };
    match result {
        Ok(code) => ExitCode::from(code),
        Err(e) => {
            eprintln!("error: ssx-ops: {e}");
            ExitCode::from(2)
        }
    }
}
async fn bases(path: &str) -> Result<u8, String> {
    let raw = bounded_file(Path::new(path), 64 * 1024)
        .map_err(|_| "cannot read bounded Dockerfile".to_owned())?;
    let source = std::str::from_utf8(&raw).map_err(|_| "Dockerfile is not UTF-8".to_owned())?;
    let bases = pinned_bases(source)?;
    let owner = ssx_infra::http::AdapterHttp::default();
    let result = verify_bases(&owner, &bases).await;
    owner.shutdown_and_wait().await;
    let observations = result?;
    let stale = observations.iter().any(|o| o.base.digest != o.actual);
    for o in observations {
        println!(
            "{}:{} expected={} registry={}",
            o.base.repository, o.base.tag, o.base.digest, o.actual
        );
    }
    if stale {
        eprintln!("Pinned Docker base digest is stale; update tag and digest together.");
    }
    Ok(u8::from(stale))
}
