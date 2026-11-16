//! Private source IDNA2003 validation for RFC2231 strict decoding.
//! Original algorithm/data notices are retained in provenance/idna/.
use crate::bounded::Error as OutputError;
use crate::{idna_data as data, punycode};
use punycode::Work;
use ssx_owned_budget::OwnedVec;
/// Private shared storage: only the legacy wrapper can extract a Vec. Charged
/// consumers retain this owner through every borrowed normalization operation.
enum Scratch<T: Copy> {
    Legacy(Vec<T>),
    Charged(OwnedVec<T>),
}
impl<T: Copy> Scratch<T> {
    fn new(work: Work<'_>) -> Result<Self, OutputError> {
        (work.checkpoint)()?;
        Ok(match work.budget {
            Some(budget) => Self::Charged(OwnedVec::new(budget)?),
            None => Self::Legacy(Vec::new()),
        })
    }
    fn push(&mut self, point: T, work: Work<'_>) -> Result<(), OutputError> {
        (work.checkpoint)()?;
        match self {
            Self::Legacy(points) => points.push(point),
            Self::Charged(points) => {
                points.try_reserve_exact(1)?;
                points.try_push(point)?;
            }
        }
        Ok(())
    }
    fn truncate(&mut self, length: usize) {
        match self {
            Self::Legacy(points) => points.truncate(length),
            Self::Charged(points) => points.truncate(length),
        }
    }
    #[cfg(test)]
    fn legacy(self) -> Vec<T> {
        match self {
            Self::Legacy(points) => points,
            Self::Charged(_) => unreachable!("only legacy wrapper extracts storage"),
        }
    }
}
impl<T: Copy> std::ops::Deref for Scratch<T> {
    type Target = [T];
    fn deref(&self) -> &[T] {
        match self {
            Self::Legacy(points) => points,
            Self::Charged(points) => points.as_slice(),
        }
    }
}
impl<T: Copy> std::ops::DerefMut for Scratch<T> {
    fn deref_mut(&mut self) -> &mut [T] {
        match self {
            Self::Legacy(points) => points,
            Self::Charged(points) => points.as_mut_slice(),
        }
    }
}
fn legacy_work() -> Work<'static> {
    Work {
        budget: None,
        checkpoint: &|| Ok(()),
    }
}
fn mapping<'a>(c: u32, index: &[(u32, usize, usize)], values: &'a [u32]) -> Option<&'a [u32]> {
    index.binary_search_by_key(&c, |r| r.0).ok().map(|i| {
        let (_, at, len) = index[i];
        &values[at..at + len]
    })
}
fn flags(c: u32) -> u8 {
    let i = data::FLAGS.partition_point(|r| r.0 <= c);
    if i != 0 && c < data::FLAGS[i - 1].1 {
        data::FLAGS[i - 1].2
    } else {
        0
    }
}
fn ccc(c: u32) -> u8 {
    data::CCC
        .binary_search_by_key(&c, |r| r.0)
        .map_or(0, |i| data::CCC[i].1)
}
fn decompose(c: u32, out: &mut Scratch<u32>, work: Work<'_>) -> Result<(), OutputError> {
    (work.checkpoint)()?;
    if (0xac00..0xd7a4).contains(&c) {
        let i = c - 0xac00;
        out.push(0x1100 + i / 588, work)?;
        out.push(0x1161 + (i % 588) / 28, work)?;
        if !i.is_multiple_of(28) {
            out.push(0x11a7 + i % 28, work)?;
        }
    } else if let Some(seq) = mapping(c, data::DECOMP_INDEX, data::DECOMP_DATA) {
        for &p in seq {
            decompose(p, out, work)?;
        }
    } else {
        out.push(c, work)?;
    }
    Ok(())
}
fn compose(a: u32, b: u32) -> Option<u32> {
    if (0x1100..0x1113).contains(&a) && (0x1161..0x1176).contains(&b) {
        return Some(0xac00 + ((a - 0x1100) * 21 + b - 0x1161) * 28);
    }
    if (0xac00..0xd7a4).contains(&a)
        && (a - 0xac00).is_multiple_of(28)
        && (0x11a8..0x11c3).contains(&b)
    {
        return Some(a + b - 0x11a7);
    }
    data::COMPOSE
        .binary_search_by_key(&(a, b), |r| (r.0, r.1))
        .ok()
        .map(|i| data::COMPOSE[i].2)
}
fn nfkc_work(input: &[u32], work: Work<'_>) -> Result<Scratch<u32>, OutputError> {
    let mut decomposed = Scratch::new(work)?;
    for &c in input {
        decompose(c, &mut decomposed, work)?;
    }
    for i in 1..decomposed.len() {
        (work.checkpoint)()?;
        let order = ccc(decomposed[i]);
        let mut at = i;
        while order != 0 && at != 0 && ccc(decomposed[at - 1]) > order {
            (work.checkpoint)()?;
            decomposed.swap(at, at - 1);
            at -= 1;
        }
    }
    let Some(&first) = decomposed.first() else {
        return Ok(decomposed);
    };
    // Composition never increases length. Compact into the already-owned
    // decomposition buffer instead of allocating a second full point Vec.
    let mut written = 1;
    let mut starter = 0;
    let mut last = ccc(first);
    for read in 1..decomposed.len() {
        (work.checkpoint)()?;
        let c = decomposed[read];
        let order = ccc(c);
        if let Some(joined) = compose(decomposed[starter], c).filter(|_| last < order || last == 0)
        {
            decomposed[starter] = joined;
        } else {
            if order == 0 {
                starter = written;
            }
            decomposed[written] = c;
            written += 1;
            last = order;
        }
    }
    decomposed.truncate(written);
    Ok(decomposed)
}
#[cfg(test)]
fn nfkc(input: &[u32]) -> Vec<u32> {
    nfkc_work(input, legacy_work())
        .expect("infallible legacy output")
        .legacy()
}
fn nameprep_work(input: &[u32], work: Work<'_>) -> Result<Option<Scratch<u32>>, OutputError> {
    let mut mapped = Scratch::new(work)?;
    for &c in input {
        (work.checkpoint)()?;
        if flags(c) & 1 == 0 {
            if let Some(seq) = mapping(c, data::B2_INDEX, data::B2_DATA) {
                for &point in seq {
                    mapped.push(point, work)?;
                }
            } else {
                mapped.push(c, work)?;
            }
        }
    }
    let out = nfkc_work(&mapped, work)?;
    drop(mapped);
    let mut randal = false;
    let mut left = false;
    for &point in out.iter() {
        (work.checkpoint)()?;
        let value = flags(point);
        if value & 2 != 0 {
            return Ok(None);
        }
        randal |= value & 4 != 0;
        left |= value & 8 != 0;
    }
    if randal
        && (left
            || out.first().is_none_or(|&p| flags(p) & 4 == 0)
            || out.last().is_none_or(|&p| flags(p) & 4 == 0))
    {
        return Ok(None);
    }
    Ok(Some(out))
}
#[cfg(test)]
fn nameprep(input: &[u32]) -> Option<Vec<u32>> {
    nameprep_work(input, legacy_work())
        .expect("infallible legacy output")
        .map(Scratch::legacy)
}
fn adapt(mut delta: u64, first: bool, count: u64) -> u64 {
    delta /= if first { 700 } else { 2 };
    delta += delta / count;
    let mut k = 0;
    while delta > 455 {
        delta /= 35;
        k += 36;
    }
    k + 36 * delta / (delta + 38)
}
fn unicode_failure() -> punycode::Failure {
    punycode::Failure::Source(punycode::StrictError::Unicode { end: 0 })
}
fn encode_work(points: &[u32], work: Work<'_>) -> Result<Scratch<u8>, punycode::Failure> {
    let mut out = Scratch::new(work)?;
    for &point in points {
        (work.checkpoint)()?;
        if point < 128 {
            out.push(point as u8, work)?;
        }
    }
    let basic = out.len();
    let mut handled = basic;
    if basic != 0 {
        out.push(b'-', work)?;
    }
    let mut n = 128u64;
    let mut delta = 0u64;
    let mut bias = 72u64;
    while handled < points.len() {
        (work.checkpoint)()?;
        let mut next = None;
        for &point in points {
            (work.checkpoint)()?;
            let point = u64::from(point);
            if point >= n {
                next = Some(next.map_or(point, |v: u64| v.min(point)));
            }
        }
        let next = next.ok_or_else(unicode_failure)?;
        delta = delta
            .checked_add(
                (next - n)
                    .checked_mul(handled as u64 + 1)
                    .ok_or_else(unicode_failure)?,
            )
            .ok_or_else(unicode_failure)?;
        n = next;
        for &c in points {
            (work.checkpoint)()?;
            let c = u64::from(c);
            if c < n {
                delta = delta.checked_add(1).ok_or_else(unicode_failure)?;
            }
            if c == n {
                let mut q = delta;
                let mut k = 36u64;
                loop {
                    (work.checkpoint)()?;
                    let t = k.saturating_sub(bias).clamp(1, 26);
                    let digit = if q < t { q } else { t + (q - t) % (36 - t) };
                    out.push(
                        if digit < 26 {
                            b'a' + digit as u8
                        } else {
                            b'0' + (digit - 26) as u8
                        },
                        work,
                    )?;
                    if q < t {
                        break;
                    }
                    q = (q - t) / (36 - t);
                    k = k.checked_add(36).ok_or_else(unicode_failure)?;
                }
                bias = adapt(delta, handled == basic, handled as u64 + 1);
                delta = 0;
                handled += 1;
            }
        }
        delta = delta.checked_add(1).ok_or_else(unicode_failure)?;
        n = n.checked_add(1).ok_or_else(unicode_failure)?;
    }
    Ok(out)
}
#[cfg(test)]
fn encode(points: &[u32]) -> Option<Vec<u8>> {
    match encode_work(points, legacy_work()) {
        Ok(value) => Some(value.legacy()),
        Err(punycode::Failure::Source(_)) => None,
        Err(punycode::Failure::Output(_)) => unreachable!("infallible legacy output"),
    }
}
fn ascii_points(points: &[u32], work: Work<'_>) -> Result<bool, OutputError> {
    for &point in points {
        (work.checkpoint)()?;
        if point >= 128 {
            return Ok(false);
        }
    }
    Ok(true)
}
fn copy_ascii(points: &[u32], work: Work<'_>) -> Result<Scratch<u8>, punycode::Failure> {
    if !(1..64).contains(&points.len()) {
        return Err(unicode_failure());
    }
    let mut out = Scratch::new(work)?;
    for &point in points {
        out.push(point as u8, work)?;
    }
    Ok(out)
}
fn to_ascii_work(input: &[u32], work: Work<'_>) -> Result<Scratch<u8>, punycode::Failure> {
    if ascii_points(input, work)? {
        return copy_ascii(input, work);
    }
    let prepared = nameprep_work(input, work)?.ok_or_else(unicode_failure)?;
    if ascii_points(&prepared, work)? {
        return copy_ascii(&prepared, work);
    }
    if prepared.get(..4).is_some_and(|s| {
        s.iter()
            .zip(b"xn--")
            .all(|(&a, &b)| a < 128 && (a as u8).eq_ignore_ascii_case(&b))
    }) {
        return Err(unicode_failure());
    }
    let mut encoded = Scratch::new(work)?;
    for &byte in b"xn--" {
        encoded.push(byte, work)?;
    }
    let payload = encode_work(&prepared, work)?;
    for &byte in payload.iter() {
        encoded.push(byte, work)?;
    }
    if encoded.len() >= 64 {
        return Err(unicode_failure());
    }
    Ok(encoded)
}
/// Successful ASCII/non-ASCII classification after validating every label.
/// Unicode failures remain distinguishable from propagated position overflow.
pub(crate) fn ascii_outcome(bytes: &[u8]) -> Result<bool, punycode::StrictError> {
    match ascii_outcome_work(bytes, legacy_work()) {
        Ok(value) => Ok(value),
        Err(punycode::Failure::Source(error)) => Err(error),
        Err(punycode::Failure::Output(_)) => unreachable!("infallible legacy output"),
    }
}
struct Decoded<'a> {
    points: Scratch<u32>,
    work: Work<'a>,
}
impl crate::bounded::PointSink for Decoded<'_> {
    fn emit(&mut self, point: u32) -> Result<(), OutputError> {
        crate::bounded::PointSequence::insert(self, self.points.len(), point)
    }
}
impl crate::bounded::PointSequence for Decoded<'_> {
    fn len(&self) -> usize {
        self.points.len()
    }
    fn insert(&mut self, index: usize, point: u32) -> Result<(), OutputError> {
        (self.work.checkpoint)()?;
        let length = self.points.len();
        if index > length {
            return Err(OutputError::OutputBound);
        }
        if point > 0x10ffff {
            return Err(OutputError::InvalidCodepoint);
        }
        self.points.push(point, self.work)?;
        self.points.copy_within(index..length, index + 1);
        self.points[index] = point;
        Ok(())
    }
}
pub(crate) fn ascii_outcome_work(bytes: &[u8], work: Work<'_>) -> Result<bool, punycode::Failure> {
    (work.checkpoint)()?;
    if bytes.is_empty() {
        return Ok(true);
    }
    let mut ace = false;
    for window in bytes.windows(4) {
        (work.checkpoint)()?;
        if window.eq_ignore_ascii_case(b"xn--") {
            ace = true;
            break;
        }
    }
    if !ace {
        for byte in bytes {
            (work.checkpoint)()?;
            if !byte.is_ascii() {
                return Err(unicode_failure());
            }
        }
        return Ok(true);
    }
    let input = bytes.strip_suffix(b".").unwrap_or(bytes);
    let mut ascii = true;
    let mut offset = 0u64;
    for label in input.split(|&c| c == b'.') {
        (work.checkpoint)()?;
        if label.len() > 1024 {
            return Err(unicode_failure());
        }
        if !label
            .get(..4)
            .is_some_and(|s| s.eq_ignore_ascii_case(b"xn--"))
        {
            for byte in label {
                (work.checkpoint)()?;
                if !byte.is_ascii() {
                    return Err(unicode_failure());
                }
            }
        } else {
            let prepared = punycode::prepare(&label[4..], true, work)
                .map_err(|error| offset_failure(error, 4 + offset))?;
            let mut decoded = Decoded {
                points: Scratch::new(work)?,
                work,
            };
            punycode::decode_prepared(prepared, work, &mut decoded)
                .map_err(|error| offset_failure(error, 4 + offset))?;
            let encoded = to_ascii_work(&decoded.points, work)?;
            if encoded.len() != label.len() {
                return Err(unicode_failure());
            }
            for (&a, &b) in encoded.iter().zip(label) {
                (work.checkpoint)()?;
                if !a.eq_ignore_ascii_case(&b) {
                    return Err(unicode_failure());
                }
            }
            ascii &= ascii_points(&decoded.points, work)?;
        }
        offset = offset
            .checked_add(label.len() as u64 + 1)
            .ok_or(punycode::Failure::Source(punycode::StrictError::Overflow))?;
    }
    Ok(ascii)
}
fn offset_failure(error: punycode::Failure, offset: u64) -> punycode::Failure {
    match error {
        punycode::Failure::Source(error) => {
            punycode::Failure::Source(punycode::offset_error(error, offset))
        }
        punycode::Failure::Output(error) => punycode::Failure::Output(error),
    }
}

/// Additive hostname encoder for the external HTTP client. All existing IDNA
/// decoding/validation callers retain their original paths and budgets.
pub(crate) fn encode_host(input: &str) -> Result<Vec<u8>, punycode::Failure> {
    if input.is_empty() {
        return Ok(Vec::new());
    }
    if input.is_ascii() {
        let labels = input.split('.').collect::<Vec<_>>();
        for (index, label) in labels.iter().enumerate() {
            if label.len() >= 64 || (label.is_empty() && index + 1 != labels.len()) {
                return Err(unicode_failure());
            }
        }
        return Ok(input.as_bytes().to_vec());
    }
    let mut labels = input
        .split(['.', '\u{3002}', '\u{ff0e}', '\u{ff61}'])
        .collect::<Vec<_>>();
    let trailing = labels.last() == Some(&"");
    if trailing {
        labels.pop();
    }
    let mut output = Vec::new();
    for (index, label) in labels.iter().enumerate() {
        if index != 0 {
            output.push(b'.');
        }
        let points = label.chars().map(u32::from).collect::<Vec<_>>();
        let ascii = to_ascii_work(&points, legacy_work())?;
        output.extend_from_slice(&ascii);
    }
    if trailing {
        output.push(b'.');
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn without_overflow(bytes: &[u8]) -> Option<bool> {
        match ascii_outcome(bytes) {
            Ok(value) => Some(value),
            Err(punycode::StrictError::Unicode { .. }) => None,
            Err(punycode::StrictError::Overflow) => {
                panic!("unexpected overflow in original source subset")
            }
        }
    }

    #[test]
    fn charged_nfkc_matches_all_84960_official_sequences_and_releases_output() {
        let budget = ssx_owned_budget::Budget::new(4096).unwrap();
        let work = Work {
            budget: Some(&budget),
            checkpoint: &|| Ok(()),
        };
        let mut executed = 0;
        for line in include_str!("../tests/NormalizationTest-3.2.0.txt").lines() {
            if line.is_empty() || line.starts_with(['#', '@']) {
                continue;
            }
            let fields: Vec<Vec<u32>> = line
                .split('#')
                .next()
                .unwrap()
                .split(';')
                .take(5)
                .map(|f| {
                    f.split_whitespace()
                        .map(|s| u32::from_str_radix(s, 16).unwrap())
                        .collect()
                })
                .collect();
            assert_eq!(fields.len(), 5);
            for input in &fields {
                let output = nfkc_work(input, work).unwrap();
                assert_eq!(&*output, fields[3].as_slice(), "{line}");
                assert!(budget.snapshot().unwrap().used >= output.len() * 4);
                drop(output);
                assert_eq!(budget.snapshot().unwrap().used, 0);
                executed += 1;
            }
        }
        assert_eq!(executed, 84960);
    }
    #[test]
    fn charged_nameprep_mid_growth_failure_and_deadline_release_all_scratch() {
        use std::cell::Cell;
        // FDFA expands to eighteen points: mapped(4) overlaps old(68) and
        // replacement(72) decomposition capacities during the last growth.
        let budget = ssx_owned_budget::Budget::new(144).unwrap();
        let work = Work {
            budget: Some(&budget),
            checkpoint: &|| Ok(()),
        };
        let actual = nameprep_work(&[0xfdfa], work).unwrap();
        assert_eq!(actual.as_deref(), nameprep(&[0xfdfa]).as_deref());
        assert_eq!(budget.snapshot().unwrap().peak, 144);
        drop(actual);
        assert_eq!(budget.snapshot().unwrap().used, 0);
        let small = ssx_owned_budget::Budget::new(143).unwrap();
        assert!(matches!(
            nameprep_work(
                &[0xfdfa],
                Work {
                    budget: Some(&small),
                    checkpoint: &|| Ok(())
                }
            ),
            Err(OutputError::Resource(ssx_owned_budget::Error::Exhausted))
        ));
        assert_eq!(small.snapshot().unwrap().used, 0);
        let calls = Cell::new(0);
        let check = || {
            calls.set(calls.get() + 1);
            if calls.get() == 12 {
                Err(OutputError::Deadline)
            } else {
                Ok(())
            }
        };
        assert!(matches!(
            nameprep_work(
                &[0xfdfa],
                Work {
                    budget: Some(&budget),
                    checkpoint: &check
                }
            ),
            Err(OutputError::Deadline)
        ));
        assert_eq!(budget.snapshot().unwrap().used, 0);
    }
    #[test]
    fn strict_idna_all_source_parameters_preserve_unicode_failure_and_ascii_outcome() {
        let source: serde_json::Value =
            serde_json::from_str(include_str!("../tests/idna-direct-source.json")).unwrap();
        let rows = source["cases"].as_array().unwrap();
        assert_eq!(rows.len(), 488);
        let budget = ssx_owned_budget::Budget::new(1024 * 1024).unwrap();
        let work = Work {
            budget: Some(&budget),
            checkpoint: &|| Ok(()),
        };
        let mut ids = std::collections::BTreeSet::new();
        for row in rows {
            let original = &row["whole_httpx_source"];
            assert!(ids.insert(original["id"].as_str().unwrap()));
            let hex = original["parameter_bytes_hex"].as_str().unwrap();
            let bytes: Vec<u8> = hex
                .as_bytes()
                .as_chunks::<2>()
                .0
                .iter()
                .map(|s| u8::from_str_radix(std::str::from_utf8(s).unwrap(), 16).unwrap())
                .collect();
            let expected = row["decoded_codepoints"]
                .as_array()
                .map(|points| points.iter().all(|p| p.as_u64().unwrap() < 128));
            assert_eq!(without_overflow(&bytes), expected, "{}", original["id"]);
            let charged = match ascii_outcome_work(&bytes, work) {
                Ok(value) => Some(value),
                Err(punycode::Failure::Source(punycode::StrictError::Unicode { .. })) => None,
                other => panic!("unexpected charged outcome {other:?}"),
            };
            assert_eq!(charged, expected, "charged {}", original["id"]);
            assert_eq!(budget.snapshot().unwrap().used, 0);
            let expected_facade = match expected {
                None => crate::StrictAsciiOutcome::UnicodeFailure,
                Some(false) => crate::StrictAsciiOutcome::NonAscii,
                Some(true) => crate::StrictAsciiOutcome::Ascii(
                    row["decoded_codepoints"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|p| char::from(p.as_u64().unwrap() as u8))
                        .collect(),
                ),
            };
            assert_eq!(
                crate::strict_idna_parameter(&bytes),
                Ok(expected_facade),
                "facade {}",
                original["id"]
            );
        }
        assert_eq!(ids.len(), 488);
    }
    #[test]
    fn native_nfkc_matches_every_official_unicode_3_2_sequence() {
        let mut executed = 0;
        for line in include_str!("../tests/NormalizationTest-3.2.0.txt").lines() {
            if line.is_empty() || line.starts_with(['#', '@']) {
                continue;
            }
            let fields: Vec<Vec<u32>> = line
                .split('#')
                .next()
                .unwrap()
                .split(';')
                .take(5)
                .map(|f| {
                    f.split_whitespace()
                        .map(|s| u32::from_str_radix(s, 16).unwrap())
                        .collect()
                })
                .collect();
            assert_eq!(fields.len(), 5);
            for input in &fields {
                assert_eq!(nfkc(input), fields[3], "{line}");
                executed += 1;
            }
        }
        assert_eq!(executed, 84960);
    }
    #[test]
    fn strict_idna_complete_source_ace_bytes_and_outcomes_for_every_codepoint() {
        use std::io::Read;
        let mut raw = Vec::new();
        flate2::read::ZlibDecoder::new(
            &include_bytes!("../tests/idna-all-points-source.bin.zlib")[..],
        )
        .read_to_end(&mut raw)
        .unwrap();
        let mut at = 0;
        let mut counts = [0usize; 3];
        let budget = ssx_owned_budget::Budget::new(1024 * 1024).unwrap();
        let work = Work {
            budget: Some(&budget),
            checkpoint: &|| Ok(()),
        };
        for c in 0..0x110000 {
            let length = usize::from(u16::from_be_bytes(raw[at..at + 2].try_into().unwrap()));
            let expected = raw[at + 2];
            at += 3;
            let source = &raw[at..at + length];
            at += length;
            assert_eq!(
                encode(&[c]).unwrap(),
                source,
                "source ACE encoding U+{c:04x}"
            );
            let mut label = b"xn--".to_vec();
            label.extend_from_slice(source);
            let actual = match without_overflow(&label) {
                None => 0,
                Some(true) => 1,
                Some(false) => 2,
            };
            assert_eq!(actual, expected, "source strict IDNA U+{c:04x}");
            let charged = match ascii_outcome_work(&label, work) {
                Ok(true) => 1,
                Ok(false) => 2,
                Err(punycode::Failure::Source(punycode::StrictError::Unicode { .. })) => 0,
                other => panic!("unexpected charged outcome U+{c:04x}: {other:?}"),
            };
            assert_eq!(charged, expected, "charged source strict IDNA U+{c:04x}");
            assert_eq!(budget.snapshot().unwrap().used, 0);
            counts[usize::from(actual)] += 1;
        }
        assert_eq!(at, raw.len());
        assert_eq!(counts, [145535, 0, 968577]);
    }
}
