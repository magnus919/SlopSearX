//! Bounded additive PEM roots for the installed remote gateway.

use reqwest::Certificate;
use std::{fs::OpenOptions, io::Read, path::Path};

const MAX_CA_BUNDLE_BYTES: usize = 4 * 1024 * 1024;

/// Load a nonempty regular PEM bundle without including its path or parser
/// details in errors. The resulting roots are additive to platform defaults.
pub async fn load_ca_bundle(path: Option<&Path>) -> crate::Result<Vec<Certificate>> {
    let Some(path) = path else {
        return Ok(Vec::new());
    };
    let path = path.to_owned();
    tokio::task::spawn_blocking(move || load_ca_bundle_sync(&path))
        .await
        .map_err(|_| "Could not load gateway CA bundle")?
}

fn load_ca_bundle_sync(path: &Path) -> crate::Result<Vec<Certificate>> {
    use std::os::unix::fs::OpenOptionsExt;
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(nix::libc::O_NOFOLLOW | nix::libc::O_NONBLOCK)
        .open(path)
        .map_err(|_| "Could not read gateway CA bundle")?;
    let metadata = file
        .metadata()
        .map_err(|_| "Could not read gateway CA bundle")?;
    if !metadata.is_file() {
        return Err("Gateway CA bundle must be a regular file".into());
    }
    let mut bytes = Vec::new();
    file.take((MAX_CA_BUNDLE_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|_| "Could not read gateway CA bundle")?;
    if bytes.is_empty() {
        return Err("Gateway CA bundle is empty".into());
    }
    if bytes.len() > MAX_CA_BUNDLE_BYTES {
        return Err("Gateway CA bundle exceeds 4 MiB".into());
    }
    let roots =
        Certificate::from_pem_bundle(&bytes).map_err(|_| "Gateway CA bundle is not valid PEM")?;
    if roots.is_empty() {
        return Err("Gateway CA bundle contains no certificates".into());
    }
    Ok(roots)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        sync::atomic::{AtomicU64, Ordering},
        time::{SystemTime, UNIX_EPOCH},
    };

    static NEXT: AtomicU64 = AtomicU64::new(0);

    struct TempFile(std::path::PathBuf);

    impl TempFile {
        fn new(bytes: &[u8]) -> Self {
            let nonce = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "ssx-gateway-ca-{}-{nonce}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::write(&path, bytes).expect("write temporary CA fixture");
            Self(path)
        }
    }

    impl Drop for TempFile {
        fn drop(&mut self) {
            let _ = fs::remove_file(&self.0);
        }
    }

    #[tokio::test]
    async fn optional_bundle_keeps_default_configuration_path() {
        assert!(load_ca_bundle(None).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn parses_a_valid_pem_root_bundle() {
        const PEM: &[u8] = b"-----BEGIN CERTIFICATE-----\n\
            MIIBtjCCAVugAwIBAgITBmyf1XSXNmY/Owua2eiedgPySjAKBggqhkjOPQQDAjA5\n\
            MQswCQYDVQQGEwJVUzEPMA0GA1UEChMGQW1hem9uMRkwFwYDVQQDExBBbWF6b24g\n\
            Um9vdCBDQSAzMB4XDTE1MDUyNjAwMDAwMFoXDTQwMDUyNjAwMDAwMFowOTELMAkG\n\
            A1UEBhMCVVMxDzANBgNVBAoTBkFtYXpvbjEZMBcGA1UEAxMQQW1hem9uIFJvb3Qg\n\
            Q0EgMzBZMBMGByqGSM49AgEGCCqGSM49AwEHA0IABCmXp8ZBf8ANm+gBG1bG8lKl\n\
            ui2yEujSLtf6ycXYqm0fc4E7O5hrOXwzpcVOho6AF2hiRVd9RFgdszflZwjrZt6j\n\
            QjBAMA8GA1UdEwEB/wQFMAMBAf8wDgYDVR0PAQH/BAQDAgGGMB0GA1UdDgQWBBSr\n\
            ttvXBp43rDCGB5Fwx5zEGbF4wDAKBggqhkjOPQQDAgNJADBGAiEA4IWSoxe3jfkr\n\
            BqWTrBqYaGFy+uGh0PsceGCmQ5nFuMQCIQCcAu/xlJyzlvnrxir4tiz+OpAUFteM\n\
            YyRIHN8wfdVoOw==\n\
            -----END CERTIFICATE-----\n";
        let file = TempFile::new(PEM);
        assert_eq!(load_ca_bundle_sync(&file.0).unwrap().len(), 1);
    }

    #[tokio::test]
    async fn rejects_empty_malformed_and_oversized_files_without_path_leakage() {
        for (bytes, expected) in [
            (Vec::new(), "Gateway CA bundle is empty"),
            (
                b"not a certificate".to_vec(),
                "Gateway CA bundle contains no certificates",
            ),
            (
                b"-----BEGIN CERTIFICATE-----\nnot-base64!\n-----END CERTIFICATE-----".to_vec(),
                "Gateway CA bundle is not valid PEM",
            ),
            (
                vec![b'x'; MAX_CA_BUNDLE_BYTES + 1],
                "Gateway CA bundle exceeds 4 MiB",
            ),
        ] {
            let file = TempFile::new(&bytes);
            let error = load_ca_bundle_sync(&file.0).err().unwrap().to_string();
            assert_eq!(error, expected);
            assert!(!error.contains(file.0.to_str().unwrap()));
        }
    }
}
