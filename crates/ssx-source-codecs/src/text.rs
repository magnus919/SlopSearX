// SPDX-License-Identifier: MIT
/// Adapter-owned Python text values. Surrogate codepoints remain distinct from
/// Unicode scalars until the source-compatible lxml helper boundary.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Text(Vec<u32>);
impl Text {
    pub fn from_codepoints(points: Vec<u32>) -> Result<Self, &'static str> {
        if points.iter().any(|p| *p > 0x10ffff) {
            Err("codepoint exceeds Unicode range")
        } else {
            Ok(Self(points))
        }
    }
    /// Move validated codepoints without scalar conversion or allocation.
    pub fn into_codepoints(self) -> Vec<u32> {
        self.0
    }
    pub fn codepoints(&self) -> &[u32] {
        &self.0
    }
    pub fn contains_ascii_lower(&self, needle: &str) -> bool {
        assert!(needle.is_ascii());
        let mut prefix = vec![0usize; needle.len()];
        contains_ascii_lower_with_scratch(&self.0, needle, &mut prefix, &|| Ok(()))
            .expect("legacy search scratch matches needle")
    }
}
/// Borrowed source-compatible ASCII substring search with caller-owned scratch.
/// No heap allocation. Codepoint ownership stays with the caller. Source lower()
/// expansion preimages are shared with Text's original implementation.
pub fn contains_ascii_lower_with_scratch(
    points: &[u32],
    needle: &str,
    prefix: &mut [usize],
    checkpoint: &dyn Fn() -> Result<(), crate::BoundedTextError>,
) -> Result<bool, crate::BoundedTextError> {
    checkpoint()?;
    if !needle.is_ascii() {
        return Err(crate::BoundedTextError::NonAscii);
    }
    let needle = needle.as_bytes();
    if needle.is_empty() {
        return Ok(true);
    }
    // Actual CPython3.14.7 exhaustive lower() observation: the only
    // non-ASCII inputs emitting ASCII are U0130 -> i,U0307 and U212A -> k.
    // Other non-ASCII lowerings cannot participate in an ASCII substring.
    // KMP avoids a full lowered-text allocation and quadratic rescanning.
    if prefix.len() != needle.len() {
        return Err(crate::BoundedTextError::OutputBound);
    }
    prefix.fill(0);
    for index in 1..needle.len() {
        checkpoint()?;
        let mut length = prefix[index - 1];
        while length != 0 && needle[index] != needle[length] {
            length = prefix[length - 1];
        }
        if needle[index] == needle[length] {
            length += 1;
        }
        prefix[index] = length;
    }
    let mut length = 0;
    for point in points {
        checkpoint()?;
        let lowered = match *point {
            0..=127 => [u32::from((*point as u8).to_ascii_lowercase()), u32::MAX],
            0x130 => [u32::from(b'i'), 0x307],
            0x212a => [u32::from(b'k'), u32::MAX],
            _ => [*point, u32::MAX],
        };
        for point in lowered.into_iter().take_while(|point| *point != u32::MAX) {
            if point > 127 {
                length = 0;
                continue;
            }
            let byte = point as u8;
            while length != 0 && byte != needle[length] {
                length = prefix[length - 1];
            }
            if byte == needle[length] {
                length += 1;
            }
            if length == needle.len() {
                return Ok(true);
            }
        }
    }
    Ok(false)
}
impl From<&str> for Text {
    fn from(s: &str) -> Self {
        Self(s.chars().map(u32::from).collect())
    }
}
impl From<String> for Text {
    fn from(s: String) -> Self {
        Self::from(s.as_str())
    }
}
