//! Native ABI wrappers. The same API driver owns handles in production and fixtures.
use super::platform_api as api;
use std::collections::BTreeMap;
type ProxyInfo = BTreeMap<String, String>;
pub(super) fn discover() -> crate::Result<ProxyInfo> {
    #[cfg(target_os = "macos")]
    {
        return api::httpx_map(api::read_macos(&macos::Native)?);
    }
    #[cfg(target_os = "windows")]
    {
        return api::httpx_map(api::read_windows(&windows::Native)?);
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        Ok(ProxyInfo::new())
    }
}
#[cfg(test)]
fn windows_map(enabled: bool, server: &str) -> ProxyInfo {
    api::windows_map(enabled, &super::source_text::text(server))
        .into_iter()
        .map(|(k, v)| {
            (
                super::source_text::string(&k).unwrap(),
                super::source_text::string(&v).unwrap(),
            )
        })
        .collect()
}
#[cfg(test)]
fn mac_entry(
    result: &mut ProxyInfo,
    protocol: &str,
    enabled: i32,
    host: Option<&str>,
    port: Option<i32>,
) {
    if enabled != 0 {
        if let Some(host) = host {
            let host = host.split('\0').next().unwrap();
            result.insert(
                protocol.into(),
                match port {
                    Some(port) => format!("http://{host}:{port}"),
                    None => format!("http://{host}"),
                },
            );
        }
    }
}
#[cfg(target_os = "macos")]
mod macos {
    use super::*;
    use std::{
        ffi::{c_void, CStr},
        ptr,
    };
    type Ref = *const c_void;
    // ABI declarations match cached core-foundation/system-configuration-sys.
    // Dictionary values and exported keys are borrowed; only CopyProxies is owned.
    #[link(name = "SystemConfiguration", kind = "framework")]
    unsafe extern "C" {
        fn SCDynamicStoreCopyProxies(store: Ref) -> Ref;
        static kSCPropNetProxiesHTTPEnable: Ref;
        static kSCPropNetProxiesHTTPProxy: Ref;
        static kSCPropNetProxiesHTTPPort: Ref;
        static kSCPropNetProxiesHTTPSEnable: Ref;
        static kSCPropNetProxiesHTTPSProxy: Ref;
        static kSCPropNetProxiesHTTPSPort: Ref;
        static kSCPropNetProxiesFTPEnable: Ref;
        static kSCPropNetProxiesFTPProxy: Ref;
        static kSCPropNetProxiesFTPPort: Ref;
        static kSCPropNetProxiesGopherEnable: Ref;
        static kSCPropNetProxiesGopherProxy: Ref;
        static kSCPropNetProxiesGopherPort: Ref;
        static kSCPropNetProxiesSOCKSEnable: Ref;
        static kSCPropNetProxiesSOCKSProxy: Ref;
        static kSCPropNetProxiesSOCKSPort: Ref;
    }
    #[link(name = "CoreFoundation", kind = "framework")]
    unsafe extern "C" {
        fn CFRelease(value: Ref);
        fn CFGetTypeID(value: Ref) -> usize;
        fn CFDictionaryGetValue(dictionary: Ref, key: Ref) -> Ref;
        fn CFNumberGetTypeID() -> usize;
        fn CFNumberGetValue(number: Ref, kind: u32, value: *mut c_void) -> bool;
        fn CFStringGetTypeID() -> usize;
        fn CFStringGetCStringPtr(string: Ref, encoding: u32) -> *const std::ffi::c_char;
        fn CFStringGetLength(string: Ref) -> isize;
        fn CFStringGetCString(
            string: Ref,
            buffer: *mut std::ffi::c_char,
            size: isize,
            encoding: u32,
        ) -> u8;
    }
    fn number(value: Ref) -> crate::Result<Option<i32>> {
        if value.is_null() {
            return Ok(None);
        }
        unsafe {
            if CFGetTypeID(value) != CFNumberGetTypeID() {
                return Err(crate::error::builder(
                    "source CFNumber argument has invalid runtime type",
                ));
            }
            let mut number = 0i32;
            let _ = CFNumberGetValue(value, 3, (&mut number as *mut i32).cast());
            Ok(Some(number))
        }
    }
    fn string(value: Ref) -> crate::Result<Option<String>> {
        if value.is_null() {
            return Ok(None);
        }
        unsafe {
            if CFGetTypeID(value) != CFStringGetTypeID() {
                return Err(crate::error::builder(
                    "native proxy host has unexpected CF type",
                ));
            }
            let direct = CFStringGetCStringPtr(value, 0x08000100);
            if !direct.is_null() {
                return CStr::from_ptr(direct)
                    .to_str()
                    .map(|s| Some(s.to_owned()))
                    .map_err(crate::error::builder);
            }
            let len = CFStringGetLength(value);
            let size = len
                .checked_mul(4)
                .ok_or_else(|| crate::error::builder("native proxy host length overflow"))?;
            if size <= 0 {
                return Ok(None);
            }
            let mut bytes = Vec::new();
            bytes
                .try_reserve_exact(size as usize)
                .map_err(crate::error::builder)?;
            bytes.resize(size as usize, 0u8);
            if CFStringGetCString(value, bytes.as_mut_ptr().cast(), size, 0x08000100) == 0 {
                return Ok(None);
            }
            let end = bytes
                .iter()
                .position(|b| *b == 0)
                .ok_or_else(|| crate::error::builder("native proxy string missing terminator"))?;
            std::str::from_utf8(&bytes[..end])
                .map(|s| Some(s.to_owned()))
                .map_err(crate::error::builder)
        }
    }
    fn key(name: &str) -> Ref {
        unsafe {
            match name {
                "HTTPEnable" => kSCPropNetProxiesHTTPEnable,
                "HTTPProxy" => kSCPropNetProxiesHTTPProxy,
                "HTTPPort" => kSCPropNetProxiesHTTPPort,
                "HTTPSEnable" => kSCPropNetProxiesHTTPSEnable,
                "HTTPSProxy" => kSCPropNetProxiesHTTPSProxy,
                "HTTPSPort" => kSCPropNetProxiesHTTPSPort,
                "FTPEnable" => kSCPropNetProxiesFTPEnable,
                "FTPProxy" => kSCPropNetProxiesFTPProxy,
                "FTPPort" => kSCPropNetProxiesFTPPort,
                "GopherEnable" => kSCPropNetProxiesGopherEnable,
                "GopherProxy" => kSCPropNetProxiesGopherProxy,
                "GopherPort" => kSCPropNetProxiesGopherPort,
                "SOCKSEnable" => kSCPropNetProxiesSOCKSEnable,
                "SOCKSProxy" => kSCPropNetProxiesSOCKSProxy,
                "SOCKSPort" => kSCPropNetProxiesSOCKSPort,
                _ => unreachable!("fixed source proxy key"),
            }
        }
    }
    pub(super) struct Native;
    impl api::MacApi for Native {
        fn copy_proxies(&self) -> usize {
            unsafe { SCDynamicStoreCopyProxies(ptr::null()) as usize }
        }
        fn release(&self, id: usize) {
            unsafe {
                CFRelease(id as Ref);
            }
        }
        fn number(&self, id: usize, name: &str) -> crate::Result<Option<i32>> {
            number(unsafe { CFDictionaryGetValue(id as Ref, key(name)) })
        }
        fn string(&self, id: usize, name: &str) -> crate::Result<Option<String>> {
            string(unsafe { CFDictionaryGetValue(id as Ref, key(name)) })
        }
    }
}
#[cfg(target_os = "windows")]
mod windows {
    use super::*;
    use std::{ffi::c_void, ptr};
    type Key = *mut c_void;
    #[link(name = "advapi32")]
    unsafe extern "system" {
        fn RegOpenKeyExW(
            key: Key,
            path: *const u16,
            options: u32,
            access: u32,
            result: *mut Key,
        ) -> u32;
        fn RegQueryValueExW(
            key: Key,
            name: *const u16,
            reserved: *const u32,
            kind: *mut u32,
            data: *mut u8,
            size: *mut u32,
        ) -> u32;
        fn RegCloseKey(key: Key) -> u32;
    }
    pub(super) struct Native;
    impl api::WindowsApi for Native {
        fn open(&self, root: isize, path: &[u16], options: u32, access: u32) -> Result<usize, u32> {
            let mut key = ptr::null_mut();
            let status =
                unsafe { RegOpenKeyExW(root as Key, path.as_ptr(), options, access, &mut key) };
            if status == 0 {
                Ok(key as usize)
            } else {
                Err(status)
            }
        }
        fn query(&self, key: usize, name: &[u16], buffer: Option<&mut [u8]>) -> (u32, u32, u32) {
            let mut kind = 0;
            let (mut size, data, kind_ptr) = match buffer {
                Some(buffer) => (
                    buffer.len() as u32,
                    buffer.as_mut_ptr(),
                    &mut kind as *mut u32,
                ),
                None => (0, ptr::null_mut(), ptr::null_mut()),
            };
            let status = unsafe {
                RegQueryValueExW(
                    key as Key,
                    name.as_ptr(),
                    ptr::null(),
                    kind_ptr,
                    data,
                    &mut size,
                )
            };
            (status, kind, size)
        }
        fn close(&self, key: usize) -> u32 {
            unsafe { RegCloseKey(key as Key) }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn windows_single_endpoint_expands_three_http_transports() {
        let map = windows_map(true, "local:8080");
        assert_eq!(map.len(), 3);
        for key in ["http", "https", "ftp"] {
            assert_eq!(map[key], "http://local:8080");
        }
    }
    #[test]
    fn windows_disabled_empty_and_missing() {
        assert!(windows_map(false, "x").is_empty());
        assert_eq!(windows_map(true, "")["http"], "http://");
        assert!(windows_map(true, ";").is_empty());
    }
    #[test]
    fn windows_malformed_preserves_partial_and_skips_socks_promotion() {
        let map = windows_map(true, "socks=a;bad;https=b");
        assert_eq!(map.len(), 1);
        assert_eq!(map["socks"], "socks://a");
    }
    #[test]
    fn windows_socks_promotes_only_missing_or_empty_http_entries() {
        let map = windows_map(true, "socks=a;http=");
        assert_eq!(map["http"], "http://");
        assert_eq!(map["https"], "socks4://a");
        assert_eq!(map["socks"], "socks://a");
    }
    #[test]
    fn windows_order_duplicates_case_and_prefix_are_source_literal() {
        let map = windows_map(
            true,
            "http=a;http=https://b;HTTP=c;socks=socks5://s;other=a/b://x",
        );
        assert_eq!(map["http"], "https://b");
        assert_eq!(map["HTTP"], "c");
        assert_eq!(map["https"], "socks5://s");
        assert_eq!(map["other"], "a/b://x");
    }
    #[test]
    fn mac_enabled_protocols_ports_and_missing_host() {
        let mut map = ProxyInfo::new();
        mac_entry(&mut map, "http", 0, Some("disabled"), Some(1));
        mac_entry(&mut map, "https", 1, Some("proxy"), None);
        mac_entry(&mut map, "ftp", 1, None, Some(2));
        mac_entry(&mut map, "socks", 1, Some("proxy"), Some(-1));
        assert_eq!(map.len(), 2);
        assert_eq!(map["https"], "http://proxy");
        assert_eq!(map["socks"], "http://proxy:-1");
        assert!(!map.contains_key("no"));
    }
    #[test]
    fn mac_cstring_truncation_and_empty_are_preserved() {
        let mut map = ProxyInfo::new();
        mac_entry(&mut map, "http", -1, Some("x\0y"), Some(80));
        mac_entry(&mut map, "https", 1, Some(""), None);
        assert_eq!(map["http"], "http://x:80");
        assert_eq!(map["https"], "http://");
    }
}
