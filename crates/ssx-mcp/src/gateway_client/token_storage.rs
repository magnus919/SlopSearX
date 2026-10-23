//! Lossless local OAuth document storage with one explicitly supervised I/O lane.
use super::{
    io_owner::{IoError, IoOwner},
    token_json::{self, JsonError},
};
use ssx_infra::adapters::source_json::{Map, Number, Text, Value};
use std::{
    fs::{self, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::Arc,
    time::Instant,
};
use tokio_util::sync::CancellationToken;
pub const MAX_TOKEN_FILE_BYTES: usize = token_json::MAX_BYTES;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageError {
    TooLarge,
    InvalidShape,
    NotRegular,
    Encoding,
    Randomness,
    Io,
    Closed,
    Cancelled,
    Deadline,
    Worker,
}
impl From<IoError> for StorageError {
    fn from(e: IoError) -> Self {
        match e {
            IoError::Closed => Self::Closed,
            IoError::Cancelled => Self::Cancelled,
            IoError::Deadline => Self::Deadline,
            IoError::Worker => Self::Worker,
        }
    }
}
impl From<JsonError> for StorageError {
    fn from(e: JsonError) -> Self {
        match e {
            JsonError::Malformed => Self::InvalidShape,
            JsonError::Encoding => Self::Encoding,
            JsonError::Limit => Self::TooLarge,
            JsonError::Deadline => Self::Deadline,
        }
    }
}
#[derive(Debug, Clone, Copy)]
pub enum Slot {
    ClientInfo,
    Tokens,
}
impl Slot {
    fn key(self) -> &'static str {
        match self {
            Self::ClientInfo => "client_info",
            Self::Tokens => "tokens",
        }
    }
}
pub struct FileTokenStorage {
    path: PathBuf,
    data: tokio::sync::Mutex<Map>,
    io: Arc<IoOwner>,
}
impl std::fmt::Debug for FileTokenStorage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("FileTokenStorage(<redacted>)")
    }
}
struct Temporary(PathBuf);
impl Drop for Temporary {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}
fn read_path(path: &Path) -> Result<Option<Vec<u8>>, StorageError> {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NONBLOCK);
    }
    let Ok(file) = options.open(path) else {
        return Ok(None);
    };
    if !file.metadata().map_err(|_| StorageError::Io)?.is_file() {
        return Err(StorageError::NotRegular);
    }
    let mut bytes = Vec::new();
    if file
        .take((MAX_TOKEN_FILE_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .is_err()
    {
        return Ok(None);
    }
    if bytes.len() > MAX_TOKEN_FILE_BYTES {
        return Err(StorageError::TooLarge);
    }
    Ok(Some(bytes))
}
fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), StorageError> {
    if bytes.len() > MAX_TOKEN_FILE_BYTES {
        return Err(StorageError::TooLarge);
    }
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    fs::create_dir_all(parent).map_err(|_| StorageError::Io)?;
    let mut random = [0u8; 16];
    rustls::crypto::aws_lc_rs::default_provider()
        .secure_random
        .fill(&mut random)
        .map_err(|_| StorageError::Randomness)?;
    let suffix: String = random.iter().map(|b| format!("{b:02x}")).collect();
    let temporary_path = parent.join(format!(".slopsearx-oauth-{suffix}.tmp"));
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(&temporary_path)
        .map_err(|_| StorageError::Io)?;
    let temporary = Temporary(temporary_path);
    file.write_all(bytes).map_err(|_| StorageError::Io)?;
    file.sync_all().map_err(|_| StorageError::Io)?;
    drop(file);
    fs::rename(&temporary.0, path).map_err(|_| StorageError::Io)?;
    Ok(())
}
fn truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(v) => *v,
        Value::Number(n @ Number::Integer(_)) => n.integer_text() != Some("0"),
        Value::Number(Number::Float(v)) => *v != 0.0,
        Value::String(v) => !v.is_empty(),
        Value::Array(v) => !v.is_empty(),
        Value::Object(v) => !v.is_empty(),
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModelStorageError {
    Storage(StorageError),
    Model(super::token_model::ModelError),
}
impl FileTokenStorage {
    pub async fn get_tokens(
        &self,
        deadline: Instant,
        cancel: &CancellationToken,
    ) -> Result<Option<super::token_model::TokenModel>, ModelStorageError> {
        let bytes = self
            .slot_json(Slot::Tokens, deadline, cancel)
            .await
            .map_err(ModelStorageError::Storage)?;
        self.io
            .bounded(deadline, cancel, async {
                let value = match bytes {
                    None => None,
                    Some(bytes) => Some(
                        token_json::parse(&bytes, deadline)
                            .await
                            .map_err(|e| ModelStorageError::Storage(e.into()))?,
                    ),
                };
                super::token_model::validate(value.as_ref(), deadline)
                    .await
                    .map_err(ModelStorageError::Model)
            })
            .await
            .map_err(|e| ModelStorageError::Storage(e.into()))?
    }
    pub async fn get_client_info(
        &self,
        deadline: Instant,
        cancel: &CancellationToken,
    ) -> Result<Option<super::client_model::ClientModel>, ModelStorageError> {
        let bytes = self
            .slot_json(Slot::ClientInfo, deadline, cancel)
            .await
            .map_err(ModelStorageError::Storage)?;
        self.io
            .bounded(deadline, cancel, async {
                let value = match bytes {
                    None => None,
                    Some(bytes) => Some(
                        token_json::parse(&bytes, deadline)
                            .await
                            .map_err(|e| ModelStorageError::Storage(e.into()))?,
                    ),
                };
                super::client_model::validate(value.as_ref(), deadline)
                    .await
                    .map_err(ModelStorageError::Model)
            })
            .await
            .map_err(|e| ModelStorageError::Storage(e.into()))?
    }

    /// Caller constructs and retains I/O owner before async startup; cancellation
    /// leaves actual file work there for explicit shutdown/join. No OS hard-stop.
    pub async fn load(
        path: PathBuf,
        io: Arc<IoOwner>,
        deadline: Instant,
        cancel: &CancellationToken,
    ) -> Result<Self, StorageError> {
        let read = path.clone();
        let bytes = io.run(deadline, cancel, move || read_path(&read)).await??;
        let data = if let Some(bytes) = bytes {
            match io
                .bounded(deadline, cancel, token_json::parse(&bytes, deadline))
                .await?
            {
                Ok(Value::Object(data)) => data,
                Err(JsonError::Malformed) => Map::new(),
                Err(error) => return Err(error.into()),
                _ => return Err(StorageError::InvalidShape),
            }
        } else {
            Map::new()
        };
        io.check(deadline, cancel)?;
        Ok(Self {
            path,
            data: tokio::sync::Mutex::new(data),
            io,
        })
    }
    /// Raw model document for the separately qualified SDK schema adapter.
    /// Unknown root values remain in the owned SourceJSON map, never projected.
    pub async fn slot_json(
        &self,
        slot: Slot,
        deadline: Instant,
        cancel: &CancellationToken,
    ) -> Result<Option<Vec<u8>>, StorageError> {
        let data = self.io.bounded(deadline, cancel, self.data.lock()).await?;
        let Some(value) = data.get(slot.key()).filter(|v| truthy(v)) else {
            return Ok(None);
        };
        Ok(Some(
            self.io
                .bounded(deadline, cancel, token_json::encode(value, deadline))
                .await??,
        ))
    }
    /// Input is a validated model object's JSON, not an arbitrary SDK/internal
    /// StoredCredentials schema. Source field/default mapping is a separate port.
    pub async fn set_slot_json(
        &self,
        slot: Slot,
        bytes: &[u8],
        deadline: Instant,
        cancel: &CancellationToken,
    ) -> Result<(), StorageError> {
        let value = self
            .io
            .bounded(deadline, cancel, token_json::parse(bytes, deadline))
            .await??;
        if !matches!(value, Value::Object(_)) {
            return Err(StorageError::InvalidShape);
        }
        let mut data = self.io.bounded(deadline, cancel, self.data.lock()).await?;
        data.insert(Text::from(slot.key()), value);
        // Borrow the owned map through encoding: cancellation releases the guard
        // without moving/dropping the credential document or its unknown fields.
        let bytes = self
            .io
            .bounded(deadline, cancel, token_json::encode_map(&data, deadline))
            .await??;
        let path = self.path.clone();
        self.io
            .run(deadline, cancel, move || atomic_write(&path, &bytes))
            .await??;
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    struct Directory(PathBuf);
    impl Directory {
        fn new() -> Self {
            let mut b = [0; 16];
            rustls::crypto::aws_lc_rs::default_provider()
                .secure_random
                .fill(&mut b)
                .unwrap();
            let s: String = b.iter().map(|b| format!("{b:02x}")).collect();
            let path = std::env::temp_dir().join(format!("ssx-t07-lossless-{s}"));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for Directory {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }
    fn deadline() -> Instant {
        Instant::now() + std::time::Duration::from_secs(10)
    }
    const TOKEN:&[u8]=br#"{"access_token":"synthetic","token_type":"Bearer","expires_in":null,"scope":null,"refresh_token":null}"#;
    #[tokio::test]
    async fn actual_private_roundtrip_unknowns_and_old_open_reader() {
        let dir = Directory::new();
        let path = dir.0.join("tokens.json");
        let io = Arc::new(IoOwner::new());
        let cancel = CancellationToken::new();
        let end = deadline();
        fs::write(&path,b"{\"unknown\":NaN,\"surrogate\":\"\\ud800\",\"huge\":100000000000000000000000000000000000001}").unwrap();
        let store = FileTokenStorage::load(path.clone(), io.clone(), end, &cancel)
            .await
            .unwrap();
        let before = fs::read(&path).unwrap();
        let mut reader = File::open(&path).unwrap();
        #[cfg(unix)]
        let inode = {
            use std::os::unix::fs::MetadataExt;
            reader.metadata().unwrap().ino()
        };
        store
            .set_slot_json(Slot::Tokens, TOKEN, end, &cancel)
            .await
            .unwrap();
        let mut old = Vec::new();
        reader.read_to_end(&mut old).unwrap();
        assert_eq!(old, before);
        #[cfg(unix)]
        {
            use std::os::unix::fs::{MetadataExt, PermissionsExt};
            assert_ne!(inode, fs::metadata(&path).unwrap().ino());
            assert_eq!(
                fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
        let current = fs::read(&path).unwrap();
        let text = std::str::from_utf8(&current).unwrap();
        assert!(text.contains("\"unknown\": NaN"));
        assert!(text.contains("\\ud800"));
        assert!(text.contains("100000000000000000000000000000000000001"));
        let reloaded = FileTokenStorage::load(path.clone(), io.clone(), end, &cancel)
            .await
            .unwrap();
        assert_eq!(
            store.slot_json(Slot::Tokens, end, &cancel).await.unwrap(),
            reloaded
                .slot_json(Slot::Tokens, end, &cancel)
                .await
                .unwrap()
        );
        assert!(!format!("{store:?}").contains("synthetic"));
        assert_eq!(fs::read_dir(&dir.0).unwrap().count(), 1);
        io.shutdown().await.unwrap();
    }
    #[tokio::test]
    async fn failed_encode_and_failed_rename_preserve_original_and_cleanup() {
        let dir = Directory::new();
        let path = dir.0.join("tokens.json");
        let io = Arc::new(IoOwner::new());
        let cancel = CancellationToken::new();
        let end = deadline();
        let store = FileTokenStorage::load(path.clone(), io.clone(), end, &cancel)
            .await
            .unwrap();
        store
            .set_slot_json(Slot::Tokens, TOKEN, end, &cancel)
            .await
            .unwrap();
        let original = fs::read(&path).unwrap();
        // Input remains below cap while ensure_ascii output expands beyond it.
        let mut huge = b"{\"access_token\":\"".to_vec();
        for _ in 0..200_000 {
            huge.extend_from_slice("雪".as_bytes());
        }
        huge.extend_from_slice(b"\"}");
        assert!(huge.len() < MAX_TOKEN_FILE_BYTES);
        assert_eq!(
            store.set_slot_json(Slot::Tokens, &huge, end, &cancel).await,
            Err(StorageError::TooLarge)
        );
        assert_eq!(fs::read(&path).unwrap(), original);
        assert_eq!(fs::read_dir(&dir.0).unwrap().count(), 1);
        let other = dir.0.join("destination");
        let other_store = FileTokenStorage::load(other.clone(), io.clone(), end, &cancel)
            .await
            .unwrap();
        fs::create_dir(&other).unwrap();
        assert_eq!(
            other_store
                .set_slot_json(Slot::Tokens, TOKEN, end, &cancel)
                .await,
            Err(StorageError::Io)
        );
        assert_eq!(fs::read_dir(&dir.0).unwrap().count(), 2);
        io.shutdown().await.unwrap();
    }
    #[tokio::test]
    async fn text_decode_and_caught_json_errors_remain_distinct() {
        let dir = Directory::new();
        let path = dir.0.join("tokens.json");
        let io = Arc::new(IoOwner::new());
        let cancel = CancellationToken::new();
        let end = deadline();
        for bytes in [b"not-json".as_slice(), b"\xef\xbb\xbf{}".as_slice()] {
            fs::write(&path, bytes).unwrap();
            let store = FileTokenStorage::load(path.clone(), io.clone(), end, &cancel)
                .await
                .unwrap();
            assert_eq!(
                store.slot_json(Slot::Tokens, end, &cancel).await.unwrap(),
                None
            );
        }
        fs::write(&path, [0xff, 0xfe, b'{', 0, b'}', 0]).unwrap();
        assert_eq!(
            FileTokenStorage::load(path.clone(), io.clone(), end, &cancel)
                .await
                .unwrap_err(),
            StorageError::Encoding
        );
        fs::write(&path, b"[]").unwrap();
        assert_eq!(
            FileTokenStorage::load(path.clone(), io.clone(), end, &cancel)
                .await
                .unwrap_err(),
            StorageError::InvalidShape
        );
        fs::write(&path, vec![b' '; MAX_TOKEN_FILE_BYTES + 1]).unwrap();
        assert_eq!(
            FileTokenStorage::load(path, io.clone(), end, &cancel)
                .await
                .unwrap_err(),
            StorageError::TooLarge
        );
        io.shutdown().await.unwrap();
    }
    #[tokio::test]
    async fn concurrent_atomic_writers_and_continuous_reader_have_no_partial_publication() {
        let dir = Directory::new();
        let path = dir.0.join("tokens.json");
        atomic_write(&path, b"{\"tokens\":{\"access_token\":\"initial\"}}").unwrap();
        let done = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let (p, d) = (path.clone(), done.clone());
        let (started, ready) = tokio::sync::oneshot::channel();
        let reader = tokio::task::spawn_blocking(move || {
            let mut reads = 0;
            let _ = started.send(());
            while !d.load(std::sync::atomic::Ordering::SeqCst) {
                let value: serde_json::Value =
                    serde_json::from_slice(&fs::read(&p).unwrap()).unwrap();
                assert!(value["tokens"]["access_token"].is_string());
                reads += 1;
            }
            reads
        });
        tokio::time::timeout(std::time::Duration::from_secs(3), ready)
            .await
            .unwrap()
            .unwrap();
        let mut writers = Vec::new();
        for id in 0..4 {
            let p = path.clone();
            writers.push(tokio::spawn(async move {
                let io = Arc::new(IoOwner::new());
                let cancel = CancellationToken::new();
                let end = deadline();
                let store = FileTokenStorage::load(p, io.clone(), end, &cancel)
                    .await
                    .unwrap();
                let token = format!("{{\"access_token\":\"synthetic-{id}\"}}");
                for _ in 0..16 {
                    store
                        .set_slot_json(Slot::Tokens, token.as_bytes(), end, &cancel)
                        .await
                        .unwrap();
                }
                io.shutdown().await.unwrap();
            }));
        }
        for writer in writers {
            writer.await.unwrap();
        }
        done.store(true, std::sync::atomic::Ordering::SeqCst);
        assert!(reader.await.unwrap() > 0);
        assert_eq!(fs::read_dir(&dir.0).unwrap().count(), 1);
    }
    #[tokio::test]
    async fn cancelled_actual_write_retains_worker_and_has_explicit_completion() {
        let dir = Directory::new();
        let path = dir.0.join("tokens.json");
        atomic_write(&path, b"old-complete").unwrap();
        let io = Arc::new(IoOwner::new());
        let cancel = CancellationToken::new();
        let (started, ready) = tokio::sync::oneshot::channel();
        let (release, hold) = std::sync::mpsc::channel();
        let (o, p, c) = (io.clone(), path.clone(), cancel.clone());
        let waiter = tokio::spawn(async move {
            o.run(deadline(), &c, move || {
                let _ = started.send(());
                hold.recv().unwrap();
                atomic_write(&p, b"new-complete")
            })
            .await
        });
        tokio::time::timeout(std::time::Duration::from_secs(3), ready)
            .await
            .unwrap()
            .unwrap();
        cancel.cancel();
        assert_eq!(waiter.await.unwrap(), Err(IoError::Cancelled));
        assert_eq!(fs::read(&path).unwrap(), b"old-complete");
        let mut drain = Box::pin(io.shutdown());
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(20), drain.as_mut())
                .await
                .is_err()
        );
        drop(drain);
        release.send(()).unwrap();
        tokio::time::timeout(std::time::Duration::from_secs(3), io.shutdown())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"new-complete");
        assert_eq!(fs::read_dir(&dir.0).unwrap().count(), 1);
    }
    #[cfg(unix)]
    #[test]
    fn special_files_are_rejected_without_waiting_for_fifo_writer() {
        if let Some(path) = std::env::var_os("SSX_T07_FIFO_CHILD") {
            assert_eq!(
                read_path(Path::new(&path)).unwrap_err(),
                StorageError::NotRegular
            );
            return;
        }
        let dir = Directory::new();
        let path = dir.0.join("fifo");
        assert!(
            std::process::Command::new("mkfifo")
                .arg(&path)
                .status()
                .unwrap()
                .success()
        );
        let name = "gateway_client::token_storage::tests::special_files_are_rejected_without_waiting_for_fifo_writer";
        let mut child = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", name, "--nocapture"])
            .env("SSX_T07_FIFO_CHILD", &path)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        let until = Instant::now() + std::time::Duration::from_secs(3);
        loop {
            if child.try_wait().unwrap().is_some() {
                break;
            }
            if Instant::now() >= until {
                let pid = child.id();
                child.kill().unwrap();
                let output = child.wait_with_output().unwrap();
                panic!("FIFO child {pid} killed/reaped: {:?}", output.status);
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        let output = child.wait_with_output().unwrap();
        let stdout = String::from_utf8(output.stdout).unwrap();
        assert!(output.status.success(), "{stdout}");
        assert!(
            stdout.lines().any(|s| s == format!("test {name} ... ok")),
            "{stdout}"
        );
        assert_eq!(read_path(&dir.0).unwrap_err(), StorageError::NotRegular);
        let regular = dir.0.join("regular");
        fs::write(&regular, b"{}").unwrap();
        let link = dir.0.join("link");
        std::os::unix::fs::symlink(regular, &link).unwrap();
        assert_eq!(read_path(&link).unwrap(), Some(b"{}".to_vec()));
    }
}
#[cfg(test)]
mod source_document_controls {
    use super::*;
    fn hex(text: &str) -> Vec<u8> {
        assert_eq!(text.len() % 2, 0);
        (0..text.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap())
            .collect()
    }
    #[tokio::test]
    async fn original_twenty_one_model_documents_are_lossless_storage_inputs() {
        // This tests persistence/serialization, not SDK model validation. Invalid
        // model cases remain pending in the separate model-adapter case map.
        let end = Instant::now() + std::time::Duration::from_secs(10);
        let cancel = CancellationToken::new();
        let fixture = ssx_infra::adapters::source_json::from_slice(
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../docs/rust-rewrite/t07/token-model-source.json"
            )),
            Some(end),
        )
        .await
        .unwrap();
        let rows = fixture.get("cases").and_then(Value::as_array).unwrap();
        assert_eq!(rows.len(), 21);
        let mut seen = std::collections::BTreeSet::new();
        let mut random = [0; 16];
        rustls::crypto::aws_lc_rs::default_provider()
            .secure_random
            .fill(&mut random)
            .unwrap();
        let suffix: String = random.iter().map(|b| format!("{b:02x}")).collect();
        let directory = std::env::temp_dir().join(format!("ssx-t07-source-docs-{suffix}"));
        fs::create_dir(&directory).unwrap();
        struct Cleanup(PathBuf);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                fs::remove_dir_all(&self.0).unwrap();
            }
        }
        let _cleanup = Cleanup(directory.clone());
        let path = directory.join("tokens.json");
        let io = Arc::new(IoOwner::new());
        for row in rows {
            let id = row.get("id").and_then(Value::as_str).unwrap();
            assert!(seen.insert(id.to_owned()));
            let original = hex(row
                .get("source_json_utf8_hex")
                .and_then(Value::as_str)
                .unwrap());
            fs::write(&path, &original).unwrap();
            let store = FileTokenStorage::load(path.clone(), io.clone(), end, &cancel)
                .await
                .unwrap();
            let expected = token_json::parse(&original, end).await.unwrap();
            let actual = store.data.lock().await;
            assert_eq!(
                token_json::encode_map(&actual, end).await.unwrap(),
                token_json::encode(&expected, end).await.unwrap(),
                "{id}"
            );
            drop(actual);
            if let Some(after) = row.get("after_model_save_utf8_hex").and_then(Value::as_str) {
                store.set_slot_json(Slot::Tokens,br#"{"access_token":"synthetic-updated","token_type":"Bearer","expires_in":null,"scope":null,"refresh_token":null}"#,end,&cancel).await.unwrap();
                assert_eq!(
                    fs::read(&path).unwrap(),
                    hex(after),
                    "exact original update bytes: {id}"
                );
            }
            println!("T07_STORAGE_INPUT {}", id);
        }
        assert_eq!(seen.len(), 21);
        io.shutdown().await.unwrap();
    }
}
#[cfg(test)]
mod actual_model_getters {
    use super::*;
    #[tokio::test]
    async fn original_model_controls_through_actual_file_storage_and_both_getters() {
        let deadline = Instant::now() + std::time::Duration::from_secs(10);
        let fixture = token_json::parse(
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../docs/rust-rewrite/t07/token-model-source.json"
            )),
            deadline,
        )
        .await
        .unwrap();
        let cases = fixture.get("cases").unwrap().as_array().unwrap();
        assert_eq!(cases.len(), 21);
        let directory =
            std::env::temp_dir().join(format!("t07-model-getters-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&directory).unwrap();
        let path = directory.join("tokens.json");
        let io = Arc::new(IoOwner::new());
        let cancel = CancellationToken::new();
        for case in cases {
            let id = case.get("id").unwrap().as_str().unwrap();
            let hex = case.get("source_json_utf8_hex").unwrap().as_str().unwrap();
            let (pairs, rest) = hex.as_bytes().as_chunks::<2>();
            assert!(rest.is_empty());
            let bytes: Vec<u8> = pairs
                .iter()
                .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
                .collect();
            fs::write(&path, &bytes).unwrap();
            let store = FileTokenStorage::load(path.clone(), io.clone(), deadline, &cancel)
                .await
                .unwrap();
            let token = store
                .get_tokens(deadline, &cancel)
                .await
                .map(|m| m.map(|m| m.json().to_vec()));
            let client = store
                .get_client_info(deadline, &cancel)
                .await
                .map(|m| m.map(|m| m.json().to_vec()));
            for (name, result) in [("get_tokens", token), ("get_client_info", client)] {
                let expected = case.get(name).unwrap();
                if expected.get("error_type").is_some() {
                    assert!(
                        matches!(
                            result,
                            Err(ModelStorageError::Model(
                                super::super::token_model::ModelError::Validation
                            ))
                        ),
                        "{id}/{name}"
                    );
                } else {
                    let actual = match result.unwrap() {
                        None => Value::Null,
                        Some(bytes) => token_json::parse(&bytes, deadline).await.unwrap(),
                    };
                    assert_eq!(&actual, expected.get("value").unwrap(), "{id}/{name}");
                }
                println!("T07_MODEL_GETTER {id}/{name}");
            }
            assert_eq!(fs::read(&path).unwrap(), bytes);
        }
        io.shutdown().await.unwrap();
        fs::remove_dir_all(directory).unwrap();
    }
}
