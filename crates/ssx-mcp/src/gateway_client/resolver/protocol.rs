use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6};

// Operational framing ceilings, not URL/hostname/source-integer validation.
pub(crate) const FRAME_LIMIT: usize = 1024 * 1024;
pub(crate) const STDERR_LIMIT: usize = 64 * 1024;
pub(crate) const MAGIC: &[u8; 16] = b"SSX-RESOLVER-v2\0";

/// Source error ordering or operational helper failure, without peer input text.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceResolveStage {
    /// The canonical decimal source integer could not be represented as C int.
    SocketCInt,
    /// C int conversion succeeded, then socket tuple port range failed.
    SocketPort,
    /// AnyIO's first hostname lookup, before socket tuple conversion.
    AnyIoGetAddrInfo,
    /// asyncio's scoped-literal lookup; returned sockaddr remains authoritative.
    AsyncIoGetAddrInfo,
    /// Native inet_ntop failed while projecting the first AnyIO lookup.
    AnyIoAddressText,
    /// No addresses returned by a successful native lookup.
    EmptyAddresses,
    /// Operational framing/resource limit, never a source URL domain limit.
    OperationalLimit,
    /// Helper protocol, artifact, IO or exit failed.
    Helper,
    /// Request drop or application shutdown cancelled the owned lookup.
    Cancelled,
    /// Application has closed admission.
    Closed,
    /// Caller-supplied lookup budget elapsed; connector maps to its DNS/TCP timeout.
    TimedOut,
    /// Invalid caller contract (canonical source integer or ASCII host).
    InputContract,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceResolveError {
    pub stage: SourceResolveStage,
    /// Native getaddrinfo error, where present; no interpreted wrapping claim.
    pub gai_code: Option<i32>,
    /// errno for EAI_SYSTEM or this runtime's native text-conversion diagnostic.
    /// Stale CRT-state equality across Python/Rust runtimes is not assumed.
    pub os_code: Option<i32>,
}
impl SourceResolveError {
    pub(crate) fn new(stage: SourceResolveStage) -> Self {
        Self {
            stage,
            gai_code: None,
            os_code: None,
        }
    }
}
impl std::fmt::Display for SourceResolveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "source endpoint resolver: {:?}", self.stage)
    }
}
impl std::error::Error for SourceResolveError {}

/// Finite roles, not a port sentinel. DnsOnly has no service field at all.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub(crate) enum RequestKind {
    DnsOnly = 0,
    AnyIoCandidates = 1,
    AsyncIoCandidate = 2,
}
impl RequestKind {
    fn from_byte(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::DnsOnly),
            1 => Some(Self::AnyIoCandidates),
            2 => Some(Self::AsyncIoCandidate),
            _ => None,
        }
    }
    fn phase(self) -> u8 {
        self as u8
    }
    fn stage(self) -> SourceResolveStage {
        match self {
            Self::AsyncIoCandidate => SourceResolveStage::AsyncIoGetAddrInfo,
            _ => SourceResolveStage::AnyIoGetAddrInfo,
        }
    }
}
#[derive(Clone, Copy, Debug)]
pub(crate) struct Request<'a> {
    pub kind: RequestKind,
    pub host: &'a str,
    pub decimal: Option<&'a str>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Family {
    V4,
    V6,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Candidate {
    pub family: Family,
    pub native_host_text: String,
}
pub(crate) struct Reply {
    pub addresses: Vec<SocketAddr>,
    #[cfg(any(test, feature = "gateway-source-endpoints"))]
    pub candidates: Vec<Candidate>,
}
#[cfg(unix)]
pub(crate) fn socket_port(decimal: &str) -> Result<u16, SourceResolveError> {
    let n = decimal
        .parse::<i32>()
        .map_err(|_| SourceResolveError::new(SourceResolveStage::SocketCInt))?;
    u16::try_from(n).map_err(|_| SourceResolveError::new(SourceResolveStage::SocketPort))
}
#[cfg(windows)]
pub(crate) fn socket_port(decimal: &str) -> Result<u16, SourceResolveError> {
    ssx_windows_owner::source_connect_port(decimal)
        .map_err(|_| SourceResolveError::new(SourceResolveStage::InputContract))
}
fn decimal_ok(value: &str) -> bool {
    let digits = value.strip_prefix('-').unwrap_or(value);
    !digits.is_empty()
        && digits.len() <= 4300
        && digits.bytes().all(|c| c.is_ascii_digit())
        && !(digits.len() > 1 && digits.starts_with('0'))
        && value != "-0"
}
pub(crate) fn request(
    kind: RequestKind,
    host: &str,
    decimal: Option<&str>,
) -> Result<Vec<u8>, SourceResolveError> {
    if !host.is_ascii()
        || host.contains('\0')
        || match kind {
            RequestKind::DnsOnly => decimal.is_some(),
            _ => decimal.is_none_or(|d| !decimal_ok(d)),
        }
    {
        return Err(SourceResolveError::new(SourceResolveStage::InputContract));
    }
    let service = decimal.unwrap_or("");
    let size = 25usize
        .checked_add(host.len())
        .and_then(|v| v.checked_add(service.len()))
        .ok_or_else(|| SourceResolveError::new(SourceResolveStage::OperationalLimit))?;
    if size > FRAME_LIMIT {
        return Err(SourceResolveError::new(
            SourceResolveStage::OperationalLimit,
        ));
    }
    let mut out = Vec::with_capacity(size);
    out.extend_from_slice(MAGIC);
    out.push(kind as u8);
    out.extend_from_slice(&(host.len() as u32).to_be_bytes());
    out.extend_from_slice(&(service.len() as u32).to_be_bytes());
    out.extend_from_slice(host.as_bytes());
    out.extend_from_slice(service.as_bytes());
    Ok(out)
}
pub(crate) fn decode_request(raw: &[u8]) -> std::io::Result<Request<'_>> {
    let bad = || std::io::Error::from(std::io::ErrorKind::InvalidData);
    if raw.len() < 25 || raw.len() > FRAME_LIMIT || &raw[..16] != MAGIC {
        return Err(bad());
    }
    let kind = RequestKind::from_byte(raw[16]).ok_or_else(bad)?;
    let h = u32::from_be_bytes(raw[17..21].try_into().map_err(|_| bad())?) as usize;
    let p = u32::from_be_bytes(raw[21..25].try_into().map_err(|_| bad())?) as usize;
    if h.checked_add(p).and_then(|n| n.checked_add(25)) != Some(raw.len()) {
        return Err(bad());
    }
    let host = std::str::from_utf8(&raw[25..25 + h]).map_err(|_| bad())?;
    let service = std::str::from_utf8(&raw[25 + h..]).map_err(|_| bad())?;
    let decimal = if kind == RequestKind::DnsOnly {
        if !service.is_empty() {
            return Err(bad());
        }
        None
    } else {
        Some(service)
    };
    request(kind, host, decimal).map_err(|_| bad())?;
    Ok(Request {
        kind,
        host,
        decimal,
    })
}
fn header(kind: RequestKind, status: u8) -> Vec<u8> {
    let mut out = MAGIC.to_vec();
    out.push(kind as u8);
    out.push(status);
    out
}
pub(crate) fn operational_response(kind: RequestKind) -> Vec<u8> {
    header(kind, 2)
}
pub(crate) fn error_response(kind: RequestKind, gai: i32, errno: i32) -> Vec<u8> {
    let mut out = header(kind, 1);
    out.push(kind.phase());
    out.extend_from_slice(&gai.to_be_bytes());
    out.extend_from_slice(&errno.to_be_bytes());
    out
}
fn text_error(kind: RequestKind, errno: i32) -> Vec<u8> {
    let mut out = header(kind, 3);
    out.push(3);
    out.extend_from_slice(&errno.to_be_bytes());
    out
}
fn native_text(address: std::net::IpAddr) -> std::io::Result<String> {
    #[cfg(unix)]
    {
        ssx_native_ip_text::numeric_host(address)
    }
    #[cfg(windows)]
    {
        ssx_windows_owner::numeric_host(address)
    }
}
fn first_candidate(address: &SocketAddr) -> std::io::Result<Candidate> {
    let mut host = native_text(address.ip())?;
    if let SocketAddr::V6(v6) = address
        && v6.scope_id() != 0
    {
        host.truncate(host.find('%').unwrap_or(host.len()));
        host.push('%');
        host.push_str(&v6.scope_id().to_string());
    }
    Ok(Candidate {
        family: if address.is_ipv4() {
            Family::V4
        } else {
            Family::V6
        },
        native_host_text: host,
    })
}
pub(crate) fn response(
    kind: RequestKind,
    addresses: &[SocketAddr],
) -> Result<Vec<u8>, SourceResolveError> {
    let mut out = header(kind, 0);
    let count = u32::try_from(addresses.len())
        .map_err(|_| SourceResolveError::new(SourceResolveStage::OperationalLimit))?;
    out.extend_from_slice(&count.to_be_bytes());
    for address in addresses {
        match address {
            SocketAddr::V4(a) => {
                out.push(4);
                out.extend_from_slice(&a.port().to_be_bytes());
                out.extend_from_slice(&a.ip().octets());
            }
            SocketAddr::V6(a) => {
                out.push(6);
                out.extend_from_slice(&a.port().to_be_bytes());
                out.extend_from_slice(&a.flowinfo().to_be_bytes());
                out.extend_from_slice(&a.scope_id().to_be_bytes());
                out.extend_from_slice(&a.ip().octets());
            }
        }
        if kind == RequestKind::AnyIoCandidates {
            let candidate = match first_candidate(address) {
                Ok(c) => c,
                Err(e) => return Ok(text_error(kind, e.raw_os_error().unwrap_or(0))),
            };
            let host = candidate.native_host_text.as_bytes();
            let len = u16::try_from(host.len())
                .map_err(|_| SourceResolveError::new(SourceResolveStage::OperationalLimit))?;
            out.extend_from_slice(&len.to_be_bytes());
            out.extend_from_slice(host);
        }
        if out.len() > FRAME_LIMIT {
            return Err(SourceResolveError::new(
                SourceResolveStage::OperationalLimit,
            ));
        }
    }
    Ok(out)
}
pub(crate) fn decode_response(
    raw: &[u8],
    expected: RequestKind,
) -> Result<Reply, SourceResolveError> {
    let bad = || SourceResolveError::new(SourceResolveStage::Helper);
    if raw.len() < 18 || raw.len() > FRAME_LIMIT || &raw[..16] != MAGIC || raw[16] != expected as u8
    {
        return Err(bad());
    }
    match raw[17] {
        2 if raw.len() == 18 => {
            return Err(SourceResolveError::new(
                SourceResolveStage::OperationalLimit,
            ));
        }
        1 if raw.len() == 27 && raw[18] == expected.phase() => {
            return Err(SourceResolveError {
                stage: expected.stage(),
                gai_code: Some(i32::from_be_bytes(
                    raw[19..23].try_into().map_err(|_| bad())?,
                )),
                os_code: match i32::from_be_bytes(raw[23..27].try_into().map_err(|_| bad())?) {
                    0 => None,
                    n => Some(n),
                },
            });
        }
        3 if expected == RequestKind::AnyIoCandidates && raw.len() == 23 && raw[18] == 3 => {
            return Err(SourceResolveError {
                stage: SourceResolveStage::AnyIoAddressText,
                gai_code: None,
                os_code: Some(i32::from_be_bytes(
                    raw[19..23].try_into().map_err(|_| bad())?,
                )),
            });
        }
        0 => (),
        _ => return Err(bad()),
    }
    if raw.len() < 22 {
        return Err(bad());
    }
    let count = u32::from_be_bytes(raw[18..22].try_into().map_err(|_| bad())?) as usize;
    if count > (raw.len() - 22) / 7 {
        return Err(bad());
    }
    let mut rest = &raw[22..];
    let mut addresses = Vec::with_capacity(count);
    #[cfg(any(test, feature = "gateway-source-endpoints"))]
    let mut candidates = Vec::new();
    for _ in 0..count {
        let n = match rest.first() {
            Some(4) => 7,
            Some(6) => 27,
            _ => return Err(bad()),
        };
        if rest.len() < n {
            return Err(bad());
        }
        let port = u16::from_be_bytes(rest[1..3].try_into().map_err(|_| bad())?);
        let address = if rest[0] == 4 {
            SocketAddr::V4(SocketAddrV4::new(
                Ipv4Addr::from(<[u8; 4]>::try_from(&rest[3..7]).map_err(|_| bad())?),
                port,
            ))
        } else {
            SocketAddr::V6(SocketAddrV6::new(
                Ipv6Addr::from(<[u8; 16]>::try_from(&rest[11..27]).map_err(|_| bad())?),
                port,
                u32::from_be_bytes(rest[3..7].try_into().map_err(|_| bad())?),
                u32::from_be_bytes(rest[7..11].try_into().map_err(|_| bad())?),
            ))
        };
        rest = &rest[n..];
        if expected == RequestKind::AnyIoCandidates {
            if rest.len() < 2 {
                return Err(bad());
            }
            let len = u16::from_be_bytes(rest[..2].try_into().map_err(|_| bad())?) as usize;
            rest = &rest[2..];
            if rest.len() < len {
                return Err(bad());
            }
            let host = std::str::from_utf8(&rest[..len]).map_err(|_| bad())?;
            rest = &rest[len..];
            if !host.is_ascii() || host.contains('\0') {
                return Err(bad());
            }
            let (ip, scope) = host
                .split_once('%')
                .map_or((host, None), |(a, b)| (a, Some(b)));
            if ip.parse::<std::net::IpAddr>().ok() != Some(address.ip()) {
                return Err(bad());
            }
            match address {
                SocketAddr::V4(_) => {
                    if scope.is_some() {
                        return Err(bad());
                    }
                }
                SocketAddr::V6(v6) => {
                    if (v6.scope_id() == 0 && scope.is_some())
                        || (v6.scope_id() != 0 && scope != Some(v6.scope_id().to_string().as_str()))
                    {
                        return Err(bad());
                    }
                }
            }
            #[cfg(any(test, feature = "gateway-source-endpoints"))]
            candidates.push(Candidate {
                family: if address.is_ipv4() {
                    Family::V4
                } else {
                    Family::V6
                },
                native_host_text: host.to_owned(),
            });
        }
        addresses.push(address);
    }
    if !rest.is_empty() {
        return Err(bad());
    }
    Ok(Reply {
        addresses,
        #[cfg(any(test, feature = "gateway-source-endpoints"))]
        candidates,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(unix)]
    #[test]
    fn socket_conversion_order_is_distinct() {
        assert_eq!(
            socket_port("65536").unwrap_err().stage,
            SourceResolveStage::SocketPort
        );
        assert_eq!(
            socket_port("-1").unwrap_err().stage,
            SourceResolveStage::SocketPort
        );
        assert_eq!(
            socket_port("2147483648").unwrap_err().stage,
            SourceResolveStage::SocketCInt
        );
        assert_eq!(
            socket_port(&"9".repeat(4300)).unwrap_err().stage,
            SourceResolveStage::SocketCInt
        );
    }
    #[test]
    fn sockaddr_response_preserves_sequence_scope_flow_and_returned_port() {
        let a = vec![
            SocketAddr::V6(SocketAddrV6::new(Ipv6Addr::LOCALHOST, 9, 17, 4)),
            SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 7)),
        ];
        assert_eq!(
            decode_response(
                &response(RequestKind::AsyncIoCandidate, &a).unwrap(),
                RequestKind::AsyncIoCandidate
            )
            .unwrap()
            .addresses,
            a
        );
    }
    #[test]
    fn response_rejects_trailing_truncated_or_hostile_count() {
        let mut b = response(RequestKind::DnsOnly, &[]).unwrap();
        b[18..22].copy_from_slice(&u32::MAX.to_be_bytes());
        assert!(decode_response(&b, RequestKind::DnsOnly).is_err());
        assert!(decode_response(&b[..18], RequestKind::DnsOnly).is_err());
        let mut trailing = response(RequestKind::DnsOnly, &[]).unwrap();
        trailing.push(0);
        assert!(decode_response(&trailing, RequestKind::DnsOnly).is_err());
    }
    #[test]
    fn real_zero_is_not_dns_only_and_v1_or_cross_kind_is_rejected() {
        let a = request(RequestKind::AnyIoCandidates, "example.invalid", Some("0")).unwrap();
        let b = request(RequestKind::DnsOnly, "example.invalid", None).unwrap();
        assert_ne!(a, b);
        assert_eq!(decode_request(&a).unwrap().decimal, Some("0"));
        assert!(request(RequestKind::DnsOnly, "example.invalid", Some("0")).is_err());
        let mut old = a.clone();
        old[..16].copy_from_slice(b"SSX-RESOLVER-v1\0");
        assert!(decode_request(&old).is_err());
        let wrong = response(RequestKind::DnsOnly, &[]).unwrap();
        assert!(decode_response(&wrong, RequestKind::AsyncIoCandidate).is_err());
    }
    #[test]
    fn first_scope_text_and_secondary_native_sockaddr_are_distinct() {
        let first = vec![SocketAddr::V6(SocketAddrV6::new(
            Ipv6Addr::LOCALHOST,
            1,
            77,
            4,
        ))];
        let row = decode_response(
            &response(RequestKind::AnyIoCandidates, &first).unwrap(),
            RequestKind::AnyIoCandidates,
        )
        .unwrap();
        assert_eq!(row.candidates[0].native_host_text, "::1%4");
        assert_eq!(row.addresses, first);
        let last = vec![SocketAddr::V6(SocketAddrV6::new(
            Ipv6Addr::LOCALHOST,
            9,
            88,
            7,
        ))];
        assert_eq!(
            decode_response(
                &response(RequestKind::AsyncIoCandidate, &last).unwrap(),
                RequestKind::AsyncIoCandidate
            )
            .unwrap()
            .addresses,
            last
        );
    }
    #[test]
    fn request_kind_and_error_phase_are_closed() {
        for k in [
            RequestKind::DnsOnly,
            RequestKind::AnyIoCandidates,
            RequestKind::AsyncIoCandidate,
        ] {
            let mut raw = error_response(k, -2, 13);
            assert_eq!(decode_response(&raw, k).err().unwrap().stage, k.stage());
            raw[18] = 255;
            assert!(decode_response(&raw, k).is_err());
        }
        let mut raw = request(RequestKind::AnyIoCandidates, "x", Some("80")).unwrap();
        raw[16] = 255;
        assert!(decode_request(&raw).is_err());
    }
}
