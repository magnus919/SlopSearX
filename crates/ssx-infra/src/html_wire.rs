//! Private checked rows component; helper integration follows shared-ledger review.
//! No storage JSON serialization, allocation, normalization or partial publication.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layout {
    DdgHtml,
    DdgLite,
    DdgImage,
    Google,
    ExploitDb,
    MitreTechnique,
    MitreGroup,
    MitreSoftware,
}
impl Layout {
    pub(crate) fn is_a10(self) -> bool {
        matches!(
            self,
            Self::ExploitDb | Self::MitreTechnique | Self::MitreGroup | Self::MitreSoftware
        )
    }
    fn tag(self) -> u8 {
        match self {
            Self::DdgHtml => 0,
            Self::DdgLite => 1,
            Self::DdgImage => 2,
            Self::Google => 3,
            Self::ExploitDb => 4,
            Self::MitreTechnique => 5,
            Self::MitreGroup => 6,
            Self::MitreSoftware => 7,
        }
    }
    pub(super) fn from_tag(tag: u8) -> Result<Self, Error> {
        match tag {
            0 => Ok(Self::DdgHtml),
            1 => Ok(Self::DdgLite),
            2 => Ok(Self::DdgImage),
            3 => Ok(Self::Google),
            4 => Ok(Self::ExploitDb),
            5 => Ok(Self::MitreTechnique),
            6 => Ok(Self::MitreGroup),
            7 => Ok(Self::MitreSoftware),
            _ => Err(Error::Layout),
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Truncated,
    Overflow,
    Version,
    Operation,
    Layout,
    Flags,
    Utf8,
    Count,
    Trailing,
    Size,
    Deadline,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Row<'a> {
    pub position: i64,
    pub query_title_fallback: bool,
    pub url: &'a str,
    pub title: &'a str,
    pub content: &'a str,
    pub img_src: Option<&'a str>,
}
// Upper bound from approved per-search allocation policy; actual reserved
// frame allowance must be smaller when other allocations overlap.
pub const MAX_FRAME_BYTES: usize = 256 * 1024 * 1024;
const HEADER: usize = 15;
#[cfg(test)]
const MIN_ROW: usize = 33;
fn valid_flags(layout: Layout, fallback: bool, image: bool) -> Result<(), Error> {
    if layout != Layout::DdgImage && (fallback || image) {
        Err(Error::Flags)
    } else {
        Ok(())
    }
}
#[cfg(test)]
pub fn encoded_size(
    layout: Layout,
    rows: &[Row<'_>],
    mut check: impl FnMut() -> Result<(), Error>,
) -> Result<usize, Error> {
    check()?;
    u64::try_from(rows.len()).map_err(|_| Error::Overflow)?;
    let mut size = HEADER;
    for row in rows {
        check()?;
        valid_flags(layout, row.query_title_fallback, row.img_src.is_some())?;
        size = size.checked_add(9).ok_or(Error::Overflow)?;
        for field in [
            Some(row.url),
            Some(row.title),
            Some(row.content),
            row.img_src,
        ]
        .into_iter()
        .flatten()
        {
            u64::try_from(field.len()).map_err(|_| Error::Overflow)?;
            size = size
                .checked_add(8)
                .and_then(|n| n.checked_add(field.len()))
                .ok_or(Error::Overflow)?;
            if size > MAX_FRAME_BYTES {
                return Err(Error::Size);
            }
        }
    }
    check()?;
    Ok(size)
}
/// Caller precharges the exact destination; this function allocates nothing.
#[cfg(test)]
pub fn encode(
    layout: Layout,
    rows: &[Row<'_>],
    destination: &mut [u8],
    mut check: impl FnMut() -> Result<(), Error>,
) -> Result<(), Error> {
    check()?;
    if destination.len() != encoded_size(layout, rows, &mut check)? {
        return Err(Error::Size);
    }
    let mut cursor = 0usize;
    let mut write = |bytes: &[u8]| -> Result<(), Error> {
        let end = cursor.checked_add(bytes.len()).ok_or(Error::Overflow)?;
        destination
            .get_mut(cursor..end)
            .ok_or(Error::Size)?
            .copy_from_slice(bytes);
        cursor = end;
        Ok(())
    };
    write(b"A05R\x02\x02")?;
    write(&[layout.tag()])?;
    write(&(rows.len() as u64).to_be_bytes())?;
    for row in rows {
        check()?;
        write(&row.position.to_be_bytes())?;
        write(&[u8::from(row.query_title_fallback) | (u8::from(row.img_src.is_some()) << 1)])?;
        for field in [
            Some(row.url),
            Some(row.title),
            Some(row.content),
            row.img_src,
        ]
        .into_iter()
        .flatten()
        {
            check()?;
            write(&(field.len() as u64).to_be_bytes())?;
            write(field.as_bytes())?;
            check()?;
        }
    }
    check()
}
/// Stream an already-owned row batch without creating a second frame buffer.
/// Checked sizing completes before any bytes are published.
#[cfg(test)]
pub fn write(
    layout: Layout,
    rows: &[Row<'_>],
    writer: &mut impl std::io::Write,
) -> std::io::Result<()> {
    let size = encoded_size(layout, rows, || Ok(()))
        .map_err(|error| std::io::Error::other(format!("{error:?}")))?;
    writer.write_all(&(size as u64).to_be_bytes())?;
    writer.write_all(b"A05R\x02\x02")?;
    writer.write_all(&[layout.tag()])?;
    writer.write_all(&(rows.len() as u64).to_be_bytes())?;
    for row in rows {
        writer.write_all(&row.position.to_be_bytes())?;
        writer.write_all(&[
            u8::from(row.query_title_fallback) | (u8::from(row.img_src.is_some()) << 1)
        ])?;
        for field in [
            Some(row.url),
            Some(row.title),
            Some(row.content),
            row.img_src,
        ]
        .into_iter()
        .flatten()
        {
            writer.write_all(&(field.len() as u64).to_be_bytes())?;
            writer.write_all(field.as_bytes())?;
        }
    }
    writer.flush()
}
#[cfg(test)]
struct Cursor<'a> {
    bytes: &'a [u8],
    at: usize,
}
#[cfg(test)]
impl<'a> Cursor<'a> {
    fn take(&mut self, length: usize) -> Result<&'a [u8], Error> {
        let end = self.at.checked_add(length).ok_or(Error::Overflow)?;
        let value = self.bytes.get(self.at..end).ok_or(Error::Truncated)?;
        self.at = end;
        Ok(value)
    }
    fn number(&mut self) -> Result<u64, Error> {
        Ok(u64::from_be_bytes(
            self.take(8)?.try_into().map_err(|_| Error::Truncated)?,
        ))
    }
    fn string(&mut self, check: &mut impl FnMut() -> Result<(), Error>) -> Result<&'a str, Error> {
        check()?;
        let length = usize::try_from(self.number()?).map_err(|_| Error::Overflow)?;
        let value = std::str::from_utf8(self.take(length)?).map_err(|_| Error::Utf8)?;
        check()?;
        Ok(value)
    }
    fn row(
        &mut self,
        layout: Layout,
        check: &mut impl FnMut() -> Result<(), Error>,
    ) -> Result<Row<'a>, Error> {
        check()?;
        let position = i64::from_be_bytes(self.take(8)?.try_into().map_err(|_| Error::Truncated)?);
        let flags = self.take(1)?[0];
        if flags & !3 != 0 {
            return Err(Error::Flags);
        }
        valid_flags(layout, flags & 1 != 0, flags & 2 != 0)?;
        let url = self.string(check)?;
        let title = self.string(check)?;
        let content = self.string(check)?;
        let img_src = if flags & 2 != 0 {
            Some(self.string(check)?)
        } else {
            None
        };
        Ok(Row {
            position,
            query_title_fallback: flags & 1 != 0,
            url,
            title,
            content,
            img_src,
        })
    }
}
/// Construction validates the entire immutable frame before any row is exposed.
/// Expected layout comes from the trusted parent operation, never the response.
#[cfg(test)]
pub struct Rows<'a> {
    bytes: &'a [u8],
    count: usize,
    layout: Layout,
}
#[cfg(test)]
impl<'a> Rows<'a> {
    pub fn validate(
        bytes: &'a [u8],
        expected: Layout,
        mut check: impl FnMut() -> Result<(), Error>,
    ) -> Result<Self, Error> {
        check()?;
        if bytes.len() > MAX_FRAME_BYTES {
            return Err(Error::Size);
        }
        let mut cursor = Cursor { bytes, at: 0 };
        if cursor.take(5)? != b"A05R\x02" {
            return Err(Error::Version);
        }
        if cursor.take(1)? != [2] {
            return Err(Error::Operation);
        }
        let layout = Layout::from_tag(cursor.take(1)?[0])?;
        if layout != expected {
            return Err(Error::Layout);
        }
        let count = usize::try_from(cursor.number()?).map_err(|_| Error::Overflow)?;
        if count > (bytes.len() - cursor.at) / MIN_ROW {
            return Err(Error::Count);
        }
        for _ in 0..count {
            cursor.row(layout, &mut check)?;
        }
        if cursor.at != bytes.len() {
            return Err(Error::Trailing);
        }
        check()?;
        Ok(Self {
            bytes,
            count,
            layout,
        })
    }
    pub fn for_each(
        &self,
        mut check: impl FnMut() -> Result<(), Error>,
        mut visit: impl FnMut(Row<'a>) -> Result<(), Error>,
    ) -> Result<(), Error> {
        let mut cursor = Cursor {
            bytes: self.bytes,
            at: HEADER,
        };
        for _ in 0..self.count {
            let row = cursor.row(self.layout, &mut check)?;
            check()?;
            visit(row)?;
            check()?;
        }
        check()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn row() -> Row<'static> {
        Row {
            position: -1,
            query_title_fallback: true,
            url: "//raw.example/é?x=1&y=2",
            title: "",
            content: " \u{1c}text\n",
            img_src: Some("data:raw"),
        }
    }
    fn frame() -> Vec<u8> {
        let rows = [row()];
        let mut frame = vec![0; encoded_size(Layout::DdgImage, &rows, || Ok(())).unwrap()];
        encode(Layout::DdgImage, &rows, &mut frame, || Ok(())).unwrap();
        frame
    }
    #[test]
    fn streaming_legacy_rows_are_byte_identical_to_checked_buffer_encoder() {
        let expected = frame();
        let mut streamed = Vec::new();
        write(Layout::DdgImage, &[row()], &mut streamed).unwrap();
        assert_eq!(&streamed[..8], &(expected.len() as u64).to_be_bytes());
        assert_eq!(&streamed[8..], expected);
    }
    #[test]
    fn complete_rows_roundtrip_preserves_raw_fields_and_explicit_fallback() {
        let data = frame();
        let rows = Rows::validate(&data, Layout::DdgImage, || Ok(())).unwrap();
        let mut actual = vec![];
        rows.for_each(
            || Ok(()),
            |r| {
                actual.push(r);
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(actual, [row()]);
        let ordinary = Row {
            query_title_fallback: false,
            img_src: None,
            ..row()
        };
        for layout in [
            Layout::DdgHtml,
            Layout::DdgLite,
            Layout::DdgImage,
            Layout::Google,
        ] {
            let mut data = vec![0; encoded_size(layout, &[ordinary], || Ok(())).unwrap()];
            encode(layout, &[ordinary], &mut data, || Ok(())).unwrap();
            Rows::validate(&data, layout, || Ok(()))
                .unwrap()
                .for_each(
                    || Ok(()),
                    |r| {
                        assert_eq!(r, ordinary);
                        Ok(())
                    },
                )
                .unwrap();
        }
    }
    #[test]
    fn full_frame_validation_rejects_truncation_trailing_tags_counts_and_utf8() {
        let data = frame();
        for length in 0..data.len() {
            assert!(
                Rows::validate(&data[..length], Layout::DdgImage, || Ok(())).is_err(),
                "prefix {length}"
            );
        }
        let mut trailing = data.clone();
        trailing.push(0);
        assert!(matches!(
            Rows::validate(&trailing, Layout::DdgImage, || Ok(())),
            Err(Error::Trailing)
        ));
        assert!(matches!(
            Rows::validate(&data, Layout::Google, || Ok(())),
            Err(Error::Layout)
        ));
        for index in [0, 4, 5, 6, 23] {
            let mut bad = data.clone();
            bad[index] = 0xff;
            assert!(Rows::validate(&bad, Layout::DdgImage, || Ok(())).is_err());
        }
        for index in [7, 24] {
            let mut bad = data.clone();
            bad[index..index + 8].fill(0xff);
            assert!(Rows::validate(&bad, Layout::DdgImage, || Ok(())).is_err());
        }
        let mut bad = data.clone();
        bad[32] = 0xff;
        assert!(matches!(
            Rows::validate(&bad, Layout::DdgImage, || Ok(())),
            Err(Error::Utf8)
        ));
        let mut cursor = Cursor {
            bytes: &[],
            at: usize::MAX,
        };
        assert_eq!(cursor.take(1), Err(Error::Overflow));
    }
    #[test]
    fn cancellation_checks_cover_encode_full_validation_and_projection() {
        let data = frame();
        let mut calls = 0;
        let result = Rows::validate(&data, Layout::DdgImage, || {
            calls += 1;
            if calls == 4 {
                Err(Error::Deadline)
            } else {
                Ok(())
            }
        });
        assert!(matches!(result, Err(Error::Deadline)));
        let rows = Rows::validate(&data, Layout::DdgImage, || Ok(())).unwrap();
        let mut visited = 0;
        assert_eq!(
            rows.for_each(
                || Err(Error::Deadline),
                |_| {
                    visited += 1;
                    Ok(())
                }
            ),
            Err(Error::Deadline)
        );
        assert_eq!(visited, 0);
        let mut output = vec![0; data.len()];
        assert_eq!(
            encode(Layout::DdgImage, &[row()], &mut output, || Err(
                Error::Deadline
            )),
            Err(Error::Deadline)
        );
        assert_eq!(
            encode(Layout::DdgImage, &[row()], &mut output[..1], || Ok(())),
            Err(Error::Size)
        );
    }
    #[test]
    fn size_counting_checks_deadline_and_whole_field_work_has_a_frame_bound() {
        let rows = [row(), row(), row()];
        let mut calls = 0;
        assert_eq!(
            encoded_size(Layout::DdgImage, &rows, || {
                calls += 1;
                if calls == 3 {
                    Err(Error::Deadline)
                } else {
                    Ok(())
                }
            }),
            Err(Error::Deadline)
        );
        assert_eq!(calls, 3);
        let mut untouched = [0xa5; 1];
        let mut calls = 0;
        assert_eq!(
            encode(Layout::DdgImage, &rows, &mut untouched, || {
                calls += 1;
                if calls == 4 {
                    Err(Error::Deadline)
                } else {
                    Ok(())
                }
            }),
            Err(Error::Deadline)
        );
        assert_eq!(untouched, [0xa5]);
        let large = "é".repeat(512 * 1024);
        let large_row = Row {
            title: &large,
            ..row()
        };
        let size = encoded_size(Layout::DdgImage, &[large_row], || Ok(())).unwrap();
        assert!(size <= MAX_FRAME_BYTES);
        let mut data = vec![0; size];
        encode(Layout::DdgImage, &[large_row], &mut data, || Ok(())).unwrap();
        let mut checks = 0;
        let parsed = Rows::validate(&data, Layout::DdgImage, || {
            checks += 1;
            Ok(())
        })
        .unwrap();
        assert!(checks >= 10); // Checks bracket whole fields; not each byte/chunk.
        parsed
            .for_each(
                || Ok(()),
                |actual| {
                    assert_eq!(actual, large_row);
                    Ok(())
                },
            )
            .unwrap();
        // Announced oversized field fails before UTF8/copy, without allocating it.
        data[24..32].copy_from_slice(&(MAX_FRAME_BYTES as u64 + 1).to_be_bytes());
        assert!(Rows::validate(&data, Layout::DdgImage, || Ok(())).is_err());
    }
}

/// Version3 carries the child's already source-sanitized image media fields.
/// Raw result URL and img_src stay separate and unchanged.
#[derive(Clone, Copy)]
pub struct ImageMedia<'a> {
    pub url: Option<&'a str>,
    pub thumbnail: Option<&'a str>,
    pub source: Option<&'a str>,
}
#[derive(Clone, Copy)]
pub struct ExtendedRow<'a> {
    pub base: Row<'a>,
    pub media: Option<ImageMedia<'a>>,
}
pub fn extended_size(layout: Layout, rows: &[ExtendedRow<'_>]) -> Result<usize, Error> {
    let mut size = HEADER;
    for row in rows {
        valid_flags(
            layout,
            row.base.query_title_fallback,
            row.base.img_src.is_some(),
        )?;
        if row.media.is_some() && (layout != Layout::DdgImage || row.base.img_src.is_none()) {
            return Err(Error::Flags);
        }
        size = size.checked_add(9).ok_or(Error::Overflow)?;
        for field in [
            Some(row.base.url),
            Some(row.base.title),
            Some(row.base.content),
            row.base.img_src,
        ]
        .into_iter()
        .flatten()
        {
            size = size
                .checked_add(8)
                .and_then(|n| n.checked_add(field.len()))
                .ok_or(Error::Overflow)?;
        }
        if let Some(media) = row.media {
            size = size.checked_add(1).ok_or(Error::Overflow)?;
            for field in [media.url, media.thumbnail, media.source]
                .into_iter()
                .flatten()
            {
                size = size
                    .checked_add(8)
                    .and_then(|n| n.checked_add(field.len()))
                    .ok_or(Error::Overflow)?;
            }
        }
        if size > MAX_FRAME_BYTES {
            return Err(Error::Size);
        }
    }
    Ok(size)
}
pub fn write_extended(
    layout: Layout,
    rows: &[ExtendedRow<'_>],
    writer: &mut impl std::io::Write,
) -> std::io::Result<()> {
    let size =
        extended_size(layout, rows).map_err(|error| std::io::Error::other(format!("{error:?}")))?;
    writer.write_all(&(size as u64).to_be_bytes())?;
    writer.write_all(b"A05R\x03\x02")?;
    writer.write_all(&[layout.tag()])?;
    writer.write_all(&(rows.len() as u64).to_be_bytes())?;
    fn field(writer: &mut impl std::io::Write, value: &str) -> std::io::Result<()> {
        writer.write_all(&(value.len() as u64).to_be_bytes())?;
        writer.write_all(value.as_bytes())
    }
    for row in rows {
        writer.write_all(&row.base.position.to_be_bytes())?;
        writer.write_all(&[u8::from(row.base.query_title_fallback)
            | (u8::from(row.base.img_src.is_some()) << 1)
            | (u8::from(row.media.is_some()) << 2)])?;
        for value in [
            Some(row.base.url),
            Some(row.base.title),
            Some(row.base.content),
            row.base.img_src,
        ]
        .into_iter()
        .flatten()
        {
            field(writer, value)?;
        }
        if let Some(media) = row.media {
            writer.write_all(&[u8::from(media.url.is_some())
                | (u8::from(media.thumbnail.is_some()) << 1)
                | (u8::from(media.source.is_some()) << 2)])?;
            for value in [media.url, media.thumbnail, media.source]
                .into_iter()
                .flatten()
            {
                field(writer, value)?;
            }
        }
    }
    writer.flush()
}
