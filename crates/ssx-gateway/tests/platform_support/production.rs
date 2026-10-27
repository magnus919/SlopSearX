//! Test-only explicit inputs; never a production runtime executable override.
use sha2::{Digest, Sha256};
use std::{
    io::Read,
    path::{Path, PathBuf},
};
pub struct Images {
    pub gateway: PathBuf,
    pub dns: PathBuf,
    #[cfg(windows)]
    pub input: PathBuf,
}
fn verified(record: &serde_json::Value, role: &str) -> PathBuf {
    let object = record["artifacts"].as_object().unwrap();
    let artifact = &object[role];
    let path = PathBuf::from(artifact["path"].as_str().unwrap());
    assert!(path.is_absolute());
    let name = format!("{role}{}", std::env::consts::EXE_SUFFIX);
    assert_eq!(path.file_name().unwrap(), std::ffi::OsStr::new(&name));
    let metadata = std::fs::symlink_metadata(&path).unwrap();
    assert!(metadata.is_file() && !metadata.file_type().is_symlink());
    assert_eq!(metadata.len(), artifact["bytes"].as_u64().unwrap());
    assert!(metadata.len() <= 512 * 1024 * 1024);
    let mut file = std::fs::File::open(&path).unwrap();
    let mut digest = Sha256::new();
    let mut buffer = [0; 65536];
    loop {
        let count = file.read(&mut buffer).unwrap();
        if count == 0 {
            break;
        }
        digest.update(&buffer[..count]);
    }
    let actual: String = digest
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    assert_eq!(Some(actual.as_str()), artifact["sha256"].as_str());
    path
}
pub fn manifest(path: &Path) -> Images {
    let file = std::fs::File::open(path).unwrap();
    let mut raw = Vec::new();
    file.take(65537).read_to_end(&mut raw).unwrap();
    assert!(raw.len() <= 65536);
    let record: serde_json::Value = serde_json::from_slice(&raw).unwrap();
    assert_eq!(record["schema"], 1);
    assert_eq!(record["os"], std::env::consts::OS);
    assert_eq!(record["arch"], std::env::consts::ARCH);
    assert_eq!(record["version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(record["features"], "production-default-client-only");
    assert_eq!(record["artifacts"].as_object().unwrap().len(), 3);
    Images {
        gateway: verified(&record, "ssx-gateway"),
        dns: verified(&record, "ssx-gateway-dns"),
        #[cfg(windows)]
        input: verified(&record, "ssx-gateway-input"),
    }
}
pub fn optional() -> Images {
    if let Some(path) = std::env::var_os("SLOPSEARX_PRODUCTION_ARTIFACT_MANIFEST") {
        return manifest(Path::new(&path));
    }
    // Existing non-shipping component qualification keeps its Cargo-built seam.
    Images {
        gateway: env!("CARGO_BIN_EXE_ssx-gateway").into(),
        dns: env!("CARGO_BIN_EXE_ssx-gateway-dns").into(),
        #[cfg(windows)]
        input: env!("CARGO_BIN_EXE_ssx-gateway-input").into(),
    }
}
