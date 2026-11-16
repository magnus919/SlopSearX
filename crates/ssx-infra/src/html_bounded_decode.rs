//! Actual bounded decoder worker and parent ownership phase. The subsequent
//! legacy tree/row phase and shared transport/RSS overhead remain separately open.
use super::{DetectorFlags, OperationFailure, decode_wire};
use crate::{
    html_budget::{Charge, Operation},
    process_pool::ProcessPool,
};
use ssx_owned_budget::{Budget, OwnedVec};
use ssx_source_codecs::{BoundedDecodeError, BoundedTextError, BudgetText, BudgetTextBuilder};
use std::{
    io::{Read, Write},
    sync::{Arc, Mutex},
};
#[cfg(test)]
const VERSION: u8 = 2;
const OPERATION: u8 = 1;
#[cfg(test)]
const FIXED_HEADER: usize = 23;
pub(crate) struct Decoded {
    pub(crate) text: BudgetText,
    pub(crate) flags: DetectorFlags,
}
struct Lease {
    _operation: Arc<Operation>,
    child: Mutex<Option<Charge>>,
    _reply: Charge,
    _header: Charge,
    _metadata: Charge,
}
impl Lease {
    fn release_verified_child(&self) {
        drop(
            self.child
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .take(),
        );
    }
}
fn resource(_: ssx_owned_budget::Error) -> OperationFailure {
    OperationFailure::Resource
}
fn output(error: BoundedTextError) -> OperationFailure {
    match error {
        BoundedTextError::Deadline => OperationFailure::Deadline,
        BoundedTextError::Resource(_) => OperationFailure::Resource,
        _ => OperationFailure::Invariant("HTML helper decoder projection invariant"),
    }
}
fn check(operation: &Operation) -> Result<(), OperationFailure> {
    if tokio::time::Instant::now() >= operation.deadline {
        Err(OperationFailure::Deadline)
    } else {
        Ok(())
    }
}
#[cfg(test)]
pub(super) async fn decode(
    process: &ProcessPool,
    bytes: &[u8],
    label: &str,
    operation: Arc<Operation>,
) -> Result<Decoded, OperationFailure> {
    check(&operation)?;
    if bytes.len() > super::MAX_POINTS {
        return Err(OperationFailure::Invariant(
            "HTTP text body exceeds transport bound",
        ));
    }
    let checkpoint = || check(&operation).map_err(|_| BoundedTextError::Deadline);
    // HTTPX whole-empty-body bypasses registry selection. Raw header ownership
    // and exact MIME parameter selection will use the separately owned selector.
    let canonical = if bytes.is_empty() {
        "utf-8"
    } else {
        ssx_source_codecs::lookup_http_encoding_bounded(label, &operation.budget.0, &checkpoint)
            .map_err(|error| match error {
                ssx_source_codecs::BoundedParameterError::Output(error) => output(error),
                ssx_source_codecs::BoundedParameterError::ValueError(message) => {
                    OperationFailure::Failure(message.into())
                }
                _ => OperationFailure::Invariant("HTML helper codec lookup invariant"),
            })?
            .unwrap_or("utf-8")
    };
    // Pinned Rust ArcInner is repr(C): two atomic usize counters then data.
    let (layout, _) = std::alloc::Layout::new::<[usize; 2]>()
        .extend(std::alloc::Layout::new::<Lease>())
        .map_err(|_| OperationFailure::Resource)?;
    let metadata_charge = operation
        .budget
        .0
        .reserve(layout.pad_to_align().size())
        .map_err(resource)?;
    let reply_bound = decode_wire::bound(bytes.len())?;
    let reply_charge = operation.budget.0.reserve(reply_bound).map_err(resource)?;
    let header_size = FIXED_HEADER
        .checked_add(canonical.len())
        .ok_or(OperationFailure::Resource)?;
    let header_charge = operation.budget.0.reserve(header_size).map_err(resource)?;
    let mut input = OwnedVec::with_capacity(&operation.budget.0, bytes.len()).map_err(resource)?;
    input.try_extend_from_slice(bytes).map_err(resource)?;
    let snapshot = operation.budget.0.snapshot().map_err(resource)?;
    let allowance = snapshot
        .limit
        .checked_sub(snapshot.used)
        .ok_or(OperationFailure::Resource)?;
    if allowance == 0 || allowance < bytes.len() {
        return Err(OperationFailure::Resource);
    }
    let child_charge = operation.budget.0.reserve(allowance).map_err(resource)?;
    let mut header = Vec::new();
    header
        .try_reserve_exact(header_size)
        .map_err(|_| OperationFailure::Resource)?;
    if header.capacity() > header_size {
        operation.budget.0.fail_closed();
        return Err(OperationFailure::Resource);
    }
    header.extend(super::PROTOCOL_MAGIC);
    header.extend([
        VERSION,
        OPERATION,
        u8::try_from(canonical.len())
            .map_err(|_| OperationFailure::Invariant("invalid canonical codec length"))?,
    ]);
    header.extend(canonical.as_bytes());
    header.extend((bytes.len() as u64).to_be_bytes());
    header.extend((allowance as u64).to_be_bytes());
    if header.len() != header_size {
        return Err(OperationFailure::Invariant(
            "HTML bounded decoder header size",
        ));
    }
    let lease = Arc::new(Lease {
        _operation: operation.clone(),
        child: Mutex::new(Some(child_charge)),
        _reply: reply_charge,
        _header: header_charge,
        _metadata: metadata_charge,
    });
    let frame = process
        .exchange_with_lease(
            operation.deadline,
            header,
            input,
            reply_bound,
            lease.clone(),
        )
        .await
        .map_err(OperationFailure::from)?;
    finish(frame, lease, reply_bound, bytes.len(), 0, &operation).await
}
async fn finish(
    frame: Vec<u8>,
    lease: Arc<Lease>,
    reply_bound: usize,
    body_len: usize,
    header_bytes: usize,
    operation: &Arc<Operation>,
) -> Result<Decoded, OperationFailure> {
    // Only a successful common response proves exit/reap. On receiver drop or
    // uncertain cleanup the independent supervisor retains the lease instead.
    lease.release_verified_child();
    if frame.capacity() > reply_bound {
        operation.budget.0.fail_closed();
        return Err(OperationFailure::Resource);
    }
    let decoded =
        decode_wire::validate_http_async(&frame, body_len, header_bytes, &|| check(operation))
            .await?;
    project(decoded, &operation.budget.0, &|| check(operation)).await
}

async fn project(
    decoded: decode_wire::Decoded<'_>,
    budget: &Budget,
    checkpoint: &(dyn Fn() -> Result<(), OperationFailure> + Sync),
) -> Result<Decoded, OperationFailure> {
    let mut text = BudgetTextBuilder::new(budget, decoded.count).map_err(output)?;
    for chunk in decoded.bytes.chunks(1024 * 4) {
        for point in chunk.as_chunks::<4>().0 {
            checkpoint()?;
            text.try_push(u32::from_be_bytes(*point)).map_err(output)?;
        }
        tokio::task::yield_now().await;
        checkpoint()?;
    }
    checkpoint()?;
    Ok(Decoded {
        text: text.finish(),
        flags: decoded.flags,
    })
}

fn read_count(input: &mut impl Read) -> Result<usize, String> {
    let mut bytes = [0; 8];
    input
        .read_exact(&mut bytes)
        .map_err(|error| error.to_string())?;
    usize::try_from(u64::from_be_bytes(bytes))
        .map_err(|_| "HTML bounded decoder count overflow".into())
}
pub(super) fn worker(input: &mut impl Read, output: &mut impl Write) -> Result<(), String> {
    let mut fields = [0; 2];
    input
        .read_exact(&mut fields)
        .map_err(|error| error.to_string())?;
    if fields[0] != OPERATION {
        return Err("invalid bounded helper operation".into());
    }
    let mut codec = [0u8; 255];
    let codec = &mut codec[..usize::from(fields[1])];
    input.read_exact(codec).map_err(|error| error.to_string())?;
    let codec = std::str::from_utf8(codec).map_err(|_| "invalid bounded helper codec")?;
    let count = read_count(input)?;
    let allowance = read_count(input)?;
    if count > super::MAX_POINTS
        || allowance == 0
        || allowance > crate::html_budget::MAX_SEARCH_BYTES
    {
        return Err("invalid bounded helper allowance/count".into());
    }
    let budget = Budget::new(allowance).map_err(|error| error.to_string())?;
    let result = (|| -> Result<_, BoundedDecodeError> {
        let canonical = ssx_source_codecs::lookup_http_encoding_bounded(codec, &budget, &|| Ok(()))
            .map_err(|error| match error {
                ssx_source_codecs::BoundedParameterError::Output(error) => {
                    BoundedDecodeError::Output(error)
                }
                _ => BoundedDecodeError::RegistryInvariant,
            })?;
        if canonical != Some(codec) {
            return Err(BoundedDecodeError::RegistryInvariant);
        }
        let mut raw = OwnedVec::with_capacity(&budget, count)
            .map_err(|error| BoundedDecodeError::Output(BoundedTextError::Resource(error)))?;
        let zeroes = [0u8; 4096];
        while raw.len() < count {
            let length = (count - raw.len()).min(zeroes.len());
            raw.try_extend_from_slice(&zeroes[..length])
                .map_err(|error| BoundedDecodeError::Output(BoundedTextError::Resource(error)))?;
        }
        input
            .read_exact(raw.as_mut_slice())
            .map_err(|_| BoundedDecodeError::RegistryInvariant)?;
        let mut trailing = [0];
        if input
            .read(&mut trailing)
            .map_err(|_| BoundedDecodeError::RegistryInvariant)?
            != 0
        {
            return Err(BoundedDecodeError::RegistryInvariant);
        }
        let text =
            ssx_source_codecs::decode_http_bounded(raw.as_slice(), codec, &budget, &|| Ok(()))?;
        let flags = crate::adapters::html_detector_flags_points(text.codepoints(), &|| Ok(()))
            .map_err(BoundedDecodeError::Output)?;
        Ok((text, flags))
    })();
    match result {
        Ok((text, flags)) => decode_wire::write_success(output, text.codepoints(), flags),
        Err(error) => {
            // Resource refusal can precede body allocation. Drain bounded input
            // before replying so the parent's concurrent writer cannot win with
            // BrokenPipe and obscure the typed resource result.
            let mut discarded = 0usize;
            let mut scratch = [0u8; 4096];
            loop {
                let count = input
                    .read(&mut scratch)
                    .map_err(|error| error.to_string())?;
                if count == 0 {
                    break;
                }
                discarded = discarded
                    .checked_add(count)
                    .ok_or("HTML helper drain overflow")?;
                if discarded > super::MAX_POINTS {
                    return Err("HTML helper oversize trailing input".into());
                }
            }
            decode_wire::write_failure(output, error)
        }
    }
    .map_err(|error| error.to_string())
}

// HeaderMap exposes no qualifying finite field-count contract. Bound scheduling
// work even before allocation/copy: at most64 pairs between cooperative yields.
async fn size_headers(
    headers: &reqwest::header::HeaderMap,
    checkpoint: &(dyn Fn() -> Result<(), OperationFailure> + Sync),
) -> Result<(usize, usize), OperationFailure> {
    let mut count = 0usize;
    let mut size = 0usize;
    for (name, value) in headers.iter() {
        checkpoint()?;
        count = count.checked_add(1).ok_or(OperationFailure::Resource)?;
        size = size
            .checked_add(16)
            .and_then(|n| n.checked_add(name.as_str().len()))
            .and_then(|n| n.checked_add(value.as_bytes().len()))
            .ok_or(OperationFailure::Resource)?;
        if count.is_multiple_of(64) {
            tokio::task::yield_now().await;
            checkpoint()?;
        }
    }
    checkpoint()?;
    Ok((count, size))
}

/// Actual HTTPX selector runs in the same admitted child as body decoding.
pub(super) async fn decode_headers(
    process: &ProcessPool,
    body: &[u8],
    headers: &reqwest::header::HeaderMap,
    operation: Arc<Operation>,
) -> Result<Decoded, OperationFailure> {
    use super::http_headers;
    check(&operation)?;
    if body.len() > super::MAX_POINTS {
        return Err(OperationFailure::Invariant(
            "HTTP text body exceeds transport bound",
        ));
    }
    let (count, size) = size_headers(headers, &|| check(&operation)).await?;
    let total = size
        .checked_add(body.len())
        .ok_or(OperationFailure::Resource)?;
    let layout = std::alloc::Layout::new::<[usize; 2]>()
        .extend(std::alloc::Layout::new::<Lease>())
        .map_err(|_| OperationFailure::Resource)?
        .0
        .pad_to_align();
    let metadata = operation
        .budget
        .0
        .reserve(layout.size())
        .map_err(resource)?;
    let reply_bound = decode_wire::bound(body.len())?;
    let reply = operation.budget.0.reserve(reply_bound).map_err(resource)?;
    let header_charge = operation
        .budget
        .0
        .reserve(http_headers::HEADER)
        .map_err(resource)?;
    let mut input = OwnedVec::with_capacity(&operation.budget.0, total).map_err(resource)?;
    for (name, value) in headers.iter() {
        check(&operation)?;
        input
            .try_extend_from_slice(&(name.as_str().len() as u64).to_be_bytes())
            .map_err(resource)?;
        input
            .try_extend_from_slice(&(value.as_bytes().len() as u64).to_be_bytes())
            .map_err(resource)?;
        for bytes in [name.as_str().as_bytes(), value.as_bytes()] {
            for chunk in bytes.chunks(4096) {
                check(&operation)?;
                input.try_extend_from_slice(chunk).map_err(resource)?;
                tokio::task::yield_now().await;
            }
        }
    }
    for chunk in body.chunks(4096) {
        check(&operation)?;
        input.try_extend_from_slice(chunk).map_err(resource)?;
        tokio::task::yield_now().await;
    }
    check(&operation)?;
    if input.len() != total {
        return Err(OperationFailure::Invariant("HTML raw-header frame size"));
    }
    let snapshot = operation.budget.0.snapshot().map_err(resource)?;
    let allowance = snapshot
        .limit
        .checked_sub(snapshot.used)
        .ok_or(OperationFailure::Resource)?;
    if allowance == 0 || allowance < total {
        return Err(OperationFailure::Resource);
    }
    let child = operation.budget.0.reserve(allowance).map_err(resource)?;
    let mut header = Vec::new();
    header
        .try_reserve_exact(http_headers::HEADER)
        .map_err(|_| OperationFailure::Resource)?;
    if header.capacity() > http_headers::HEADER {
        operation.budget.0.fail_closed();
        return Err(OperationFailure::Resource);
    }
    header.extend(super::PROTOCOL_MAGIC);
    header.extend([http_headers::VERSION, http_headers::OPERATION]);
    for number in [count, size, body.len(), allowance] {
        header.extend((number as u64).to_be_bytes());
    }
    let lease = Arc::new(Lease {
        _operation: operation.clone(),
        child: Mutex::new(Some(child)),
        _reply: reply,
        _header: header_charge,
        _metadata: metadata,
    });
    let frame = process
        .exchange_with_lease(
            operation.deadline,
            header,
            input,
            reply_bound,
            lease.clone(),
        )
        .await
        .map_err(OperationFailure::from)?;
    finish(frame, lease, reply_bound, body.len(), size, &operation).await
}

pub(super) fn worker_headers(input: &mut impl Read, output: &mut impl Write) -> Result<(), String> {
    use super::http_headers;
    enum Failure {
        Decode(BoundedDecodeError),
        Selector(crate::adapters::http_charset::SelectorError),
    }
    impl From<BoundedDecodeError> for Failure {
        fn from(e: BoundedDecodeError) -> Self {
            Self::Decode(e)
        }
    }
    let mut operation = [0];
    input
        .read_exact(&mut operation)
        .map_err(|e| e.to_string())?;
    if operation[0] != http_headers::OPERATION {
        return Err("invalid raw-header helper operation".into());
    }
    let count = read_count(input)?;
    let header_bytes = read_count(input)?;
    let body_bytes = read_count(input)?;
    let allowance = read_count(input)?;
    let total = header_bytes
        .checked_add(body_bytes)
        .ok_or("HTML raw-header count overflow")?;
    if body_bytes > super::MAX_POINTS
        || count > header_bytes / 16
        || allowance == 0
        || allowance > crate::html_budget::MAX_SEARCH_BYTES
        || total > allowance
    {
        return Err("invalid raw-header helper allowance/count".into());
    }
    let budget = Budget::new(allowance).map_err(|e| e.to_string())?;
    let result = (|| -> Result<_, Failure> {
        let mut raw = OwnedVec::with_capacity(&budget, total)
            .map_err(|e| Failure::Decode(BoundedDecodeError::Output(e.into())))?;
        let zeroes = [0; 4096];
        while raw.len() < total {
            let n = (total - raw.len()).min(zeroes.len());
            raw.try_extend_from_slice(&zeroes[..n])
                .map_err(|e| Failure::Decode(BoundedDecodeError::Output(e.into())))?;
        }
        input
            .read_exact(raw.as_mut_slice())
            .map_err(|_| Failure::Decode(BoundedDecodeError::RegistryInvariant))?;
        let mut trailing = [0];
        if input
            .read(&mut trailing)
            .map_err(|_| Failure::Decode(BoundedDecodeError::RegistryInvariant))?
            != 0
        {
            return Err(Failure::Decode(BoundedDecodeError::RegistryInvariant));
        }
        let (header_data, body) = raw.as_slice().split_at(header_bytes);
        let headers = http_headers::Headers::validate(header_data, count)
            .map_err(|_| Failure::Decode(BoundedDecodeError::RegistryInvariant))?;
        let codec = crate::adapters::http_charset::select_http_encoding_bounded(
            headers.iter(),
            body.is_empty(),
            &budget,
            &|| Ok(()),
        )
        .map_err(Failure::Selector)?;
        let text = ssx_source_codecs::decode_http_bounded(body, codec, &budget, &|| Ok(()))?;
        let flags = crate::adapters::html_detector_flags_points(text.codepoints(), &|| Ok(()))
            .map_err(|e| Failure::Decode(BoundedDecodeError::Output(e)))?;
        Ok((text, flags))
    })();
    match result {
        Ok((text, flags)) => decode_wire::write_success(output, text.codepoints(), flags),
        Err(error) => {
            // A quota refusal may precede input allocation; consume at most the
            // validated request envelope so the writer cannot mask typed errors.
            let mut drained = 0usize;
            let mut scratch = [0; 4096];
            loop {
                let n = input.read(&mut scratch).map_err(|e| e.to_string())?;
                if n == 0 {
                    break;
                }
                drained = drained.checked_add(n).ok_or("HTML header drain overflow")?;
                if drained > total {
                    return Err("HTML raw-header trailing input".into());
                }
            }
            match error {
                Failure::Decode(error) => decode_wire::write_failure(output, error),
                Failure::Selector(error) => decode_wire::write_selector_failure(output, error),
            }
        }
    }
    .map_err(|e| e.to_string())
}

#[cfg(test)]
mod parent_phase_tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    #[tokio::test]
    async fn cancellation_during_actual_header_sizing_keeps_owner_until_drop() {
        let budget = Budget::new(1024 * 1024).unwrap();
        struct Input {
            headers: reqwest::header::HeaderMap,
            _charge: Charge,
        }
        let mut headers = reqwest::header::HeaderMap::new();
        for _ in 0..8192 {
            headers.append("x-a05", reqwest::header::HeaderValue::from_static(""));
        }
        let input = Input {
            headers,
            _charge: budget.reserve(8192 * 16).unwrap(),
        };
        let entered = Arc::new(tokio::sync::Notify::new());
        let signal = entered.clone();
        let checks = Arc::new(AtomicUsize::new(0));
        let observed = checks.clone();
        let task = tokio::spawn(async move {
            let checkpoint = || {
                if observed.fetch_add(1, Ordering::SeqCst) == 31 {
                    signal.notify_one();
                }
                Ok(())
            };
            let result = size_headers(&input.headers, &checkpoint).await;
            drop(input);
            result
        });
        tokio::time::timeout(std::time::Duration::from_secs(1), entered.notified())
            .await
            .unwrap();
        assert!(!task.is_finished());
        assert_eq!(budget.snapshot().unwrap().used, 8192 * 16);
        task.abort();
        assert!(task.await.unwrap_err().is_cancelled());
        assert_eq!(budget.snapshot().unwrap().used, 0);
        assert!((32..8192).contains(&checks.load(Ordering::SeqCst)));
        let mut tiny = reqwest::header::HeaderMap::new();
        tiny.append("x-a05", reqwest::header::HeaderValue::from_static("a"));
        tiny.append("x-a05", reqwest::header::HeaderValue::from_static("bb"));
        assert_eq!(
            size_headers(&tiny, &|| Ok(())).await.unwrap(),
            (2, 16 + 5 + 1 + 16 + 5 + 2)
        );
        assert!(matches!(
            size_headers(&tiny, &|| Err(OperationFailure::Deadline)).await,
            Err(OperationFailure::Deadline)
        ));
    }
    #[tokio::test]
    async fn actual_header_worker_composes_all_source_selector_boundaries() {
        use base64::Engine;
        let corpus: serde_json::Value = serde_json::from_str(include_str!(
            "../../../docs/rust-rewrite/a06/source-charset-boundaries.json"
        ))
        .unwrap();
        let rows = corpus["records"].as_array().unwrap();
        assert_eq!(rows.len(), 179);
        for row in rows {
            let mut payload = Vec::new();
            let pairs = row["headers_b64"].as_array().unwrap();
            for pair in pairs {
                let name = base64::engine::general_purpose::STANDARD
                    .decode(pair[0].as_str().unwrap())
                    .unwrap();
                let value = base64::engine::general_purpose::STANDARD
                    .decode(pair[1].as_str().unwrap())
                    .unwrap();
                payload.extend((name.len() as u64).to_be_bytes());
                payload.extend((value.len() as u64).to_be_bytes());
                payload.extend(name);
                payload.extend(value);
            }
            let headers_len = payload.len();
            let body: &[u8] = if row["body_empty"] == true { b"" } else { b"A" };
            payload.extend(body);
            let mut request = vec![super::super::http_headers::OPERATION];
            for n in [pairs.len(), headers_len, body.len(), 16 * 1024 * 1024] {
                request.extend((n as u64).to_be_bytes());
            }
            request.extend(payload);
            let mut output = Vec::new();
            worker_headers(&mut request.as_slice(), &mut output).unwrap();
            assert_eq!(
                u64::from_be_bytes(output[..8].try_into().unwrap()) as usize,
                output.len() - 8
            );
            let result =
                decode_wire::validate_http_async(&output[8..], body.len(), headers_len, &|| Ok(()))
                    .await;
            if row["error"].is_object() {
                let error = match result {
                    Err(OperationFailure::Selector(error)) => error,
                    other => panic!("{}: wrong selector stage {:?}", row["id"], other.err()),
                };
                assert_eq!(
                    error.to_string(),
                    row["error"]["message"].as_str().unwrap(),
                    "{}",
                    row["id"]
                );
            } else {
                // This assertion qualifies composition with the already-qualified
                // body codec, not new actual-source body outcomes for these rows.
                let codec = row["selected_encoding"].as_str().unwrap();
                match ssx_source_codecs::decode_http(body, codec) {
                    Ok(expected) => {
                        let decoded = result.unwrap_or_else(|e| panic!("{}: {e}", row["id"]));
                        let points = decoded
                            .bytes
                            .as_chunks::<4>()
                            .0
                            .iter()
                            .map(|p| u32::from_be_bytes(*p))
                            .collect::<Vec<_>>();
                        assert_eq!(points, expected.codepoints(), "{}", row["id"]);
                    }
                    Err(_) => assert!(
                        matches!(result, Err(OperationFailure::Codec(_))),
                        "{}",
                        row["id"]
                    ),
                }
            }
            println!("A05_SELECTOR_WORKER {}", row["id"].as_str().unwrap());
        }
    }
    // Exercise the exact runtime async phase functions. The signal is emitted
    // from a real per-point checkpoint; the observer can run only once the
    // phase yields. Frame storage and its charge move together into the task.
    #[tokio::test]
    async fn cancellation_during_validation_and_projection_releases_owned_phase() {
        for projection in [false, true] {
            let budget = Budget::new(1024 * 1024).unwrap();
            let points = vec![65u32; 8192];
            let mut encoded = Vec::new();
            decode_wire::write_success(
                &mut encoded,
                &points,
                DetectorFlags {
                    duckduckgo_challenge: false,
                    duckduckgo_legitimate_empty: false,
                    google_challenge: false,
                },
            )
            .unwrap();
            let bytes = encoded.split_off(8);
            struct Frame {
                bytes: Vec<u8>,
                _charge: Charge,
            }
            let frame = Frame {
                _charge: budget.reserve(bytes.capacity()).unwrap(),
                bytes,
            };
            let entered = Arc::new(tokio::sync::Notify::new());
            let signal = entered.clone();
            let checks = Arc::new(AtomicUsize::new(0));
            let observed = checks.clone();
            let owned_budget = budget.clone();
            let task = tokio::spawn(async move {
                let checkpoint = || {
                    if observed.fetch_add(1, Ordering::SeqCst) == 10 {
                        signal.notify_one();
                    }
                    Ok(())
                };
                if projection {
                    let decoded =
                        decode_wire::validate(&frame.bytes, points.len(), &|| Ok(())).unwrap();
                    let _result = project(decoded, &owned_budget, &checkpoint).await;
                } else {
                    let _result =
                        decode_wire::validate_async(&frame.bytes, points.len(), &checkpoint).await;
                }
                drop(frame);
            });
            tokio::time::timeout(std::time::Duration::from_secs(1), entered.notified())
                .await
                .unwrap();
            assert!(!task.is_finished(), "phase completed before cancellation");
            // Both phases retain the frame; projection additionally retains its
            // full fixed-capacity output while only the first chunk is filled.
            let live = budget.snapshot().unwrap().used;
            assert!(live >= 8192 * 4);
            if projection {
                assert!(live >= 8192 * 8);
            }
            task.abort();
            assert!(task.await.unwrap_err().is_cancelled());
            assert_eq!(budget.snapshot().unwrap().used, 0);
            assert!(checks.load(Ordering::SeqCst) <= 1024 + 2);
        }
    }
}
