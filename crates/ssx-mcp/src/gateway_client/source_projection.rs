//! Projection derived from gateway.py declarations and extraction functions.
//! Source-derived contract; this is not an executed Python observation.
use crate::{Result, opaque::Catalogs};
use serde_json::value::RawValue;
use std::collections::BTreeMap;
type Fields = BTreeMap<String, Box<RawValue>>;
const PROMPTS: [(&str, &str); 5] = [
    ("research_with_source_coverage", "question"),
    ("investigate_vulnerability", "target"),
    ("find_company_jobs", "company"),
    ("compare_package_or_project", "name"),
    ("plan_research_with_evidence", "question"),
];
pub fn catalogs() -> Result<Catalogs> {
    let resources = serde_json::json!({"resources":[
 {"uri":"slopsearx://capabilities","name":"_capabilities_resource","title":"Engine catalog (remote)","description":"Current generated engine and category catalog, redacted","mimeType":"text/plain"},
 {"uri":"slopsearx://routing-profiles","name":"_routing_profiles_resource","title":"Routing profiles (remote)","description":"Intent-to-category/profile definitions and their provenance","mimeType":"text/plain"},
 {"uri":"slopsearx://health/summary","name":"_health_summary_resource","title":"Health summary (remote)","description":"Current server health with the active-health limitation stated","mimeType":"text/plain"}]});
    let templates = serde_json::json!({"resourceTemplates":[{"uriTemplate":"slopsearx://capabilities/{engine}","name":"_engine_capability_resource","title":"Engine capability (remote)","description":"One engine's metadata, caveats, and auth class","mimeType":"text/plain"}]});
    let prompts:Vec<_>=PROMPTS.iter().map(|(name,arg)|serde_json::json!({"name":name,"arguments":[{"name":arg,"required":true}]})).collect();
    let mut resources = resources;
    let mut templates = templates;
    let mut prompts = prompts;
    for row in resources["resources"].as_array_mut().unwrap() {
        row["_meta"] = serde_json::json!({"fastmcp":{"tags":[]}});
    }
    for row in templates["resourceTemplates"].as_array_mut().unwrap() {
        row["_meta"] = serde_json::json!({"fastmcp":{"tags":[]}});
    }
    for row in &mut prompts {
        row["_meta"] = serde_json::json!({"fastmcp":{"tags":[]}});
    }
    [
        ("resources/list", resources),
        ("resources/templates/list", templates),
        ("prompts/list", serde_json::json!({"prompts":prompts})),
    ]
    .into_iter()
    .map(|(k, v)| Ok((k.into(), RawValue::from_string(v.to_string())?)))
    .collect::<Result<_>>()
}
pub fn resource_allowed(uri: &str) -> bool {
    matches!(
        uri,
        "slopsearx://capabilities" | "slopsearx://routing-profiles" | "slopsearx://health/summary"
    ) || uri
        .strip_prefix("slopsearx://capabilities/")
        .is_some_and(|engine| !engine.is_empty() && !engine.contains(['/', '?', '#']))
}
pub fn prompt_allowed(name: &str) -> bool {
    prompt_argument(name).is_some()
}
pub fn prompt_argument(name: &str) -> Option<&'static str> {
    PROMPTS
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, arg)| *arg)
}
fn fields(raw: &str) -> Result<Fields> {
    Ok(serde_json::from_str(raw)?)
}
fn rows(fields: &Fields, key: &str) -> Result<Vec<Fields>> {
    Ok(fields
        .get(key)
        .map(|v| serde_json::from_str(v.get()))
        .transpose()?
        .unwrap_or_default())
}
fn text(row: &Fields) -> Result<Option<String>> {
    row.get("text")
        .filter(|v| v.get() != "null")
        .map(|v| Ok(serde_json::from_str(v.get())?))
        .transpose()
}
/// Normalize Python json.loads' legacy text path without imposing Value's 100-level
/// depth or u64 number domain. JSON object insertion order and last duplicate value
/// follow dict; nonfinite Python floats serialize as null at FastMCP's JSON boundary.
fn legacy_json(text: &str) -> Result<Option<Box<RawValue>>> {
    let mut normalized = Vec::with_capacity(text.len());
    let bytes = text.as_bytes();
    let mut i = 0;
    let mut quoted = false;
    let mut escaped = false;
    while i < bytes.len() {
        let ch = bytes[i];
        if quoted {
            normalized.push(ch);
            if escaped {
                escaped = false;
            } else if ch == b'\\' {
                escaped = true;
            } else if ch == b'"' {
                quoted = false;
            }
            i += 1;
        } else if ch == b'"' {
            quoted = true;
            normalized.push(b'"');
            i += 1;
        } else if let Some(token) = ["-Infinity", "Infinity", "NaN"]
            .into_iter()
            .find(|token| bytes[i..].starts_with(token.as_bytes()))
        {
            normalized.extend_from_slice(b"null");
            i += token.len();
        } else {
            normalized.push(ch);
            i += 1;
        }
    }
    // Original UTF-8 bytes, including quoted strings, remain untouched.
    let normalized = String::from_utf8(normalized)?;
    let Ok(raw) = RawValue::from_string(normalized) else {
        return Ok(None);
    };
    fn canonical(raw: &str) -> Result<String> {
        let raw = raw.trim();
        if raw.starts_with('{') {
            let fields: indexmap::IndexMap<String, Box<RawValue>> = serde_json::from_str(raw)?;
            let mut output = Vec::new();
            for (key, value) in fields {
                output.push(format!(
                    "{}:{}",
                    serde_json::to_string(&key)?,
                    canonical(value.get())?
                ));
            }
            Ok(format!("{{{}}}", output.join(",")))
        } else if raw.starts_with('[') {
            let values: Vec<Box<RawValue>> = serde_json::from_str(raw)?;
            Ok(format!(
                "[{}]",
                values
                    .into_iter()
                    .map(|v| canonical(v.get()))
                    .collect::<Result<Vec<_>>>()?
                    .join(",")
            ))
        } else if raw.starts_with('"') {
            Ok(serde_json::to_string(&serde_json::from_str::<String>(
                raw,
            )?)?)
        } else if raw
            .as_bytes()
            .first()
            .is_some_and(|b| b.is_ascii_digit() || *b == b'-')
        {
            if raw.contains(['.', 'e', 'E']) {
                let value: f64 = raw.parse()?;
                if value.is_finite() {
                    Ok(ssx_json::python_float(value)?)
                } else {
                    Ok("null".into())
                }
            } else {
                if raw.trim_start_matches('-').len() > 4300 {
                    return Err("legacy JSON integer exceeds source 4300-digit limit".into());
                }
                Ok(raw.into())
            }
        } else {
            Ok(raw.into())
        }
    }
    Ok(Some(RawValue::from_string(canonical(raw.get())?)?))
}
fn legacy_nonfinite(raw: &str) -> Result<bool> {
    let data = fields(raw)?;
    if data.get("isError").is_some_and(|v| v.get() == "true")
        || data
            .get("structuredContent")
            .is_some_and(|v| v.get() != "null")
    {
        return Ok(false);
    }
    let text = rows(&data, "content")?
        .iter()
        .filter_map(|row| text(row).transpose())
        .collect::<Result<Vec<_>>>()?
        .into_iter()
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("\n");
    Ok(matches!(
        text.trim_matches(ssx_json::python_whitespace),
        "NaN" | "Infinity" | "-Infinity"
    ) || text.trim().parse::<f64>().is_ok_and(|v| !v.is_finite()))
}
/// Logical value returned by _extract_tool_result, including empty structured objects.
pub fn extract_tool(raw: &str) -> Result<Box<RawValue>> {
    let data = fields(raw)?;
    let mut texts = Vec::new();
    for row in rows(&data, "content")? {
        if let Some(value) = text(&row)?
            && !value.is_empty()
        {
            texts.push(value)
        }
    }
    let joined = texts.join("\n");
    let joined = joined.trim_matches(ssx_json::python_whitespace);
    if data.get("isError").is_some_and(|v| v.get() == "true") {
        return Ok(RawValue::from_string(serde_json::json!({"error":{"code":"remote_error","message":if joined.is_empty(){"remote tool failed"}else{joined}}}).to_string())?);
    }
    if let Some(value) = data.get("structuredContent").filter(|v| v.get() != "null") {
        return Ok(value.clone());
    }
    if joined.is_empty() {
        return Ok(RawValue::from_string("{}".into())?);
    }
    // Validate losslessly before selecting the legacy JSON fallback.
    if let Some(value) = legacy_json(joined)? {
        return Ok(value);
    }
    Ok(RawValue::from_string(
        serde_json::json!({"result":joined}).to_string(),
    )?)
}
/// Resource/prompt projection preserves the original newline-joining behavior.
pub fn project_reply(request: &str, response: &str) -> Result<String> {
    let req = fields(request)?;
    let method: String = serde_json::from_str(
        req.get("method")
            .ok_or("gateway request method absent")?
            .get(),
    )?;
    let mut envelope = fields(response)?;
    if method == "tools/call"
        && let Some(error) = envelope.remove("error")
    {
        let error = fields(error.get())?;
        if error.get("code").is_some_and(|v| v.get() == "-32021") {
            envelope.insert(
                "error".into(),
                RawValue::from_string(serde_json::to_string(&error)?)?,
            );
            return Ok(serde_json::to_string(&envelope)?);
        }

        let message: String = error
            .get("message")
            .map(|v| serde_json::from_str(v.get()))
            .transpose()?
            .unwrap_or_else(|| "remote tool failed".into());
        envelope.insert(
            "result".into(),
            RawValue::from_string(
                serde_json::json!({"content":[{"type":"text","text":message}],"isError":true})
                    .to_string(),
            )?,
        );
        return Ok(serde_json::to_string(&envelope)?);
    }
    let Some(result) = envelope.get("result") else {
        return Ok(response.into());
    };
    let output = match method.as_str() {
        "tools/call" => {
            let value = match extract_tool(result.get()) {
                Ok(value) => value,
                Err(_) => {
                    envelope.insert("result".into(), RawValue::from_string(serde_json::json!({"content":[{"type":"text","text":"Error executing tool: remote result conversion failed"}],"isError":true}).to_string())?);
                    return Ok(serde_json::to_string(&envelope)?);
                }
            };
            let object = value.get().trim_start().starts_with('{');
            let content = if value.get() == "null" && !legacy_nonfinite(result.get())? {
                "[]".into()
            } else {
                let text = if value.get().starts_with('"') {
                    serde_json::from_str::<String>(value.get())?
                } else {
                    crate::opaque::compact(value.get())?
                };
                format!(
                    "[{{\"type\":\"text\",\"text\":{}}}]",
                    serde_json::to_string(&text)?
                )
            };
            let mut out = format!("{{\"content\":{content},\"isError\":false");
            if object {
                out.push_str(",\"structuredContent\":");
                out.push_str(value.get());
            }
            out.push('}');
            out
        }
        "resources/read" => {
            let result = fields(result.get())?;
            let mut parts = Vec::new();
            for row in rows(&result, "contents")? {
                if let Some(value) = text(&row)? {
                    parts.push(value)
                }
            }
            let params = fields(req.get("params").ok_or("resource params absent")?.get())?;
            let uri = params.get("uri").ok_or("resource URI absent")?;
            format!(
                "{{\"contents\":[{{\"uri\":{},\"mimeType\":\"text/plain\",\"text\":{}}}]}}",
                uri.get(),
                serde_json::to_string(&parts.join("\n"))?
            )
        }
        "prompts/get" => {
            let result = fields(result.get())?;
            let mut parts = Vec::new();
            for row in rows(&result, "messages")? {
                if let Some(content) = row.get("content")
                    && let Some(value) = text(&fields(content.get())?)?
                    && !value.is_empty()
                {
                    parts.push(value)
                }
            }
            serde_json::json!({"messages":[{"role":"user","content":{"type":"text","text":parts.join("\n")}}]}).to_string()
        }
        _ => return Ok(response.into()),
    };
    envelope.insert("result".into(), RawValue::from_string(output)?);
    Ok(serde_json::to_string(&envelope)?)
}

/// Preserve schema independently of the callable signature, as _register_proxy.
pub fn tool_catalog(raw: &str) -> Result<Box<RawValue>> {
    let data = fields(raw)?;
    let mut tools = Vec::new();
    for tool in rows(&data, "tools")? {
        let name: String = serde_json::from_str(tool.get("name").ok_or("tool name absent")?.get())?;
        let description: String = tool
            .get("description")
            .filter(|v| v.get() != "null")
            .map(|v| serde_json::from_str(v.get()))
            .transpose()?
            .unwrap_or_default();
        let schema = tool
            .get("inputSchema")
            .filter(|v| v.get() != "null")
            .map(|v| v.get())
            .unwrap_or("{}");
        super::arguments::validate_schema(schema)?;
        let description = clean_doc(&description);
        tools.push(format!("{{\"name\":{},\"title\":{},\"description\":{},\"inputSchema\":{},\"_meta\":{{\"fastmcp\":{{\"tags\":[]}}}}}}",serde_json::to_string(&name)?,serde_json::to_string(&super::title::default_title(&name))?,serde_json::to_string(&description)?,schema));
    }
    Ok(RawValue::from_string(format!(
        "{{\"tools\":[{}]}}",
        tools.join(",")
    ))?)
}
pub fn prepare_request(catalogs: &Catalogs, raw: &str) -> Result<String> {
    let mut request = fields(raw)?;
    if request
        .get("method")
        .is_some_and(|v| v.get() == r#""prompts/get""#)
    {
        let mut params = fields(request.get("params").ok_or("prompt params absent")?.get())?;
        let name: String =
            serde_json::from_str(params.get("name").ok_or("prompt name absent")?.get())?;
        let arg = prompt_argument(&name).ok_or("unknown prompt")?;
        let arguments = fields(
            params
                .get("arguments")
                .ok_or("prompt arguments absent")?
                .get(),
        )?;
        let value = arguments.get(arg).ok_or("missing prompt argument")?;
        params.insert(
            "arguments".into(),
            RawValue::from_string(format!(
                "{{{}:{}}}",
                serde_json::to_string(arg)?,
                value.get()
            ))?,
        );
        request.insert(
            "params".into(),
            RawValue::from_string(serde_json::to_string(&params)?)?,
        );
        return Ok(serde_json::to_string(&request)?);
    }
    if request
        .get("method")
        .is_none_or(|v| v.get() != r#""tools/call""#)
    {
        return Ok(raw.into());
    }
    let mut params = fields(request.get("params").ok_or("tool params absent")?.get())?;
    let name: String = serde_json::from_str(params.get("name").ok_or("tool name absent")?.get())?;
    let catalog = fields(
        catalogs
            .get("tools/list")
            .ok_or("tool catalog absent")?
            .get(),
    )?;
    let tool = rows(&catalog, "tools")?
        .into_iter()
        .find(|tool| {
            tool.get("name")
                .and_then(|v| serde_json::from_str::<String>(v.get()).ok())
                .is_some_and(|n| n == name)
        })
        .ok_or("unknown gateway tool")?;
    let schema = tool.get("inputSchema").ok_or("tool schema absent")?;
    let arguments = params
        .get("arguments")
        .filter(|v| v.get() != "null")
        .map(|v| v.get())
        .unwrap_or("{}");
    let normalized = super::arguments::normalize_raw(schema.get(), arguments)?;
    params.insert("arguments".into(), RawValue::from_string(normalized)?);
    request.insert(
        "params".into(),
        RawValue::from_string(serde_json::to_string(&params)?)?,
    );
    Ok(serde_json::to_string(&request)?)
}
pub fn initialize_info(protocol: &str) -> Result<Box<RawValue>> {
    let value = serde_json::json!({"protocolVersion":protocol,"capabilities":{"tools":{"listChanged":true},"resources":{"subscribe":false,"listChanged":true},"prompts":{"listChanged":true},"logging":{}},"serverInfo":{"name":"slopsearx-remote","version":env!("CARGO_PKG_VERSION")},"instructions":GATEWAY_INSTRUCTIONS});
    Ok(RawValue::from_string(value.to_string())?)
}
const GATEWAY_INSTRUCTIONS: &str = "SlopSearX remote gateway.\n\nThis process is a proxy: it forwards every tool call over a streamable HTTP\nconnection to a remote SlopSearX server. Behavior, results, warnings, and\nlimitations are identical to connecting to that server directly — see the\nremote server's instructions. All server-side configuration (Valkey, engine\nkeys, grants) lives on the remote host, not here.\n";

fn clean_doc(doc: &str) -> String {
    let mut expanded = String::new();
    let mut column = 0;
    for c in doc.chars() {
        match c {
            '\t' => {
                let spaces = 8 - column % 8;
                expanded.extend(std::iter::repeat_n(' ', spaces));
                column += spaces;
            }
            '\n' | '\r' => {
                expanded.push(c);
                column = 0;
            }
            _ => {
                expanded.push(c);
                column += 1;
            }
        }
    }
    let mut lines: Vec<String> = expanded.split('\n').map(str::to_owned).collect();
    let margin = lines
        .iter()
        .skip(1)
        .filter(|line| {
            !line
                .trim_start_matches(ssx_json::python_whitespace)
                .is_empty()
        })
        .map(|line| {
            line.chars().count()
                - line
                    .trim_start_matches(ssx_json::python_whitespace)
                    .chars()
                    .count()
        })
        .min();
    if let Some(first) = lines.first_mut() {
        *first = first.trim_start_matches(ssx_json::python_whitespace).into();
    }
    if let Some(margin) = margin {
        for line in lines.iter_mut().skip(1) {
            *line = line.chars().skip(margin).collect();
        }
    }
    while lines.last().is_some_and(String::is_empty) {
        lines.pop();
    }
    while lines.first().is_some_and(String::is_empty) {
        lines.remove(0);
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fixed_projection_does_not_expose_remote_extras() {
        let c = catalogs().unwrap();
        assert_eq!(c.len(), 3);
        assert!(resource_allowed("slopsearx://capabilities/wikipedia"));
        assert!(!resource_allowed("private://credentials"));
        assert!(!prompt_allowed("private_admin"));
    }
    #[test]
    fn source_extraction_error_precedes_structured_and_empty_is_retained() {
        assert_eq!(extract_tool(r#"{"content":[{"type":"text","text":" no "}],"isError":true,"structuredContent":{"secret":1}}"#).unwrap().get(),r#"{"error":{"code":"remote_error","message":"no"}}"#);
        assert_eq!(
            extract_tool(
                r#"{"content":[{"type":"text","text":"fallback"}],"structuredContent":{}}"#
            )
            .unwrap()
            .get(),
            "{}"
        );
    }
    #[test]
    fn legacy_json_preserves_large_numbers_and_plain_fallback() {
        let raw = r#"{"content":[{"type":"text","text":"18446744073709551617"}]}"#;
        assert_eq!(extract_tool(raw).unwrap().get(), "18446744073709551617");
        assert_eq!(
            extract_tool(r#"{"content":[{"type":"text","text":" note "}]}"#)
                .unwrap()
                .get(),
            r#"{"result":"note"}"#
        );
    }
}

#[cfg(test)]
mod packaging_tests {
    use super::*;
    #[test]
    fn source_nonobject_packaging_distinguishes_null_string_array_and_numbers() {
        for (value, expected) in [
            ("null", None),
            ("\"hello\"", Some("hello")),
            ("[1,2]", Some("[1,2]")),
            ("3", Some("3")),
        ] {
            let request = r#"{"method":"tools/call"}"#;
            let response = format!(
                "{{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{{\"structuredContent\":{value},\"content\":[{{\"type\":\"text\",\"text\":{}}}]}}}}",
                serde_json::to_string(value).unwrap()
            );
            let result: serde_json::Value =
                serde_json::from_str(&project_reply(request, &response).unwrap()).unwrap();
            assert!(result["result"].get("structuredContent").is_none());
            match expected {
                None => assert_eq!(result["result"]["content"], serde_json::json!([])),
                Some(value) => assert_eq!(result["result"]["content"][0]["text"], value),
            }
        }
    }
}

#[cfg(test)]
mod metadata_tests {
    use super::*;
    #[test]
    fn original_doc_cleaning_and_generated_signature_failures_are_not_silently_reexposed() {
        assert_eq!(
            clean_doc("  first\n\tsecond\n\tthird\n"),
            "first\nsecond\nthird"
        );
        assert!(
            super::super::arguments::validate_schema(
                r#"{"properties":{"location-name":{"type":"string"}}}"#
            )
            .is_err()
        );
        let raw = r#"{"tools":[{"name":"new_tool","description":"   A\n    B","inputSchema":{"properties":{},"x-future":{"raw":18446744073709551617}}}]}"#;
        let projected = tool_catalog(raw).unwrap();
        assert!(projected.get().contains("18446744073709551617"));
        assert!(projected.get().contains("A\\nB"));
    }
}

#[cfg(test)]
mod framework_error_tests {
    use super::*;
    #[test]
    fn remote_client_capability_error_stays_protocol_error_and_other_tool_errors_are_results() {
        let request = r#"{"method":"tools/call"}"#;
        let remote = r#"{"jsonrpc":"2.0","id":1,"error":{"code":-32021,"message":"missing client capability"}}"#;
        let value: serde_json::Value =
            serde_json::from_str(&project_reply(request, remote).unwrap()).unwrap();
        assert_eq!(value["error"]["code"], -32021);
        let remote = remote.replace("-32021", "-32602");
        let value: serde_json::Value =
            serde_json::from_str(&project_reply(request, &remote).unwrap()).unwrap();
        assert_eq!(value["result"]["isError"], true);
    }
}

#[cfg(test)]
mod legacy_controls {
    use super::*;
    #[test]
    fn legacy_json_retains_deep_values_and_duplicate_last_value() {
        let nested = format!(
            "{}184467440737095516170{}",
            "[".repeat(200),
            "]".repeat(200)
        );
        assert_eq!(legacy_json(&nested).unwrap().unwrap().get(), nested);
        assert_eq!(
            legacy_json(r#"{"x":1,"y":2,"x":3}"#)
                .unwrap()
                .unwrap()
                .get(),
            r#"{"x":3,"y":2}"#
        );
        assert_eq!(
            legacy_json(r#"[NaN,Infinity,-Infinity,1e1,"café"]"#)
                .unwrap()
                .unwrap()
                .get(),
            r#"[null,null,null,10.0,"café"]"#
        );
        assert!(legacy_json(&"9".repeat(4301)).is_err());
    }
    #[test]
    fn nonfinite_legacy_scalar_is_text_null_not_empty_content() {
        let response =
            r#"{"jsonrpc":"2.0","id":1,"result":{"content":[{"type":"text","text":"NaN"}]}}"#;
        let output = project_reply(r#"{"method":"tools/call"}"#, response).unwrap();
        assert!(output.contains(r#""text":"null""#));
    }
}
