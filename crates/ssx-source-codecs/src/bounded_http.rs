//! Charge-owning replacement-body dispatcher. Request-owned calls belong inside
//! the supervised worker; borrowed body/header ownership is charged by its caller.
use crate::{
    BudgetText, BudgetTextBuilder,
    bounded::{Error, PointSequence, PointSink},
};
use ssx_owned_budget::{Budget, OwnedString};

#[derive(Debug, PartialEq, Eq)]
pub enum BoundedDecodeError {
    Codec(&'static str),
    Output(Error),
    RegistryInvariant,
}
impl From<Error> for BoundedDecodeError {
    fn from(error: Error) -> Self {
        Self::Output(error)
    }
}
impl From<ssx_owned_budget::Error> for BoundedDecodeError {
    fn from(error: ssx_owned_budget::Error) -> Self {
        Self::Output(Error::Resource(error))
    }
}
fn visit_label(
    label: &str,
    check: &dyn Fn() -> Result<(), Error>,
    mut emit: impl FnMut(u8) -> Result<(), BoundedDecodeError>,
) -> Result<(), BoundedDecodeError> {
    let mut punctuation = false;
    let mut emitted = false;
    for character in label.chars() {
        check()?;
        if crate::label_alnum::is_alnum(character) || character == '.' {
            if punctuation && emitted {
                emit(b'_')?;
            }
            if character.is_ascii() {
                emit(character.to_ascii_lowercase() as u8)?;
                emitted = true;
            }
            punctuation = false;
        } else {
            punctuation = true;
        }
    }
    Ok(())
}
pub(crate) fn canonical(
    label: &str,
    budget: &Budget,
    check: &dyn Fn() -> Result<(), Error>,
) -> Result<Option<&'static str>, BoundedDecodeError> {
    let mut count = 0usize;
    visit_label(label, check, |_| {
        count = count.checked_add(1).ok_or(Error::OutputBound)?;
        Ok(())
    })?;
    let mut normalized = OwnedString::new(budget)?;
    normalized.try_reserve_exact(count)?;
    visit_label(label, check, |byte| {
        normalized.try_push(char::from(byte))?;
        Ok(())
    })?;
    if normalized.len() != count {
        return Err(BoundedDecodeError::RegistryInvariant);
    }
    if let Some(name) = crate::alias_only::canonical(normalized.as_str()) {
        return Ok(Some(name));
    }
    if normalized.as_str().contains('.') {
        let mut dotted = OwnedString::new(budget)?;
        dotted.try_reserve_exact(normalized.len())?;
        for byte in normalized.as_bytes() {
            check()?;
            dotted.try_push(char::from(if *byte == b'.' { b'_' } else { *byte }))?;
        }
        return Ok(crate::alias_only::canonical(dotted.as_str()));
    }
    Ok(crate::aliases::canonical(normalized.as_str()))
}
struct Checked<'a> {
    output: &'a mut BudgetTextBuilder,
    check: &'a dyn Fn() -> Result<(), Error>,
}
impl PointSink for Checked<'_> {
    fn emit(&mut self, point: u32) -> Result<(), Error> {
        (self.check)()?;
        self.output.try_push(point)
    }
}
impl PointSequence for Checked<'_> {
    fn len(&self) -> usize {
        self.output.len()
    }
    fn insert(&mut self, index: usize, point: u32) -> Result<(), Error> {
        (self.check)()?;
        self.output.try_insert(index, point)?;
        (self.check)()
    }
}
fn bool_success(result: Result<bool, Error>) -> Result<(), BoundedDecodeError> {
    if result? {
        Ok(())
    } else {
        Err(BoundedDecodeError::RegistryInvariant)
    }
}
fn puny_error(error: crate::punycode::Failure) -> BoundedDecodeError {
    match error {
        crate::punycode::Failure::Output(error) => BoundedDecodeError::Output(error),
        crate::punycode::Failure::Source(_) => BoundedDecodeError::RegistryInvariant,
    }
}
/// Same replacement-body policy as decode_http, with owned output/scratch charges.
/// HTTPX/email label selection (including final NUL errors) remains caller-owned.
/// No allocating legacy decoder is used. Resource/Deadline never become Codec.
pub fn decode_http_bounded(
    body: &[u8],
    label: &str,
    budget: &Budget,
    checkpoint: &dyn Fn() -> Result<(), Error>,
) -> Result<BudgetText, BoundedDecodeError> {
    checkpoint()?;
    if body.is_empty() {
        return Ok(BudgetTextBuilder::new(budget, 0)?.finish());
    }
    let name = canonical(label, budget, checkpoint)?.unwrap_or("utf-8");
    let rejected = match name {
        "base64" | "bz2" | "hex" | "quopri" | "uu" | "zlib" => Some(""),
        "rot-13" => {
            Some("descriptor 'translate' for 'str' objects doesn't apply to a 'bytes' object")
        }
        "idna" => Some("Unsupported error handling: replace"),
        "undefined" => Some("undefined encoding"),
        _ => None,
    };
    if let Some(message) = rejected {
        return Err(BoundedDecodeError::Codec(message));
    }
    crate::unicode::validate_input(body, name).map_err(BoundedDecodeError::Codec)?;
    let work = crate::punycode::Work {
        budget: Some(budget),
        checkpoint,
    };
    let prepared = if name == "punycode" {
        Some(crate::punycode::prepare(body, false, work).map_err(puny_error)?)
    } else {
        None
    };
    let mut builder = BudgetTextBuilder::new(budget, body.len())?;
    let mut output = Checked {
        output: &mut builder,
        check: checkpoint,
    };
    if let Some(table) = crate::singlebyte::table(name) {
        for &byte in body {
            output.emit(table[usize::from(byte)])?;
        }
    } else if let Some(result) = crate::unicode::decode_into(body, name, &mut output) {
        result.map_err(|error| match error {
            crate::unicode::DecodeFailure::Source(message) => BoundedDecodeError::Codec(message),
            crate::unicode::DecodeFailure::Output(error) => BoundedDecodeError::Output(error),
        })?;
    } else if let Some(result) = crate::doublebyte::decode_into(body, name, false, &mut output) {
        bool_success(result)?;
    } else if let Some(result) = crate::iso2022::decode_into(body, name, false, &mut output) {
        result.map_err(|error| match error {
            crate::iso2022::SinkError::Runtime(message) => BoundedDecodeError::Codec(message),
            crate::iso2022::SinkError::Output(error) => BoundedDecodeError::Output(error),
            crate::iso2022::SinkError::Unicode => BoundedDecodeError::RegistryInvariant,
        })?;
    } else {
        match name {
            "utf-7" => bool_success(crate::utf7::decode_into(body, false, &mut output))?,
            "hz" => bool_success(crate::hz::decode_into(body, false, &mut output))?,
            "unicode-escape" => {
                bool_success(crate::unicode_escape::decode_into(body, false, &mut output))?
            }
            "raw-unicode-escape" => bool_success(crate::raw_unicode_escape::decode_into(
                body,
                false,
                &mut output,
            ))?,
            "punycode" => crate::punycode::decode_prepared(
                prepared.ok_or(BoundedDecodeError::RegistryInvariant)?,
                work,
                &mut output,
            )
            .map_err(puny_error)?,
            "ascii" => {
                for &byte in body {
                    output.emit(if byte.is_ascii() {
                        u32::from(byte)
                    } else {
                        0xfffd
                    })?;
                }
            }
            "charmap" | "iso8859-1" => {
                for &byte in body {
                    output.emit(u32::from(byte))?;
                }
            }
            _ => return Err(BoundedDecodeError::RegistryInvariant),
        }
    }
    checkpoint()?;
    Ok(builder.finish())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn charged_aliases_match_all_retained_source_registry_observations() {
        for (raw, count) in [
            (include_str!("../tests/label-alias-source.json"), 479),
            (include_str!("../tests/label-dotted-source.json"), 1451),
        ] {
            let source: serde_json::Value = serde_json::from_str(raw).unwrap();
            let rows = source["cases"].as_array().unwrap();
            assert_eq!(rows.len(), count);
            for row in rows {
                let budget = Budget::new(64 * 1024).unwrap();
                let label = row["label"].as_str().unwrap();
                assert_eq!(
                    canonical(label, &budget, &|| Ok(())),
                    Ok(row["canonical"].as_str()),
                    "{label:?}"
                );
                let lookup = crate::lookup_http_encoding_bounded(label, &budget, &|| Ok(()));
                if label.contains('\0') {
                    assert_eq!(
                        lookup,
                        Err(crate::BoundedParameterError::ValueError(
                            "embedded null character"
                        ))
                    );
                } else {
                    assert_eq!(
                        lookup,
                        Ok(row["canonical"].as_str()),
                        "bounded final lookup {label:?}"
                    );
                }
                assert_eq!(budget.snapshot().unwrap().used, 0);
            }
        }
        let budget = Budget::new(9).unwrap();
        assert!(matches!(
            canonical("utf.8", &budget, &|| Ok(())),
            Err(BoundedDecodeError::Output(Error::Resource(
                ssx_owned_budget::Error::Exhausted
            )))
        ));
        assert_eq!(budget.snapshot().unwrap().used, 0);
        let budget = Budget::new(10).unwrap();
        assert_eq!(canonical("utf.8", &budget, &|| Ok(())), Ok(None));
        assert_eq!(budget.snapshot().unwrap().peak, 10);
        assert_eq!(budget.snapshot().unwrap().used, 0);
    }
}
