//! Injected native API ownership used by production platform discovery itself.
use super::source_text::{self as st, Text};
use std::collections::BTreeMap;
pub(super) type RawMap = BTreeMap<Text, Text>;
pub(super) fn windows_map(enabled: bool, server: &[u32]) -> RawMap {
    let mut map = RawMap::new();
    if !enabled {
        return map;
    }
    let expanded;
    let server = if !server.contains(&61) && !server.contains(&59) {
        let mut value = Vec::new();
        for (i, key) in ["http=", "https=", "ftp="].iter().enumerate() {
            if i > 0 {
                value.push(59);
            }
            value.extend(st::text(key));
            value.extend_from_slice(server);
        }
        expanded = value;
        &expanded[..]
    } else {
        server
    };
    for entry in server.split(|c| *c == 59) {
        let Some(eq) = entry.iter().position(|c| *c == 61) else {
            return map;
        };
        let protocol = &entry[..eq];
        let address = &entry[eq + 1..];
        let scheme = address.windows(3).position(|v| v == [58, 47, 47]);
        let prefixed =
            scheme.is_some_and(|at| at > 0 && !address[..at].iter().any(|c| matches!(c, 47 | 58)));
        let mut value = if prefixed {
            Vec::new()
        } else if ["http", "https", "ftp"]
            .iter()
            .any(|k| protocol == st::text(k))
        {
            st::text("http://")
        } else if protocol == st::text("socks") {
            st::text("socks://")
        } else {
            Vec::new()
        };
        value.extend_from_slice(address);
        map.insert(protocol.to_vec(), value);
    }
    if let Some(socks) = map
        .get(&st::text("socks"))
        .filter(|v| !v.is_empty())
        .cloned()
    {
        let value = if socks.starts_with(&st::text("socks://")) {
            [st::text("socks4://"), socks[8..].to_vec()].concat()
        } else {
            socks
        };
        for key in ["http", "https"] {
            let key = st::text(key);
            if map.get(&key).is_none_or(Vec::is_empty) {
                map.insert(key, value.clone());
            }
        }
    }
    map
}
/// Only HTTPX-consumed values must become Unicode scalar strings. Ignored
/// unknown/FTP entries still suppress environment fallback before this boundary.
pub(super) fn httpx_map(raw: RawMap) -> crate::Result<BTreeMap<String, String>> {
    if raw.get(&st::text("no")).is_some_and(|v|v.split(|c|*c==44).any(|h|{let start=h.iter().position(|c|!matches!(*c,9..=13|28..=32|133|160|5760|8192..=8202|8232|8233|8239|8287|12288)).unwrap_or(h.len());let end=h.iter().rposition(|c|!matches!(*c,9..=13|28..=32|133|160|5760|8192..=8202|8232|8233|8239|8287|12288)).map_or(start,|p|p+1);h[start..end]==[42]})){return Ok(BTreeMap::from([("no".into(),"*".into())]));}
    let mut out = BTreeMap::new();
    for (key, value) in raw {
        let Some(key) = st::string(&key) else {
            out.insert("\0source-opaque-proxy-key".into(), String::new());
            continue;
        };
        if ["http", "https", "all", "no"].contains(&key.as_str()) {
            let value = st::string(&value).ok_or_else(|| {
                crate::error::builder("source HTTPX URL input contains an unpaired surrogate")
            })?;
            out.insert(key, value);
        } else {
            out.insert(key, st::string(&value).unwrap_or_default());
        }
    }
    Ok(out)
}
pub(super) trait WindowsApi {
    fn open(&self, root: isize, path: &[u16], options: u32, access: u32) -> Result<usize, u32>;
    /// None is the size query. Returns actual native status, type and byte size.
    fn query(&self, key: usize, name: &[u16], buffer: Option<&mut [u8]>) -> (u32, u32, u32);
    fn close(&self, key: usize) -> u32;
}
struct Key<'a, A: WindowsApi> {
    api: &'a A,
    key: usize,
}
impl<A: WindowsApi> Drop for Key<'_, A> {
    fn drop(&mut self) {
        self.api.close(self.key);
    }
}
fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}
fn query(api: &impl WindowsApi, key: usize, name: &str) -> crate::Result<Option<st::Value>> {
    let name = wide(name);
    let (status, _, reported) = api.query(key, &name, None);
    let mut size = if status == 234 {
        256
    } else if status == 0 {
        reported
    } else {
        return Ok(None);
    };
    loop {
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(size as usize)
            .map_err(crate::error::builder)?;
        bytes.resize(size as usize, 0);
        let (status, kind, actual) = api.query(key, &name, Some(&mut bytes));
        if status == 234 {
            size = size.checked_mul(2).ok_or_else(|| {
                crate::error::builder("registry source retry allocation overflow")
            })?;
            continue;
        }
        if status != 0 {
            return Ok(None);
        }
        if actual > size {
            return Err(crate::error::builder(
                "registry API success exceeds allocated buffer",
            ));
        }
        bytes.truncate(actual as usize);
        return st::registry(kind, &bytes).map(Some);
    }
}
pub(super) fn read_windows(api: &impl WindowsApi) -> crate::Result<RawMap> {
    let key = match api.open(
        -2147483647,
        &wide("Software\\Microsoft\\Windows\\CurrentVersion\\Internet Settings"),
        0,
        131097,
    ) {
        Ok(key) => Key { api, key },
        Err(_) => return Ok(RawMap::new()),
    };
    let Some(enabled) = query(api, key.key, "ProxyEnable")? else {
        return Ok(RawMap::new());
    };
    if !enabled.truth() {
        return Ok(RawMap::new());
    }
    let Some(server) = query(api, key.key, "ProxyServer")? else {
        return Ok(RawMap::new());
    };
    Ok(windows_map(true, &server.spelling()))
}
pub(super) trait MacApi {
    fn copy_proxies(&self) -> usize;
    fn release(&self, dictionary: usize);
    fn number(&self, dictionary: usize, key: &str) -> crate::Result<Option<i32>>;
    fn string(&self, dictionary: usize, key: &str) -> crate::Result<Option<String>>;
}
struct Dictionary<'a, A: MacApi> {
    api: &'a A,
    id: usize,
}
impl<A: MacApi> Drop for Dictionary<'_, A> {
    fn drop(&mut self) {
        self.api.release(self.id);
    }
}
pub(super) fn read_macos(api: &impl MacApi) -> crate::Result<RawMap> {
    let id = api.copy_proxies();
    if id == 0 {
        return Ok(RawMap::new());
    }
    let dictionary = Dictionary { api, id };
    let mut out = RawMap::new();
    for (protocol, prefix) in [
        ("http", "HTTP"),
        ("https", "HTTPS"),
        ("ftp", "FTP"),
        ("gopher", "Gopher"),
        ("socks", "SOCKS"),
    ] {
        let enabled = api
            .number(dictionary.id, &format!("{prefix}Enable"))?
            .unwrap_or(0);
        if enabled == 0 {
            continue;
        }
        let host = api.string(dictionary.id, &format!("{prefix}Proxy"))?;
        if let Some(host) = host {
            let port = api.number(dictionary.id, &format!("{prefix}Port"))?;
            let host = host.split('\0').next().unwrap();
            out.insert(
                st::text(protocol),
                st::text(&match port {
                    Some(port) => format!("http://{host}:{port}"),
                    None => format!("http://{host}"),
                }),
            );
        }
    }
    Ok(out)
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::collections::VecDeque;
    struct Win {
        denied: bool,
        close_status: u32,
        events: RefCell<Vec<String>>,
        replies: RefCell<VecDeque<(u32, u32, Vec<u8>, u32)>>,
    }
    impl Win {
        fn new(replies: Vec<(u32, u32, Vec<u8>, u32)>) -> Self {
            Self {
                denied: false,
                close_status: 0,
                events: RefCell::new(Vec::new()),
                replies: RefCell::new(replies.into()),
            }
        }
    }
    impl WindowsApi for Win {
        fn open(&self, root: isize, path: &[u16], options: u32, access: u32) -> Result<usize, u32> {
            assert_eq!(root, -2147483647);
            assert_eq!(
                path,
                wide("Software\\Microsoft\\Windows\\CurrentVersion\\Internet Settings")
            );
            assert_eq!(options, 0);
            assert_eq!(access, 131097, "source OpenKey must request KEY_READ");
            self.events.borrow_mut().push("open".into());
            if self.denied {
                Err(5)
            } else {
                Ok(42)
            }
        }
        fn query(&self, key: usize, name: &[u16], buffer: Option<&mut [u8]>) -> (u32, u32, u32) {
            assert_eq!(key, 42);
            let label = String::from_utf16(&name[..name.len() - 1]).unwrap();
            let cap = buffer.as_ref().map_or(0, |b| b.len());
            self.events.borrow_mut().push(format!("{label}:{cap}"));
            let (status, kind, bytes, size) = self
                .replies
                .borrow_mut()
                .pop_front()
                .expect("finite API fixture exhausted");
            if let Some(buffer) = buffer {
                if status == 0 {
                    buffer[..bytes.len()].copy_from_slice(&bytes);
                }
            }
            (status, kind, size)
        }
        fn close(&self, key: usize) -> u32 {
            assert_eq!(key, 42);
            self.events.borrow_mut().push("close".into());
            self.close_status
        }
    }
    fn value(kind: u32, bytes: Vec<u8>) -> Vec<(u32, u32, Vec<u8>, u32)> {
        let len = bytes.len() as u32;
        vec![(0, kind, vec![], len), (0, kind, bytes, len)]
    }
    fn utf16(s: &str) -> Vec<u8> {
        s.encode_utf16()
            .chain(Some(0))
            .flat_map(u16::to_le_bytes)
            .collect()
    }
    #[test]
    fn windows_api_key_read_acl_denial_has_no_query_or_close() {
        let mut api = Win::new(vec![]);
        api.denied = true;
        assert!(read_windows(&api).unwrap().is_empty());
        assert_eq!(*api.events.borrow(), ["open"]);
    }
    #[test]
    fn windows_api_multisz_truth_and_scalar_server_read_close() {
        let mut replies = value(7, vec![0, 0, 0, 0]);
        replies.extend(value(4, 42u32.to_le_bytes().to_vec()));
        let api = Win::new(replies);
        let map = read_windows(&api).unwrap();
        assert_eq!(map[&st::text("http")], st::text("http://42"));
        assert_eq!(api.events.borrow().last().unwrap(), "close");
        assert_eq!(
            api.events.borrow().iter().filter(|e| *e == "close").count(),
            1
        );
        assert!(api.replies.borrow().is_empty());
    }
    #[test]
    fn windows_api_more_data_retries_source_256_and_doubles_then_closes() {
        let mut replies = vec![
            (234, 0, vec![], 9),
            (234, 4, vec![], 500),
            (0, 4, 1u32.to_le_bytes().to_vec(), 4),
        ];
        replies.extend(value(1, utf16("proxy:80")));
        let api = Win::new(replies);
        assert_eq!(
            read_windows(&api).unwrap()[&st::text("https")],
            st::text("http://proxy:80")
        );
        assert_eq!(
            &api.events.borrow()[1..4],
            ["ProxyEnable:0", "ProxyEnable:256", "ProxyEnable:512"]
        );
        assert_eq!(api.events.borrow().last().unwrap(), "close");
    }
    #[test]
    fn windows_api_read_error_and_decode_error_release_owned_key() {
        for replies in [vec![(2, 0, vec![], 0)], value(4, vec![1])] {
            let api = Win::new(replies);
            let result = read_windows(&api);
            assert!(result.is_err() || result.unwrap().is_empty());
            assert_eq!(api.events.borrow().last().unwrap(), "close");
            assert_eq!(
                api.events.borrow().iter().filter(|e| *e == "close").count(),
                1
            );
        }
    }
    #[test]
    fn windows_api_binary_server_and_partial_parse_are_source_spelled() {
        let mut replies = value(4, 1u32.to_le_bytes().to_vec());
        replies.extend(value(3, b"http=x;bad".to_vec()));
        let api = Win::new(replies);
        let map = read_windows(&api).unwrap();
        assert_eq!(map[&st::text("b'http")], st::text("x"));
        assert_eq!(map.len(), 1);
        assert_eq!(api.events.borrow().last().unwrap(), "close");
    }
    struct Mac {
        empty: bool,
        error: bool,
        events: RefCell<Vec<String>>,
    }
    impl MacApi for Mac {
        fn copy_proxies(&self) -> usize {
            self.events.borrow_mut().push("copy".into());
            if self.empty {
                0
            } else {
                7
            }
        }
        fn release(&self, id: usize) {
            assert_eq!(id, 7);
            self.events.borrow_mut().push("release".into());
        }
        fn number(&self, id: usize, key: &str) -> crate::Result<Option<i32>> {
            assert_eq!(id, 7);
            self.events.borrow_mut().push(key.into());
            Ok(match key {
                "HTTPEnable" => Some(1),
                "HTTPPort" => Some(-1),
                _ => None,
            })
        }
        fn string(&self, id: usize, key: &str) -> crate::Result<Option<String>> {
            assert_eq!(id, 7);
            assert_eq!(key, "HTTPProxy");
            self.events.borrow_mut().push(key.into());
            if self.error {
                Err(crate::error::builder("injected CF conversion error"))
            } else {
                Ok(Some("proxy".into()))
            }
        }
    }
    #[test]
    fn mac_api_null_copy_does_not_release() {
        let api = Mac {
            empty: true,
            error: false,
            events: RefCell::new(vec![]),
        };
        assert!(read_macos(&api).unwrap().is_empty());
        assert_eq!(*api.events.borrow(), ["copy"]);
    }
    #[test]
    fn mac_api_read_success_and_error_release_exactly_once() {
        for error in [false, true] {
            let api = Mac {
                empty: false,
                error,
                events: RefCell::new(vec![]),
            };
            let result = read_macos(&api);
            if error {
                assert!(result.is_err());
            } else {
                assert_eq!(
                    result.unwrap()[&st::text("http")],
                    st::text("http://proxy:-1")
                );
            }
            assert_eq!(api.events.borrow().last().unwrap(), "release");
            assert_eq!(
                api.events
                    .borrow()
                    .iter()
                    .filter(|e| *e == "release")
                    .count(),
                1
            );
        }
    }
    #[test]
    fn windows_api_close_error_preserves_completed_map() {
        let mut replies = value(4, 1u32.to_le_bytes().to_vec());
        replies.extend(value(1, utf16("proxy")));
        let mut api = Win::new(replies);
        api.close_status = 5;
        let map = read_windows(&api).unwrap();
        assert_eq!(map[&st::text("http")], st::text("http://proxy"));
        assert_eq!(
            api.events.borrow().iter().filter(|e| *e == "close").count(),
            1
        );
    }

    struct MacOrder {
        missing_host: bool,
        events: RefCell<Vec<String>>,
    }
    impl MacApi for MacOrder {
        fn copy_proxies(&self) -> usize {
            self.events.borrow_mut().push("copy".into());
            7
        }
        fn release(&self, id: usize) {
            assert_eq!(id, 7);
            self.events.borrow_mut().push("release".into());
        }
        fn number(&self, id: usize, key: &str) -> crate::Result<Option<i32>> {
            assert_eq!(id, 7);
            self.events.borrow_mut().push(key.into());
            match key {
                "HTTPEnable" | "HTTPSEnable" => Ok(Some(1)),
                "HTTPPort" => Err(crate::error::builder("incompatible reached Port")),
                "HTTPSPort" => Ok(Some(81)),
                _ => Ok(None),
            }
        }
        fn string(&self, id: usize, key: &str) -> crate::Result<Option<String>> {
            assert_eq!(id, 7);
            self.events.borrow_mut().push(key.into());
            match key {
                "HTTPProxy" => Ok(if self.missing_host {
                    None
                } else {
                    Some("early".into())
                }),
                "HTTPSProxy" => Ok(Some("later".into())),
                _ => unreachable!("fixed enabled source protocol"),
            }
        }
    }
    #[test]
    fn mac_api_missing_host_skips_port_and_reached_port_error_releases() {
        for missing_host in [true, false] {
            let api = MacOrder {
                missing_host,
                events: RefCell::new(vec![]),
            };
            let result = read_macos(&api);
            let events = api.events.borrow();
            assert_eq!(events.last().map(String::as_str), Some("release"));
            assert_eq!(events.iter().filter(|e| e.as_str() == "release").count(), 1);
            if missing_host {
                assert!(
                    result.is_ok(),
                    "source missing host must skip Port conversion before retaining later protocol"
                );
                let map = result.unwrap();
                assert_eq!(map.len(), 1);
                assert_eq!(map[&st::text("https")], st::text("http://later:81"));
                assert!(!events.iter().any(|e| e == "HTTPPort"));
            } else {
                assert!(result.is_err());
                assert_eq!(
                    events.iter().filter(|e| e.as_str() == "HTTPPort").count(),
                    1
                );
                assert!(!events.iter().any(|e| e == "HTTPSEnable"));
            }
        }
    }
}
