use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6};

// Operational framing ceilings, not URL/hostname/source-integer validation.
pub(crate) const FRAME_LIMIT: usize = 1024 * 1024;
pub(crate) const STDERR_LIMIT: usize = 64 * 1024;
pub(crate) const MAGIC: &[u8; 16] = b"SSX-RESOLVER-v1\0";

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
    /// errno captured immediately for EAI_SYSTEM.
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

pub(crate) fn socket_port(decimal: &str) -> Result<u16, SourceResolveError> {
    let value = decimal
        .parse::<i32>()
        .map_err(|_| SourceResolveError::new(SourceResolveStage::SocketCInt))?;
    u16::try_from(value).map_err(|_| SourceResolveError::new(SourceResolveStage::SocketPort))
}
pub(crate) fn request(host: &str, decimal: &str) -> Result<Vec<u8>, SourceResolveError> {
    let digits = decimal.strip_prefix('-').unwrap_or(decimal);
    if !host.is_ascii()
        || host.contains('\0')
        || digits.is_empty()
        || digits.len() > 4300
        || !digits.bytes().all(|b| b.is_ascii_digit())
        || (digits.len() > 1 && digits.starts_with('0'))
        || decimal == "-0"
    {
        return Err(SourceResolveError::new(SourceResolveStage::InputContract));
    }
    let size = MAGIC
        .len()
        .checked_add(8)
        .and_then(|n| n.checked_add(host.len()))
        .and_then(|n| n.checked_add(decimal.len()))
        .ok_or_else(|| SourceResolveError::new(SourceResolveStage::OperationalLimit))?;
    if size > FRAME_LIMIT {
        return Err(SourceResolveError::new(
            SourceResolveStage::OperationalLimit,
        ));
    }
    let mut result = Vec::with_capacity(size);
    result.extend_from_slice(MAGIC);
    result.extend_from_slice(&(host.len() as u32).to_be_bytes());
    result.extend_from_slice(&(decimal.len() as u32).to_be_bytes());
    result.extend_from_slice(host.as_bytes());
    result.extend_from_slice(decimal.as_bytes());
    Ok(result)
}
pub(crate) fn decode_request(bytes: &[u8]) -> std::io::Result<(&str, &str)> {
    fn bad() -> std::io::Error {
        std::io::Error::from(std::io::ErrorKind::InvalidData)
    }
    if bytes.len() < 24 || bytes.len() > FRAME_LIMIT || &bytes[..16] != MAGIC {
        return Err(bad());
    }
    let h = u32::from_be_bytes(bytes[16..20].try_into().map_err(|_| bad())?) as usize;
    let p = u32::from_be_bytes(bytes[20..24].try_into().map_err(|_| bad())?) as usize;
    if h.checked_add(p).and_then(|n| n.checked_add(24)) != Some(bytes.len()) {
        return Err(bad());
    }
    let host = std::str::from_utf8(&bytes[24..24 + h]).map_err(|_| bad())?;
    let port = std::str::from_utf8(&bytes[24 + h..]).map_err(|_| bad())?;
    request(host, port).map_err(|_| bad())?;
    Ok((host, port))
}
// Success records carry family + port + flow + scope + exact address octets.
// Stream TCP hints ensure all supported records are AF_INET/AF_INET6.
pub(crate) fn response(addrs: &[SocketAddr]) -> Result<Vec<u8>, SourceResolveError> {
    let mut result = Vec::new();
    result.extend_from_slice(MAGIC);
    result.push(0);
    result.extend_from_slice(&(addrs.len() as u32).to_be_bytes());
    for addr in addrs {
        match addr {
            SocketAddr::V4(addr) => {
                result.push(4);
                result.extend_from_slice(&addr.port().to_be_bytes());
                result.extend_from_slice(&addr.ip().octets());
            }
            SocketAddr::V6(addr) => {
                result.push(6);
                result.extend_from_slice(&addr.port().to_be_bytes());
                result.extend_from_slice(&addr.flowinfo().to_be_bytes());
                result.extend_from_slice(&addr.scope_id().to_be_bytes());
                result.extend_from_slice(&addr.ip().octets());
            }
        }
        if result.len() > FRAME_LIMIT {
            return Err(SourceResolveError::new(
                SourceResolveStage::OperationalLimit,
            ));
        }
    }
    Ok(result)
}
pub(crate) fn error_response(gai: i32, errno: i32) -> Vec<u8> {
    let mut result = MAGIC.to_vec();
    result.push(1);
    result.extend_from_slice(&gai.to_be_bytes());
    result.extend_from_slice(&errno.to_be_bytes());
    result
}
pub(crate) fn decode_response(
    bytes: &[u8],
    stage: SourceResolveStage,
) -> Result<Vec<SocketAddr>, SourceResolveError> {
    let bad = || SourceResolveError::new(SourceResolveStage::Helper);
    if bytes.len() < 17 || bytes.len() > FRAME_LIMIT || &bytes[..16] != MAGIC {
        return Err(bad());
    }
    if bytes[16] == 2 && bytes.len() == 17 {
        return Err(SourceResolveError::new(
            SourceResolveStage::OperationalLimit,
        ));
    }
    if bytes[16] == 1 && bytes.len() == 25 {
        return Err(SourceResolveError {
            stage,
            gai_code: Some(i32::from_be_bytes(
                bytes[17..21].try_into().map_err(|_| bad())?,
            )),
            os_code: match i32::from_be_bytes(bytes[21..25].try_into().map_err(|_| bad())?) {
                0 => None,
                n => Some(n),
            },
        });
    }
    if bytes[16] != 0 || bytes.len() < 21 {
        return Err(bad());
    }
    let count = u32::from_be_bytes(bytes[17..21].try_into().map_err(|_| bad())?) as usize;
    // Minimum wire record size validates count before allocation.
    if count > (bytes.len() - 21) / 7 {
        return Err(bad());
    }
    let mut rest = &bytes[21..];
    let mut addrs = Vec::with_capacity(count);
    for _ in 0..count {
        let needed = match rest.first() {
            Some(4) => 7,
            Some(6) => 27,
            _ => return Err(bad()),
        };
        if rest.len() < needed {
            return Err(bad());
        }
        let port = u16::from_be_bytes(rest[1..3].try_into().map_err(|_| bad())?);
        addrs.push(if rest[0] == 4 {
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
        });
        rest = &rest[needed..];
    }
    if !rest.is_empty() {
        return Err(bad());
    }
    Ok(addrs)
}

#[cfg(test)]
mod tests {
    use super::*;
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
        let addresses = vec![
            SocketAddr::V6(SocketAddrV6::new(Ipv6Addr::LOCALHOST, 9, 17, 4)),
            SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 7)),
        ];
        assert_eq!(
            decode_response(
                &response(&addresses).unwrap(),
                SourceResolveStage::AsyncIoGetAddrInfo
            )
            .unwrap(),
            addresses
        );
    }
    #[test]
    fn response_rejects_trailing_truncated_or_hostile_count() {
        let mut wire = response(&[]).unwrap();
        wire[17..21].copy_from_slice(&u32::MAX.to_be_bytes());
        assert!(decode_response(&wire, SourceResolveStage::AnyIoGetAddrInfo).is_err());
        assert!(decode_response(&wire[..17], SourceResolveStage::AnyIoGetAddrInfo).is_err());
    }
}
