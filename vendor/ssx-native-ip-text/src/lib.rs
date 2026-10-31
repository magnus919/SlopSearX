//! Private audited Unix inet_ntop boundary; no sockets, DNS or raw pointers escape.
#![cfg(unix)]
use std::{ffi::CStr, io, net::IpAddr};
unsafe extern "C" {
    fn inet_ntop(
        family: libc::c_int,
        source: *const libc::c_void,
        target: *mut libc::c_char,
        size: libc::socklen_t,
    ) -> *const libc::c_char;
}
/// Same native formatting operation as CPython make_ipv4_addr/make_ipv6_addr.
pub fn numeric_host(address: IpAddr) -> io::Result<String> {
    let mut text = [0 as libc::c_char; 46];
    // SAFETY: properly aligned family-specific native structs remain live for
    // the synchronous call. Destination has the complete INET6_ADDRSTRLEN46.
    let result = match address {
        IpAddr::V4(ip) => {
            let a = libc::in_addr {
                s_addr: u32::from_ne_bytes(ip.octets()),
            };
            unsafe {
                inet_ntop(
                    libc::AF_INET,
                    (&a as *const libc::in_addr).cast(),
                    text.as_mut_ptr(),
                    text.len() as libc::socklen_t,
                )
            }
        }
        IpAddr::V6(ip) => {
            let a = libc::in6_addr {
                s6_addr: ip.octets(),
            };
            unsafe {
                inet_ntop(
                    libc::AF_INET6,
                    (&a as *const libc::in6_addr).cast(),
                    text.as_mut_ptr(),
                    text.len() as libc::socklen_t,
                )
            }
        }
    };
    if result.is_null() {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: successful inet_ntop guarantees termination within supplied span.
    unsafe { CStr::from_ptr(text.as_ptr()) }
        .to_str()
        .map(str::to_owned)
        .map_err(|_| io::ErrorKind::InvalidData.into())
}
