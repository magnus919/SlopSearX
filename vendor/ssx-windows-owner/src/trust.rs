use sha2::{Digest, Sha256};
use std::{
    ffi::{OsStr, OsString},
    fs::{File, OpenOptions},
    io::{self, Read, Seek, SeekFrom},
    mem,
    os::windows::{
        ffi::{OsStrExt, OsStringExt},
        fs::OpenOptionsExt,
        io::{AsRawHandle, FromRawHandle, OwnedHandle},
    },
    path::{Path, PathBuf},
    ptr,
    sync::Arc,
};
use windows_sys::Win32::{
    Foundation::*,
    Security::{Authorization::*, *},
    Storage::FileSystem::*,
    System::{
        Console::{GetStdHandle, STD_INPUT_HANDLE},
        SystemServices::{ACCESS_ALLOWED_ACE_TYPE, ACCESS_DENIED_ACE_TYPE},
        Threading::*,
    },
};

pub(crate) fn wide(value: &OsStr) -> io::Result<Vec<u16>> {
    let mut units: Vec<_> = value.encode_wide().collect();
    if units.contains(&0) {
        return Err(io::ErrorKind::InvalidInput.into());
    }
    units.push(0);
    Ok(units)
}
pub(crate) fn handle(raw: HANDLE) -> io::Result<OwnedHandle> {
    if raw.is_null() || raw == INVALID_HANDLE_VALUE {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: only newly created or duplicated handles enter this constructor.
    Ok(unsafe { OwnedHandle::from_raw_handle(raw.cast()) })
}
pub(crate) fn raw(handle: &OwnedHandle) -> HANDLE {
    handle.as_raw_handle().cast()
}
pub(crate) fn duplicate(handle: HANDLE, inherit: bool) -> io::Result<OwnedHandle> {
    let mut output = ptr::null_mut();
    // SAFETY: the source is borrowed from a live owned handle or GetStdHandle;
    // the result is a distinct kernel reference and becomes uniquely owned.
    if unsafe {
        DuplicateHandle(
            GetCurrentProcess(),
            handle,
            GetCurrentProcess(),
            &mut output,
            0,
            inherit.into(),
            DUPLICATE_SAME_ACCESS,
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }
    self::handle(output)
}
pub fn current_input() -> io::Result<OwnedHandle> {
    duplicate(unsafe { GetStdHandle(STD_INPUT_HANDLE) }, false)
}

struct LocalAllocation(*mut core::ffi::c_void);
impl Drop for LocalAllocation {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe {
                LocalFree(self.0);
            }
        }
    }
}
struct TokenSid {
    _token: OwnedHandle,
    storage: Vec<usize>,
}
impl TokenSid {
    fn new() -> io::Result<Self> {
        let mut token = ptr::null_mut();
        if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) } == 0 {
            return Err(io::Error::last_os_error());
        }
        let token = handle(token)?;
        let mut bytes = 0;
        unsafe {
            GetTokenInformation(raw(&token), TokenUser, ptr::null_mut(), 0, &mut bytes);
        }
        if bytes == 0 || bytes > 65536 {
            return Err(io::ErrorKind::InvalidData.into());
        }
        let mut storage = vec![0usize; (bytes as usize).div_ceil(mem::size_of::<usize>())];
        if unsafe {
            GetTokenInformation(
                raw(&token),
                TokenUser,
                storage.as_mut_ptr().cast(),
                bytes,
                &mut bytes,
            )
        } == 0
        {
            return Err(io::Error::last_os_error());
        }
        Ok(Self {
            _token: token,
            storage,
        })
    }
    fn sid(&self) -> PSID {
        // SAFETY: GetTokenInformation produced a TOKEN_USER in aligned storage;
        // the backing allocation is retained unchanged for this whole borrow.
        unsafe { (*(self.storage.as_ptr().cast::<TOKEN_USER>())).User.Sid }
    }
}
pub(crate) fn private_sddl() -> io::Result<Vec<u16>> {
    let user = TokenSid::new()?;
    let mut text = ptr::null_mut();
    if unsafe { ConvertSidToStringSidW(user.sid(), &mut text) } == 0 {
        return Err(io::Error::last_os_error());
    }
    let allocation = LocalAllocation(text.cast());
    let mut len = 0usize;
    while unsafe { *text.add(len) } != 0 {
        len += 1;
        if len > 1024 {
            return Err(io::ErrorKind::InvalidData.into());
        }
    }
    let sid = OsString::from_wide(unsafe { std::slice::from_raw_parts(text, len) });
    let sid = sid.to_str().ok_or(io::ErrorKind::InvalidData)?;
    let result = wide(OsStr::new(&format!(
        "D:P(A;;GA;;;{sid})(A;;GA;;;SY)(A;;GA;;;BA)"
    )));
    drop(allocation);
    result
}
pub(crate) struct PrivateSecurity {
    allocation: LocalAllocation,
    pub attributes: SECURITY_ATTRIBUTES,
}
impl PrivateSecurity {
    pub fn new() -> io::Result<Self> {
        let text = private_sddl()?;
        let mut descriptor = ptr::null_mut();
        if unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                text.as_ptr(),
                SDDL_REVISION_1,
                &mut descriptor,
                ptr::null_mut(),
            )
        } == 0
        {
            return Err(io::Error::last_os_error());
        }
        Ok(Self {
            allocation: LocalAllocation(descriptor),
            attributes: SECURITY_ATTRIBUTES {
                nLength: mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
                lpSecurityDescriptor: descriptor,
                bInheritHandle: 0,
            },
        })
    }
}
impl Drop for PrivateSecurity {
    fn drop(&mut self) {
        let _ = &self.allocation;
    }
}

/// Require a non-reparse package object whose owner is current user/System/Admin
/// and whose DACL grants write/delete/ownership only to those trusted principals.
fn check_acl(file: &File) -> io::Result<()> {
    check_acl_policy(file, false)
}
fn check_acl_policy(file: &File, confidential: bool) -> io::Result<()> {
    let current = TokenSid::new()?;
    let mut owner = ptr::null_mut();
    let mut acl = ptr::null_mut();
    let mut descriptor = ptr::null_mut();
    let code = unsafe {
        GetSecurityInfo(
            file.as_raw_handle().cast(),
            SE_FILE_OBJECT,
            OWNER_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION,
            &mut owner,
            ptr::null_mut(),
            &mut acl,
            ptr::null_mut(),
            &mut descriptor,
        )
    };
    if code != 0 {
        return Err(io::Error::from_raw_os_error(code as i32));
    }
    let _descriptor = LocalAllocation(descriptor);
    let trusted = |sid: PSID| -> bool {
        !sid.is_null()
            && unsafe { IsValidSid(sid) } != 0
            && (unsafe { EqualSid(sid, current.sid()) } != 0
                || unsafe { IsWellKnownSid(sid, WinLocalSystemSid) } != 0
                || unsafe { IsWellKnownSid(sid, WinBuiltinAdministratorsSid) } != 0)
    };
    if !trusted(owner) || acl.is_null() || unsafe { IsValidAcl(acl) } == 0 {
        return Err(io::ErrorKind::PermissionDenied.into());
    }
    let write = GENERIC_WRITE
        | GENERIC_ALL
        | DELETE
        | WRITE_DAC
        | WRITE_OWNER
        | FILE_WRITE_DATA
        | FILE_APPEND_DATA
        | FILE_WRITE_EA
        | FILE_WRITE_ATTRIBUTES
        | FILE_DELETE_CHILD;
    for index in 0..unsafe { (*acl).AceCount } {
        let mut address = ptr::null_mut();
        if unsafe { GetAce(acl, u32::from(index), &mut address) } == 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: IsValidAcl/GetAce establish this ACE span in the live OS
        // descriptor; allowed ACE SID starts inside the validated ACE layout.
        let header = unsafe { &*(address.cast::<ACE_HEADER>()) };
        match header.AceType {
            kind if kind == ACCESS_DENIED_ACE_TYPE as u8 => (),
            kind if kind == ACCESS_ALLOWED_ACE_TYPE as u8 => {
                if usize::from(header.AceSize) < mem::size_of::<ACCESS_ALLOWED_ACE>() {
                    return Err(io::ErrorKind::InvalidData.into());
                }
                let ace = unsafe { &*(address.cast::<ACCESS_ALLOWED_ACE>()) };
                let sid = std::ptr::addr_of!(ace.SidStart).cast_mut().cast();
                // Package integrity permits public readers. Token documents do
                // not: generic/read/execute rights are an additional boundary.
                let forbidden = write
                    | if confidential {
                        GENERIC_READ | GENERIC_EXECUTE | FILE_GENERIC_READ | FILE_GENERIC_EXECUTE
                    } else {
                        0
                    };
                if ace.Mask & forbidden != 0 && !trusted(sid) {
                    return Err(io::ErrorKind::PermissionDenied.into());
                }
            }
            _ => return Err(io::ErrorKind::PermissionDenied.into()),
        }
    }
    Ok(())
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Identity {
    volume: u32,
    index_hi: u32,
    index_lo: u32,
    size_hi: u32,
    size_lo: u32,
    write_hi: u32,
    write_lo: u32,
}
pub(crate) fn identity(file: &File, directory: bool) -> io::Result<Identity> {
    let mut info: BY_HANDLE_FILE_INFORMATION = unsafe { mem::zeroed() };
    if unsafe { GetFileInformationByHandle(file.as_raw_handle().cast(), &mut info) } == 0 {
        return Err(io::Error::last_os_error());
    }
    if info.dwFileAttributes & FILE_ATTRIBUTE_REPARSE_POINT != 0
        || (info.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY != 0) != directory
        || info.nNumberOfLinks != 1
    {
        return Err(io::ErrorKind::InvalidData.into());
    }
    Ok(Identity {
        volume: info.dwVolumeSerialNumber,
        index_hi: info.nFileIndexHigh,
        index_lo: info.nFileIndexLow,
        size_hi: info.nFileSizeHigh,
        size_lo: info.nFileSizeLow,
        write_hi: info.ftLastWriteTime.dwHighDateTime,
        write_lo: info.ftLastWriteTime.dwLowDateTime,
    })
}
pub(crate) fn opened(path: &Path, directory: bool) -> io::Result<File> {
    opened_policy(path, directory, false)
}
pub(crate) fn opened_confidential(path: &Path) -> io::Result<File> {
    opened_policy(path, false, true)
}
fn opened_policy(path: &Path, directory: bool, confidential: bool) -> io::Result<File> {
    use std::os::windows::fs::MetadataExt;
    if !path.is_absolute() {
        return Err(io::ErrorKind::InvalidInput.into());
    }
    for component in path.ancestors() {
        if std::fs::symlink_metadata(component)?.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT
            != 0
        {
            return Err(io::ErrorKind::InvalidData.into());
        }
    }
    // Retain no-delete/no-write sharing for exact package bytes through launch.
    let file = OpenOptions::new()
        .read(true)
        .access_mode(FILE_GENERIC_READ | READ_CONTROL)
        .share_mode(FILE_SHARE_READ | if confidential { FILE_SHARE_DELETE } else { 0 })
        .custom_flags(
            FILE_FLAG_OPEN_REPARSE_POINT
                | if directory {
                    FILE_FLAG_BACKUP_SEMANTICS
                } else {
                    0
                },
        )
        .open(path)?;
    identity(&file, directory)?;
    check_acl_policy(&file, confidential)?;
    Ok(file)
}
pub fn check_private_directory(path: &Path) -> io::Result<()> {
    opened(path, true).map(drop)
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ImageRole {
    Dns,
    Input,
}
#[derive(Clone)]
pub struct TrustedImage {
    path: PathBuf,
    file: Arc<File>,
    identity: Identity,
    digest: [u8; 32],
    role: ImageRole,
}
impl std::fmt::Debug for TrustedImage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("TrustedImage(<package-owned>)")
    }
}
impl TrustedImage {
    pub fn open(path: PathBuf, expected_sha256: [u8; 32], role: ImageRole) -> io::Result<Self> {
        let name = match role {
            ImageRole::Dns => "ssx-gateway-dns.exe",
            ImageRole::Input => "ssx-gateway-input.exe",
        };
        if path.file_name() != Some(OsStr::new(name)) {
            return Err(io::ErrorKind::InvalidInput.into());
        }
        let mut file = opened(&path, false)?;
        let before = identity(&file, false)?;
        let mut hash = Sha256::new();
        let mut buffer = [0u8; 65536];
        let mut length = 0usize;
        loop {
            let n = file.read(&mut buffer)?;
            if n == 0 {
                break;
            }
            length = length.checked_add(n).ok_or(io::ErrorKind::InvalidData)?;
            if length > 512 * 1024 * 1024 {
                return Err(io::ErrorKind::InvalidData.into());
            }
            hash.update(&buffer[..n]);
        }
        if length == 0
            || <[u8; 32]>::from(hash.finalize()) != expected_sha256
            || identity(&file, false)? != before
        {
            return Err(io::ErrorKind::InvalidData.into());
        }
        file.seek(SeekFrom::Start(0))?;
        Ok(Self {
            path,
            file: Arc::new(file),
            identity: before,
            digest: expected_sha256,
            role,
        })
    }
    pub(crate) fn application(&self, role: ImageRole) -> io::Result<Vec<u16>> {
        if self.role != role || identity(&self.file, false)? != self.identity {
            return Err(io::ErrorKind::InvalidData.into());
        }
        // Reopen checks the name still resolves to the same pinned file; the
        // original open denies replacement/write during this trusted lifetime.
        let current = opened(&self.path, false)?;
        if identity(&current, false)? != self.identity {
            return Err(io::ErrorKind::InvalidData.into());
        }
        let _ = self.digest;
        wide(self.path.as_os_str())
    }
}

/// Read a package record through the same no-reparse/ACL/identity checks.
pub fn read_package_file(path: &Path, limit: usize) -> io::Result<Vec<u8>> {
    let mut file = opened(path, false)?;
    let before = identity(&file, false)?;
    let mut raw = Vec::new();
    Read::by_ref(&mut file)
        .take((limit + 1) as u64)
        .read_to_end(&mut raw)?;
    if raw.len() > limit || identity(&file, false)? != before {
        return Err(io::ErrorKind::InvalidData.into());
    }
    Ok(raw)
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FileDigest {
    pub bytes: u64,
    pub sha256: [u8; 32],
}
pub fn build_file_digest(path: &Path) -> io::Result<FileDigest> {
    // Build paths are explicit Cargo-emitted artifacts, not installed command
    // admission. Installed copies still require strict ACL/digest validation.
    let mut file = OpenOptions::new()
        .read(true)
        .share_mode(FILE_SHARE_READ)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
        .open(path)?;
    let before = identity(&file, false)?;
    let mut hash = Sha256::new();
    let mut bytes = 0u64;
    let mut block = [0u8; 65536];
    loop {
        let n = file.read(&mut block)?;
        if n == 0 {
            break;
        }
        bytes = bytes
            .checked_add(n as u64)
            .ok_or(io::ErrorKind::InvalidData)?;
        if bytes > 512 * 1024 * 1024 {
            return Err(io::ErrorKind::InvalidData.into());
        }
        hash.update(&block[..n]);
    }
    if bytes == 0 || identity(&file, false)? != before {
        return Err(io::ErrorKind::InvalidData.into());
    }
    Ok(FileDigest {
        bytes,
        sha256: hash.finalize().into(),
    })
}

pub fn package_file_digest(path: &Path) -> io::Result<FileDigest> {
    let mut file = opened(path, false)?;
    let before = identity(&file, false)?;
    let mut hash = Sha256::new();
    let mut bytes = 0u64;
    let mut buffer = [0u8; 65536];
    loop {
        let n = file.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        bytes = bytes
            .checked_add(n as u64)
            .ok_or(io::ErrorKind::InvalidData)?;
        if bytes > 512 * 1024 * 1024 {
            return Err(io::ErrorKind::InvalidData.into());
        }
        hash.update(&buffer[..n]);
    }
    if bytes == 0 || identity(&file, false)? != before {
        return Err(io::ErrorKind::InvalidData.into());
    }
    Ok(FileDigest {
        bytes,
        sha256: hash.finalize().into(),
    })
}

pub fn create_private_directory(path: &Path) -> io::Result<()> {
    let name = wide(path.as_os_str())?;
    let security = PrivateSecurity::new()?;
    if unsafe { CreateDirectoryW(name.as_ptr(), &security.attributes) } == 0 {
        let error = io::Error::last_os_error();
        if error.raw_os_error() != Some(ERROR_ALREADY_EXISTS as i32) {
            return Err(error);
        }
    }
    check_private_directory(path)
}

pub fn create_private_file(path: &Path) -> io::Result<File> {
    let name = wide(path.as_os_str())?;
    let security = PrivateSecurity::new()?;
    let owned = handle(unsafe {
        CreateFileW(
            name.as_ptr(),
            FILE_GENERIC_READ | FILE_GENERIC_WRITE | READ_CONTROL,
            0,
            &security.attributes,
            CREATE_NEW,
            FILE_ATTRIBUTE_NORMAL | FILE_FLAG_OPEN_REPARSE_POINT,
            ptr::null_mut(),
        )
    })?;
    let file = File::from(owned);
    identity(&file, false)?;
    check_acl(&file)?;
    Ok(file)
}
pub fn package_lock(path: &Path) -> io::Result<File> {
    if path.file_name() != Some(OsStr::new(".gateway-package.lock")) {
        return Err(io::ErrorKind::InvalidInput.into());
    }
    // Every contender must be able to open the same lock identity before
    // File::lock serializes staging. Keep deletion sharing disabled and use
    // the private security descriptor atomically when the lock is first made.
    let name = wide(path.as_os_str())?;
    let security = PrivateSecurity::new()?;
    let owned = handle(unsafe {
        CreateFileW(
            name.as_ptr(),
            FILE_GENERIC_READ | FILE_GENERIC_WRITE | READ_CONTROL,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            &security.attributes,
            OPEN_ALWAYS,
            FILE_ATTRIBUTE_NORMAL | FILE_FLAG_OPEN_REPARSE_POINT,
            ptr::null_mut(),
        )
    })?;
    let file = File::from(owned);
    identity(&file, false)?;
    check_acl(&file)?;
    if file.metadata()?.len() != 0 {
        return Err(io::ErrorKind::InvalidData.into());
    }
    file.lock()?;
    Ok(file)
}

/// Qualification-only adversarial ACL; never compiled into a production owner.
#[cfg(feature = "fixtures")]
pub fn fixture_public_access(path: &Path, writable: bool) -> io::Result<()> {
    let mut text = private_sddl()?;
    text.pop();
    text.extend(format!("(A;;{};;;WD)", if writable { "GA" } else { "GR" }).encode_utf16());
    text.push(0);
    let mut descriptor = ptr::null_mut();
    if unsafe {
        ConvertStringSecurityDescriptorToSecurityDescriptorW(
            text.as_ptr(),
            SDDL_REVISION_1,
            &mut descriptor,
            ptr::null_mut(),
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }
    let _allocation = LocalAllocation(descriptor);
    let path = wide(path.as_os_str())?;
    // SAFETY: the fixture owns this exact private path; descriptor allocation
    // remains alive through this synchronous OS call. No caller raw pointer.
    if unsafe {
        SetFileSecurityW(
            path.as_ptr(),
            DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION,
            descriptor,
        )
    } == 0
    {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}
