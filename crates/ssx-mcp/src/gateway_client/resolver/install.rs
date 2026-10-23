//! Trusted package layout for zero-configuration installed helper discovery.
//! Hashes bind opened bytes under stable installation ownership. Path execution
//! still relies on that trust: this is not descriptor-exec or TOCTOU elimination.
use super::SourceResolverHelper;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    io::{self, Read, Write},
    os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
};
const GATEWAY: &str = "bin/ssx-gateway";
const HELPER: &str = "libexec/slopsearx/ssx-gateway-dns";
const RECORD: &str = "share/slopsearx/native-install.json";
const PACKAGE: &str = "slopsearx-native-gateway";
const FILE_LIMIT: u64 = 512 * 1024 * 1024;
/// Complete package-owned record. No path or digest is read from request input,
/// command arguments, PATH, or a user-supplied helper environment variable.
#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct InstallRecord {
    pub schema: u32,
    pub package: String,
    pub version: String,
    pub os: String,
    pub arch: String,
    pub gateway_path: String,
    pub gateway_bytes: u64,
    pub gateway_sha256: String,
    pub helper_path: String,
    pub helper_bytes: u64,
    pub helper_sha256: String,
}
fn invalid() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        "native gateway package identity invalid",
    )
}
fn require(ok: bool) -> io::Result<()> {
    if ok { Ok(()) } else { Err(invalid()) }
}
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
fn decode(value: &str) -> io::Result<[u8; 32]> {
    require(value.len() == 64 && value.bytes().all(|c| c.is_ascii_hexdigit()))?;
    let mut out = [0; 32];
    for (i, b) in out.iter_mut().enumerate() {
        *b = u8::from_str_radix(&value[i * 2..i * 2 + 2], 16).map_err(|_| invalid())?;
    }
    Ok(out)
}
fn same(a: &fs::Metadata, b: &fs::Metadata) -> bool {
    (
        a.dev(),
        a.ino(),
        a.len(),
        a.mtime(),
        a.mtime_nsec(),
        a.uid(),
        a.mode(),
        a.nlink(),
    ) == (
        b.dev(),
        b.ino(),
        b.len(),
        b.mtime(),
        b.mtime_nsec(),
        b.uid(),
        b.mode(),
        b.nlink(),
    )
}
fn regular(path: &Path, uid: Option<u32>, executable: bool, installed: bool) -> io::Result<File> {
    let before = fs::symlink_metadata(path)?;
    require(
        before.is_file()
            && (!installed || (before.nlink() == 1 && before.mode() & 0o022 == 0))
            && uid.is_none_or(|uid| uid == before.uid())
            && (!executable || before.mode() & 0o111 != 0),
    )?;
    let f = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)?;
    require(same(&before, &f.metadata()?))?;
    Ok(f)
}
fn hash(
    path: &Path,
    uid: Option<u32>,
    executable: bool,
    installed: bool,
) -> io::Result<(u64, [u8; 32])> {
    let mut file = regular(path, uid, executable, installed)?;
    let before = file.metadata()?;
    require(before.len() > 0 && before.len() <= FILE_LIMIT)?;
    let mut digest = Sha256::new();
    let mut buffer = [0; 65536];
    let mut count = 0u64;
    loop {
        let n = file.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        count += n as u64;
        require(count <= FILE_LIMIT)?;
        digest.update(&buffer[..n]);
    }
    require(
        count == before.len()
            && same(&before, &file.metadata()?)
            && same(&before, &fs::symlink_metadata(path)?),
    )?;
    Ok((count, digest.finalize().into()))
}
fn directory(path: &Path, uid: u32) -> io::Result<()> {
    let s = fs::symlink_metadata(path)?;
    require(s.is_dir() && s.uid() == uid && s.mode() & 0o022 == 0)
}
impl InstallRecord {
    fn shape(&self) -> io::Result<()> {
        require(
            self.schema == 1
                && self.package == PACKAGE
                && self.version == env!("CARGO_PKG_VERSION")
                && self.os == std::env::consts::OS
                && self.arch == std::env::consts::ARCH
                && self.gateway_path == GATEWAY
                && self.helper_path == HELPER
                && self.gateway_bytes > 0
                && self.helper_bytes > 0,
        )?;
        decode(&self.gateway_sha256)?;
        decode(&self.helper_sha256)?;
        Ok(())
    }
}
/// Validate an actual installed executable's fixed package role and record.
/// Public to let packaging/tests validate a concrete artifact; production main
/// supplies only current_exe(), never a user override or arbitrary fallback.
pub fn discover(executable: &Path) -> io::Result<SourceResolverHelper> {
    require(executable.is_absolute())?;
    let executable = executable.canonicalize()?;
    require(executable.file_name().is_some_and(|n| n == "ssx-gateway"))?;
    let bin = executable.parent().ok_or_else(invalid)?;
    require(bin.file_name().is_some_and(|n| n == "bin"))?;
    let prefix = bin.parent().ok_or_else(invalid)?;
    let uid = regular(&executable, None, true, true)?.metadata()?.uid();
    for name in [
        "",
        "bin",
        "libexec",
        "libexec/slopsearx",
        "share",
        "share/slopsearx",
    ] {
        directory(&prefix.join(name), uid)?;
    }
    let path = prefix.join(RECORD);
    let mut f = regular(&path, Some(uid), false, true)?;
    let before = f.metadata()?;
    require(before.len() <= 16384)?;
    let mut bytes = Vec::new();
    Read::by_ref(&mut f).take(16385).read_to_end(&mut bytes)?;
    require(
        bytes.len() <= 16384
            && same(&before, &f.metadata()?)
            && same(&before, &fs::symlink_metadata(&path)?),
    )?;
    let record: InstallRecord = serde_json::from_slice(&bytes).map_err(|_| invalid())?;
    record.shape()?;
    let (size, digest) = hash(&executable, Some(uid), true, true)?;
    require(size == record.gateway_bytes && digest == decode(&record.gateway_sha256)?)?;
    let helper = prefix.join(HELPER);
    let (size, digest) = hash(&helper, Some(uid), true, true)?;
    require(size == record.helper_bytes && digest == decode(&record.helper_sha256)?)?;
    SourceResolverHelper::new(helper, digest)
}
/// Automatic installed-layout discovery, after main has parsed help/version.
pub fn current_helper() -> io::Result<SourceResolverHelper> {
    discover(&std::env::current_exe()?)
}
fn private_directory(path: &Path) -> io::Result<()> {
    fs::create_dir(path)?;
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))
}
/// Stage a complete package beneath an explicitly selected build/package output
/// root. Existing matching bundles are revalidated, never overwritten. No build,
/// helper process, request or network call is made by this function.
pub fn stage(gateway: &Path, helper: &Path, output_root: &Path) -> io::Result<PathBuf> {
    require(gateway.is_absolute() && helper.is_absolute() && output_root.is_absolute())?;
    // Cargo build inputs may be multiply linked/writable under its build owner.
    // Only copied installed outputs must meet the strict package policy.
    let (gateway_bytes, gateway_digest) = hash(gateway, None, true, false)?;
    let (helper_bytes, helper_digest) = hash(helper, None, true, false)?;
    let record = InstallRecord {
        schema: 1,
        package: PACKAGE.into(),
        version: env!("CARGO_PKG_VERSION").into(),
        os: std::env::consts::OS.into(),
        arch: std::env::consts::ARCH.into(),
        gateway_path: GATEWAY.into(),
        gateway_bytes,
        gateway_sha256: hex(&gateway_digest),
        helper_path: HELPER.into(),
        helper_bytes,
        helper_sha256: hex(&helper_digest),
    };
    let mut data = serde_json::to_vec_pretty(&record).map_err(|_| invalid())?;
    data.push(b'\n');
    require(data.len() <= 16384)?;
    require(fs::symlink_metadata(output_root)?.is_dir())?;
    let root = output_root.canonicalize()?;
    let root_meta = fs::symlink_metadata(&root)?;
    require(root_meta.is_dir() && root_meta.mode() & 0o022 == 0)?;
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW)
        .open(root.join(".gateway-package.lock"))?;
    require(
        lock.metadata()?.is_file()
            && lock.metadata()?.len() == 0
            && lock.metadata()?.nlink() == 1
            && lock.metadata()?.uid() == root_meta.uid()
            && lock.metadata()?.mode() & 0o077 == 0,
    )?;
    lock.lock()?; // Safe std file lock; supported by the pinned Rust1.99 toolchain.
    let _lock = lock; // Lifetime covers validation/staging; close releases lock.
    let prefix = root.join(format!("ssx-gateway-{}", hex(&Sha256::digest(&data))));
    if prefix.exists() {
        directory(&prefix, root_meta.uid())?;
        let mut sealed = regular(&prefix.join(RECORD), Some(root_meta.uid()), false, true)?;
        let mut actual = Vec::new();
        Read::by_ref(&mut sealed)
            .take(16385)
            .read_to_end(&mut actual)?;
        require(actual == data)?; // Existing cache role must match these build inputs.
        discover(&prefix.join(GATEWAY))?;
        return Ok(prefix.join(GATEWAY));
    }
    private_directory(&prefix)?;
    for name in [
        "bin",
        "libexec",
        "libexec/slopsearx",
        "share",
        "share/slopsearx",
    ] {
        private_directory(&prefix.join(name))?;
    }
    // A failed stage stays visible for explicit owned cleanup; no recursive
    // deletion, overwrite, or implicit unchanged retry is performed.
    for (source, name) in [(gateway, GATEWAY), (helper, HELPER)] {
        let mut input = regular(source, None, true, false)?;
        let mut output = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o500)
            .open(prefix.join(name))?;
        let copied = io::copy(
            &mut Read::by_ref(&mut input).take(FILE_LIMIT + 1),
            &mut output,
        )?;
        require(copied <= FILE_LIMIT)?;
        output.sync_all()?;
    }
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o400)
        .open(prefix.join(RECORD))?;
    output.write_all(&data)?;
    output.sync_all()?;
    drop(output);
    discover(&prefix.join(GATEWAY))?;
    Ok(prefix.join(GATEWAY))
}
/// Deterministic process fixtures supply both exact Cargo-emitted binaries.
/// The controller preregisters the private package output root; no PATH search,
/// guessed sibling selection, default TMP fallback or build occurs here.
pub fn stage_cargo_gateway(gateway: &Path, helper: &Path) -> io::Result<PathBuf> {
    let output = std::env::var_os("SLOPSEARX_INSTALL_TEST_ROOT")
        .map(PathBuf::from)
        .ok_or_else(invalid)?;
    stage(gateway, helper, &output)
}
/// Stage a concrete already-selected pair into a relocatable distribution.
pub fn stage_distribution(gateway: &Path, helper: &Path, output: &Path) -> io::Result<PathBuf> {
    let executable = stage(gateway, helper, output)?;
    let prefix = executable
        .parent()
        .and_then(Path::parent)
        .ok_or_else(invalid)?;
    for name in [
        "",
        "bin",
        "libexec",
        "libexec/slopsearx",
        "share",
        "share/slopsearx",
    ] {
        fs::set_permissions(prefix.join(name), fs::Permissions::from_mode(0o755))?;
    }
    for name in [GATEWAY, HELPER] {
        fs::set_permissions(prefix.join(name), fs::Permissions::from_mode(0o555))?;
    }
    fs::set_permissions(prefix.join(RECORD), fs::Permissions::from_mode(0o444))?;
    discover(&executable)?;
    Ok(executable)
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);
    fn inputs() -> (PathBuf, PathBuf, PathBuf) {
        let root = PathBuf::from(
            std::env::var_os("SLOPSEARX_INSTALL_TEST_ROOT")
                .expect("root controller must preregister owned install fixtures"),
        );
        assert!(root.is_absolute() && fs::symlink_metadata(&root).unwrap().is_dir());
        let case = root.join(format!(
            "case-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        private_directory(&case).unwrap();
        let gateway = case.join("built-gateway");
        let helper = case.join("built-helper");
        fs::write(&gateway, b"controlled gateway bytes").unwrap();
        fs::write(&helper, b"controlled helper bytes").unwrap();
        for p in [&gateway, &helper] {
            fs::set_permissions(p, fs::Permissions::from_mode(0o700)).unwrap();
        }
        let output = case.join("packages");
        private_directory(&output).unwrap();
        (gateway, helper, output)
    }
    fn edit(executable: &Path, mutate: impl FnOnce(&mut serde_json::Value)) {
        let path = executable.parent().unwrap().parent().unwrap().join(RECORD);
        let mut value: serde_json::Value =
            serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        mutate(&mut value);
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        fs::write(path, serde_json::to_vec(&value).unwrap()).unwrap();
    }
    #[test]
    fn staged_record_discovers_helper_without_runtime_user_configuration() {
        let (g, h, out) = inputs();
        let installed = stage(&g, &h, &out).unwrap();
        assert!(discover(&installed).is_ok());
        // Controlled byte files qualify install discovery only, never native exec.
    }
    #[test]
    fn normal_cargo_input_hardlinks_are_copied_to_single_link_private_outputs() {
        let (g, h, out) = inputs();
        fs::hard_link(&g, g.with_extension("linked")).unwrap();
        fs::hard_link(&h, h.with_extension("linked")).unwrap();
        let installed = stage(&g, &h, &out).unwrap();
        assert_eq!(fs::metadata(&installed).unwrap().nlink(), 1);
        assert_eq!(fs::metadata(&installed).unwrap().mode() & 0o777, 0o500);
        assert!(discover(&installed).is_ok());
    }
    #[test]
    fn missing_wrong_role_or_changed_record_fails_before_any_helper_spawn() {
        for key in [
            "package",
            "version",
            "os",
            "arch",
            "gateway_path",
            "helper_path",
            "gateway_sha256",
            "helper_sha256",
        ] {
            let (g, h, out) = inputs();
            let installed = stage(&g, &h, &out).unwrap();
            edit(&installed, |v| {
                v[key] = serde_json::Value::String("changed".into())
            });
            assert!(discover(&installed).is_err());
        }
        let (g, h, out) = inputs();
        let installed = stage(&g, &h, &out).unwrap();
        let prefix = installed.parent().unwrap().parent().unwrap();
        fs::rename(prefix.join(RECORD), prefix.join("saved-record")).unwrap();
        assert!(discover(&installed).is_err());
        assert!(discover(&g).is_err());
    }
    #[test]
    fn changed_gateway_or_helper_bytes_cannot_reuse_record_identity() {
        for leaf in [GATEWAY, HELPER] {
            let (g, h, out) = inputs();
            let installed = stage(&g, &h, &out).unwrap();
            let path = installed.parent().unwrap().parent().unwrap().join(leaf);
            fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
            fs::write(&path, b"changed artifact").unwrap();
            assert!(discover(&installed).is_err());
        }
    }
    #[test]
    fn installed_symlinks_hardlinks_and_writable_package_entries_are_rejected() {
        let (g, h, out) = inputs();
        let installed = stage(&g, &h, &out).unwrap();
        let helper = installed.parent().unwrap().parent().unwrap().join(HELPER);
        fs::remove_file(&helper).unwrap();
        std::os::unix::fs::symlink(&h, &helper).unwrap();
        assert!(discover(&installed).is_err());
        let (g, h, out) = inputs();
        let installed = stage(&g, &h, &out).unwrap();
        fs::hard_link(&installed, installed.with_extension("linked")).unwrap();
        assert!(discover(&installed).is_err());
        let (g, h, out) = inputs();
        let installed = stage(&g, &h, &out).unwrap();
        fs::set_permissions(
            installed.parent().unwrap(),
            fs::Permissions::from_mode(0o777),
        )
        .unwrap();
        assert!(discover(&installed).is_err());
    }
    #[test]
    fn duplicate_unknown_record_fields_and_bad_shape_are_rejected() {
        let (g, h, out) = inputs();
        let installed = stage(&g, &h, &out).unwrap();
        edit(&installed, |v| {
            v["unexpected"] = serde_json::Value::Bool(true)
        });
        assert!(discover(&installed).is_err());
        let (g, h, out) = inputs();
        let installed = stage(&g, &h, &out).unwrap();
        let record = installed.parent().unwrap().parent().unwrap().join(RECORD);
        let raw = fs::read_to_string(&record).unwrap();
        fs::set_permissions(&record, fs::Permissions::from_mode(0o600)).unwrap();
        fs::write(&record, raw.replacen("{", "{\"schema\":1,", 1)).unwrap();
        assert!(discover(&installed).is_err());
    }
    #[test]
    fn repeated_staging_reuses_only_exact_matching_complete_package() {
        let (g, h, out) = inputs();
        let first = stage(&g, &h, &out).unwrap();
        let inode = fs::metadata(&first).unwrap().ino();
        assert_eq!(stage(&g, &h, &out).unwrap(), first);
        assert_eq!(fs::metadata(&first).unwrap().ino(), inode);
        edit(&first, |v| {
            v["gateway_sha256"] = serde_json::Value::String("0".repeat(64))
        });
        assert!(stage(&g, &h, &out).is_err());
    }
}
