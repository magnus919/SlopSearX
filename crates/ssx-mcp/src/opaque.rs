//! Opaque descriptor boundary: rmcp's Value model cannot represent every JSON number.
//! RawValue retains schema/metadata tokens without a global arbitrary_precision feature.
use crate::{Registry, Result};
use serde::Deserialize;
use serde_json::{Value, json, value::RawValue};
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    time::Duration,
};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
pub type Catalogs = BTreeMap<String, Box<RawValue>>;
/// Original admitted HTTP request, installed only inside the transport middleware.
#[derive(Clone)]
pub struct OriginalRequest(std::sync::Arc<str>);
impl OriginalRequest {
    pub(crate) fn new(json: String) -> Self {
        Self(json.into())
    }
    pub fn json(&self) -> &str {
        &self.0
    }
}
/// JSON-lines framing removes only insignificant ASCII whitespace outside strings.
/// Validation is performed by RawValue first; number and string tokens stay untouched.
pub fn compact(raw: &str) -> Result<String> {
    let _: Box<RawValue> = serde_json::from_str(raw)?;
    let mut output = String::with_capacity(raw.len());
    let mut quoted = false;
    let mut escaped = false;
    for ch in raw.chars() {
        if quoted {
            output.push(ch);
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                quoted = false;
            }
        } else if ch == '"' {
            quoted = true;
            output.push(ch);
        } else if !matches!(ch, ' ' | '\t' | '\r' | '\n') {
            output.push(ch);
        }
    }
    Ok(output)
}
#[derive(Deserialize)]
struct Envelope {
    #[serde(default)]
    id: Option<Box<RawValue>>,
    #[serde(default)]
    method: Option<String>,
}
/// Source MCP 2.3 lifecycle: initialize response grants requests; the notification
/// is advisory. Keep rmcp's dispatcher initialized without changing wire ordering.
#[derive(Default)]
pub struct Lifecycle {
    accepted: bool,
    initializing: HashSet<String>,
}
impl Lifecycle {
    /// None means forward; Some(None) means consume notification; Some(Some(reply))
    /// means return the source-compatible error without advancing the SDK.
    pub fn request(&mut self, line: &str) -> Result<Option<Option<String>>> {
        let envelope: Envelope = serde_json::from_str(line)?;
        let Some(method) = envelope.method.as_deref() else {
            return Ok(None);
        };
        if method == "notifications/initialized" {
            return Ok(Some(None));
        }
        let Some(id) = envelope.id else {
            return Ok(None);
        };
        let fields: BTreeMap<String, Box<RawValue>> = serde_json::from_str(line)?;
        let initialize_valid = method == "initialize"
            && fields.get("params").is_some_and(|p| {
                serde_json::from_str::<rmcp::model::InitializeRequestParams>(p.get()).is_ok()
            });
        if initialize_valid {
            self.initializing.insert(key(&id)?);
            return Ok(None);
        }
        if method == "ping" || (self.accepted && method != "initialize") {
            return Ok(None);
        }
        let known = matches!(
            method,
            "initialize"
                | "tools/list"
                | "tools/call"
                | "resources/list"
                | "resources/templates/list"
                | "resources/read"
                | "prompts/list"
                | "prompts/get"
                | "logging/setLevel"
        );
        let error = if known {
            json!({"code":-32602,"message":"Invalid request parameters","data":""})
        } else {
            json!({"code":-32601,"message":"Method not found","data":method})
        };
        Ok(Some(Some(format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":{},\"error\":{}}}",
            id.get(),
            error
        ))))
    }
    pub fn remap(&mut self, original: &str, rewritten: &str) -> Result<()> {
        if original != rewritten {
            let original: Envelope = serde_json::from_str(original)?;
            let rewritten: Envelope = serde_json::from_str(rewritten)?;
            if let (Some(old), Some(new)) = (original.id, rewritten.id)
                && self.initializing.remove(&key(&old)?)
            {
                self.initializing.insert(key(&new)?);
            }
        }
        Ok(())
    }
    pub fn response(&mut self, line: &str) -> Result<bool> {
        let fields: BTreeMap<String, Box<RawValue>> = serde_json::from_str(line)?;
        if let Some(id) = fields.get("id")
            && self.initializing.remove(&key(id)?)
            && fields.contains_key("result")
            && !self.accepted
        {
            self.accepted = true;
            return Ok(true);
        }
        Ok(false)
    }
}
pub const INITIALIZED: &[u8] = b"{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\"}\n";
/// Match only a successful SDK response to a registered list request. Protocol errors and
/// lifecycle validation remain the SDK's authority; only the opaque result payload changes.
#[derive(Default)]
pub struct Overlay {
    pending: HashMap<String, String>,
    native: HashSet<String>,
}
pub fn key(id: &RawValue) -> Result<String> {
    let raw = id.get();
    if raw.starts_with('"') {
        return Ok(format!("string:{}", serde_json::from_str::<String>(raw)?));
    }
    let digits = raw.strip_prefix('-').unwrap_or(raw);
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return Err("MCP request id must be a string or integer".into());
    }
    Ok(format!("integer:{}", if raw == "-0" { "0" } else { raw }))
}
/// Independent trusted input catalogs select only native tool/prompt names.
/// Output-only handlers (including W05) retain byte-for-byte input behavior.
fn native_input_catalog<'a>(
    catalogs: &'a Catalogs,
    method: Option<&str>,
) -> Option<(&'a RawValue, bool)> {
    match method {
        Some("tools/call") => catalogs
            .get("ssx/native-raw-arguments")
            .map(|v| (v.as_ref(), false)),
        Some("prompts/get") => catalogs
            .get("ssx/native-raw-prompts")
            .map(|v| (v.as_ref(), true)),
        _ => None,
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NativePromptInputProfile {
    names: Vec<String>,
    validate_unknown_names: bool,
    #[serde(default)]
    validate_invalid_params: bool,
}
fn native_input_name_allowed(profile: &RawValue, prompt: bool, name: &str) -> Result<bool> {
    if prompt {
        let profile: NativePromptInputProfile = serde_json::from_str(profile.get())?;
        Ok(profile.validate_unknown_names || profile.names.iter().any(|n| n == name))
    } else {
        let names: Vec<String> = serde_json::from_str(profile.get())?;
        Ok(names.iter().any(|n| n == name))
    }
}
/// A trusted prompt handler can validate semantic parameters on an otherwise
/// valid JSONRPC request. Scalar/array params are invalid JSONRPC grammar and
/// remain byte-for-byte under the transport SDK's existing rejection path.
fn native_prompt_invalid_params(profile: &RawValue, params: Option<&RawValue>) -> Result<bool> {
    let profile: NativePromptInputProfile = serde_json::from_str(profile.get())?;
    if !profile.validate_invalid_params {
        return Ok(false);
    }
    let Some(params) = params.filter(|p| p.get() != "null") else {
        return Ok(true);
    };
    let Ok(params) = serde_json::from_str::<BTreeMap<String, Box<RawValue>>>(params.get()) else {
        return Ok(false);
    };
    Ok(params
        .get("name")
        .and_then(|n| serde_json::from_str::<String>(n.get()).ok())
        .is_none())
}
/// Replace only arguments on the SDK-facing copy of an admitted allowlisted
/// native request. Original bytes remain in the existing bounded request record.
/// Tools retain object-only replacement. Prompt handlers additionally validate
/// raw null/scalar/array shapes, matching the source's RPC error envelope.
pub fn native_arguments_for_sdk(catalogs: &Catalogs, request: &str) -> Result<String> {
    if !catalogs.contains_key("ssx/native-raw-arguments")
        && !catalogs.contains_key("ssx/native-raw-prompts")
    {
        return Ok(request.into());
    }
    let mut fields: BTreeMap<String, Box<RawValue>> = serde_json::from_str(request)?;
    let method = fields
        .get("method")
        .and_then(|m| serde_json::from_str::<String>(m.get()).ok());
    let Some((names, prompt)) = native_input_catalog(catalogs, method.as_deref()) else {
        return Ok(request.into());
    };
    if !fields.get("id").is_some_and(|id| key(id).is_ok()) {
        return Ok(request.into());
    }
    if prompt && native_prompt_invalid_params(names, fields.get("params").map(Box::as_ref))? {
        let mut params = fields
            .get("params")
            .and_then(|p| serde_json::from_str::<BTreeMap<String, Box<RawValue>>>(p.get()).ok())
            .unwrap_or_default();
        // This is solely an SDK-facing placeholder. The native handler must
        // reject the original missing/non-string name before identity checks.
        params.insert("name".into(), RawValue::from_string("\"\"".into())?);
        params.insert("arguments".into(), RawValue::from_string("{}".into())?);
        fields.insert(
            "params".into(),
            RawValue::from_string(serde_json::to_string(&params)?)?,
        );
        return compact(&serde_json::to_string(&fields)?);
    }
    let Some(raw) = fields.get("params") else {
        return Ok(request.into());
    };
    let Ok(mut params) = serde_json::from_str::<BTreeMap<String, Box<RawValue>>>(raw.get()) else {
        return Ok(request.into());
    };
    let name = params
        .get("name")
        .and_then(|n| serde_json::from_str::<String>(n.get()).ok());
    let Some(name) = name else {
        return Ok(request.into());
    };
    if !native_input_name_allowed(names, prompt, &name)? {
        return Ok(request.into());
    }
    let Some(arguments) = params.get("arguments") else {
        return Ok(request.into());
    };
    if !prompt && !arguments.get().trim_start().starts_with('{') {
        return Ok(request.into());
    }
    params.insert("arguments".into(), RawValue::from_string("{}".into())?);
    fields.insert(
        "params".into(),
        RawValue::from_string(serde_json::to_string(&params)?)?,
    );
    compact(&serde_json::to_string(&fields)?)
}
fn native_input_enabled(catalogs: &Catalogs, request: &str) -> Result<bool> {
    if !catalogs.contains_key("ssx/native-raw-arguments")
        && !catalogs.contains_key("ssx/native-raw-prompts")
    {
        return Ok(false);
    }
    let fields: BTreeMap<String, Box<RawValue>> = serde_json::from_str(request)?;
    let method = fields
        .get("method")
        .and_then(|m| serde_json::from_str::<String>(m.get()).ok());
    let Some((names, prompt)) = native_input_catalog(catalogs, method.as_deref()) else {
        return Ok(false);
    };
    if prompt && native_prompt_invalid_params(names, fields.get("params").map(Box::as_ref))? {
        return Ok(true);
    }
    let Some(params) = fields
        .get("params")
        .and_then(|p| serde_json::from_str::<BTreeMap<String, Box<RawValue>>>(p.get()).ok())
    else {
        return Ok(false);
    };
    let Some(name) = params
        .get("name")
        .and_then(|n| serde_json::from_str::<String>(n.get()).ok())
    else {
        return Ok(false);
    };
    native_input_name_allowed(names, prompt, &name)
}
/// Validate SDK envelope shape using disposable aliases, including cancellation IDs.
/// Real session alias ownership stays with WireState in the transport pump.
pub fn sdk_validation(line: &str) -> Result<String> {
    let mut wire = WireState::default();
    let fields: BTreeMap<String, Box<RawValue>> = serde_json::from_str(line)?;
    if fields
        .get("method")
        .is_some_and(|m| m.get() == "\"notifications/cancelled\"")
        && let Some(params) = fields.get("params")
    {
        let params: BTreeMap<String, Box<RawValue>> = serde_json::from_str(params.get())?;
        if let Some(id) = params.get("requestId") {
            wire.request(&format!(
                "{{\"jsonrpc\":\"2.0\",\"id\":{},\"method\":\"tools/list\"}}",
                id.get()
            ))?;
        }
    }
    wire.request(line)
}
pub struct Frames<R> {
    reader: BufReader<R>,
    partial: Vec<u8>,
    limit: crate::output_limits::OutputLimits,
}
impl<R: tokio::io::AsyncRead + Unpin> Frames<R> {
    pub fn new(reader: R) -> Self {
        Self {
            reader: BufReader::new(reader),
            partial: Vec::new(),
            limit: crate::output_limits::OutputLimits::HTTP,
        }
    }
    pub fn with_limit(reader: R, limit: crate::output_limits::OutputLimits) -> Self {
        Self {
            reader: BufReader::new(reader),
            partial: Vec::new(),
            limit,
        }
    }
    pub async fn next(&mut self) -> Result<Option<String>> {
        loop {
            let bytes = self.reader.fill_buf().await?;
            if bytes.is_empty() {
                return if self.partial.is_empty() {
                    Ok(None)
                } else {
                    Ok(Some(String::from_utf8(std::mem::take(&mut self.partial))?))
                };
            }
            let end = bytes
                .iter()
                .position(|b| *b == b'\n')
                .map(|p| p + 1)
                .unwrap_or(bytes.len());
            if self.partial.len() + end > self.limit.bytes() {
                return Err(format!("MCP frame exceeds {} bytes", self.limit.bytes()).into());
            }
            let complete = bytes[end - 1] == b'\n';
            self.partial.extend_from_slice(&bytes[..end]);
            self.reader.consume(end);
            if complete {
                self.partial.pop();
                if self.partial.last() == Some(&b'\r') {
                    self.partial.pop();
                }
                return Ok(Some(String::from_utf8(std::mem::take(&mut self.partial))?));
            }
        }
    }
}
struct PendingRequest {
    original: String,
    native: bool,
    handler_complete: bool,
    cancel_requested: bool,
}
#[derive(Default)]
pub struct WireState {
    requests: HashMap<String, PendingRequest>,
    completed: std::sync::Arc<tokio::sync::Notify>,
    native_namespace: Option<String>,
    native_sequence: u64,
    replies: HashMap<String, String>,
    synthetic: HashMap<String, String>,
    aliases: HashMap<String, Box<RawValue>>,
    reverse: HashMap<String, Box<RawValue>>,
    active: HashSet<String>,
}
/// The synthetic echo has an unbounded integer schema. Validate raw integer
/// tokens before Value decoding, give the SDK exact safe placeholders, and keep
/// the real echo payload for replacement only after handler success. This is a
/// qualification fixture boundary, not a generic domain JSON interpreter.
fn synthetic_integer_call(line: &str) -> Result<Option<(String, String)>> {
    let mut fields: BTreeMap<String, Box<RawValue>> = serde_json::from_str(line)?;
    if fields
        .get("method")
        .is_none_or(|v| v.get() != "\"tools/call\"")
    {
        return Ok(None);
    }
    let Some(id) = fields.get("id").cloned() else {
        return Ok(None);
    };
    let Some(params) = fields.get("params") else {
        return Ok(None);
    };
    let mut params: BTreeMap<String, Box<RawValue>> = match serde_json::from_str(params.get()) {
        Ok(value) => value,
        Err(_) => return Ok(None),
    };
    if !params
        .get("name")
        .is_some_and(|v| ["\"t01_echo\"", "\"t01_large\""].contains(&v.get()))
    {
        return Ok(None);
    }
    let Some(args) = params.get("arguments") else {
        return Ok(None);
    };
    let mut args: BTreeMap<String, Box<RawValue>> = match serde_json::from_str(args.get()) {
        Ok(value) => value,
        Err(_) => return Ok(None),
    };
    let Some(payload) = args.get("payload").cloned() else {
        return Ok(None);
    };
    let mut safe_payload: BTreeMap<String, Box<RawValue>> =
        match serde_json::from_str(payload.get()) {
            Ok(v) => v,
            Err(_) => return Ok(None),
        };
    let Some(items) = safe_payload.get("items") else {
        return Ok(None);
    };
    let mut items: Vec<Box<RawValue>> = match serde_json::from_str(items.get()) {
        Ok(v) => v,
        Err(_) => return Ok(None),
    };
    let mut changed = false;
    for item in &mut items {
        let raw = item.get();
        let digits = raw.strip_prefix('-').unwrap_or(raw);
        if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
            return Ok(None);
        }
        if raw.parse::<i64>().is_err() && raw.parse::<u64>().is_err() {
            *item = RawValue::from_string("0".into())?;
            changed = true;
        }
    }
    if !changed {
        return Ok(None);
    }
    let note = args.get("note").map(|v| v.get()).unwrap_or("null");
    let echoed = compact(&format!(
        "{{\"payload\":{},\"note\":{note}}}",
        payload.get()
    ))?;
    let reply = compact(&format!(
        "{{\"jsonrpc\":\"2.0\",\"id\":{},\"result\":{{\"content\":[{{\"type\":\"text\",\"text\":{}}}],\"structuredContent\":{echoed},\"isError\":false}}}}",
        id.get(),
        serde_json::to_string(&echoed)?
    ))?;
    safe_payload.insert(
        "items".into(),
        RawValue::from_string(serde_json::to_string(&items)?)?,
    );
    args.insert(
        "payload".into(),
        RawValue::from_string(serde_json::to_string(&safe_payload)?)?,
    );
    params.insert(
        "arguments".into(),
        RawValue::from_string(serde_json::to_string(&args)?)?,
    );
    fields.insert(
        "params".into(),
        RawValue::from_string(serde_json::to_string(&params)?)?,
    );
    Ok(Some((compact(&serde_json::to_string(&fields)?)?, reply)))
}
impl WireState {
    /// Read the actual admitted bytes by the SDK's private request identity.
    /// Ownership stays in the existing 32-request map; reply/cancel/EOF cleanup
    /// is unchanged. Clone before parsing so no mutex is held during validation.
    pub fn original_request(&self, id: &rmcp::model::RequestId) -> Result<Option<String>> {
        let raw = RawValue::from_string(serde_json::to_string(id)?)?;
        Ok(self.requests.get(&key(&raw)?).map(|r| r.original.clone()))
    }
    pub fn completion_notification(&self) -> std::sync::Arc<tokio::sync::Notify> {
        self.completed.clone()
    }
    /// Trusted native handler calls this only after all owned request work drains.
    pub fn native_handler_complete(&mut self, id: &rmcp::model::RequestId) -> Result<()> {
        let raw = RawValue::from_string(serde_json::to_string(id)?)?;
        if let Some(request) = self.requests.get_mut(&key(&raw)?)
            && request.native
        {
            request.handler_complete = true;
            if request.cancel_requested {
                self.completed.notify_one();
            }
        }
        Ok(())
    }
    /// Drain only canceled native generations whose handler has actually finished.
    /// Admission stays charged until this pump-owned cleanup; no second registry.
    pub fn drain_cancelled_native(&mut self) -> Result<Vec<String>> {
        let ids: Vec<_> = self
            .requests
            .iter()
            .filter(|(_, r)| r.native && r.cancel_requested && r.handler_complete)
            .map(|(key, _)| {
                serde_json::to_string(
                    key.strip_prefix("string:")
                        .expect("native SDK aliases are strings"),
                )
            })
            .collect::<std::result::Result<_, _>>()?;
        for id in &ids {
            self.abandon(&format!("{{\"id\":{id}}}"));
        }
        Ok(ids)
    }
    /// Native generations have a private per-connection namespace. A canceled
    /// generation may already have buffered SDK output; discard that output
    /// without a tombstone registry or affecting ordinary unmatched RPC replies.
    pub fn stale_native_generation(&self, line: &str) -> Result<bool> {
        let fields: BTreeMap<String, Box<RawValue>> = serde_json::from_str(line)?;
        let Some(id) = fields.get("id") else {
            return Ok(false);
        };
        let Some(namespace) = &self.native_namespace else {
            return Ok(false);
        };
        let issued = serde_json::from_str::<String>(id.get())
            .ok()
            .is_some_and(|id| {
                id.strip_prefix(namespace).is_some_and(|suffix| {
                    suffix.parse::<u64>().ok().is_some_and(|n| {
                        n > 0 && n <= self.native_sequence && n.to_string() == suffix
                    })
                })
            });
        if !issued {
            return Ok(false);
        }
        Ok(self
            .requests
            .get(&key(id)?)
            .is_none_or(|r| r.native && r.cancel_requested && r.handler_complete))
    }
    pub fn native_response(&mut self, line: &str) -> Result<Option<String>> {
        if self.stale_native_generation(line)? {
            return Ok(None);
        }
        self.response(line).map(Some)
    }
    pub fn request_for(&mut self, catalogs: &Catalogs, line: &str) -> Result<String> {
        let native = native_input_enabled(catalogs, line)?;
        self.request_inner(line, native)
    }
    pub fn request(&mut self, line: &str) -> Result<String> {
        self.request_inner(line, false)
    }
    fn request_inner(&mut self, line: &str, native: bool) -> Result<String> {
        let envelope: Envelope = serde_json::from_str(line)?;
        if envelope.method.as_deref() == Some("notifications/cancelled") {
            let mut fields: BTreeMap<String, Box<RawValue>> = serde_json::from_str(line)?;
            if let Some(params) = fields.get_mut("params") {
                let mut values: BTreeMap<String, Box<RawValue>> =
                    serde_json::from_str(params.get())?;
                if let Some(id) = values.get("requestId") {
                    let original = key(id)?;
                    let internal = self
                        .reverse
                        .get(&original)
                        .map(|id| key(id))
                        .transpose()?
                        .unwrap_or(original);
                    if let Some(request) = self.requests.get_mut(&internal)
                        && request.native
                    {
                        request.cancel_requested = true;
                        if request.handler_complete {
                            self.completed.notify_one();
                        }
                    }
                }
                if let Some(id) = values.get("requestId")
                    && let Some(alias) = self.reverse.get(&key(id)?)
                {
                    values.insert("requestId".into(), alias.clone());
                    *params = RawValue::from_string(serde_json::to_string(&values)?)?;
                    return compact(&serde_json::to_string(&fields)?);
                }
            }
        }
        if let (Some(id), Some(_)) = (envelope.id, envelope.method) {
            let original = key(&id)?;
            if self.active.contains(&original) || (!native && self.requests.contains_key(&original))
            {
                return Err("duplicate MCP request id".into());
            }
            if self.active.len() >= 32 {
                return Err("MCP peer exceeds 32 concurrent requests".into());
            }
            let mut alias_binding = None;
            let mut next_native_sequence = None;
            let mut rewritten = line.to_owned();
            let mut internal = original.clone();
            if native || (!id.get().starts_with('"') && id.get().parse::<i64>().is_err()) {
                let alias = if native {
                    let namespace = self
                        .native_namespace
                        .get_or_insert_with(|| format!("ssx-native-{}-", uuid::Uuid::new_v4()))
                        .clone();
                    // Every committed sequence is issued. A collision fails this
                    // admission without burning a sequence or creating a history gap.
                    let next = self
                        .native_sequence
                        .checked_add(1)
                        .ok_or("native SDK generation exhausted")?;
                    let candidate = format!("{namespace}{next}");
                    let candidate_key = format!("string:{candidate}");
                    if self.active.contains(&candidate_key)
                        || self.requests.contains_key(&candidate_key)
                    {
                        return Err("native SDK alias collides with active caller id".into());
                    }
                    next_native_sequence = Some(next);
                    candidate
                } else {
                    format!("ssx-{}", uuid::Uuid::new_v4())
                };
                let alias = RawValue::from_string(serde_json::to_string(&alias)?)?;
                internal = key(&alias)?;
                alias_binding = Some((id, alias.clone()));
                let mut fields: BTreeMap<String, Box<RawValue>> = serde_json::from_str(line)?;
                fields.insert("id".into(), alias);
                rewritten = compact(&serde_json::to_string(&fields)?)?;
            }
            let synthetic = synthetic_integer_call(&rewritten)?;
            // Admission maps and sequence are transactional.
            if let Some(next) = next_native_sequence {
                self.native_sequence = next;
            }
            self.active.insert(original.clone());
            if let Some((id, alias)) = alias_binding {
                self.aliases.insert(internal.clone(), id);
                self.reverse.insert(original, alias);
            }
            if let Some((safe, reply)) = synthetic {
                self.synthetic.insert(internal.clone(), reply);
                rewritten = safe;
            }
            self.requests.insert(
                internal,
                PendingRequest {
                    original: line.into(),
                    native,
                    handler_complete: false,
                    cancel_requested: false,
                },
            );
            return Ok(rewritten);
        }
        Ok(line.into())
    }
    pub fn abandon(&mut self, request: &str) {
        if let Ok(envelope) = serde_json::from_str::<Envelope>(request)
            && let Some(id) = envelope.id
        {
            let _ = self.response(&format!("{{\"jsonrpc\":\"2.0\",\"id\":{},\"error\":{{\"code\":-32603,\"message\":\"cancelled\"}}}}",id.get()));
        }
    }
    pub fn response(&mut self, line: &str) -> Result<String> {
        let mut fields: BTreeMap<String, Box<RawValue>> = serde_json::from_str(line)?;
        let Some(id) = fields.get("id") else {
            return Ok(line.into());
        };
        let internal = key(id)?;
        if !self.requests.contains_key(&internal) {
            return Ok(line.into());
        }
        self.requests.remove(&internal);
        let synthetic = self.synthetic.remove(&internal);
        let success = fields.get("result").is_some_and(|result| {
            serde_json::from_str::<BTreeMap<String, Box<RawValue>>>(result.get())
                .ok()
                .is_some_and(|fields| {
                    fields
                        .get("isError")
                        .is_none_or(|value| value.get() != "true")
                })
        }) && !fields.contains_key("error");
        let mut result = self
            .replies
            .remove(&internal)
            .or(if success { synthetic } else { None })
            .unwrap_or_else(|| line.into());
        if let Some(original) = self.aliases.remove(&internal) {
            self.reverse.remove(&key(&original)?);
            self.active.remove(&key(&original)?);
            fields = serde_json::from_str(&result)?;
            fields.insert("id".into(), original);
            result = compact(&serde_json::to_string(&fields)?)?;
        } else {
            self.active.remove(&internal);
        }
        Ok(result)
    }
}
impl Overlay {
    pub fn abandon(&mut self, raw_id: &str) -> Result<()> {
        let id = RawValue::from_string(raw_id.into())?;
        let id = key(&id)?;
        self.pending.remove(&id);
        self.native.remove(&id);
        Ok(())
    }
    pub fn has_pending(&self) -> bool {
        !self.pending.is_empty() || !self.native.is_empty()
    }
    pub fn request(&mut self, catalogs: &Catalogs, request: &str) -> Result<()> {
        let envelope: Envelope = serde_json::from_str(request)?;
        if envelope.method.as_deref() == Some("tools/call")
            && let Some(id) = envelope.id.as_ref()
            && let Some(names) = catalogs.get("ssx/native-tools")
        {
            // Inspect only the method name: arbitrary caller numbers elsewhere
            // must not pass through an SDK floating-point Value before the
            // native handler reads the admitted original request bytes.
            let fields: BTreeMap<String, Box<RawValue>> = serde_json::from_str(request)?;
            let name = fields
                .get("params")
                .and_then(|params| {
                    serde_json::from_str::<BTreeMap<String, Box<RawValue>>>(params.get()).ok()
                })
                .and_then(|params| {
                    params
                        .get("name")
                        .and_then(|name| serde_json::from_str::<String>(name.get()).ok())
                });
            let names: Vec<String> = serde_json::from_str(names.get())?;
            if name.as_ref().is_some_and(|name| names.contains(name)) {
                self.native.insert(key(id)?);
            }
        }
        if let (Some(id), Some(method)) = (envelope.id, envelope.method)
            && let Some(result) = catalogs.get(&method)
        {
            self.pending.insert(
                key(&id)?,
                compact(&format!(
                    "{{\"jsonrpc\":\"2.0\",\"id\":{},\"result\":{}}}",
                    id.get(),
                    result.get()
                ))?,
            );
        }
        Ok(())
    }
    pub fn response(&mut self, response: &str) -> Result<String> {
        let mut fields: BTreeMap<String, Box<RawValue>> = serde_json::from_str(response)?;
        let admitted = fields
            .get("id")
            .map(|id| key(id))
            .transpose()?
            .is_some_and(|id| self.native.remove(&id));
        if let Some(raw) = fields.get("result")
            && raw.get().trim_start().starts_with('{')
        {
            let mut result: BTreeMap<String, Box<RawValue>> = serde_json::from_str(raw.get())?;
            let mut meta: BTreeMap<String, Box<RawValue>> = result
                .get("_meta")
                .and_then(|m| serde_json::from_str(m.get()).ok())
                .unwrap_or_default();
            let marker = meta.remove("ssx/native-structured-text");
            if let Some(marker) = marker {
                // An internal marker is scrubbed on every response, even when
                // this request was not admitted or the handler returned an error.
                let complete = result.get("isError").is_none_or(|v| v.get() != "true");
                if admitted && marker.get() == "true" && complete && !fields.contains_key("error") {
                    if !result.contains_key("structuredContent") {
                        return Err("marked native output has no structured content".into());
                    }
                    let content: Vec<Value> = result
                        .get("content")
                        .map(|c| serde_json::from_str(c.get()))
                        .transpose()?
                        .unwrap_or_default();
                    if content.len() != 1 || content[0]["type"] != "text" {
                        return Err(
                            "native structured output requires one text JSON document".into()
                        );
                    }
                    let text = content[0]["text"]
                        .as_str()
                        .ok_or("missing native JSON text")?;
                    let value: Box<RawValue> = serde_json::from_str(text)?;
                    if !value.get().starts_with('{') {
                        return Err("native structured output must be an object".into());
                    }
                    result.insert("structuredContent".into(), value);
                }
                if meta.is_empty() {
                    result.remove("_meta");
                } else {
                    result.insert(
                        "_meta".into(),
                        RawValue::from_string(serde_json::to_string(&meta)?)?,
                    );
                }
                fields.insert(
                    "result".into(),
                    RawValue::from_string(serde_json::to_string(&result)?)?,
                );
                return compact(&serde_json::to_string(&fields)?);
            }
        }
        if let Some(id) = fields.get("id")
            && let Some(replacement) = self.pending.remove(&key(id)?)
            && fields.contains_key("result")
            && !fields.contains_key("error")
        {
            return Ok(replacement);
        }
        Ok(response.into())
    }
}
/// Streaming SSE frame rewrite, preserving event IDs, retry fields and comments.
pub fn overlay_event(overlay: &mut Overlay, raw: &str) -> Result<String> {
    rewrite_event(raw, |data| overlay.response(data))
}
pub fn wire_event(wire: &mut WireState, raw: &str) -> Result<String> {
    rewrite_event(raw, |data| wire.response(data))
}
fn rewrite_event(raw: &str, mut rewrite: impl FnMut(&str) -> Result<String>) -> Result<String> {
    let mut data = String::new();
    let mut fields = Vec::new();
    for line in raw.lines() {
        if let Some(value) = line.strip_prefix("data:") {
            if !data.is_empty() {
                data.push('\n');
            }
            data.push_str(value.strip_prefix(' ').unwrap_or(value));
        } else if !line.is_empty() {
            fields.push(line.to_owned());
        }
    }
    if data.is_empty() {
        return Ok(raw.into());
    }
    let rewritten = rewrite(&data)?;
    if rewritten == data {
        return Ok(raw.into());
    }
    fields.push(format!("data: {}", compact(&rewritten)?));
    Ok(format!("{}\n\n", fields.join("\n")))
}
/// Stdio framing adapter around the same real rmcp service. Never interprets catalog numbers.
pub async fn stdio(registry: Registry) -> Result<()> {
    let catalogs = registry.opaque.clone();
    stdio_handler(
        registry,
        catalogs,
        Default::default(),
        crate::output_limits::OutputLimits::HTTP,
    )
    .await
}
pub async fn stdio_with_factory<H: rmcp::ServerHandler>(
    factory: impl FnOnce(std::sync::Arc<std::sync::Mutex<WireState>>) -> H,
    catalogs: std::sync::Arc<Catalogs>,
) -> Result<()> {
    let wire = std::sync::Arc::new(std::sync::Mutex::new(WireState::default()));
    stdio_handler(
        factory(wire.clone()),
        catalogs,
        wire,
        crate::output_limits::OutputLimits::HTTP,
    )
    .await
}
pub async fn stdio_for<H: rmcp::ServerHandler>(
    handler: H,
    catalogs: std::sync::Arc<Catalogs>,
) -> Result<()> {
    stdio_handler(
        handler,
        catalogs,
        Default::default(),
        crate::output_limits::OutputLimits::HTTP,
    )
    .await
}
pub async fn stdio_with_factory_and_limits<H: rmcp::ServerHandler>(
    factory: impl FnOnce(std::sync::Arc<std::sync::Mutex<WireState>>) -> H,
    catalogs: std::sync::Arc<Catalogs>,
    limits: crate::output_limits::OutputLimits,
) -> Result<()> {
    let wire = std::sync::Arc::new(std::sync::Mutex::new(WireState::default()));
    stdio_handler(factory(wire.clone()), catalogs, wire, limits).await
}
/// Explicit caller stop; owns cancellation/join of the SDK service waiting task.
/// Service waiting completion alone is not proof of arbitrary handler completion.
pub async fn stdio_with_factory_owned<H: rmcp::ServerHandler>(
    factory: impl FnOnce(std::sync::Arc<std::sync::Mutex<WireState>>) -> H,
    catalogs: std::sync::Arc<Catalogs>,
    cancel: tokio_util::sync::CancellationToken,
) -> Result<()> {
    let wire = std::sync::Arc::new(std::sync::Mutex::new(WireState::default()));
    stdio_handler_owned(
        factory(wire.clone()),
        catalogs,
        wire,
        crate::output_limits::OutputLimits::HTTP,
        cancel,
    )
    .await
}
async fn stdio_handler<H: rmcp::ServerHandler>(
    handler: H,
    catalogs: std::sync::Arc<Catalogs>,
    wire: std::sync::Arc<std::sync::Mutex<WireState>>,
    output_limits: crate::output_limits::OutputLimits,
) -> Result<()> {
    stdio_handler_owned(
        handler,
        catalogs,
        wire,
        output_limits,
        tokio_util::sync::CancellationToken::new(),
    )
    .await
}
async fn stdio_handler_owned<H: rmcp::ServerHandler>(
    handler: H,
    catalogs: std::sync::Arc<Catalogs>,
    wire: std::sync::Arc<std::sync::Mutex<WireState>>,
    output_limits: crate::output_limits::OutputLimits,
    cancel: tokio_util::sync::CancellationToken,
) -> Result<()> {
    stdio_handler_io(
        handler,
        catalogs,
        wire,
        output_limits,
        cancel,
        tokio::io::stdin(),
        tokio::io::stdout(),
    )
    .await
}
/// Internal transport seam: production passes OS stdio, tests pass owned duplex IO.
/// The SDK initialization future stays inline so dropping this owner drops it too.
async fn stdio_handler_io<H, I, O>(
    handler: H,
    catalogs: std::sync::Arc<Catalogs>,
    wire: std::sync::Arc<std::sync::Mutex<WireState>>,
    output_limits: crate::output_limits::OutputLimits,
    cancel: tokio_util::sync::CancellationToken,
    input_io: I,
    mut stdout: O,
) -> Result<()>
where
    H: rmcp::ServerHandler,
    I: tokio::io::AsyncRead + Unpin,
    O: tokio::io::AsyncWrite + Unpin,
{
    use rmcp::ServiceExt;
    let (server, io) = tokio::io::duplex(65536);
    let (read, mut write) = tokio::io::split(io);
    let stop = tokio_util::sync::CancellationToken::new();
    let service_stop = stop.clone();
    let serving = async move {
        let service = tokio::select! {biased;_=service_stop.cancelled()=>return Ok::<(),crate::Error>(()),service=handler.serve(server)=>service?};
        let service_cancel = service.cancellation_token();
        let waiting = service.waiting();
        tokio::pin!(waiting);
        tokio::select! {result=&mut waiting=>{result?;},_=service_stop.cancelled()=>{service_cancel.cancel();waiting.await?;}}
        Ok::<(), crate::Error>(())
    };
    tokio::pin!(serving);
    let pump = async {
        let completed = wire.lock().unwrap().completion_notification();
        let mut input = Frames::new(input_io);
        let mut output = Frames::with_limit(read, output_limits);
        let mut overlay = Overlay::default();
        let mut lifecycle = Lifecycle::default();
        loop {
            tokio::select! {
                _=completed.notified()=>{let ids=wire.lock().unwrap().drain_cancelled_native()?;for id in ids{overlay.abandon(&id)?;}},
                line=input.next()=>{let Some(line)=line? else{break;};if let Some(reply)=lifecycle.request(&line)?{if let Some(reply)=reply{stdout.write_all(reply.as_bytes()).await?;stdout.write_all(b"\n").await?;stdout.flush().await?;}continue;}let rewritten=wire.lock().unwrap().request_for(&catalogs,&line)?;lifecycle.remap(&line,&rewritten)?;let line=rewritten;overlay.request(&catalogs,&line)?;let line=native_arguments_for_sdk(&catalogs,&line)?;write.write_all(line.as_bytes()).await?;write.write_all(b"\n").await?;},
                line=output.next()=>{let Some(line)=line? else{break;};if wire.lock().unwrap().stale_native_generation(&line)?{continue;}
                    if lifecycle.response(&line)?{write.write_all(INITIALIZED).await?;}let line=overlay.response(&line)?;let Some(line)=wire.lock().unwrap().native_response(&line)? else{continue;};let line=compact(&line)?;if line.len()>output_limits.bytes(){return Err("native output exceeds configured byte budget".into());}stdout.write_all(line.as_bytes()).await?;stdout.write_all(b"\n").await?;stdout.flush().await?;}
            }
        }
        Ok::<(), crate::Error>(())
    };
    let (result, joined) = tokio::select! {biased;_=cancel.cancelled()=>(Ok(()),false),r=&mut serving=>(r,true),r=pump=>(r,false)};
    stop.cancel();
    let cleanup = if joined { Ok(()) } else { serving.await };
    result.and(cleanup)
}
fn validate_reply(fields: &BTreeMap<String, Box<RawValue>>, expected: Option<&str>) -> Result<()> {
    if fields.get("jsonrpc").map(|v| v.get()) != Some("\"2.0\"")
        || fields.contains_key("result") == fields.contains_key("error")
    {
        return Err("invalid remote JSON-RPC response".into());
    }
    let actual = fields.get("id").map(|v| key(v)).transpose()?;
    if actual.as_deref() != expected {
        return Err("remote JSON-RPC response id mismatch".into());
    }
    Ok(())
}
/// Incremental upstream lines: every byte is visited once, with no front-drains.
/// The exchange enforces its cumulative wire-byte limit before feeding chunks.
#[derive(Default)]
struct GatewaySse {
    line: Vec<u8>,
    data: Vec<u8>,
    has_data: bool,
    skip_lf: bool,
    events: usize,
    #[cfg(test)]
    inspected: usize,
}
impl GatewaySse {
    fn push(&mut self, chunk: &[u8], expected: Option<&str>) -> Result<Option<String>> {
        for &byte in chunk {
            #[cfg(test)]
            {
                self.inspected += 1;
            }
            if self.skip_lf {
                self.skip_lf = false;
                if byte == b'\n' {
                    continue;
                }
            }
            if byte == b'\r' || byte == b'\n' {
                self.skip_lf = byte == b'\r';
                if self.line.is_empty() {
                    if self.has_data {
                        self.has_data = false;
                        // rmcp stateful priming events carry empty data. SSE
                        // dispatch ignores empty assembled data; preserve the
                        // flag separately so empty multiline fields still join.
                        if self.data.is_empty() {
                            continue;
                        }
                        self.events += 1;
                        if self.events > 128 {
                            return Err("MCP SSE exceeds 128 events".into());
                        }
                        let message = String::from_utf8(std::mem::take(&mut self.data))?;
                        let fields: BTreeMap<String, Box<RawValue>> =
                            serde_json::from_str(&message)?;
                        if fields.contains_key("result") || fields.contains_key("error") {
                            validate_reply(&fields, expected)?;
                            return Ok(Some(message));
                        }
                    }
                } else if self.line == b"data" || self.line.starts_with(b"data:") {
                    let value = self.line.get(5..).unwrap_or_default();
                    let value = value.strip_prefix(b" ").unwrap_or(value);
                    if self.has_data {
                        self.data.push(b'\n');
                    }
                    self.has_data = true;
                    self.data.extend_from_slice(value);
                }
                self.line.clear();
            } else {
                self.line.push(byte);
            }
        }
        Ok(None)
    }
}
pub type GatewayOpenFuture<'a> = std::pin::Pin<
    Box<dyn std::future::Future<Output = Result<Box<dyn GatewayHttpResponse>>> + Send + 'a>,
>;
pub type GatewayChunkFuture<'a> = std::pin::Pin<
    Box<dyn std::future::Future<Output = Result<Option<axum::body::Bytes>>> + Send + 'a>,
>;
/// Protected gateway executor. Implementations own their TLS/proxy/auth policy
/// and must retain connection/admission ownership in the returned response until
/// EOF, correlated early-stop, cancellation or drop. No adapter overrides.
pub trait GatewayHttp: Send + Sync {
    fn open(&self, request: GatewayHttpRequest) -> GatewayOpenFuture<'_>;
}
/// Authentication admission is chosen by the transport owner. Cancellation and
/// session cleanup use only the already-held bearer and cannot acquire OAuth
/// state, refresh tokens or start authorization while a request is being joined.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum GatewayAuthMode {
    Interactive,
    ExistingBearer,
}
/// Deliberate non-Debug descriptor: bearer, body and session can be sensitive.
pub struct GatewayHttpRequest {
    pub auth: GatewayAuthMode,
    pub method: reqwest::Method,
    pub url: String,
    pub headers: Vec<(&'static str, String)>,
    pub body: Option<String>,
}
/// Incremental response, not a buffered HTTP body. A POST SSE stream may remain
/// open after its correlated reply. Implementations retain all body leases.
pub trait GatewayHttpResponse: Send {
    fn status(&self) -> reqwest::StatusCode;
    fn headers(&self) -> &reqwest::header::HeaderMap;
    fn content_length(&self) -> Option<u64>;
    fn chunk(&mut self) -> GatewayChunkFuture<'_>;
}
struct LegacyGatewayHttp(reqwest::Client);
struct LegacyGatewayResponse(reqwest::Response);
impl GatewayHttpResponse for LegacyGatewayResponse {
    fn status(&self) -> reqwest::StatusCode {
        self.0.status()
    }
    fn headers(&self) -> &reqwest::header::HeaderMap {
        self.0.headers()
    }
    fn content_length(&self) -> Option<u64> {
        self.0.content_length()
    }
    fn chunk(&mut self) -> GatewayChunkFuture<'_> {
        Box::pin(async move { Ok(self.0.chunk().await?) })
    }
}
impl GatewayHttp for LegacyGatewayHttp {
    fn open(&self, request: GatewayHttpRequest) -> GatewayOpenFuture<'_> {
        Box::pin(async move {
            let mut builder = self.0.request(request.method, request.url);
            for (name, value) in request.headers {
                if name == "authorization" {
                    // Legacy only receives the same static bearer descriptor;
                    // retain reqwest's sensitive header marking/error behavior.
                    builder = builder.bearer_auth(
                        value
                            .strip_prefix("Bearer ")
                            .ok_or("invalid legacy bearer descriptor")?,
                    );
                } else {
                    builder = builder.header(name, value);
                }
            }
            if let Some(body) = request.body {
                builder = builder.body(body);
            }
            let response = builder.send().await?;
            Ok(Box::new(LegacyGatewayResponse(response)) as Box<dyn GatewayHttpResponse>)
        })
    }
}
/// Final DELETE grace matches the pinned rmcp streamable-HTTP session cleanup.
/// Normal source requests retain their independent transport/authorization budgets.
pub const GATEWAY_SESSION_CLEANUP_TIMEOUT: Duration = Duration::from_secs(5);

async fn gateway_until<T>(
    deadline: Option<tokio::time::Instant>,
    future: impl std::future::Future<Output = Result<T>>,
) -> Result<T> {
    match deadline {
        Some(deadline) => tokio::time::timeout_at(deadline, future).await?,
        None => future.await,
    }
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum GatewayStartPhase {
    Cold,
    Initialized,
    Ready,
    Failed,
    Closed,
}
/// Construct synchronously before async startup. Keep this object and the
/// supplied HTTP owner until explicit close/drain on every failure/EOF/signal.
/// Drop cannot issue async DELETE or promise joined HTTP shutdown.
pub struct RawGatewayStart {
    gateway: RawGateway,
    phase: GatewayStartPhase,
}
impl RawGatewayStart {
    pub fn new(
        url: String,
        token: Option<String>,
        http: std::sync::Arc<dyn GatewayHttp>,
    ) -> Result<Self> {
        Self::new_inner(url, token, http, false)
    }
    /// Original gateway accepts URL userinfo/fragments; the client HTTP owner
    /// applies the original Basic-vs-OAuth precedence and strips wire fragments.
    pub fn new_source(
        url: String,
        token: Option<String>,
        http: std::sync::Arc<dyn GatewayHttp>,
    ) -> Result<Self> {
        Self::new_inner(url, token, http, true)
    }
    fn new_inner(
        url: String,
        token: Option<String>,
        http: std::sync::Arc<dyn GatewayHttp>,
        source_urls: bool,
    ) -> Result<Self> {
        let parsed = reqwest::Url::parse(&url)?;
        if !source_urls
            && (!parsed.username().is_empty()
                || parsed.password().is_some()
                || parsed.fragment().is_some())
        {
            return Err("invalid MCP remote URL".into());
        }
        Ok(Self {
            gateway: RawGateway {
                http,
                url,
                token,
                catalogs: BTreeMap::new(),
                deadline: Duration::ZERO,
                transport_budgeted: true,
                source_projection: false,
                #[cfg(windows)]
                input_runtime: Default::default(),
                session: Default::default(),
                initialize_result: None,
                protocol: "2025-11-25".into(),
            },
            phase: GatewayStartPhase::Cold,
        })
    }
    /// Static source profile supplies Some(connect_timeout); OAuth supplies
    /// None, retaining its independent HTTP and human-authorization budgets.
    pub async fn initialize(&mut self, limit: Option<Duration>) -> Result<()> {
        if self.phase != GatewayStartPhase::Cold {
            return Err("gateway initialization phase invalid".into());
        }
        let deadline = limit
            .map(|d| {
                tokio::time::Instant::now()
                    .checked_add(d)
                    .ok_or("gateway phase deadline overflow")
            })
            .transpose()?;
        self.phase = GatewayStartPhase::Failed;
        if limit.is_some_and(|d| d.is_zero()) {
            return Err("gateway phase deadline exceeded".into());
        }
        let result = gateway_until(deadline, self.gateway.initialize_remote()).await;
        self.phase = if result.is_ok() {
            GatewayStartPhase::Initialized
        } else {
            GatewayStartPhase::Failed
        };
        result
    }
    /// Source gateway publishes tools only after complete bounded discovery;
    /// source static resource/prompt projection is the owning client's concern.
    pub async fn discover_tools(&mut self, limit: Duration) -> Result<()> {
        if self.phase != GatewayStartPhase::Initialized {
            return Err("gateway discovery phase invalid".into());
        }
        let deadline = tokio::time::Instant::now()
            .checked_add(limit)
            .ok_or("gateway phase deadline overflow")?;
        self.phase = GatewayStartPhase::Failed;
        if limit.is_zero() {
            return Err("gateway phase deadline exceeded".into());
        }
        let result = gateway_until(
            Some(deadline),
            self.gateway.discover_remote(&[("tools/list", "tools")]),
        )
        .await;
        self.phase = if result.is_ok() {
            GatewayStartPhase::Ready
        } else {
            GatewayStartPhase::Failed
        };
        result
    }
    /// Local gateway resources/prompts are fixed source declarations. Only tools
    /// come from remote discovery; installing this does not issue remote calls.
    pub fn install_source_projection(&mut self, catalogs: Catalogs) -> Result<()> {
        if self.phase != GatewayStartPhase::Ready {
            return Err("gateway projection before discovery".into());
        }
        for (method, catalog) in catalogs {
            if method == "tools/list" {
                return Err("gateway tool catalog override refused".into());
            }
            self.gateway.catalogs.insert(method, catalog);
        }
        let tools = crate::gateway_client::source_projection::tool_catalog(
            self.gateway.catalogs["tools/list"].get(),
        )?;
        self.gateway.catalogs.insert("tools/list".into(), tools);
        self.gateway.initialize_result = Some(
            crate::gateway_client::source_projection::initialize_info(&self.gateway.protocol)?,
        );
        self.gateway.source_projection = true;
        Ok(())
    }
    pub fn ready(&self) -> Result<RawGateway> {
        if self.phase != GatewayStartPhase::Ready {
            return Err("gateway is not ready".into());
        }
        Ok(self.gateway.clone())
    }
    pub async fn close(&mut self) -> Result<()> {
        self.phase = GatewayStartPhase::Failed;
        let result = self.gateway.close().await;
        if result.is_ok() {
            self.phase = GatewayStartPhase::Closed;
        }
        result
    }
}

#[derive(Clone)]
pub struct RawGateway {
    http: std::sync::Arc<dyn GatewayHttp>,
    url: String,
    token: Option<String>,
    pub catalogs: Catalogs,
    deadline: Duration,
    transport_budgeted: bool,
    source_projection: bool,
    #[cfg(windows)]
    input_runtime: crate::gateway_client::stdio_input_windows::InputRuntime,
    session: std::sync::Arc<std::sync::Mutex<Option<std::sync::Arc<String>>>>,
    initialize_result: Option<Box<RawValue>>,
    protocol: String,
}
impl RawGateway {
    async fn exchange(&self, body: &str) -> Result<Option<String>> {
        self.exchange_with_auth(body, GatewayAuthMode::Interactive)
            .await
    }
    async fn exchange_with_auth(
        &self,
        body: &str,
        auth: GatewayAuthMode,
    ) -> Result<Option<String>> {
        let deadline =
            (!self.transport_budgeted).then(|| tokio::time::Instant::now() + self.deadline);
        let mut headers = vec![
            ("accept", "application/json, text/event-stream".into()),
            ("content-type", "application/json".into()),
            ("mcp-protocol-version", self.protocol.clone()),
        ];
        if let Some(token) = &self.token {
            headers.push(("authorization", format!("Bearer {token}")));
        }
        let attempted_session = self.session.lock().unwrap().clone();
        if let Some(session) = &attempted_session {
            headers.push(("mcp-session-id", session.to_string()));
        }
        let mut response = gateway_until(
            deadline,
            self.http.open(GatewayHttpRequest {
                auth,
                method: reqwest::Method::POST,
                url: self.url.clone(),
                headers,
                body: Some(body.to_owned()),
            }),
        )
        .await?;
        if let Some(id) = response.headers().get("mcp-session-id") {
            let id = id.to_str()?.to_owned();
            let mut current = self.session.lock().unwrap();
            // Only the generation that sent this POST may update identity.
            // A delayed response from a closed/replaced session cannot revive it.
            let same_generation = match (&attempted_session, current.as_ref()) {
                (None, None) => true,
                (Some(attempted), Some(active)) => std::sync::Arc::ptr_eq(attempted, active),
                _ => false,
            };
            if same_generation {
                if current.as_ref().is_some_and(|old| old.as_str() != id) {
                    return Err("remote MCP session identity changed".into());
                }
                if current.is_none() {
                    *current = Some(std::sync::Arc::new(id));
                }
            }
        }
        if response.status() == 202 || response.status() == 204 {
            return Ok(None);
        }
        if !response.status().is_success() {
            return Err(format!("remote MCP HTTP status {}", response.status()).into());
        }
        if response
            .content_length()
            .is_some_and(|bytes| bytes > crate::http_client::MAX_RPC_BYTES as u64)
        {
            return Err("remote MCP Content-Length exceeds 4 MiB".into());
        }
        let sse = response
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .is_some_and(|v| v.starts_with("text/event-stream"));
        let request: Envelope = serde_json::from_str(body)?;
        let expected = request.id.as_ref().map(|v| key(v)).transpose()?;
        let read = async {
            let mut bytes = Vec::new();
            let mut frames = GatewaySse::default();
            let mut total = 0usize;
            while let Some(chunk) = response.chunk().await? {
                total = total.saturating_add(chunk.len());
                if total > 4 * 1024 * 1024 {
                    return Err("remote MCP body exceeds 4 MiB".into());
                }
                if sse {
                    if let Some(message) = frames.push(&chunk, expected.as_deref())? {
                        return Ok(Some(message));
                    }
                } else {
                    bytes.extend_from_slice(&chunk);
                }
            }
            if sse {
                return Err("MCP SSE ended without correlated response".into());
            }
            let body = String::from_utf8(bytes)?;
            let fields: BTreeMap<String, Box<RawValue>> = serde_json::from_str(&body)?;
            validate_reply(&fields, expected.as_deref())?;
            Ok(Some(body))
        };
        gateway_until(deadline, read).await
    }
    async fn initialize_remote(&mut self) -> Result<()> {
        let initialize=self.exchange(&json!({"jsonrpc":"2.0","id":0,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"ssx-native-proxy","version":"0.1.0"}}}).to_string()).await?.ok_or("remote initialize had no result")?;
        let mut fields: BTreeMap<String, Box<RawValue>> = serde_json::from_str(&initialize)?;
        if !fields.contains_key("result") || fields.contains_key("error") {
            return Err("remote initialize failed".into());
        }
        self.initialize_result = fields.remove("result");
        let initialize_fields: BTreeMap<String, Box<RawValue>> = serde_json::from_str(
            self.initialize_result
                .as_ref()
                .ok_or("initialize result")?
                .get(),
        )?;
        let version: String = serde_json::from_str(
            initialize_fields
                .get("protocolVersion")
                .ok_or("remote protocol version missing")?
                .get(),
        )?;
        if !["2024-11-05", "2025-03-26", "2025-06-18", "2025-11-25"].contains(&version.as_str()) {
            return Err("unsupported remote initialize protocol version".into());
        }
        self.protocol = version;
        self.exchange(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#)
            .await?;
        Ok(())
    }
    async fn discover_remote(&mut self, catalogs: &[(&str, &str)]) -> Result<()> {
        let mut budget = crate::gateway::DiscoveryBudget::default();
        for &(method, field) in catalogs {
            let mut items: Vec<Box<RawValue>> = Vec::new();
            let mut names = HashSet::new();
            let mut cursors = HashSet::new();
            let mut cursor = None;
            let mut complete = false;
            for page in 0..128 {
                let params = cursor
                    .as_ref()
                    .map(|c| json!({"cursor":c}))
                    .unwrap_or(json!({}));
                let body = self
                    .exchange(
                        &json!({"jsonrpc":"2.0","id":page+1,"method":method,"params":params})
                            .to_string(),
                    )
                    .await?
                    .ok_or("empty catalog response")?;
                budget.charge(body.len())?;
                let envelope: BTreeMap<String, Box<RawValue>> = serde_json::from_str(&body)?;
                if envelope.contains_key("error") {
                    return Err("remote catalog error".into());
                }
                let result: BTreeMap<String, Box<RawValue>> = serde_json::from_str(
                    envelope
                        .get("result")
                        .ok_or("catalog result missing")?
                        .get(),
                )?;
                let rows: Vec<Box<RawValue>> = serde_json::from_str(
                    result.get(field).ok_or("catalog entries missing")?.get(),
                )?;
                for row in rows {
                    let data: BTreeMap<String, Box<RawValue>> = serde_json::from_str(row.get())?;
                    let identity = crate::gateway::catalog_identity(method)?;
                    let name: String = serde_json::from_str(
                        data.get(identity).ok_or("catalog identity missing")?.get(),
                    )?;
                    if !names.insert(name) {
                        return Err("duplicate catalog name".into());
                    }
                    items.push(row);
                    if items.len() > 4096 {
                        return Err("catalog exceeds 4096 items".into());
                    }
                }
                let next: Option<String> = result
                    .get("nextCursor")
                    .map(|c| serde_json::from_str(c.get()))
                    .transpose()?
                    .flatten();
                let Some(next) = next else {
                    complete = true;
                    break;
                };
                if !cursors.insert(next.clone()) {
                    return Err("repeated catalog cursor".into());
                }
                cursor = Some(next);
            }
            if !complete {
                return Err("catalog exceeds 128 pages".into());
            }
            let raw = RawValue::from_string(format!(
                "{{{}:{}}}",
                serde_json::to_string(field)?,
                serde_json::to_string(&items)?
            ))?;
            self.catalogs.insert(method.into(), raw);
        }
        Ok(())
    }
    /// Raw upstream RPC for the owning source gateway projection.
    pub async fn rpc(&self, body: &str) -> Result<Option<String>> {
        self.exchange(body).await
    }
    pub async fn connect(url: String, token: Option<String>, deadline: Duration) -> Result<Self> {
        let parsed = reqwest::Url::parse(&url)?;
        if !parsed.username().is_empty()
            || parsed.password().is_some()
            || parsed.fragment().is_some()
        {
            return Err("invalid MCP remote URL".into());
        }
        let http = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .build()?;
        let mut gateway = Self {
            http: std::sync::Arc::new(LegacyGatewayHttp(http)),
            url,
            token,
            catalogs: BTreeMap::new(),
            deadline,
            transport_budgeted: false,
            source_projection: false,
            #[cfg(windows)]
            input_runtime: Default::default(),
            session: Default::default(),
            initialize_result: None,
            protocol: "2025-11-25".into(),
        };
        let startup = tokio::time::timeout(deadline, async {
            gateway.initialize_remote().await?;
            gateway
                .discover_remote(&[
                    ("tools/list", "tools"),
                    ("resources/list", "resources"),
                    ("prompts/list", "prompts"),
                    ("resources/templates/list", "resourceTemplates"),
                ])
                .await?;
            Ok::<(), crate::Error>(())
        })
        .await
        .map_err(|e| -> crate::Error { e.into() })
        .and_then(|v| v);
        if let Err(error) = startup {
            let _ = gateway.close().await;
            return Err(error);
        }
        Ok(gateway)
    }
    pub async fn stdio_owned(self, cancel: tokio_util::sync::CancellationToken) -> Result<()> {
        // stdio() owns its SDK service. Cancellation is delivered through EOF
        // below rather than dropping that service's waiting future.
        let wire: std::sync::Arc<std::sync::Mutex<WireState>> = Default::default();
        let handler = RawForwarder {
            gateway: self.clone(),
            wire: wire.clone(),
        };
        #[cfg(any(unix, windows))]
        {
            #[cfg(unix)]
            let mut input = crate::gateway_client::stdio_input::InputOwner::stdin()?;
            #[cfg(windows)]
            let mut input = self
                .input_runtime
                .stdin(&crate::gateway_client::resolver::install::current_input_image()?)?;
            let result = stdio_handler_io(
                handler,
                std::sync::Arc::new(self.catalogs.clone()),
                wire,
                crate::output_limits::OutputLimits::HTTP,
                cancel,
                input.reader(),
                tokio::io::stdout(),
            )
            .await;
            let drained = input.close().await;
            result.and(drained)
        }
        #[cfg(not(any(unix, windows)))]
        {
            let _ = (handler, wire, cancel);
            Err("gateway cancellable OS input is not implemented for this platform".into())
        }
    }
    pub async fn stdio(self) -> Result<()> {
        let wire: std::sync::Arc<std::sync::Mutex<WireState>> = Default::default();
        let handler = RawForwarder {
            gateway: self.clone(),
            wire: wire.clone(),
        };
        let result = stdio_handler(
            handler,
            std::sync::Arc::new(self.catalogs.clone()),
            wire,
            crate::output_limits::OutputLimits::HTTP,
        )
        .await;
        self.close().await?;
        result
    }
    fn clear_closed_session(&self, attempted: &std::sync::Arc<String>) {
        let mut current = self.session.lock().unwrap();
        if current
            .as_ref()
            .is_some_and(|id| std::sync::Arc::ptr_eq(id, attempted))
        {
            *current = None;
        }
    }
    pub async fn close(&self) -> Result<()> {
        #[cfg(windows)]
        let input = self.input_runtime.close().await;
        let session_result: Result<()> = (async {
            let session = self.session.lock().unwrap().clone();
            if let Some(session) = session {
                let mut headers = vec![
                    ("mcp-session-id", session.to_string()),
                    ("mcp-protocol-version", self.protocol.clone()),
                ];
                if let Some(token) = &self.token {
                    headers.push(("authorization", format!("Bearer {token}")));
                }
                // Final cleanup must reach the retained network/resolver join even
                // when DELETE needs fresh DNS or the peer holds response headers.
                // Match rmcp's five-second session-cleanup grace. Unlike rmcp's
                // logged timeout, our existing fail-closed cleanup contract returns
                // failure and retains the session identity for a possible retry.
                let limit = if self.transport_budgeted {
                    GATEWAY_SESSION_CLEANUP_TIMEOUT
                } else {
                    self.deadline
                };
                let deadline = Some(tokio::time::Instant::now() + limit);
                let response = gateway_until(
                    deadline,
                    self.http.open(GatewayHttpRequest {
                        auth: GatewayAuthMode::ExistingBearer,
                        method: reqwest::Method::DELETE,
                        url: self.url.clone(),
                        headers,
                        body: None,
                    }),
                )
                .await?;
                let status = response.status();
                if status.is_success() || status == 404 {
                    self.clear_closed_session(&session);
                } else if status != 405 {
                    return Err("remote session cleanup failed".into());
                }
                // Like rmcp, unsupported DELETE (405) is not a cleanup error. It does
                // not establish remote closure: retain identity for a later attempt.
            }
            Ok(())
        })
        .await;
        #[cfg(windows)]
        {
            input.and(session_result)
        }
        #[cfg(not(windows))]
        {
            session_result
        }
    }
}

#[derive(Clone)]
struct RawForwarder {
    gateway: RawGateway,
    wire: std::sync::Arc<std::sync::Mutex<WireState>>,
}
impl RawForwarder {
    fn decode<T: serde::de::DeserializeOwned>(
        value: Value,
    ) -> std::result::Result<T, rmcp::ErrorData> {
        serde_json::from_value(value)
            .map_err(|e| rmcp::ErrorData::internal_error(e.to_string(), None))
    }
    async fn forward<T: serde::de::DeserializeOwned>(
        &self,
        context: rmcp::service::RequestContext<rmcp::RoleServer>,
    ) -> std::result::Result<T, rmcp::ErrorData> {
        let id = key(&RawValue::from_string(
            serde_json::to_string(&context.id)
                .map_err(|e| rmcp::ErrorData::internal_error(e.to_string(), None))?,
        )
        .map_err(|e| rmcp::ErrorData::internal_error(e.to_string(), None))?)
        .map_err(|e| rmcp::ErrorData::internal_error(e.to_string(), None))?;
        let request = self
            .wire
            .lock()
            .unwrap()
            .requests
            .get(&id)
            .map(|request| request.original.clone())
            .ok_or_else(|| {
                rmcp::ErrorData::internal_error("original wire request missing", None)
            })?;
        let request = if self.gateway.source_projection {
            crate::gateway_client::source_projection::prepare_request(
                &self.gateway.catalogs,
                &request,
            )
            .map_err(|error| rmcp::ErrorData::invalid_params(error.to_string(), None))?
        } else {
            request
        };
        // Retain the pending HTTP future until cancellation is delivered. Dropping
        // it first tears down the remote request's alias lease before the cancel
        // notification can correlate its original arbitrary-size ID.
        let exchange = self.gateway.exchange(&request);
        tokio::pin!(exchange);
        let response = tokio::select! {
            r=&mut exchange=>r.map_err(|_|rmcp::ErrorData::internal_error("remote transport unavailable",None))?.ok_or_else(||rmcp::ErrorData::internal_error("remote RPC response missing",None))?,
            _=context.ct.cancelled()=>{
                let fields:BTreeMap<String,Box<RawValue>>=serde_json::from_str(&request).map_err(|e|rmcp::ErrorData::internal_error(e.to_string(),None))?;
                if let Some(id)=fields.get("id"){let notification=format!("{{\"jsonrpc\":\"2.0\",\"method\":\"notifications/cancelled\",\"params\":{{\"requestId\":{}}}}}",id.get());let _=self.gateway.exchange_with_auth(&notification, GatewayAuthMode::ExistingBearer).await;}
                return Err(rmcp::ErrorData::internal_error("cancelled",None));
            }
        };
        let response = if self.gateway.source_projection {
            crate::gateway_client::source_projection::project_reply(&request, &response).map_err(
                |_| rmcp::ErrorData::internal_error("remote result projection failed", None),
            )?
        } else {
            response
        };
        let fields: BTreeMap<String, Box<RawValue>> = serde_json::from_str(&response)
            .map_err(|e| rmcp::ErrorData::internal_error(e.to_string(), None))?;
        let decoded = if let Some(error) = fields.get("error") {
            Err(serde_json::from_str::<rmcp::ErrorData>(error.get())
                .map_err(|e| rmcp::ErrorData::internal_error(e.to_string(), None))?)
        } else {
            Ok(serde_json::from_str::<T>(
                fields
                    .get("result")
                    .ok_or_else(|| rmcp::ErrorData::internal_error("remote result missing", None))?
                    .get(),
            )
            .map_err(|e| rmcp::ErrorData::internal_error(e.to_string(), None))?)
        };
        self.wire.lock().unwrap().replies.insert(id, response);
        decoded
    }
}
impl rmcp::ServerHandler for RawForwarder {
    fn get_info(&self) -> rmcp::model::ServerConfig {
        self.gateway
            .initialize_result
            .as_ref()
            .and_then(|v| serde_json::from_str(v.get()).ok())
            .unwrap_or_default()
    }
    async fn list_tools(
        &self,
        _: Option<rmcp::model::PaginatedRequestParams>,
        _: rmcp::service::RequestContext<rmcp::RoleServer>,
    ) -> std::result::Result<rmcp::model::ListToolsResult, rmcp::ErrorData> {
        Self::decode(
            serde_json::from_str(self.gateway.catalogs["tools/list"].get()).unwrap_or_default(),
        )
    }
    async fn list_resources(
        &self,
        _: Option<rmcp::model::PaginatedRequestParams>,
        _: rmcp::service::RequestContext<rmcp::RoleServer>,
    ) -> std::result::Result<rmcp::model::ListResourcesResult, rmcp::ErrorData> {
        Self::decode(
            serde_json::from_str(self.gateway.catalogs["resources/list"].get()).unwrap_or_default(),
        )
    }
    async fn list_prompts(
        &self,
        _: Option<rmcp::model::PaginatedRequestParams>,
        _: rmcp::service::RequestContext<rmcp::RoleServer>,
    ) -> std::result::Result<rmcp::model::ListPromptsResult, rmcp::ErrorData> {
        Self::decode(
            serde_json::from_str(self.gateway.catalogs["prompts/list"].get()).unwrap_or_default(),
        )
    }
    async fn list_resource_templates(
        &self,
        _: Option<rmcp::model::PaginatedRequestParams>,
        _: rmcp::service::RequestContext<rmcp::RoleServer>,
    ) -> std::result::Result<rmcp::model::ListResourceTemplatesResult, rmcp::ErrorData> {
        Self::decode(
            serde_json::from_str(self.gateway.catalogs["resources/templates/list"].get())
                .unwrap_or_default(),
        )
    }
    async fn call_tool(
        &self,
        params: rmcp::model::CallToolRequestParams,
        c: rmcp::service::RequestContext<rmcp::RoleServer>,
    ) -> std::result::Result<rmcp::model::CallToolResponse, rmcp::ErrorData> {
        if self.gateway.source_projection {
            let catalog: BTreeMap<String, Box<RawValue>> =
                serde_json::from_str(self.gateway.catalogs["tools/list"].get()).map_err(|_| {
                    rmcp::ErrorData::internal_error("gateway catalog invalid", None)
                })?;
            let tools: Vec<BTreeMap<String, Box<RawValue>>> =
                serde_json::from_str(catalog["tools"].get()).map_err(|_| {
                    rmcp::ErrorData::internal_error("gateway catalog invalid", None)
                })?;
            let known = tools.iter().any(|tool| {
                tool.get("name")
                    .and_then(|name| serde_json::from_str::<String>(name.get()).ok())
                    .is_some_and(|name| name == params.name)
            });
            if !known {
                return Ok(rmcp::model::CallToolResult::error(vec![
                    rmcp::model::ContentBlock::text(format!("Unknown tool: {:?}", params.name)),
                ])
                .into());
            }
        }
        match self.forward::<rmcp::model::CallToolResult>(c).await {
            Ok(result) => Ok(result.into()),
            Err(error) if self.gateway.source_projection && error.code.0 != -32021 => Ok(
                rmcp::model::CallToolResult::error(vec![rmcp::model::ContentBlock::text(
                    error.message.to_string(),
                )])
                .into(),
            ),
            Err(error) => Err(error),
        }
    }
    async fn read_resource(
        &self,
        params: rmcp::model::ReadResourceRequestParams,
        c: rmcp::service::RequestContext<rmcp::RoleServer>,
    ) -> std::result::Result<rmcp::model::ReadResourceResponse, rmcp::ErrorData> {
        if self.gateway.source_projection
            && !crate::gateway_client::source_projection::resource_allowed(&params.uri)
        {
            return Err(rmcp::ErrorData::invalid_params(
                format!("Resource not found: {:?}", params.uri),
                Some(json!({"uri":params.uri})),
            ));
        }
        self.forward::<rmcp::model::ReadResourceResult>(c)
            .await
            .map(Into::into)
    }
    async fn get_prompt(
        &self,
        params: rmcp::model::GetPromptRequestParams,
        c: rmcp::service::RequestContext<rmcp::RoleServer>,
    ) -> std::result::Result<rmcp::model::GetPromptResponse, rmcp::ErrorData> {
        if self.gateway.source_projection
            && !crate::gateway_client::source_projection::prompt_allowed(&params.name)
        {
            return Err(rmcp::ErrorData::invalid_params(
                format!("Unknown prompt: {:?}", params.name),
                None,
            ));
        }
        if self.gateway.source_projection {
            if params
                .arguments
                .as_ref()
                .is_some_and(|args| args.values().any(|v| !v.is_string()))
            {
                return Err(rmcp::ErrorData::invalid_params(
                    "prompt arguments must be strings",
                    None,
                ));
            }
            let arg = crate::gateway_client::source_projection::prompt_argument(&params.name)
                .ok_or_else(|| rmcp::ErrorData::invalid_params("unknown prompt", None))?;
            if params
                .arguments
                .as_ref()
                .is_none_or(|args| !args.contains_key(arg))
            {
                return Err(rmcp::ErrorData::internal_error(
                    format!("Missing required arguments: {arg}"),
                    None,
                ));
            }
        }
        self.forward::<rmcp::model::GetPromptResult>(c)
            .await
            .map(Into::into)
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    struct InitProbe {
        entered: std::sync::Arc<tokio::sync::Notify>,
        dropped: std::sync::Arc<std::sync::atomic::AtomicBool>,
        hold: bool,
        owner_dropped: std::sync::Arc<tokio::sync::Notify>,
    }
    impl Drop for InitProbe {
        fn drop(&mut self) {
            self.owner_dropped.notify_one();
        }
    }
    struct InitDrop(std::sync::Arc<std::sync::atomic::AtomicBool>);
    impl Drop for InitDrop {
        fn drop(&mut self) {
            self.0.store(true, std::sync::atomic::Ordering::SeqCst);
        }
    }
    impl rmcp::ServerHandler for InitProbe {
        async fn initialize(
            &self,
            request: rmcp::model::InitializeRequestParams,
            context: rmcp::service::RequestContext<rmcp::RoleServer>,
        ) -> std::result::Result<rmcp::model::InitializeResult, rmcp::ErrorData> {
            let _drop = InitDrop(self.dropped.clone());
            self.entered.notify_one();
            if self.hold {
                std::future::pending::<()>().await;
            }
            context.peer.set_peer_info(request.clone());
            self.negotiate_initialize(&request)
        }
    }
    const OWNED_INIT: &[u8] = b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{\"protocolVersion\":\"2024-11-05\",\"capabilities\":{},\"clientInfo\":{\"name\":\"owned-stdio-test\",\"version\":\"1\"}}}\n";
    fn owned_probe(
        hold: bool,
    ) -> (
        InitProbe,
        std::sync::Arc<tokio::sync::Notify>,
        std::sync::Arc<std::sync::atomic::AtomicBool>,
        std::sync::Arc<tokio::sync::Notify>,
    ) {
        let entered = std::sync::Arc::new(tokio::sync::Notify::new());
        let dropped = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let owner_dropped = std::sync::Arc::new(tokio::sync::Notify::new());
        (
            InitProbe {
                entered: entered.clone(),
                dropped: dropped.clone(),
                hold,
                owner_dropped: owner_dropped.clone(),
            },
            entered,
            dropped,
            owner_dropped,
        )
    }
    // This uses the production stdio owner and real SDK initialization, not a
    // synthetic pending future. A spawned/detached initialization fails dropped.
    #[tokio::test]
    async fn owned_stdio_outer_abort_drops_held_sdk_initialization() -> Result<()> {
        let (handler, entered, dropped, _) = owned_probe(true);
        let (input, mut caller) = tokio::io::duplex(4096);
        let task = tokio::spawn(stdio_handler_io(
            handler,
            Default::default(),
            Default::default(),
            crate::output_limits::OutputLimits::HTTP,
            tokio_util::sync::CancellationToken::new(),
            input,
            tokio::io::sink(),
        ));
        caller.write_all(OWNED_INIT).await?;
        tokio::time::timeout(Duration::from_secs(2), entered.notified()).await?;
        task.abort();
        assert!(task.await.unwrap_err().is_cancelled());
        assert!(
            dropped.load(std::sync::atomic::Ordering::SeqCst),
            "held SDK initialization detached after owner abort"
        );
        Ok(())
    }
    #[tokio::test]
    async fn owned_stdio_eof_drops_held_sdk_initialization() -> Result<()> {
        let (handler, entered, dropped, _) = owned_probe(true);
        let (input, mut caller) = tokio::io::duplex(4096);
        let task = tokio::spawn(stdio_handler_io(
            handler,
            Default::default(),
            Default::default(),
            crate::output_limits::OutputLimits::HTTP,
            tokio_util::sync::CancellationToken::new(),
            input,
            tokio::io::sink(),
        ));
        caller.write_all(OWNED_INIT).await?;
        tokio::time::timeout(Duration::from_secs(2), entered.notified()).await?;
        drop(caller);
        tokio::time::timeout(Duration::from_secs(2), task).await???;
        assert!(dropped.load(std::sync::atomic::Ordering::SeqCst));
        Ok(())
    }
    #[tokio::test]
    async fn owned_stdio_explicit_cancel_drops_held_sdk_initialization() -> Result<()> {
        let (handler, entered, dropped, _) = owned_probe(true);
        let (input, mut caller) = tokio::io::duplex(4096);
        let cancel = tokio_util::sync::CancellationToken::new();
        let task = tokio::spawn(stdio_handler_io(
            handler,
            Default::default(),
            Default::default(),
            crate::output_limits::OutputLimits::HTTP,
            cancel.clone(),
            input,
            tokio::io::sink(),
        ));
        caller.write_all(OWNED_INIT).await?;
        tokio::time::timeout(Duration::from_secs(2), entered.notified()).await?;
        cancel.cancel();
        tokio::time::timeout(Duration::from_secs(2), task).await???;
        assert!(dropped.load(std::sync::atomic::Ordering::SeqCst));
        Ok(())
    }
    #[tokio::test]
    async fn owned_stdio_normal_eof_and_running_service_cancel_join() -> Result<()> {
        for explicit_cancel in [false, true] {
            let (handler, _, _, owner_dropped) = owned_probe(false);
            let (input, mut caller) = tokio::io::duplex(4096);
            let (output, reader) = tokio::io::duplex(4096);
            let cancel = tokio_util::sync::CancellationToken::new();
            let task = tokio::spawn(stdio_handler_io(
                handler,
                Default::default(),
                Default::default(),
                crate::output_limits::OutputLimits::HTTP,
                cancel.clone(),
                input,
                output,
            ));
            caller.write_all(OWNED_INIT).await?;
            let mut reader = BufReader::new(reader);
            let mut response = String::new();
            tokio::time::timeout(Duration::from_secs(2), reader.read_line(&mut response)).await??;
            let response: Value = serde_json::from_str(&response)?;
            assert!(response.get("result").is_some());
            if explicit_cancel {
                cancel.cancel();
            } else {
                drop(caller);
            }
            tokio::time::timeout(Duration::from_secs(2), task).await???;
            tokio::time::timeout(Duration::from_secs(2), owner_dropped.notified()).await?;
        }
        Ok(())
    }
    // SDK DropGuard requests asynchronous internal cancellation on outer-drop.
    // This checks idle owner release, not synchronous join or arbitrary handlers.
    #[tokio::test]
    async fn owned_stdio_initialized_outer_abort_releases_idle_sdk_owner() -> Result<()> {
        let (handler, _, _, owner_dropped) = owned_probe(false);
        let (input, mut caller) = tokio::io::duplex(4096);
        let (output, reader) = tokio::io::duplex(4096);
        let task = tokio::spawn(stdio_handler_io(
            handler,
            Default::default(),
            Default::default(),
            crate::output_limits::OutputLimits::HTTP,
            tokio_util::sync::CancellationToken::new(),
            input,
            output,
        ));
        caller.write_all(OWNED_INIT).await?;
        let mut response = String::new();
        tokio::time::timeout(
            Duration::from_secs(2),
            BufReader::new(reader).read_line(&mut response),
        )
        .await??;
        assert!(
            serde_json::from_str::<Value>(&response)?
                .get("result")
                .is_some()
        );
        task.abort();
        assert!(task.await.unwrap_err().is_cancelled());
        tokio::time::timeout(Duration::from_secs(2), owner_dropped.notified()).await?;
        Ok(())
    }
    #[tokio::test]
    async fn owned_stdio_pre_cancel_does_not_dispatch_initialize() -> Result<()> {
        let (handler, entered, _, owner_dropped) = owned_probe(true);
        let (input, mut caller) = tokio::io::duplex(4096);
        caller.write_all(OWNED_INIT).await?;
        let cancel = tokio_util::sync::CancellationToken::new();
        cancel.cancel();
        tokio::time::timeout(
            Duration::from_secs(2),
            stdio_handler_io(
                handler,
                Default::default(),
                Default::default(),
                crate::output_limits::OutputLimits::HTTP,
                cancel,
                input,
                tokio::io::sink(),
            ),
        )
        .await??;
        tokio::time::timeout(Duration::from_secs(2), owner_dropped.notified()).await?;
        assert!(
            tokio::time::timeout(Duration::from_millis(10), entered.notified())
                .await
                .is_err()
        );
        Ok(())
    }

    #[test]
    fn rejected_requests_do_not_leak_admission_capacity() -> Result<()> {
        let mut wire = WireState::default();
        let request = |id: u128| format!(r#"{{"jsonrpc":"2.0","id":{id},"method":"tools/list"}}"#);
        let base = u128::from(u64::MAX) + 1;
        let mut admitted = Vec::new();
        for id in base..base + 32 {
            admitted.push(wire.request(&request(id))?);
        }
        assert!(
            wire.request(&request(base))
                .unwrap_err()
                .to_string()
                .contains("duplicate")
        );
        for id in base + 32..base + 1032 {
            assert!(wire.request(&request(id)).is_err());
        }
        assert_eq!(wire.active.len(), 32);
        assert_eq!(wire.requests.len(), 32);
        assert_eq!(wire.aliases.len(), 32);
        for request in admitted {
            wire.abandon(&request);
        }
        assert!(wire.active.is_empty());
        assert!(wire.requests.is_empty());
        assert!(wire.aliases.is_empty());
        assert!(wire.reverse.is_empty());
        for id in base..base + 32 {
            assert!(wire.request(&request(id)).is_ok());
        }
        Ok(())
    }
    #[test]
    fn adjacent_large_request_ids_do_not_alias_and_sdk_ids_restore_exactly() -> Result<()> {
        let mut wire = WireState::default();
        let mut aliases = Vec::new();
        for id in ["18446744073709551616", "18446744073709551617"] {
            let request = format!("{{\"jsonrpc\":\"2.0\",\"id\":{id},\"method\":\"tools/list\"}}");
            let rewritten = wire.request(&request)?;
            let envelope: Envelope = serde_json::from_str(&rewritten)?;
            let alias = envelope.id.unwrap();
            assert!(alias.get().starts_with('"'));
            aliases.push((id, alias));
        }
        assert_ne!(key(&aliases[0].1)?, key(&aliases[1].1)?);
        for (original, alias) in aliases.into_iter().rev() {
            let response = format!(
                "{{\"jsonrpc\":\"2.0\",\"id\":{},\"result\":{{}}}}",
                alias.get()
            );
            let restored = wire.response(&response)?;
            let envelope: Envelope = serde_json::from_str(&restored)?;
            assert_eq!(envelope.id.unwrap().get(), original);
        }
        assert!(wire.active.is_empty());
        assert!(key(&RawValue::from_string("1.5".into())?).is_err());
        Ok(())
    }
    #[tokio::test]
    async fn newline_free_frame_is_rejected_before_eof_or_unbounded_allocation() -> Result<()> {
        let (mut writer, reader) = tokio::io::duplex(16384);
        let task = tokio::spawn(async move {
            let block = vec![b'x'; 16384];
            for _ in 0..257 {
                writer.write_all(&block).await?;
            }
            std::future::pending::<std::io::Result<()>>().await
        });
        let mut frames = Frames::new(reader);
        let result = tokio::time::timeout(Duration::from_secs(2), frames.next()).await?;
        assert!(result.is_err());
        assert!(frames.partial.len() <= 4 * 1024 * 1024);
        task.abort();
        let _ = task.await;
        Ok(())
    }
    #[test]
    fn compaction_preserves_strings_numbers_and_reserved_objects() -> Result<()> {
        let source = r#" { "x" : 18446744073709551617, "text": " spaces \" retained ", "y":{"$serde_json::private::Number":"123"} } "#;
        let out = compact(source)?;
        assert!(out.contains("18446744073709551617"));
        assert!(out.contains(r#"" spaces \" retained ""#));
        assert!(out.contains("$serde_json::private::Number"));
        Ok(())
    }
}

#[cfg(test)]
mod close_tests {
    use super::*;
    #[test]
    fn gateway_sse_visits_line_heavy_input_once() -> Result<()> {
        let mut input = b":\n".repeat(1_000_000);
        input.extend_from_slice(b"data: {\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{}}\n\n");
        for size in [input.len(), 1, 8192] {
            let mut parser = GatewaySse::default();
            let mut result = None;
            for chunk in input.chunks(size) {
                result = parser.push(chunk, Some("integer:1"))?.or(result);
            }
            assert!(result.is_some());
            assert_eq!(parser.inspected, input.len());
        }
        Ok(())
    }
    #[test]
    fn gateway_sse_preserves_fragmented_lines_and_event_limits() -> Result<()> {
        for delimiter in ["\n", "\r\n", "\r"] {
            let event = format!(
                "data: {{\"jsonrpc\":\"2.0\",{delimiter}data: \"id\":1,\"result\":\"雪\"}}{delimiter}{delimiter}"
            );
            for size in 1..=event.len() {
                let mut parser = GatewaySse::default();
                let mut result = None;
                for chunk in event.as_bytes().chunks(size) {
                    result = parser.push(chunk, Some("integer:1"))?.or(result);
                }
                let fields: Value = serde_json::from_str(&result.unwrap())?;
                assert_eq!(fields["result"], "雪");
            }
        }
        let notification = b"data: {\"jsonrpc\":\"2.0\",\"method\":\"progress\"}\n\n";
        let mut parser = GatewaySse::default();
        for _ in 0..128 {
            assert!(parser.push(notification, Some("integer:1"))?.is_none());
        }
        assert!(parser.push(notification, Some("integer:1")).is_err());
        assert!(GatewaySse::default().push(b"data: \xff\n\n", None).is_err());
        assert!(GatewaySse::default().push(b"data: {}\n", None)?.is_none());
        Ok(())
    }
    #[test]
    fn gateway_sse_skips_empty_sdk_priming_without_losing_multiline_data() -> Result<()> {
        // rmcp 3.5.1 ServerSseMessage::priming emits empty data, id and retry.
        for delimiter in ["\n", "\r\n", "\r"] {
            let prime =
                format!("id: 0{delimiter}retry: 1000{delimiter}data:{delimiter}{delimiter}");
            let reply = format!(
                "data:{delimiter}data: {{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":\"雪\"}}{delimiter}{delimiter}"
            );
            for size in 1..=reply.len() {
                let mut parser = GatewaySse::default();
                for _ in 0..129 {
                    for chunk in prime.as_bytes().chunks(size) {
                        assert!(parser.push(chunk, Some("integer:1"))?.is_none());
                    }
                }
                assert_eq!(parser.events, 0);
                let mut result = None;
                for chunk in reply.as_bytes().chunks(size) {
                    result = parser.push(chunk, Some("integer:1"))?.or(result);
                }
                let result = result.unwrap();
                assert!(result.starts_with('\n')); // Empty first data line survives joining.
                assert_eq!(serde_json::from_str::<Value>(&result)?["result"], "雪");
                assert_eq!(parser.events, 1);
            }
        }
        // Empty data is ignored, but a nonempty invalid JSON event still fails.
        assert!(GatewaySse::default().push(b"data: nope\n\n", None).is_err());
        Ok(())
    }
    #[tokio::test]
    async fn exchange_shares_deadline_across_headers_and_body() -> Result<()> {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        for sse in [false, true] {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
            let address = listener.local_addr()?;
            let server = tokio::spawn(async move {
                let (mut socket, _) = listener.accept().await.unwrap();
                let mut input = [0; 1];
                socket.read_exact(&mut input).await.unwrap();
                tokio::time::sleep(Duration::from_millis(700)).await;
                let body = if sse {
                    "data: {\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{}}\n\n"
                } else {
                    "{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{}}"
                };
                let mime = if sse {
                    "text/event-stream"
                } else {
                    "application/json"
                };
                let headers = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: {mime}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                socket.write_all(headers.as_bytes()).await.unwrap();
                tokio::time::sleep(Duration::from_millis(700)).await;
                let _ = socket.write_all(body.as_bytes()).await;
            });
            let gateway = RawGateway {
                http: std::sync::Arc::new(LegacyGatewayHttp(reqwest::Client::new())),
                url: format!("http://{address}/mcp"),
                token: None,
                catalogs: BTreeMap::new(),
                deadline: Duration::from_secs(1),
                transport_budgeted: false,
                source_projection: false,
                #[cfg(windows)]
                input_runtime: Default::default(),
                session: Default::default(),
                initialize_result: None,
                protocol: String::new(),
            };
            let result = gateway
                .exchange(r#"{"jsonrpc":"2.0","id":1,"method":"ping"}"#)
                .await;
            server.await?;
            assert!(
                result
                    .err()
                    .is_some_and(|error| error.is::<tokio::time::error::Elapsed>()),
                "split header/body delays exceeded one deadline: SSE={sse}"
            );
        }
        Ok(())
    }
    #[test]
    fn stale_close_cannot_clear_replaced_session_generation() {
        let gateway = RawGateway {
            http: std::sync::Arc::new(LegacyGatewayHttp(reqwest::Client::new())),
            url: String::new(),
            token: None,
            catalogs: BTreeMap::new(),
            deadline: Duration::from_secs(1),
            transport_budgeted: false,
            source_projection: false,
            #[cfg(windows)]
            input_runtime: Default::default(),
            session: Default::default(),
            initialize_result: None,
            protocol: String::new(),
        };
        let old = std::sync::Arc::new("same-remote-id".to_owned());
        let replacement = std::sync::Arc::new("same-remote-id".to_owned());
        *gateway.session.lock().unwrap() = Some(replacement.clone());
        gateway.clear_closed_session(&old);
        assert!(std::sync::Arc::ptr_eq(
            gateway.session.lock().unwrap().as_ref().unwrap(),
            &replacement
        ));
        gateway.clear_closed_session(&replacement);
        assert!(gateway.session.lock().unwrap().is_none());
        // Repeated completion is harmless.
        gateway.clear_closed_session(&replacement);
    }
    #[tokio::test]
    async fn late_post_session_header_does_not_resurrect_closed_identity() -> Result<()> {
        use axum::{response::IntoResponse, routing::post};
        use std::sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        };
        for failed_close in [false, true] {
            let entered = Arc::new(tokio::sync::Notify::new());
            let release = Arc::new(tokio::sync::Notify::new());
            let deletes = Arc::new(AtomicUsize::new(0));
            let (seen, resume, closed) = (entered.clone(), release.clone(), deletes.clone());
            let router = axum::Router::new().route("/mcp", post(move |body: String| {
                let (seen, resume) = (seen.clone(), resume.clone());
                async move {
                    let request: Value = serde_json::from_str(&body).unwrap();
                    let method = request["method"].as_str().unwrap();
                    if request.get("id").is_none() { return axum::http::StatusCode::ACCEPTED.into_response(); }
                    let result = match method {
                        "initialize" => json!({"protocolVersion":"2025-11-25","capabilities":{"tools":{},"resources":{},"prompts":{}},"serverInfo":{"name":"delayed-header","version":"1"}}),
                        "tools/list" => json!({"tools":[]}),
                        "resources/list" => json!({"resources":[]}),
                        "prompts/list" => json!({"prompts":[]}),
                        "tools/call" => { seen.notify_one(); resume.notified().await; json!({"content":[]}) },
                        _ => json!({"resourceTemplates":[]}),
                    };
                    let mut response = axum::Json(json!({"jsonrpc":"2.0","id":request["id"],"result":result})).into_response();
                    // The peer echoes its session on delayed responses, a legal
                    // upstream behavior beyond the SDK's initialize-only header.
                    response.headers_mut().insert("mcp-session-id", "session-one".parse().unwrap());
                    response
                }
            }).delete(move || {
                let closed = closed.clone();
                async move {
                    let count = closed.fetch_add(1, Ordering::SeqCst);
                    if failed_close && count == 0 { axum::http::StatusCode::INTERNAL_SERVER_ERROR }
                    else { axum::http::StatusCode::NO_CONTENT }
                }
            }));
            let host =
                crate::harness::Host::start(router, tokio_util::sync::CancellationToken::new())
                    .await?;
            let gateway = RawGateway::connect(
                format!("http://{}/mcp", host.address),
                None,
                Duration::from_secs(2),
            )
            .await?;
            let original = gateway.session.lock().unwrap().clone().unwrap();
            let calling = gateway.clone();
            let call = tokio::spawn(async move {
                calling.exchange(r#"{"jsonrpc":"2.0","id":10,"method":"tools/call","params":{"name":"delayed"}}"#).await
            });
            tokio::time::timeout(Duration::from_secs(2), entered.notified()).await?;
            assert_eq!(gateway.close().await.is_err(), failed_close);
            release.notify_one();
            assert!(
                tokio::time::timeout(Duration::from_secs(2), call)
                    .await???
                    .is_some()
            );
            if failed_close {
                assert!(Arc::ptr_eq(
                    gateway.session.lock().unwrap().as_ref().unwrap(),
                    &original
                ));
                gateway.close().await?;
                assert_eq!(deletes.load(Ordering::SeqCst), 2);
            } else {
                assert!(
                    gateway.session.lock().unwrap().is_none(),
                    "late POST resurrected successfully closed session identity"
                );
                gateway.close().await?;
                assert_eq!(deletes.load(Ordering::SeqCst), 1);
            }
            host.shutdown().await?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod native_completion_tests {
    use super::*;
    #[test]
    fn native_sequence_exhaustion_and_caller_alias_collision_are_atomic() -> Result<()> {
        let catalogs = [(
            "ssx/native-raw-arguments".into(),
            RawValue::from_string("[\"native\"]".into())?,
        )]
        .into_iter()
        .collect();
        let request = r#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"native","arguments":{}}}"#;
        let mut exhausted = WireState {
            native_sequence: u64::MAX,
            ..Default::default()
        };
        assert!(exhausted.request_for(&catalogs, request).is_err());
        assert!(exhausted.requests.is_empty());
        assert!(exhausted.active.is_empty());
        assert!(exhausted.aliases.is_empty());
        let mut wire = WireState {
            native_namespace: Some("ssx-native-private-".into()),
            ..Default::default()
        };
        let caller = r#"{"jsonrpc":"2.0","id":"ssx-native-private-1","method":"tools/list"}"#;
        wire.request(caller)?;
        assert!(wire.request_for(&catalogs, request).is_err());
        assert_eq!(wire.native_sequence, 0);
        assert_eq!(wire.requests.len(), 1);
        assert_eq!(wire.active.len(), 1);
        assert!(!wire.stale_native_generation(r#"{"id":"ssx-native-private-1","result":null}"#)?);
        wire.response(r#"{"id":"ssx-native-private-1","result":null}"#)?;
        let native = wire.request_for(&catalogs, request)?;
        assert!(native.contains("ssx-native-private-1"));
        assert_eq!(wire.requests.len(), 1);
        assert_eq!(wire.active.len(), 1);
        assert!(
            wire.native_response(r#"{"id":"ssx-native-other-1","result":null}"#)?
                .is_some()
        );
        assert!(
            wire.native_response(r#"{"id":"ssx-native-private-3","result":null}"#)?
                .is_some()
        );
        assert!(
            wire.native_response(r#"{"id":"ssx-native-private-01","result":null}"#)?
                .is_some()
        );
        Ok(())
    }
}
