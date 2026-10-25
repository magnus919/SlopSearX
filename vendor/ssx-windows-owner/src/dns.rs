use std::{
    ffi::CString,
    io, mem,
    net::{Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6},
    ptr,
};
use windows_sys::Win32::Networking::WinSock::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LookupError {
    Gai(i32),
    Startup(i32),
    OperationalLimit,
    InputContract,
}
impl std::fmt::Display for LookupError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "native Windows name lookup failed ({self:?})")
    }
}
impl std::error::Error for LookupError {}

fn canonical_decimal(decimal: &str) -> io::Result<(bool, &str)> {
    let (negative, digits) = decimal
        .strip_prefix('-')
        .map_or((false, decimal), |s| (true, s));
    if digits.is_empty()
        || digits.len() > 4300
        || !digits.bytes().all(|c| c.is_ascii_digit())
        || (digits.len() > 1 && digits.starts_with('0'))
        || decimal == "-0"
    {
        return Err(io::ErrorKind::InvalidInput.into());
    }
    Ok((negative, digits))
}
pub fn source_connect_port(decimal: &str) -> io::Result<u16> {
    let (negative, digits) = canonical_decimal(decimal)?;
    let low = digits.bytes().fold(0u16, |n, c| {
        n.wrapping_mul(10).wrapping_add(u16::from(c - b'0'))
    });
    Ok(if negative {
        0u16.wrapping_sub(low)
    } else {
        low
    })
}

struct Winsock;
impl Winsock {
    fn start() -> Result<Self, LookupError> {
        // SAFETY: WSADATA is valid writable storage; this owner balances only
        // its own successful startup reference, never another library's count.
        let mut data: WSADATA = unsafe { mem::zeroed() };
        let status = unsafe { WSAStartup(0x0101, &mut data) };
        if status == 0 {
            Ok(Self)
        } else {
            Err(LookupError::Startup(status))
        }
    }
}
impl Drop for Winsock {
    fn drop(&mut self) {
        unsafe {
            WSACleanup();
        }
    }
}
struct Addresses(*mut ADDRINFOA);
impl Drop for Addresses {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe {
                freeaddrinfo(self.0);
            }
        }
    }
}

/// Synchronous GAI only inside the separately owned DNS helper process. The
/// gateway never calls this on a runtime thread or spawn_blocking worker.
/// There is no dummy UDP socket/bind used to initialize Winsock.
pub fn lookup(host: &str, decimal: &str) -> Result<Vec<SocketAddr>, LookupError> {
    let invalid = || LookupError::InputContract;
    if !host.is_ascii() || host.contains('\0') || canonical_decimal(decimal).is_err() {
        return Err(invalid());
    }
    let host = CString::new(host).map_err(|_| invalid())?;
    let service = CString::new(decimal).map_err(|_| invalid())?;
    let _startup = Winsock::start()?;
    // SAFETY: initialized hints and NUL-terminated stable strings live until
    // synchronous getaddrinfo returns; result is freed exactly once by guard.
    let mut hints: ADDRINFOA = unsafe { mem::zeroed() };
    hints.ai_family = AF_UNSPEC as i32;
    hints.ai_socktype = SOCK_STREAM;
    let mut first = ptr::null_mut();
    let code = unsafe {
        getaddrinfo(
            host.as_ptr().cast(),
            service.as_ptr().cast(),
            &hints,
            &mut first,
        )
    };
    if code != 0 {
        return Err(LookupError::Gai(code));
    }
    let guard = Addresses(first);
    let mut cursor = guard.0;
    let mut result = Vec::new();
    let mut wire_bytes = 21usize;
    while !cursor.is_null() {
        // SAFETY: cursor belongs to the live OS-owned linked result list.
        let row = unsafe { &*cursor };
        if row.ai_addr.is_null() {
            return Err(invalid());
        }
        wire_bytes = wire_bytes
            .checked_add(if row.ai_family == AF_INET as i32 {
                7
            } else {
                27
            })
            .ok_or(LookupError::OperationalLimit)?;
        if wire_bytes > 1024 * 1024 {
            return Err(LookupError::OperationalLimit);
        }
        let address = if row.ai_family == AF_INET as i32 {
            if row.ai_addrlen != mem::size_of::<SOCKADDR_IN>() {
                return Err(invalid());
            }
            let addr = unsafe { ptr::read_unaligned(row.ai_addr.cast::<SOCKADDR_IN>()) };
            let ip = Ipv4Addr::from(unsafe { addr.sin_addr.S_un.S_addr }.to_ne_bytes());
            SocketAddr::V4(SocketAddrV4::new(ip, u16::from_be(addr.sin_port)))
        } else if row.ai_family == AF_INET6 as i32 {
            if row.ai_addrlen != mem::size_of::<SOCKADDR_IN6>() {
                return Err(invalid());
            }
            let addr = unsafe { ptr::read_unaligned(row.ai_addr.cast::<SOCKADDR_IN6>()) };
            let ip = Ipv6Addr::from(unsafe { addr.sin6_addr.u.Byte });
            SocketAddr::V6(SocketAddrV6::new(
                ip,
                u16::from_be(addr.sin6_port),
                addr.sin6_flowinfo,
                unsafe { addr.Anonymous.sin6_scope_id },
            ))
        } else {
            return Err(invalid());
        };
        result.push(address);
        cursor = row.ai_next;
    }
    Ok(result)
}

// Exact ABI from Microsoft CRT _get_errno documentation. Link/CRT selection and
// stale-errno equivalence to CPython remain actual Windows qualification work.
unsafe extern "C" {
    fn _get_errno(value: *mut i32) -> i32;
}
/// Native CPython-style first-lookup text, without DNS/socket/extra startup.
/// Microsoft's inet_ntop needs no Winsock initialization for conversion.
pub fn numeric_host(address: std::net::IpAddr) -> io::Result<String> {
    let mut text = [0u8; 46];
    // SAFETY: aligned native address structs contain the typed address bytes;
    // the46-byte writable destination and all inputs outlive the synchronous call.
    let result = match address {
        std::net::IpAddr::V4(ip) => {
            let mut native: IN_ADDR = unsafe { mem::zeroed() };
            native.S_un.S_addr = u32::from_ne_bytes(ip.octets());
            unsafe {
                inet_ntop(
                    AF_INET as i32,
                    (&native as *const IN_ADDR).cast(),
                    text.as_mut_ptr(),
                    text.len(),
                )
            }
        }
        std::net::IpAddr::V6(ip) => {
            let mut native: IN6_ADDR = unsafe { mem::zeroed() };
            native.u.Byte = ip.octets();
            unsafe {
                inet_ntop(
                    AF_INET6 as i32,
                    (&native as *const IN6_ADDR).cast(),
                    text.as_mut_ptr(),
                    text.len(),
                )
            }
        }
    };
    if result.is_null() {
        let mut errno = 0;
        // SAFETY: nonnull aligned writable i32. Capture THIS runtime's CRT value
        // immediately; do not substitute WSAGetLastError/GetLastError or assert
        // that a stale CRT value equals a separately linked Python interpreter.
        if unsafe { _get_errno(&mut errno) } != 0 {
            return Err(io::ErrorKind::Other.into());
        }
        return Err(io::Error::from_raw_os_error(errno));
    }
    // SAFETY: successful inet_ntop guarantees a terminator in supplied span.
    unsafe { std::ffi::CStr::from_ptr(text.as_ptr().cast()) }
        .to_str()
        .map(str::to_owned)
        .map_err(|_| io::ErrorKind::InvalidData.into())
}
