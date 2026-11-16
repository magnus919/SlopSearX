//! Borrowed row frames with cooperative UTF8 validation and no unchecked cast.
//! Fields remain validated byte views; consumers copy only bounded UTF8 pieces.
use super::row_wire::{Error, Layout, MAX_FRAME_BYTES};
const HEADER: usize = 15;
const MIN_ROW: usize = 33;
const CHUNK: usize = 4096;
#[derive(Clone, Copy)]
pub(crate) struct Field<'a>(&'a [u8]);
impl<'a> Field<'a> {
    pub(crate) fn len(self) -> usize {
        self.0.len()
    }
    pub(crate) fn parts(self) -> Parts<'a> {
        Parts(self.0)
    }
}
pub(crate) struct Parts<'a>(&'a [u8]);
impl<'a> Iterator for Parts<'a> {
    type Item = Result<&'a str, Error>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.0.is_empty() {
            return None;
        }
        let count = self.0.len().min(CHUNK);
        let prefix = &self.0[..count];
        match std::str::from_utf8(prefix) {
            Ok(part) => {
                self.0 = &self.0[count..];
                Some(Ok(part))
            }
            Err(error) if error.error_len().is_none() && error.valid_up_to() != 0 => {
                let count = error.valid_up_to();
                let part = std::str::from_utf8(&prefix[..count]).map_err(|_| Error::Utf8);
                self.0 = &self.0[count..];
                Some(part)
            }
            Err(_) => {
                self.0 = &[];
                Some(Err(Error::Utf8))
            }
        }
    }
}
#[derive(Clone, Copy)]
pub(crate) struct Row<'a> {
    pub(crate) position: i64,
    pub(crate) fallback: bool,
    pub(crate) url: Field<'a>,
    pub(crate) title: Field<'a>,
    pub(crate) content: Field<'a>,
    pub(crate) image: Option<Field<'a>>,
    pub(crate) media: Option<Media<'a>>,
}
#[derive(Clone, Copy)]
pub(crate) struct Media<'a> {
    pub(crate) url: Option<Field<'a>>,
    pub(crate) thumbnail: Option<Field<'a>>,
    pub(crate) source: Option<Field<'a>>,
}
struct Cursor<'a> {
    bytes: &'a [u8],
    offset: usize,
}
impl<'a> Cursor<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], Error> {
        let end = self.offset.checked_add(n).ok_or(Error::Overflow)?;
        let value = self.bytes.get(self.offset..end).ok_or(Error::Truncated)?;
        self.offset = end;
        Ok(value)
    }
    fn number(&mut self) -> Result<u64, Error> {
        Ok(u64::from_be_bytes(
            self.take(8)?.try_into().map_err(|_| Error::Truncated)?,
        ))
    }
    fn field(&mut self) -> Result<Field<'a>, Error> {
        let n = usize::try_from(self.number()?).map_err(|_| Error::Overflow)?;
        Ok(Field(self.take(n)?))
    }
    fn row(&mut self, layout: Layout) -> Result<Row<'a>, Error> {
        let position = i64::from_be_bytes(self.take(8)?.try_into().map_err(|_| Error::Truncated)?);
        let flags = self.take(1)?[0];
        if flags & !7 != 0 || (layout != Layout::DdgImage && flags != 0) {
            return Err(Error::Flags);
        }
        let url = self.field()?;
        let title = self.field()?;
        let content = self.field()?;
        let image = if flags & 2 != 0 {
            Some(self.field()?)
        } else {
            None
        };
        let media = if flags & 4 != 0 {
            if image.is_none() {
                return Err(Error::Flags);
            }
            let flags = self.take(1)?[0];
            if flags & !7 != 0 {
                return Err(Error::Flags);
            }
            Some(Media {
                url: if flags & 1 != 0 {
                    Some(self.field()?)
                } else {
                    None
                },
                thumbnail: if flags & 2 != 0 {
                    Some(self.field()?)
                } else {
                    None
                },
                source: if flags & 4 != 0 {
                    Some(self.field()?)
                } else {
                    None
                },
            })
        } else {
            None
        };
        Ok(Row {
            position,
            fallback: flags & 1 != 0,
            url,
            title,
            content,
            image,
            media,
        })
    }
}
pub(crate) struct Rows<'a> {
    bytes: &'a [u8],
    count: usize,
    layout: Layout,
}
impl<'a> Rows<'a> {
    pub(crate) async fn validate(
        bytes: &'a [u8],
        expected: Layout,
        check: &(dyn Fn() -> Result<(), Error> + Sync),
    ) -> Result<Self, Error> {
        check()?;
        if bytes.len() > MAX_FRAME_BYTES {
            return Err(Error::Size);
        }
        let mut cursor = Cursor { bytes, offset: 0 };
        if cursor.take(5)? != b"A05R\x03" {
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
        if count > (bytes.len() - cursor.offset) / MIN_ROW {
            return Err(Error::Count);
        }
        for _ in 0..count {
            check()?;
            let row = cursor.row(layout)?;
            for field in [
                Some(row.url),
                Some(row.title),
                Some(row.content),
                row.image,
                row.media.and_then(|m| m.url),
                row.media.and_then(|m| m.thumbnail),
                row.media.and_then(|m| m.source),
            ]
            .into_iter()
            .flatten()
            {
                for part in field.parts() {
                    check()?;
                    part?;
                    tokio::task::yield_now().await;
                    check()?;
                }
            }
            tokio::task::yield_now().await;
        }
        if cursor.offset != bytes.len() {
            return Err(Error::Trailing);
        }
        check()?;
        Ok(Self {
            bytes,
            count,
            layout,
        })
    }
    pub(crate) fn count(&self) -> usize {
        self.count
    }
    pub(crate) fn iter(&self) -> impl Iterator<Item = Result<Row<'a>, Error>> {
        let mut cursor = Cursor {
            bytes: self.bytes,
            offset: HEADER,
        };
        (0..self.count).map(move |_| cursor.row(self.layout))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn image_raw_sanitized_and_optional_empty_fields_are_distinct_and_closed() {
        use super::super::row_wire::{ExtendedRow, ImageMedia, Row, write_extended};
        let row = ExtendedRow {
            base: Row {
                position: 2,
                query_title_fallback: true,
                url: "https://x/?token=s&q=%2B",
                title: "",
                content: "é",
                img_src: Some("//img/?api_key=s&x=1"),
            },
            media: Some(ImageMedia {
                url: Some("//img/?x=1"),
                thumbnail: Some(""),
                source: None,
            }),
        };
        let mut encoded = Vec::new();
        write_extended(Layout::DdgImage, &[row], &mut encoded).unwrap();
        let frame = &encoded[8..];
        let rows = Rows::validate(frame, Layout::DdgImage, &|| Ok(()))
            .await
            .unwrap();
        let parsed = rows.iter().next().unwrap().unwrap();
        let text = |field: Field<'_>| field.parts().collect::<Result<String, _>>().unwrap();
        assert!(parsed.fallback);
        assert_eq!(text(parsed.url), row.base.url);
        assert_eq!(text(parsed.image.unwrap()), row.base.img_src.unwrap());
        let media = parsed.media.unwrap();
        assert_eq!(text(media.url.unwrap()), "//img/?x=1");
        assert_eq!(text(media.thumbnail.unwrap()), "");
        assert!(media.source.is_none());
        for length in 0..frame.len() {
            assert!(
                Rows::validate(&frame[..length], Layout::DdgImage, &|| Ok(()))
                    .await
                    .is_err()
            );
        }
        for flags in [4, 8, 255] {
            let mut invalid = frame.to_vec();
            invalid[HEADER + 8] = flags;
            assert!(
                Rows::validate(&invalid, Layout::DdgImage, &|| Ok(()))
                    .await
                    .is_err()
            );
        }
        let mut oversized = frame.to_vec();
        oversized[HEADER + 9..HEADER + 17].copy_from_slice(&u64::MAX.to_be_bytes());
        assert!(
            Rows::validate(&oversized, Layout::DdgImage, &|| Ok(()))
                .await
                .is_err()
        );
        let mut invalid = frame.to_vec();
        invalid.push(0);
        assert!(
            Rows::validate(&invalid, Layout::DdgImage, &|| Ok(()))
                .await
                .is_err()
        );
        assert!(
            Rows::validate(frame, Layout::Google, &|| Ok(()))
                .await
                .is_err()
        );
    }
    #[tokio::test]
    async fn cooperative_fields_preserve_split_utf8_and_reject_malformed_complete_frames() {
        let title = "é𐀀".repeat(3000);
        let row = super::super::row_wire::Row {
            position: 3,
            query_title_fallback: false,
            url: "/relative",
            title: &title,
            content: "\u{1c} S",
            img_src: None,
        };
        let mut frame = Vec::new();
        super::super::row_wire::write_extended(
            Layout::Google,
            &[super::super::row_wire::ExtendedRow {
                base: row,
                media: None,
            }],
            &mut frame,
        )
        .unwrap();
        let frame = &frame[8..];
        let validated = Rows::validate(frame, Layout::Google, &|| Ok(()))
            .await
            .unwrap();
        let row = validated.iter().next().unwrap().unwrap();
        assert_eq!(row.position, 3);
        assert!(!row.fallback);
        assert_eq!(row.title.len(), title.len());
        assert_eq!(
            row.title.parts().collect::<Result<String, _>>().unwrap(),
            title
        );
        for n in 0..frame.len() {
            assert!(
                Rows::validate(&frame[..n], Layout::Google, &|| Ok(()))
                    .await
                    .is_err()
            );
        }
        assert!(
            Rows::validate(frame, Layout::DdgHtml, &|| Ok(()))
                .await
                .is_err()
        );
        let mut invalid = frame.to_vec();
        invalid.extend([0]);
        assert!(
            Rows::validate(&invalid, Layout::Google, &|| Ok(()))
                .await
                .is_err()
        );
        let mut invalid = frame.to_vec();
        invalid[HEADER + 9 + 8] = 255;
        assert!(
            Rows::validate(&invalid, Layout::Google, &|| Ok(()))
                .await
                .is_err()
        );
        assert!(matches!(
            Rows::validate(frame, Layout::Google, &|| Err(Error::Deadline)).await,
            Err(Error::Deadline)
        ));
    }
}
