//! Source-compatible validation occurs before any application service call.
use rmcp::ErrorData;
use ssx_json::{Map, Value};
pub(crate) struct Arguments {
    values: Map<String, Value>,
}
#[derive(Clone)]
enum Observed {
    Scalar(Value),
    Infinite(bool),
    Array(Vec<Observed>),
    Object(Map<String, Observed>),
}
impl Observed {
    fn from_value(value: Value) -> Self {
        match value {
            Value::Array(a) => Self::Array(a.into_iter().map(Self::from_value).collect()),
            Value::Object(o) => Self::Object(
                o.into_iter()
                    .map(|(k, v)| (k, Self::from_value(v)))
                    .collect(),
            ),
            v => Self::Scalar(v),
        }
    }
    fn parse(raw: &str, depth: usize) -> Result<Self, ErrorData> {
        if depth > 100 {
            return Err(ErrorData::invalid_params(
                "argument JSON depth limit exceeded",
                None,
            ));
        }
        let raw = raw.trim();
        let error = |e: serde_json::Error| ErrorData::invalid_params(e.to_string(), None);
        match raw.as_bytes().first() {
            Some(b'{') => {
                let map: Map<String, Box<serde_json::value::RawValue>> =
                    serde_json::from_str(raw).map_err(error)?;
                map.into_iter()
                    .map(|(k, v)| Ok((k, Self::parse(v.get(), depth + 1)?)))
                    .collect::<Result<Map<_, _>, ErrorData>>()
                    .map(Self::Object)
            }
            Some(b'[') => {
                let array: Vec<Box<serde_json::value::RawValue>> =
                    serde_json::from_str(raw).map_err(error)?;
                array
                    .into_iter()
                    .map(|v| Self::parse(v.get(), depth + 1))
                    .collect::<Result<Vec<_>, _>>()
                    .map(Self::Array)
            }
            Some(b'-' | b'0'..=b'9')
                if raw.contains(['.', 'e', 'E'])
                    && raw.parse::<f64>().is_ok_and(|n| n.is_infinite()) =>
            {
                let _: Box<serde_json::value::RawValue> =
                    serde_json::from_str(raw).map_err(error)?;
                Ok(Self::Infinite(raw.starts_with('-')))
            }
            _ => ssx_json::from_str(raw).map(Self::Scalar).map_err(error),
        }
    }
    fn is_null(&self) -> bool {
        matches!(self, Self::Scalar(Value::Null))
    }
    fn is_string(&self) -> bool {
        matches!(self, Self::Scalar(Value::String(_)))
    }
    fn finite(&self) -> Option<&Value> {
        if let Self::Scalar(v) = self {
            Some(v)
        } else {
            None
        }
    }
    fn into_value(self) -> Value {
        match self {
            Self::Scalar(v) => v,
            Self::Array(a) => Value::Array(a.into_iter().map(Self::into_value).collect()),
            Self::Object(o) => {
                Value::Object(o.into_iter().map(|(k, v)| (k, v.into_value())).collect())
            }
            Self::Infinite(_) => unreachable!("non-finite argument never passed validation"),
        }
    }
    fn kind(&self) -> &'static str {
        match self {
            Self::Infinite(_) => "float",
            Self::Array(_) => "list",
            Self::Object(_) => "dict",
            Self::Scalar(v) => match v {
                Value::Null => "NoneType",
                Value::Bool(_) => "bool",
                Value::String(_) => "str",
                Value::Number(n) => {
                    if n.is_f64() {
                        "float"
                    } else {
                        "int"
                    }
                }
                _ => unreachable!(),
            },
        }
    }
    fn python_repr(&self) -> String {
        match self {
            Self::Infinite(negative) => if *negative { "-inf" } else { "inf" }.into(),
            Self::Array(a) => format!(
                "[{}]",
                a.iter()
                    .map(Self::python_repr)
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            Self::Object(o) => format!(
                "{{{}}}",
                o.iter()
                    .map(|(k, v)| format!(
                        "{}: {}",
                        Self::Scalar(Value::String(k.clone())).python_repr(),
                        v.python_repr()
                    ))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            Self::Scalar(v) => {
                if v.is_string() {
                    let wrapped = ssx_domain::storage::python_value_string(&ssx_json::json!([v]));
                    wrapped[1..wrapped.len() - 1].to_owned()
                } else {
                    ssx_domain::storage::python_value_string(v)
                }
            }
        }
    }
}
struct Failure {
    location: String,
    message: &'static str,
    kind: &'static str,
    input: Observed,
}
fn failure(
    location: impl Into<String>,
    message: &'static str,
    kind: &'static str,
    input: &Observed,
) -> Failure {
    Failure {
        location: location.into(),
        message,
        kind,
        input: input.clone(),
    }
}
fn repr(v: &Observed) -> String {
    let s = v.python_repr();
    let chars: Vec<_> = s.chars().collect();
    if chars.len() > 50 {
        chars[..25]
            .iter()
            .chain(['.', '.', '.'].iter())
            .chain(chars[chars.len() - 24..].iter())
            .collect()
    } else {
        s
    }
}
fn validation_error(name: &str, errors: Vec<Failure>) -> ErrorData {
    let count = errors.len();
    let mut text = format!(
        "{count} validation error{} for call[{name}]",
        if count == 1 { "" } else { "s" }
    );
    for e in errors {
        text.push_str(&format!("\n{}\n  {} [type={}, input_value={}, input_type={}]\n    For further information visit https://errors.pydantic.dev/2.13/v/{}",e.location,e.message,e.kind,repr(&e.input),e.input.kind(),e.kind));
    }
    ErrorData::invalid_params(text, None)
}
fn clamp_integer(n: f64) -> i64 {
    if n >= i64::MAX as f64 {
        i64::MAX
    } else if n <= i64::MIN as f64 {
        i64::MIN
    } else {
        n as i64
    }
}
fn integer(value: &Value, strict: bool) -> Result<i64, (&'static str, &'static str)> {
    if let Some(n) = value.as_i64() {
        return Ok(n);
    }
    if let Some(n) = value.as_u64() {
        return Ok(i64::try_from(n).unwrap_or(i64::MAX));
    }
    if let Value::Number(n) = value
        && !n.is_f64()
    {
        return Ok(if n.as_str().starts_with('-') {
            i64::MIN
        } else {
            i64::MAX
        });
    }
    if strict {
        return Err(("Input should be a valid integer", "int_type"));
    }
    match value {
        Value::Bool(b) => Ok(i64::from(*b)),
        Value::Number(n) => match n.as_f64() {
            Some(n) if n.is_finite() && n.fract() == 0.0 => Ok(clamp_integer(n)),
            Some(n) if n.is_finite() => Err((
                "Input should be a valid integer, got a number with a fractional part",
                "int_from_float",
            )),
            _ => Err(("Input should be a finite number", "finite_number")),
        },
        Value::String(s) => {
            if s.len() > 4300 {
                return Err((
                    "Unable to parse input string as an integer, exceeded maximum size",
                    "int_parsing_size",
                ));
            }
            let s = s.trim_matches(ssx_json::python_whitespace);
            let digits = s.strip_prefix(['+', '-']).unwrap_or(s);
            let mut integer_digits = digits;
            if let Some((a, b)) = digits.split_once('.') {
                if b.bytes().all(|v| v == b'0') {
                    integer_digits = a;
                } else {
                    return Err((
                        "Input should be a valid integer, unable to parse string as an integer",
                        "int_parsing",
                    ));
                }
            }
            if !integer_digits.is_empty()
                && integer_digits
                    .bytes()
                    .all(|b| b.is_ascii_digit() || b == b'_')
                && !integer_digits.starts_with('_')
                && !integer_digits.ends_with('_')
                && !integer_digits.contains("__")
            {
                let number = s.replace('_', "").parse::<f64>().map_err(|_| {
                    (
                        "Input should be a valid integer, unable to parse string as an integer",
                        "int_parsing",
                    )
                })?;
                return Ok(clamp_integer(number));
            }
            Err((
                "Input should be a valid integer, unable to parse string as an integer",
                "int_parsing",
            ))
        }
        _ => Err(("Input should be a valid integer", "int_type")),
    }
}
fn boolean(value: &Value) -> Result<bool, (&'static str, &'static str)> {
    match value {
        Value::Bool(b) => Ok(*b),
        Value::Number(n) => match n.as_f64() {
            Some(0.0) => Ok(false),
            Some(1.0) => Ok(true),
            Some(f) if f.fract() == 0.0 && f >= i64::MIN as f64 && f < i64::MAX as f64 => Err((
                "Input should be a valid boolean, unable to interpret input",
                "bool_parsing",
            )),
            _ => Err(("Input should be a valid boolean", "bool_type")),
        },
        Value::String(s) => match s.to_ascii_lowercase().as_str() {
            "true" | "1" | "yes" | "on" | "y" | "t" => Ok(true),
            "false" | "0" | "no" | "off" | "n" | "f" => Ok(false),
            _ => Err((
                "Input should be a valid boolean, unable to interpret input",
                "bool_parsing",
            )),
        },
        _ => Err(("Input should be a valid boolean", "bool_type")),
    }
}
fn strict_integer(name: &str, key: &str) -> bool {
    key == "version"
        || (crate::evidence::tools::TOOL_NAMES.contains(&name)
            && ["limit", "max_depth", "max_nodes"].contains(&key))
}
fn validate_item(rule: &Value, value: &Observed, location: &str, failures: &mut Vec<Failure>) {
    match rule["type"].as_str() {
        Some("string") if !value.is_string() => failures.push(failure(
            location,
            "Input should be a valid string",
            "string_type",
            value,
        )),
        Some("object") => {
            if let Observed::Object(object) = value {
                if rule["additionalProperties"] == Value::Bool(true) {
                    for (key, value) in object {
                        finite_object(value, &format!("{location}.{key}"), failures);
                    }
                }
                if rule["additionalProperties"]["type"].as_str() == Some("string") {
                    for (key, value) in object {
                        validate_item(
                            &rule["additionalProperties"],
                            value,
                            &format!("{location}.{key}"),
                            failures,
                        );
                    }
                }
            } else {
                failures.push(failure(
                    location,
                    "Input should be a valid dictionary",
                    "dict_type",
                    value,
                ));
            }
        }
        _ => {}
    }
}
fn finite_object(value: &Observed, location: &str, failures: &mut Vec<Failure>) {
    match value {
        Observed::Infinite(_) => failures.push(failure(
            location,
            "Input should be a finite number",
            "finite_number",
            value,
        )),
        Observed::Array(a) => {
            for (i, v) in a.iter().enumerate() {
                finite_object(v, &format!("{location}.{i}"), failures);
            }
        }
        Observed::Object(o) => {
            for (k, v) in o {
                finite_object(v, &format!("{location}.{k}"), failures);
            }
        }
        _ => {}
    }
}
impl Arguments {
    pub(crate) fn new(name: &str, values: Map<String, Value>) -> Result<Self, ErrorData> {
        Self::validate(
            name,
            values
                .into_iter()
                .map(|(k, v)| (k, Observed::from_value(v)))
                .collect(),
        )
    }
    pub(crate) fn raw(name: &str, json: &str) -> Result<Self, ErrorData> {
        let Observed::Object(values) = Observed::parse(json, 0)? else {
            return Err(ErrorData::invalid_params(
                "arguments must be an object",
                None,
            ));
        };
        Self::validate(name, values)
    }
    fn validate(name: &str, mut values: Map<String, Observed>) -> Result<Self, ErrorData> {
        let descriptors = &super::source_contract()["catalog"];
        let Some(tool) = descriptors["tools"]
            .as_array()
            .and_then(|a| a.iter().find(|v| v["name"].as_str() == Some(name)))
            .or_else(|| crate::evidence::tools::descriptor(name))
        else {
            return Err(ErrorData::invalid_params(
                format!("Unknown tool: {name}"),
                None,
            ));
        };
        let schema = &tool["inputSchema"];
        let mut failures = Vec::new();
        // Function parameters are validated in declaration order; extra keys follow.
        let ordered = schema["properties"].as_object().expect("parameters");
        for (key, _) in ordered {
            let Some(value) = values.get_mut(key) else {
                if schema["required"]
                    .as_array()
                    .is_some_and(|a| a.iter().any(|v| v.as_str() == Some(key)))
                {
                    failures.push(failure(
                        key,
                        "Missing required argument",
                        "missing_argument",
                        &Observed::Object(values.clone()),
                    ));
                }
                continue;
            };
            let rule = &schema["properties"][key.as_str()];
            if value.is_null()
                && rule["anyOf"]
                    .as_array()
                    .is_some_and(|a| a.iter().any(|v| v["type"] == "null"))
            {
                continue;
            }
            let rule = rule["anyOf"]
                .as_array()
                .and_then(|a| a.iter().find(|v| v["type"] != "null"))
                .unwrap_or(rule);
            match rule["type"].as_str() {
                Some("string") => {
                    if !value.is_string() {
                        failures.push(failure(
                            key,
                            "Input should be a valid string",
                            "string_type",
                            value,
                        ));
                    }
                }
                Some("array") => {
                    if let Observed::Array(array) = value {
                        for (i, v) in array.iter().enumerate() {
                            validate_item(&rule["items"], v, &format!("{key}.{i}"), &mut failures);
                        }
                    } else {
                        failures.push(failure(
                            key,
                            "Input should be a valid list",
                            "list_type",
                            value,
                        ));
                    }
                }
                Some("object") => validate_item(rule, value, key, &mut failures),
                Some("integer") => match value
                    .finite()
                    .map(|v| integer(v, strict_integer(name, key)))
                    .unwrap_or_else(|| {
                        Err(
                            if matches!(value, Observed::Infinite(_)) && !strict_integer(name, key)
                            {
                                ("Input should be a finite number", "finite_number")
                            } else {
                                ("Input should be a valid integer", "int_type")
                            },
                        )
                    }) {
                    Ok(n) => *value = Observed::Scalar(ssx_json::json!(n)),
                    Err((message, kind)) => failures.push(failure(key, message, kind, value)),
                },
                Some("boolean") => match value
                    .finite()
                    .map(boolean)
                    .unwrap_or(Err(("Input should be a valid boolean", "bool_type")))
                {
                    Ok(b) => *value = Observed::Scalar(Value::Bool(b)),
                    Err((message, kind)) => failures.push(failure(key, message, kind, value)),
                },
                _ => failures.push(failure(
                    key,
                    "Input should match the declared schema",
                    "value_error",
                    value,
                )),
            }
        }
        for (key, value) in &values {
            if schema["properties"].get(key).is_none() {
                failures.push(failure(
                    key,
                    "Unexpected keyword argument",
                    "unexpected_keyword_argument",
                    value,
                ));
            }
        }
        if !failures.is_empty() {
            return Err(validation_error(name, failures));
        }
        Ok(Self {
            values: values
                .into_iter()
                .map(|(k, v)| (k, v.into_value()))
                .collect(),
        })
    }
    pub(crate) fn values(&self) -> &Map<String, Value> {
        &self.values
    }
    pub(super) fn string<'a>(&'a self, key: &str, default: &'a str) -> &'a str {
        self.string_opt(key).unwrap_or(default)
    }
    pub(super) fn string_opt(&self, key: &str) -> Option<&str> {
        self.values.get(key).and_then(Value::as_str)
    }
    pub(super) fn strings(&self, key: &str) -> Option<Vec<String>> {
        self.values.get(key).and_then(Value::as_array).map(|a| {
            a.iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect()
        })
    }
    pub(super) fn boolean(&self, key: &str, default: bool) -> bool {
        self.values
            .get(key)
            .and_then(Value::as_bool)
            .unwrap_or(default)
    }
    pub(super) fn integer(&self, key: &str) -> Option<i64> {
        self.values.get(key).and_then(Value::as_i64)
    }
    pub(super) fn bound(&self, key: &str, maximum: u64) -> u64 {
        self.integer(key)
            .filter(|v| *v > 0)
            .map_or(maximum, |v| (v as u64).min(maximum))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn actual_source_argument_matrix_preserves_validation_text_and_coercions() {
        let capture: Value = ssx_json::from_str(include_str!(
            "../../../../docs/rust-rewrite/t06/validation.json"
        ))
        .unwrap();
        let mut mismatches = Vec::new();
        for row in capture["cases"].as_array().unwrap() {
            let name = row["name"].as_str().unwrap();
            let args = row["arguments"].as_object().unwrap().clone();
            let actual = Arguments::new(name, args);
            let rejected = row["result"]["isError"].as_bool() == Some(true);
            match actual {
                Ok(_) if rejected => mismatches.push(format!(
                    "{name}/{}: source rejects, native accepts",
                    row["case"]
                )),
                Err(e) if !rejected => mismatches.push(format!(
                    "{name}/{}: source accepts, native rejects {}",
                    row["case"], e.message
                )),
                Err(e)
                    if e.message.as_ref()
                        != row["result"]["content"][0]["text"].as_str().unwrap() =>
                {
                    mismatches.push(format!(
                        "{name}/{}: text mismatch\nactual={}\nexpected={}",
                        row["case"], e.message, row["result"]["content"][0]["text"]
                    ));
                }
                _ => {}
            }
        }
        assert!(
            mismatches.is_empty(),
            "{} mismatches:\n{}",
            mismatches.len(),
            mismatches.join("\n")
        );
    }
}
