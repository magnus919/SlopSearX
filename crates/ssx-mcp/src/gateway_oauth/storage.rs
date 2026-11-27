//! Local credential storage compatible with the Python gateway's OAuth file.

use nix::{
    errno::Errno,
    fcntl::{Flock, FlockArg},
};
use serde::{Deserialize, Serialize};
use serde_json::value::RawValue;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    env,
    fs::{self, DirBuilder, File, Metadata, OpenOptions},
    io::{self, Read, Write},
    os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
};
use uuid::Uuid;

const MAX_FILE_BYTES: usize = 1024 * 1024;

pub enum Section {
    ClientInfo,
    Tokens,
}

impl Section {
    fn key(self) -> &'static str {
        match self {
            Self::ClientInfo => "client_info",
            Self::Tokens => "tokens",
        }
    }
}

/// Preserve fields this client does not interpret, including nested metadata.
/// The document deliberately has no Debug implementation.
#[derive(Clone, Default, Deserialize, Serialize)]
#[serde(transparent)]
pub struct TokenDocument(BTreeMap<String, Box<RawValue>>);

impl TokenDocument {
    pub fn section(&self, section: Section) -> Option<&RawValue> {
        self.0.get(section.key()).map(Box::as_ref)
    }

    pub fn set_section(&mut self, section: Section, value: Box<RawValue>) {
        self.0.insert(section.key().into(), value);
    }
}

pub fn default_path(server_url: &str) -> io::Result<PathBuf> {
    let config = env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| env::home_dir().map(|home| home.join(".config")))
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "OAuth config directory missing"))?;
    Ok(path_under(&config, server_url))
}

fn path_under(config: &Path, server_url: &str) -> PathBuf {
    let digest = Sha256::digest(server_url.as_bytes());
    let basis: String = digest[..8]
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    config
        .join("slopsearx")
        .join("oauth")
        .join(format!("{basis}.json"))
}

pub fn load(path: &Path) -> io::Result<TokenDocument> {
    let file = match OpenOptions::new()
        .read(true)
        .custom_flags(nix::libc::O_NOFOLLOW | nix::libc::O_NONBLOCK)
        .open(path)
    {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Ok(TokenDocument::default());
        }
        Err(error) => return Err(error),
    };
    check_private_file(&file.metadata()?)?;
    let mut data = Vec::new();
    file.take((MAX_FILE_BYTES + 1) as u64)
        .read_to_end(&mut data)?;
    if data.len() > MAX_FILE_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "OAuth token file exceeds 1 MiB",
        ));
    }
    serde_json::from_slice(&data).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "OAuth token file is not a JSON object",
        )
    })
}

pub struct Transaction {
    path: PathBuf,
    _lock: Flock<File>,
    document: TokenDocument,
}

impl Transaction {
    pub fn document(&self) -> &TokenDocument {
        &self.document
    }

    pub fn save(&self, document: &TokenDocument) -> io::Result<()> {
        save(&self.path, document)
    }
}

pub fn try_begin(path: &Path) -> io::Result<Option<Transaction>> {
    let name = path.file_name().ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidInput, "OAuth token file name missing")
    })?;
    let parent = token_parent(path);
    DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(parent)?;
    // The token JSON is replaced by rename. Lock a stable sibling so all
    // gateways still coordinate after the token's inode changes.
    let digest: String = Sha256::digest(name.as_encoded_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    let lock_path = parent.join(format!(".slopsearx-oauth-{digest}.lock"));
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .custom_flags(nix::libc::O_NOFOLLOW | nix::libc::O_NONBLOCK)
        .open(lock_path)?;
    check_private_file(&file.metadata()?)?;
    let lock = match Flock::lock(file, FlockArg::LockExclusiveNonblock) {
        Ok(lock) => lock,
        Err((_, Errno::EWOULDBLOCK)) => return Ok(None),
        Err((_, error)) => return Err(error.into()),
    };
    let document = load(path)?;
    Ok(Some(Transaction {
        path: path.to_owned(),
        _lock: lock,
        document,
    }))
}

fn token_parent(path: &Path) -> &Path {
    path.parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or(Path::new("."))
}

fn save(path: &Path, document: &TokenDocument) -> io::Result<()> {
    let data = serde_json::to_vec(document).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "Could not encode OAuth token file",
        )
    })?;
    if data.len() > MAX_FILE_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "OAuth token file exceeds 1 MiB",
        ));
    }
    match fs::symlink_metadata(path) {
        Ok(meta) => check_private_file(&meta)?,
        Err(error) if error.kind() == io::ErrorKind::NotFound => (),
        Err(error) => return Err(error),
    }
    let parent = token_parent(path);

    let tmp = parent.join(format!(".slopsearx-oauth-{}.tmp", Uuid::new_v4()));
    let mut file: File = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&tmp)?;
    let result = (|| {
        file.write_all(&data)?;
        file.sync_all()?;
        fs::rename(&tmp, path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(tmp);
    }
    result
}

fn check_private_file(meta: &Metadata) -> io::Result<()> {
    if !meta.is_file() || meta.permissions().mode() & 0o077 != 0 {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "OAuth token path must be a private regular file",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::os::unix::fs::symlink;

    struct Scratch(PathBuf);

    impl Scratch {
        fn new() -> io::Result<Self> {
            let base = env::var_os("SLOPSEARX_TEST_TEMP_ROOT")
                .map(PathBuf::from)
                .unwrap_or_else(env::temp_dir);
            let path = base.join(format!("ssx-oauth-{}", Uuid::new_v4()));
            DirBuilder::new().mode(0o700).create(&path)?;
            Ok(Self(path))
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn uses_python_default_path() {
        assert_eq!(
            path_under(Path::new("/fixture/config"), "https://example.test/mcp"),
            Path::new("/fixture/config/slopsearx/oauth/5cbd7e2fea4eb608.json")
        );
    }

    #[test]
    fn updates_python_tokens_and_preserves_other_fields() -> crate::Result<()> {
        let scratch = Scratch::new()?;
        let path = scratch.0.join("oauth").join("server.json");
        assert!(load(&path)?.section(Section::Tokens).is_none());
        let original = json!({
            "client_info": {"client_id": "registered-client", "redirect_uris": ["http://127.0.0.1:8765/callback"]},
            "tokens": {"token_type": "Bearer", "access_token": "old", "refresh_token": "refresh"},
            "future_extension": {"untouched": [1, true, null]}
        });
        let transaction = try_begin(&path)?.ok_or("OAuth test lock unexpectedly busy")?;
        transaction.save(&TokenDocument::default())?;
        drop(transaction);
        // Python serializes the complete document into an indented JSON file.
        fs::write(&path, serde_json::to_vec_pretty(&original)?)?;
        let transaction = try_begin(&path)?.ok_or("OAuth test lock unexpectedly busy")?;
        let mut document = transaction.document().clone();
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(
                document.section(Section::ClientInfo).unwrap().get()
            )?,
            original["client_info"]
        );
        let fresh =
            json!({"token_type": "Bearer", "access_token": "new", "refresh_token": "refresh"});
        document.set_section(Section::Tokens, serde_json::value::to_raw_value(&fresh)?);
        assert!(try_begin(&path)?.is_none());
        transaction.save(&document)?;
        assert!(try_begin(&path)?.is_none());
        drop(transaction);
        let next = try_begin(&path)?.ok_or("OAuth test lock was not released")?;
        let mut expected = original;
        expected["tokens"] = fresh;
        assert_eq!(serde_json::to_value(next.document())?, expected);
        assert_eq!(fs::metadata(&path)?.permissions().mode() & 0o777, 0o600);
        assert_eq!(
            fs::metadata(path.parent().unwrap())?.permissions().mode() & 0o777,
            0o700
        );
        let mut siblings = fs::read_dir(path.parent().unwrap())?.collect::<io::Result<Vec<_>>>()?;
        siblings.retain(|entry| entry.path() != path);
        assert_eq!(siblings.len(), 1);
        assert_eq!(siblings[0].metadata()?.permissions().mode() & 0o777, 0o600);
        Ok(())
    }

    #[test]
    fn refuses_insecure_paths_and_preserves_existing_file_on_oversize() -> crate::Result<()> {
        use nix::{sys::stat::Mode, unistd::mkfifo};

        let scratch = Scratch::new()?;
        let path = scratch.0.join("server.json");
        let link = scratch.0.join("link.json");
        let fifo = scratch.0.join("fifo.json");
        let mut document = TokenDocument::default();
        save(&path, &document)?;
        symlink(&path, &link)?;
        mkfifo(&fifo, Mode::S_IRUSR | Mode::S_IWUSR)?;
        for unsafe_path in [&link, &fifo] {
            assert!(load(unsafe_path).is_err());
            assert!(save(unsafe_path, &document).is_err());
        }
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644))?;
        assert!(load(&path).is_err());
        assert!(save(&path, &document).is_err());
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600))?;
        let before = fs::read(&path)?;
        document.set_section(
            Section::Tokens,
            serde_json::value::to_raw_value(&"x".repeat(MAX_FILE_BYTES))?,
        );
        assert_eq!(
            save(&path, &document).unwrap_err().kind(),
            io::ErrorKind::InvalidData
        );
        assert_eq!(fs::read(&path)?, before);
        Ok(())
    }
}
