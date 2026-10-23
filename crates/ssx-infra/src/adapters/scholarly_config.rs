//! Raw F06-reachable scalar settings. Projection happens at the engine's source
//! operation, never by generic effective_i64 coercion. Transport policy is separate.
use std::{borrow::Cow, sync::Arc};

#[derive(Clone, Debug, PartialEq)]
pub enum Scalar {
    Boolean(bool),
    Integer(i64),
    Float(f64),
    Text(Arc<str>),
}
impl Scalar {
    /// Borrow source text unchanged; numeric formatting is shared with the
    /// accepted Python formatter. Its small temporary is not yet budgeted here.
    pub fn python_text(&self) -> Cow<'_, str> {
        match self {
            Self::Boolean(false) => Cow::Borrowed("False"),
            Self::Boolean(true) => Cow::Borrowed("True"),
            Self::Integer(value) => Cow::Owned(value.to_string()),
            Self::Float(value) if value.is_nan() => Cow::Borrowed("nan"),
            Self::Float(value) if *value == f64::INFINITY => Cow::Borrowed("inf"),
            Self::Float(value) if *value == f64::NEG_INFINITY => Cow::Borrowed("-inf"),
            Self::Float(value) => Cow::Owned(ssx_json::python_float(*value).expect("finite float")),
            Self::Text(value) => Cow::Borrowed(value),
        }
    }
    /// HTTPX QueryParams special-cases bool, then uses Python str for these
    /// reachable scalar types. Rejected config null/containers are not admitted.
    pub fn query_text(&self) -> Cow<'_, str> {
        match self {
            Self::Boolean(false) => Cow::Borrowed("false"),
            Self::Boolean(true) => Cow::Borrowed("true"),
            _ => self.python_text(),
        }
    }
    /// OpenAlex slices only AFTER HTTP status and JSON decoding. The caller must
    /// preserve that ordering. Integral floats and numeric-looking text still fail.
    pub fn slice_end(&self, len: usize) -> Result<usize, &'static str> {
        let value = match self {
            Self::Boolean(value) => i64::from(*value),
            Self::Integer(value) => *value,
            Self::Float(_) | Self::Text(_) => {
                return Err("slice indices must be integers or None or have an __index__ method");
            }
        };
        Ok(if value < 0 {
            len.saturating_sub(usize::try_from(value.unsigned_abs()).unwrap_or(usize::MAX))
        } else {
            len.min(usize::try_from(value).unwrap_or(usize::MAX))
        })
    }
    /// Source division does not parse numeric-looking strings. The actual numeric
    /// timeout/deadline projection awaits the reviewed finite SearchContext seam.
    pub fn validate_timeout_operand(&self, integer_divisor: bool) -> Result<(), &'static str> {
        match self {
            Self::Text(_) if integer_divisor => {
                Err("unsupported operand type(s) for /: 'str' and 'int'")
            }
            Self::Text(_) => Err("unsupported operand type(s) for /: 'str' and 'float'"),
            _ => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::scholarly_wire;

    fn scalar(value: &serde_json::Value) -> Option<Scalar> {
        use serde_json::Value as V;
        Some(match value {
            V::Bool(v) => Scalar::Boolean(*v),
            V::String(v) => Scalar::Text(Arc::from(v.as_str())),
            V::Number(v) if v.is_i64() => Scalar::Integer(v.as_i64().unwrap()),
            V::Number(v) => Scalar::Float(v.as_f64().unwrap()),
            V::Object(v) => Scalar::Float(match v.get("$source_nonfinite_float")?.as_str()? {
                "nan" => f64::NAN,
                "inf" => f64::INFINITY,
                "-inf" => f64::NEG_INFINITY,
                _ => return None,
            }),
            _ => return None,
        })
    }

    #[test]
    fn raw_config_request_and_operation_projection_matches_actual_source() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../docs/rust-rewrite/a06/source-config.json"
        ))
        .unwrap();
        let rows = fixture["records"].as_array().unwrap();
        assert_eq!(rows.len(), 153);
        let (mut source_only, mut requests, mut slices, mut timeout_errors, mut timeout_operands) =
            (0, 0, 0, 0, 0);
        for row in rows {
            let id = row["id"].as_str().unwrap();
            let config = row["config"].as_object().unwrap();
            let (field, value) = config.iter().next().unwrap();
            if row["native_reachability"]
                .as_str()
                .unwrap()
                .starts_with("direct-source-only")
            {
                assert!(scalar(value).is_none(), "{id}");
                source_only += 1;
                continue;
            }
            let value = scalar(value).unwrap();
            let engine = row["engine"].as_str().unwrap();
            if field == "max_results" {
                let query = row["query"].as_str().unwrap();
                let actual = match engine {
                    "arxiv" => scholarly_wire::arxiv_url_scalar(
                        "https://export.arxiv.org/api/query",
                        query,
                        &value,
                    )
                    .unwrap(),
                    "semanticscholar" => scholarly_wire::semantic_scholar_url_scalar(
                        "https://api.semanticscholar.org/graph/v1/paper/search",
                        query,
                        &value,
                    )
                    .unwrap(),
                    "openalex" => scholarly_wire::openalex_url_scalar(
                        "https://api.openalex.org",
                        query,
                        &value,
                        None,
                        None,
                    ),
                    _ => panic!("unexpected engine"),
                };
                assert_eq!(row["requests"].as_array().unwrap().len(), 1, "{id}");
                assert_eq!(actual, row["requests"][0]["url"].as_str().unwrap(), "{id}");
                requests += 1;
                if engine == "openalex" && id.ends_with(".valid") {
                    match value.slice_end(3) {
                        Ok(end) => assert_eq!(
                            row["output"]["results"].as_array().unwrap().len(),
                            end,
                            "{id}"
                        ),
                        Err(message) => assert_eq!(
                            row["output"]["error_message"].as_str().unwrap(),
                            message,
                            "{id}"
                        ),
                    }
                    slices += 1;
                }
            } else {
                match value.validate_timeout_operand(engine == "openalex") {
                    Err(message) => {
                        assert!(row["requests"].as_array().unwrap().is_empty(), "{id}");
                        assert_eq!(
                            row["output"]["error_message"].as_str().unwrap(),
                            message,
                            "{id}"
                        );
                        timeout_errors += 1;
                    }
                    Ok(()) => {
                        // Classification only: MockTransport does not establish
                        // real zero/negative/nonfinite socket behavior.
                        assert_eq!(row["client_options"].as_array().unwrap().len(), 1, "{id}");
                        timeout_operands += 1;
                    }
                }
            }
        }
        assert_eq!(
            (
                source_only,
                requests,
                slices,
                timeout_errors,
                timeout_operands
            ),
            (27, 84, 14, 9, 33)
        );
        assert_eq!(Scalar::Integer(i64::MIN).slice_end(3), Ok(0));
        assert_eq!(Scalar::Integer(i64::MAX).slice_end(3), Ok(3));
    }
}
