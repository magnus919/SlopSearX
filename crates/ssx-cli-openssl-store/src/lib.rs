//! CLI-private safe boundary for vetted OpenSSL store-directory lookup.
//!
//! CPython 3.13.6 / OpenSSL 3.0.16 registers SSL_CERT_DIR as one raw store URI,
//! in addition to hash_dir's independently split path list. The store provider
//! admits a matching hash.1 even when hash.0 is absent. OpenSSL 3.6 no longer
//! registers that default URI. Register it explicitly without enumerating PEM,
//! computing subject hashes, changing verification parameters, or owning store.
use openssl::{error::ErrorStack, x509::store::X509StoreBuilderRef};
use std::ffi::{CString, OsStr};

#[derive(Debug)]
pub enum DirectoryError {
    InteriorNul,
    PathEncoding,
    OpenSsl(ErrorStack),
}

/// Add exactly one raw URI. OpenSSL copies it and owns its lookup state.
/// The mutable borrow prevents concurrent use; callers retain the store owner.
/// An unavailable URI returns an error, like OpenSSL's store loader. CPython's
/// default-path setup ignores such load errors, leaving hash_dir usable.
pub fn add_source_directory(
    store: &mut X509StoreBuilderRef,
    raw_directory: &str,
) -> Result<(), DirectoryError> {
    add_source_directory_os(store, OsStr::new(raw_directory))
}
fn encoded(path: &OsStr) -> Result<CString, DirectoryError> {
    #[cfg(unix)]
    let bytes = {
        use std::os::unix::ffi::OsStrExt;
        path.as_bytes()
    };
    #[cfg(windows)]
    let bytes = path
        .to_str()
        .ok_or(DirectoryError::PathEncoding)?
        .as_bytes();
    CString::new(bytes).map_err(|_| DirectoryError::InteriorNul)
}
/// Preserve Unix filesystem bytes; reject NUL before FFI. On Windows OpenSSL's
/// narrow path API receives valid UTF8 only; undecodable UTF16 fails closed.
pub fn add_source_directory_os(
    store: &mut X509StoreBuilderRef,
    path: &OsStr,
) -> Result<(), DirectoryError> {
    ffi::load(store, &encoded(path)?, ffi::Load::Store).map_err(DirectoryError::OpenSsl)
}
/// OpenSSL default FILE loading, without rust-openssl's Path::to_str().unwrap().
pub fn load_source_file(
    store: &mut X509StoreBuilderRef,
    path: &OsStr,
) -> Result<(), DirectoryError> {
    ffi::load(store, &encoded(path)?, ffi::Load::File).map_err(DirectoryError::OpenSsl)
}
/// Vetted OpenSSL hash-directory/path-list lookup; no PEM enumeration or hashing.
pub fn add_source_hash_directory(
    store: &mut X509StoreBuilderRef,
    path: &OsStr,
) -> Result<(), DirectoryError> {
    ffi::load(store, &encoded(path)?, ffi::Load::Directory).map_err(DirectoryError::OpenSsl)
}

// Only this module permits unsafe code. openssl-sys links the exact statically
// built OpenSSL; rust-openssl 0.10.81 lacks this one store URI binding.
#[allow(unsafe_code)]
mod ffi {
    use super::*;
    use foreign_types::ForeignTypeRef;
    use std::ffi::{CStr, c_char, c_int};
    unsafe extern "C" {
        fn X509_STORE_load_file(store: *mut openssl_sys::X509_STORE, path: *const c_char) -> c_int;
        fn X509_STORE_load_path(store: *mut openssl_sys::X509_STORE, path: *const c_char) -> c_int;
        fn X509_STORE_load_store(store: *mut openssl_sys::X509_STORE, uri: *const c_char) -> c_int;
    }
    pub(super) enum Load {
        File,
        Directory,
        Store,
    }
    pub(super) fn load(
        store: &mut X509StoreBuilderRef,
        uri: &CStr,
        kind: Load,
    ) -> Result<(), ErrorStack> {
        // SAFETY: rust-openssl owns a live initialized X509_STORE for the entire
        // exclusive borrow. CStr supplies a live NUL-terminated URI for this
        // synchronous call. OpenSSL copies the URI; it neither takes ownership
        // nor frees store/URI. No pointer escapes this wrapper. On failure,
        // ErrorStack::get drains OpenSSL's thread-local error queue.
        if unsafe {
            match kind {
                Load::File => X509_STORE_load_file(store.as_ptr(), uri.as_ptr()),
                Load::Directory => X509_STORE_load_path(store.as_ptr(), uri.as_ptr()),
                Load::Store => X509_STORE_load_store(store.as_ptr(), uri.as_ptr()),
            }
        } == 1
        {
            Ok(())
        } else {
            Err(ErrorStack::get())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use openssl::{
        stack::Stack,
        x509::{X509, X509StoreContext, store::X509StoreBuilder},
    };
    struct OwnedDirectory(std::path::PathBuf);
    impl OwnedDirectory {
        fn new() -> Self {
            static SEQUENCE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            for _ in 0..32 {
                let sequence = SEQUENCE.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                let path = std::env::temp_dir().join(format!(
                    "ssx-store-owned-{}-{now}-{sequence}",
                    std::process::id()
                ));
                match std::fs::create_dir(&path) {
                    Ok(()) => return Self(path),
                    Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                    Err(error) => panic!("owned fixture directory: {error}"),
                }
            }
            panic!("owned fixture directory collision budget");
        }
    }
    impl Drop for OwnedDirectory {
        fn drop(&mut self) {
            if let Err(error) = std::fs::remove_dir_all(&self.0)
                && !std::thread::panicking()
            {
                panic!("owned fixture cleanup: {error}");
            }
        }
    }
    const CA: &[u8] = include_bytes!("../../../docs/rust-rewrite/t08a/tls/ca.pem");
    const LEAF: &[u8] = include_bytes!("../../../docs/rust-rewrite/t08a/tls/server.pem");
    fn usable_after_error(mut store: X509StoreBuilder) {
        store.add_cert(X509::from_pem(CA).unwrap()).unwrap();
        assert_eq!(store.build().all_certificates().len(), 1);
    }
    #[test]
    fn interior_nul_is_rejected_without_mutating_or_freeing_store() {
        let mut store = X509StoreBuilder::new().unwrap();
        assert!(matches!(
            add_source_directory(&mut store, "not\0a-uri"),
            Err(DirectoryError::InteriorNul)
        ));
        assert!(matches!(
            load_source_file(&mut store, OsStr::new("not\0a-file")),
            Err(DirectoryError::InteriorNul)
        ));
        assert!(matches!(
            add_source_hash_directory(&mut store, OsStr::new("not\0a-path")),
            Err(DirectoryError::InteriorNul)
        ));
        usable_after_error(store);
    }
    #[test]
    fn unavailable_uri_returns_native_error_without_owning_store() {
        let mut store = X509StoreBuilder::new().unwrap();
        match add_source_directory(&mut store, "ssx-unregistered-scheme://fixture") {
            Err(DirectoryError::OpenSsl(error)) => assert!(!error.errors().is_empty()),
            value => panic!("unexpected URI result: {value:?}"),
        }
        usable_after_error(store);
    }
    #[test]
    fn copied_uri_outlives_input_and_lazily_loads_matching_dot_one() {
        let owned = OwnedDirectory::new();
        let directory = &owned.0;
        let ca = X509::from_pem(CA).unwrap();
        let hash = ca.subject_name_hash();
        assert!(!directory.join(format!("{hash:08x}.0")).exists());
        std::fs::write(directory.join(format!("{hash:08x}.1")), CA).unwrap();
        let mut store = X509StoreBuilder::new().unwrap();
        let uri = directory.to_str().unwrap().to_owned();
        add_source_directory(&mut store, &uri).unwrap();
        drop(uri);
        let store = store.build();
        assert_eq!(store.all_certificates().len(), 0);
        let leaf = X509::from_pem(LEAF).unwrap();
        let chain = Stack::new().unwrap();
        let mut context = X509StoreContext::new().unwrap();
        assert!(
            context
                .init(&store, &leaf, &chain, |ctx| ctx.verify_cert())
                .unwrap()
        );
        drop(context);
        drop(store);
        let retired_path = directory.clone();
        drop(owned);
        assert!(!retired_path.exists());
    }
    #[cfg(unix)]
    #[test]
    fn owned_os_file_and_hash_directory_preserve_bytes_and_validation() {
        #[cfg(not(target_os = "macos"))]
        use std::os::unix::ffi::OsStrExt;
        let owned = OwnedDirectory::new();
        #[cfg(not(target_os = "macos"))]
        let name = OsStr::from_bytes(b"raw-\xff");
        #[cfg(target_os = "macos")]
        let name = OsStr::new("raw-unicode-☃");
        let path = owned.0.join(name);
        std::fs::create_dir(&path).unwrap();
        let file = path.join("ca.pem");
        std::fs::write(&file, CA).unwrap();
        let ca = X509::from_pem(CA).unwrap();
        std::fs::write(path.join(format!("{:08x}.0", ca.subject_name_hash())), CA).unwrap();
        for directory in [false, true] {
            let mut store = X509StoreBuilder::new().unwrap();
            if directory {
                add_source_hash_directory(&mut store, path.as_os_str()).unwrap();
            } else {
                load_source_file(&mut store, file.as_os_str()).unwrap();
            }
            let store = store.build();
            let leaf = X509::from_pem(LEAF).unwrap();
            let chain = Stack::new().unwrap();
            let mut context = X509StoreContext::new().unwrap();
            assert!(
                context
                    .init(&store, &leaf, &chain, |ctx| ctx.verify_cert())
                    .unwrap()
            );
        }
    }
}
