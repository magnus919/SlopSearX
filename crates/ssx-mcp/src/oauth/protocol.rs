use super::*;
use axum::{
    body::to_bytes,
    extract::{FromRequest, Multipart},
    http::header::HeaderName,
};
use base64::{
    Engine as _,
    engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD},
};
use serde_json::Map;
use sha2::{Digest, Sha256};
use ssx_domain::oauth::{ClientRegistration, CodeGrant, Denial, TokenPair};
use std::collections::BTreeMap;
type Params = BTreeMap<String, String>;
struct Failure {
    code: &'static str,
    description: String,
    status: StatusCode,
}
impl Failure {
    fn new(code: &'static str, description: impl Into<String>, status: StatusCode) -> Self {
        Self {
            code,
            description: description.into(),
            status,
        }
    }
    fn request(description: impl Into<String>) -> Self {
        Self::new("invalid_request", description, StatusCode::BAD_REQUEST)
    }
    fn auth(description: impl Into<String>) -> Self {
        Self::new("invalid_client", description, StatusCode::UNAUTHORIZED)
    }
    fn response(self, cache: u8) -> Response {
        response(
            self.status,
            json!({"error":self.code,"error_description":self.description}),
            cache,
        )
    }
}
fn response(status: StatusCode, value: Value, cache: u8) -> Response {
    let mut r = (status, axum::Json(value)).into_response();
    if cache > 0 {
        r.headers_mut()
            .insert("cache-control", "no-store".parse().unwrap());
    }
    if cache > 1 {
        r.headers_mut()
            .insert("pragma", "no-cache".parse().unwrap());
    }
    r
}
pub(super) fn unavailable() -> Response {
    response(
        StatusCode::SERVICE_UNAVAILABLE,
        json!({"error":"temporarily_unavailable","error_description":"OAuth authority is unavailable"}),
        2,
    )
}
async fn bytes(request: Request) -> Result<(HeaderMap, Vec<u8>), Box<Response>> {
    let (parts, body) = request.into_parts();
    let bytes = to_bytes(body, BODY_LIMIT)
        .await
        .map_err(|_| (StatusCode::PAYLOAD_TOO_LARGE, "Request body too large").into_response())?;
    Ok((parts.headers, bytes.to_vec()))
}
fn parameters(raw: &[u8]) -> Result<Params, Failure> {
    if raw.len() > BODY_LIMIT {
        return Err(Failure::request("OAuth parameter bytes exceed limit"));
    }
    let mut out = Params::new();
    for (k, v) in url::form_urlencoded::parse(raw) {
        if k.len() > 128 || v.len() > 4096 {
            return Err(Failure::request("OAuth parameter exceeds limit"));
        }
        if out.len() >= 64 || out.insert(k.into_owned(), v.into_owned()).is_some() {
            return Err(Failure::request("Duplicate or excessive OAuth parameters"));
        }
    }
    Ok(out)
}
async fn form(request: Request) -> Result<(HeaderMap, Params), Box<Response>> {
    let (headers, raw) = bytes(request).await?;
    if headers.get_all("content-type").iter().count() > 1 {
        return Err(Box::new(
            Failure::request("Ambiguous Content-Type headers").response(0),
        ));
    }
    let content_type = headers
        .get("content-type")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("");
    if content_type.len() > 512 {
        return Err(Box::new(
            Failure::request("Content type exceeds limit").response(0),
        ));
    }
    let typ = content_type
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    if typ == "application/x-www-form-urlencoded" {
        let params = parameters(&raw).map_err(|e| e.response(0))?;
        return Ok((headers, params));
    }
    if typ != "multipart/form-data" {
        return Ok((headers, Params::new()));
    }
    // The parser never receives more than BODY_LIMIT bytes, independently of its larger default.
    let mut request = Request::new(Body::from(raw));
    *request.headers_mut() = headers.clone();
    let mut multipart = Multipart::from_request(request, &())
        .await
        .map_err(|_| Failure::request("Malformed multipart form").response(0))?;
    let mut params = Params::new();
    let mut count = 0;
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|_| Failure::request("Malformed multipart form").response(0))?
    {
        count += 1;
        if count > 64 {
            return Err(Box::new(
                Failure::request("Excessive OAuth fields").response(0),
            ));
        }
        let name = field.name().unwrap_or("").to_owned();
        if name.len() > 128 {
            return Err(Box::new(
                Failure::request("OAuth field name exceeds limit").response(0),
            ));
        }
        let is_file = field.file_name().is_some();
        let value = field
            .bytes()
            .await
            .map_err(|_| Failure::request("Malformed multipart field").response(0))?;
        if value.len() > 4096 {
            return Err(Box::new(
                Failure::request("OAuth parameter exceeds limit").response(0),
            ));
        }
        if is_file {
            if matches!(
                name.as_str(),
                "client_id"
                    | "client_secret"
                    | "grant_type"
                    | "code"
                    | "redirect_uri"
                    | "code_verifier"
                    | "refresh_token"
                    | "scope"
                    | "resource"
                    | "token"
                    | "token_type_hint"
                    | "response_type"
                    | "code_challenge"
                    | "code_challenge_method"
                    | "state"
            ) {
                return Err(Box::new(
                    Failure::request("OAuth fields must be text").response(0),
                ));
            }
            continue;
        }
        let value = String::from_utf8(value.to_vec())
            .map_err(|_| Failure::request("OAuth fields must be UTF-8").response(0))?;
        if params.insert(name, value).is_some() {
            return Err(Box::new(
                Failure::request("Duplicate OAuth parameters").response(0),
            ));
        }
    }
    Ok((headers, params))
}
fn required<'a>(params: &'a Params, name: &str) -> Result<&'a str, Failure> {
    params
        .get(name)
        .map(String::as_str)
        .ok_or_else(|| Failure::request(format!("{name}: Field required")))
}
fn normalized_uri(raw: &str, http_only: bool) -> Result<String, ()> {
    let u = url::Url::parse(raw).map_err(|_| ())?;
    if http_only && !matches!(u.scheme(), "http" | "https") {
        return Err(());
    }
    // MCP's Pydantic metadata and client models preserve an originally empty URL path.
    let mut out = u.to_string();
    if u.path() == "/"
        && u.query().is_none()
        && u.fragment().is_none()
        && let Some((_, authority)) = raw.trim().split_once("://")
        && !authority.contains('/')
    {
        out.pop();
    }
    Ok(out)
}
fn strings(value: &Value, field: &str) -> Result<Vec<String>, Failure> {
    let Some(values) = value.as_array() else {
        return Err(Failure::request(format!(
            "{field}: Input should be a valid list"
        )));
    };
    values
        .iter()
        .enumerate()
        .map(|(i, v)| {
            v.as_str().map(str::to_owned).ok_or_else(|| {
                Failure::request(format!("{field}.{i}: Input should be a valid string"))
            })
        })
        .collect()
}
fn metadata(input: Value) -> Result<Map<String, Value>, Failure> {
    let invalid = |description: String| {
        Failure::new(
            "invalid_client_metadata",
            description,
            StatusCode::BAD_REQUEST,
        )
    };
    let Some(input) = input.as_object() else {
        return Err(invalid(": Input should be an object".into()));
    };
    let mut out = Map::new();
    let redirects = input
        .get("redirect_uris")
        .ok_or_else(|| invalid("redirect_uris: Field required".into()))?;
    if redirects.is_null() {
        out.insert("redirect_uris".into(), Value::Null);
    } else {
        let values = strings(redirects, "redirect_uris").map_err(|e| invalid(e.description))?;
        if values.is_empty() {
            return Err(invalid(
                "redirect_uris: List should have at least 1 item after validation, not 0".into(),
            ));
        }
        let values=values.into_iter().enumerate().map(|(i,v)|normalized_uri(&v,false).map(Value::String).map_err(|()|invalid(format!("redirect_uris.{i}: Input should be a valid URL, relative URL without a base")))).collect::<Result<Vec<_>,_>>()?;
        out.insert("redirect_uris".into(), json!(values));
    }
    for (name, default) in [
        ("response_types", json!(["code"])),
        (
            "grant_types",
            json!(["authorization_code", "refresh_token"]),
        ),
    ] {
        let value = input.get(name).unwrap_or(&default);
        let values = strings(value, name).map_err(|e| invalid(e.description))?;
        out.insert(name.into(), json!(values));
    }
    let method = input
        .get("token_endpoint_auth_method")
        .filter(|v| !v.is_null())
        .and_then(Value::as_str)
        .unwrap_or("client_secret_post");
    if !matches!(
        method,
        "none" | "client_secret_post" | "client_secret_basic" | "private_key_jwt"
    ) || input
        .get("token_endpoint_auth_method")
        .is_some_and(|v| !v.is_null() && !v.is_string())
    {
        return Err(invalid("token_endpoint_auth_method: Input should be 'none', 'client_secret_post', 'client_secret_basic' or 'private_key_jwt'".into()));
    }
    if method == "private_key_jwt" {
        return Err(invalid(
            "token_endpoint_auth_method 'private_key_jwt' is not supported".into(),
        ));
    }
    out.insert("token_endpoint_auth_method".into(), json!(method));
    let application = input
        .get("application_type")
        .and_then(Value::as_str)
        .unwrap_or("native");
    if !matches!(application, "native" | "web")
        || input
            .get("application_type")
            .is_some_and(|v| !v.is_string())
    {
        return Err(invalid(
            "application_type: Input should be 'web' or 'native'".into(),
        ));
    }
    out.insert("application_type".into(), json!(application));
    for name in ["scope", "client_name", "software_id", "software_version"] {
        let value = input.get(name).unwrap_or(&Value::Null);
        if !value.is_null() && !value.is_string() {
            return Err(invalid(format!("{name}: Input should be a valid string")));
        }
        out.insert(
            name.into(),
            if value == "" {
                Value::Null
            } else {
                value.clone()
            },
        );
    }
    for name in [
        "client_uri",
        "logo_uri",
        "tos_uri",
        "policy_uri",
        "jwks_uri",
    ] {
        let value = input.get(name).unwrap_or(&Value::Null);
        let value = if value.is_null() || value == "" {
            Value::Null
        } else {
            let Some(raw) = value.as_str() else {
                return Err(invalid(format!("{name}: Input should be a valid string")));
            };
            json!(
                normalized_uri(raw, true)
                    .map_err(|()| invalid(format!("{name}: Input should be a valid URL")))?
            )
        };
        out.insert(name.into(), value);
    }
    if let Some(contacts) = input.get("contacts").filter(|v| !v.is_null()) {
        out.insert(
            "contacts".into(),
            json!(strings(contacts, "contacts").map_err(|e| invalid(e.description))?),
        );
    } else {
        out.insert("contacts".into(), Value::Null);
    }
    out.insert(
        "jwks".into(),
        input
            .get("jwks")
            .filter(|v| *v != "")
            .cloned()
            .unwrap_or(Value::Null),
    );
    let grants = out["grant_types"].as_array().unwrap();
    if !grants.iter().any(|v| v == "authorization_code") {
        return Err(invalid(
            "grant_types must include 'authorization_code'".into(),
        ));
    }
    if grants
        .iter()
        .any(|v| v == "urn:ietf:params:oauth:grant-type:jwt-bearer")
    {
        return Err(invalid("grant_types must not include 'urn:ietf:params:oauth:grant-type:jwt-bearer'; the identity-assertion grant requires a pre-registered client".into()));
    }
    if !out["response_types"]
        .as_array()
        .unwrap()
        .iter()
        .any(|v| v == "code")
    {
        return Err(invalid(
            "response_types must include 'code' for authorization_code grant".into(),
        ));
    }
    Ok(out)
}
pub(super) async fn register(State(s): State<Arc<OAuthEndpoints>>, request: Request) -> Response {
    let (_, raw) = match bytes(request).await.map_err(|e| *e) {
        Ok(v) => v,
        Err(e) => return e,
    };
    let input = match serde_json::from_slice(&raw) {
        Ok(v) => v,
        Err(_) => {
            return Failure::new(
                "invalid_client_metadata",
                "Invalid JSON",
                StatusCode::BAD_REQUEST,
            )
            .response(0);
        }
    };
    let mut metadata = match metadata(input) {
        Ok(m) => m,
        Err(e) => return e.response(0),
    };
    let now = match s.store.now_ms().await {
        Ok(v) => v / 1000,
        Err(_) => return unavailable(),
    };
    let secret = if metadata["token_endpoint_auth_method"] != "none" {
        let mut bytes = [0; 32];
        if rustls::crypto::aws_lc_rs::default_provider()
            .secure_random
            .fill(&mut bytes)
            .is_err()
        {
            return unavailable();
        }
        Some(
            Secret::new(bytes.iter().map(|b| format!("{b:02x}")).collect())
                .expect("generated credential"),
        )
    } else {
        None
    };
    metadata.insert("client_id_issued_at".into(), json!(now));
    metadata.insert(
        "client_secret_expires_at".into(),
        if secret.is_some() {
            json!(0)
        } else {
            Value::Null
        },
    );
    let client = ClientRegistration {
        client_id: uuid::Uuid::new_v4().to_string(),
        client_secret: secret,
        metadata,
    };
    let output = client_response(&client);
    match s.store.register_client(client).await {
        Ok(()) => response(StatusCode::CREATED, output, 0),
        Err(OAuthError::InvalidInput) => Failure::new(
            "invalid_client_metadata",
            "Client metadata exceeds storage bounds",
            StatusCode::BAD_REQUEST,
        )
        .response(0),
        Err(_) => unavailable(),
    }
}
fn client_response(client: &ClientRegistration) -> Value {
    let mut data = client.metadata.clone();
    data.retain(|_, v| !v.is_null());
    data.insert("client_id".into(), json!(client.client_id));
    if let Some(secret) = &client.client_secret {
        data.insert("client_secret".into(), json!(secret.expose()));
    }
    Value::Object(data)
}
fn unquote(value: &str) -> String {
    let encoded = format!("v={}", value.replace('+', "%2B").replace('&', "%26"));
    url::form_urlencoded::parse(encoded.as_bytes())
        .next()
        .map(|(_, v)| v.into_owned())
        .unwrap_or_default()
}
async fn authenticate(
    s: &OAuthEndpoints,
    headers: &HeaderMap,
    params: &Params,
) -> Result<ClientRegistration, Box<Response>> {
    let id = params
        .get("client_id")
        .filter(|v| !v.is_empty())
        .ok_or_else(|| Failure::auth("Missing client_id").response(2))?;
    let client = match s.store.get_client(id).await {
        Ok(c) => c,
        Err(OAuthError::Denied(_)) => {
            return Err(Box::new(Failure::auth("Invalid client_id").response(2)));
        }
        Err(_) => return Err(Box::new(unavailable())),
    };
    let mut auth = headers.get_all("authorization").iter();
    let first = auth.next();
    if auth.next().is_some() {
        return Err(Box::new(
            Failure::auth("Ambiguous Authorization headers").response(2),
        ));
    }
    // Durable metadata is intentionally lossless in T04. Authority-bearing
    // fields must be typed here rather than coercing corruption into absence.
    let expires = match client.metadata.get("client_secret_expires_at") {
        None | Some(Value::Null) => None,
        Some(Value::Number(value)) => Some(value.as_u64().ok_or_else(|| Box::new(unavailable()))?),
        Some(_) => return Err(Box::new(unavailable())),
    };
    let method = match client
        .metadata
        .get("token_endpoint_auth_method")
        .and_then(Value::as_str)
    {
        Some(method @ ("none" | "client_secret_post" | "client_secret_basic")) => method,
        _ => return Err(Box::new(unavailable())),
    };
    let supplied = match method {
        "client_secret_basic" => {
            let header = first
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.strip_prefix("Basic "))
                .ok_or_else(|| {
                    Failure::auth("Missing or invalid Basic authentication in Authorization header")
                        .response(2)
                })?;
            let decoded = STANDARD
                .decode(header)
                .ok()
                .and_then(|v| String::from_utf8(v).ok())
                .ok_or_else(|| Failure::auth("Invalid Basic authentication header").response(2))?;
            let (id, secret) = decoded
                .split_once(':')
                .ok_or_else(|| Failure::auth("Invalid Basic authentication header").response(2))?;
            if unquote(id) != client.client_id {
                return Err(Box::new(
                    Failure::auth("Client ID mismatch in Basic auth").response(2),
                ));
            }
            Some(unquote(secret))
        }
        "client_secret_post" => params.get("client_secret").cloned(),
        "none" => None,
        _ => {
            return Err(Box::new(
                Failure::auth(format!("Unsupported auth method: {method}")).response(2),
            ));
        }
    };
    if method != "none" && client.client_secret.is_none() {
        return Err(Box::new(
            Failure::auth(
                "Client is registered for secret-based authentication but has no stored secret",
            )
            .response(2),
        ));
    }
    if let Some(secret) = &client.client_secret {
        let Some(value) = supplied.filter(|v| !v.is_empty()) else {
            return Err(Box::new(
                Failure::auth("Client secret is required").response(2),
            ));
        };
        if !equal(secret.expose(), &value) {
            return Err(Box::new(Failure::auth("Invalid client_secret").response(2)));
        }
        let now = s.store.now_ms().await.map_err(|_| unavailable())? / 1000;
        if expires.is_some_and(|v| v != 0 && v < now) {
            return Err(Box::new(
                Failure::auth("Client secret has expired").response(2),
            ));
        }
    }
    Ok(client)
}
fn redirect(client: &ClientRegistration, raw: Option<&str>) -> Result<String, Failure> {
    let uris = client
        .metadata
        .get("redirect_uris")
        .and_then(Value::as_array);
    if let Some(raw) = raw {
        let uri = normalized_uri(raw, false)
            .map_err(|()| Failure::request("redirect_uri: Input should be a valid URL"))?;
        if !uris.is_some_and(|a| a.iter().any(|v| v.as_str() == Some(&uri))) {
            return Err(Failure::request(format!(
                "Redirect URI '{uri}' not registered for client"
            )));
        }
        Ok(uri)
    } else {
        uris.filter(|v|v.len()==1).and_then(|v|v[0].as_str()).map(str::to_owned).ok_or_else(||Failure::request("redirect_uri must be specified unless the client has exactly one registered URI"))
    }
}
fn authorization_error(
    e: Failure,
    params: &Params,
    client: Option<&ClientRegistration>,
) -> Response {
    let mut value = json!({"error":e.code,"error_description":e.description});
    if let Some(v) = params.get("state") {
        value["state"] = json!(v);
    }
    if let Some(uri) =
        client.and_then(|c| redirect(c, params.get("redirect_uri").map(String::as_str)).ok())
    {
        return redirect_response(
            &uri,
            value
                .as_object()
                .unwrap()
                .iter()
                .filter_map(|(k, v)| v.as_str().map(|v| (k.as_str(), v))),
        );
    }
    response(StatusCode::BAD_REQUEST, value, 1)
}
fn redirect_response<'a>(uri: &str, values: impl Iterator<Item = (&'a str, &'a str)>) -> Response {
    let Ok(mut u) = url::Url::parse(uri) else {
        return Failure::request("Invalid redirect URI").response(1);
    };
    u.query_pairs_mut().extend_pairs(values);
    let mut r = Body::empty().into_response();
    *r.status_mut() = StatusCode::FOUND;
    r.headers_mut()
        .insert("location", u.as_str().parse().expect("encoded URI"));
    r.headers_mut()
        .insert("cache-control", "no-store".parse().unwrap());
    r
}
pub(super) async fn authorize(State(s): State<Arc<OAuthEndpoints>>, request: Request) -> Response {
    let params = if request.method() == axum::http::Method::GET {
        match parameters(request.uri().query().unwrap_or("").as_bytes()) {
            Ok(v) => v,
            Err(e) => return e.response(1),
        }
    } else {
        match form(request).await.map_err(|e| *e) {
            Ok((_, p)) => p,
            Err(e) => return e,
        }
    };
    let client = if let Some(id) = params.get("client_id") {
        match s.store.get_client(id).await {
            Ok(c) => Some(c),
            Err(OAuthError::Denied(_)) => None,
            Err(_) => return unavailable(),
        }
    } else {
        None
    };
    let validation = (|| -> Result<(), Failure> {
        // SDK model validation collects failures in field declaration order before
        // choosing response_type literal failure as the OAuth error discriminator.
        let mut errors = Vec::new();
        if !params.contains_key("client_id") {
            errors.push("client_id: Field required");
        }
        if params
            .get("redirect_uri")
            .is_some_and(|v| normalized_uri(v, false).is_err())
        {
            errors.push("redirect_uri: Input should be a valid URL, relative URL without a base");
        }
        let bad_response = params.get("response_type").is_some_and(|v| v != "code");
        match params.get("response_type") {
            None => errors.push("response_type: Field required"),
            Some(_) if bad_response => errors.push("response_type: Input should be 'code'"),
            _ => (),
        }
        if !params.contains_key("code_challenge") {
            errors.push("code_challenge: Field required");
        }
        if params
            .get("code_challenge_method")
            .is_some_and(|v| v != "S256")
        {
            errors.push("code_challenge_method: Input should be 'S256'");
        }
        if !errors.is_empty() {
            return Err(Failure::new(
                if bad_response {
                    "unsupported_response_type"
                } else {
                    "invalid_request"
                },
                errors.join("\n"),
                StatusCode::BAD_REQUEST,
            ));
        }
        if params["code_challenge"].is_empty() {
            return Err(Failure::request("code_challenge must not be empty"));
        }
        Ok(())
    })();
    if let Err(e) = validation {
        return authorization_error(e, &params, client.as_ref());
    }
    let Some(client) = client else {
        return authorization_error(
            Failure::request(format!("Client ID '{}' not found", params["client_id"])),
            &params,
            None,
        );
    };
    let uri = match redirect(&client, params.get("redirect_uri").map(String::as_str)) {
        Ok(u) => u,
        Err(e) => return authorization_error(e, &params, Some(&client)),
    };
    let scopes = if let Some(raw) = params.get("scope") {
        let requested = raw.split(' ').map(str::to_owned).collect::<Vec<_>>();
        let allowed = client
            .metadata
            .get("scope")
            .and_then(Value::as_str)
            .unwrap_or("")
            .split(' ')
            .collect::<Vec<_>>();
        if let Some(bad) = requested.iter().find(|v| !allowed.contains(&v.as_str())) {
            return authorization_error(
                Failure::new(
                    "invalid_scope",
                    format!("Client was not registered with scope {bad}"),
                    StatusCode::BAD_REQUEST,
                ),
                &params,
                Some(&client),
            );
        }
        requested
    } else {
        vec![]
    };
    if scopes.len() > 128
        || scopes
            .iter()
            .any(|v| v.is_empty() || v.len() > 4096 || v.chars().any(char::is_control))
    {
        return authorization_error(
            Failure::new(
                "invalid_scope",
                "Requested scopes exceed durable grant bounds",
                StatusCode::BAD_REQUEST,
            ),
            &params,
            Some(&client),
        );
    }
    if params
        .get("resource")
        .is_some_and(|v| v != &s.settings.resource_server_url)
    {
        return authorization_error(
            Failure::new(
                "invalid_target",
                "Resource does not match this server",
                StatusCode::BAD_REQUEST,
            ),
            &params,
            Some(&client),
        );
    }
    let grant = CodeGrant {
        client_id: client.client_id.clone(),
        scopes,
        code_challenge: params["code_challenge"].clone(),
        redirect_uri: uri.clone(),
        redirect_uri_provided_explicitly: params.contains_key("redirect_uri"),
        resource: params.get("resource").cloned(),
        subject: s.subject.clone(),
    };
    match s.store.create_code(grant).await {
        Ok(code) => {
            let mut values = vec![("code", code.code.expose())];
            if let Some(state) = params.get("state") {
                values.push(("state", state));
            }
            redirect_response(&uri, values.into_iter())
        }
        Err(OAuthError::InvalidInput) => authorization_error(
            Failure::request("Authorization parameters exceed durable grant bounds"),
            &params,
            Some(&client),
        ),
        Err(_) => unavailable(),
    }
}
fn pair_response(pair: TokenPair) -> Response {
    let mut body = json!({"access_token":pair.access_token.expose(),"token_type":"Bearer","expires_in":pair.expires_in,"refresh_token":pair.refresh_token.expose()});
    if !pair.grant.scopes.is_empty() {
        body["scope"] = json!(pair.grant.scopes.join(" "));
    }
    response(StatusCode::OK, body, 2)
}
// Full-client read projection: SDK placeholders use the declared default, while
// an explicit empty list denies every grant. Never rewrite durable metadata.
fn effective_client_grants(client: &ClientRegistration) -> Option<Vec<String>> {
    let defaults = || vec!["authorization_code".into(), "refresh_token".into()];
    match client.metadata.get("grant_types") {
        None | Some(Value::Null) => Some(defaults()),
        Some(Value::String(value)) if value.is_empty() => Some(defaults()),
        Some(Value::Array(values)) => values
            .iter()
            .map(|v| v.as_str().map(str::to_owned))
            .collect(),
        Some(_) => None,
    }
}
pub(super) async fn token(State(s): State<Arc<OAuthEndpoints>>, request: Request) -> Response {
    let (headers, params) = match form(request).await.map_err(|e| *e) {
        Ok(v) => v,
        Err(e) => return e,
    };
    let client = match authenticate(&s, &headers, &params).await.map_err(|e| *e) {
        Ok(c) => c,
        Err(e) => return e,
    };
    let Some(grants) = effective_client_grants(&client) else {
        return unavailable();
    };
    let grant_type = match required(&params, "grant_type") {
        Ok(v) => v,
        Err(e) => return e.response(2),
    };
    if !matches!(
        grant_type,
        "authorization_code" | "refresh_token" | "urn:ietf:params:oauth:grant-type:jwt-bearer"
    ) {
        return Failure::request("grant_type: Input should be 'authorization_code', 'refresh_token' or 'urn:ietf:params:oauth:grant-type:jwt-bearer'").response(2);
    }
    // Authentication is first; the selected request model is validated before
    // client grant permissions and any authorization-code/token lookup.
    let mut errors = Vec::new();
    let mut token_redirect = None;
    match grant_type {
        "authorization_code" => {
            if !params.contains_key("code") {
                errors.push("authorization_code.code: Field required");
            }
            match params.get("redirect_uri").map(|v| normalized_uri(v, false)).transpose() {
                Ok(uri) => token_redirect = uri,
                Err(()) => errors.push("authorization_code.redirect_uri: Input should be a valid URL, relative URL without a base"),
            }
            if !params.contains_key("code_verifier") {
                errors.push("authorization_code.code_verifier: Field required");
            }
        }
        "refresh_token" if !params.contains_key("refresh_token") => {
            errors.push("refresh_token.refresh_token: Field required")
        }
        "urn:ietf:params:oauth:grant-type:jwt-bearer" if !params.contains_key("assertion") => {
            errors.push("urn:ietf:params:oauth:grant-type:jwt-bearer.assertion: Field required")
        }
        _ => (),
    }
    if !errors.is_empty() {
        return Failure::request(errors.join("\n")).response(2);
    }
    if !grants.iter().any(|v| v == grant_type) {
        return Failure::new(
            "unsupported_grant_type",
            format!(
                "Unsupported grant type (supported grant types are {})",
                ssx_domain::storage::python_value_string(&ssx_json::Value::Array(
                    grants
                        .iter()
                        .cloned()
                        .map(ssx_json::Value::String)
                        .collect()
                ))
            ),
            StatusCode::BAD_REQUEST,
        )
        .response(2);
    }
    let invalid = |description: &str| {
        Failure::new("invalid_grant", description, StatusCode::UNAUTHORIZED).response(2)
    };
    if grant_type == "authorization_code" {
        let code = match required(&params, "code")
            .and_then(|v| Secret::new(v.into()).map_err(|_| Failure::request("Invalid code")))
        {
            Ok(v) => v,
            Err(e) => return e.response(2),
        };
        let verifier = match required(&params, "code_verifier") {
            Ok(v) => v,
            Err(e) => return e.response(2),
        };
        let loaded = match s.store.load_code(&code).await {
            Ok(v) if v.grant.client_id == client.client_id => v,
            Ok(_) | Err(OAuthError::Denied(_)) => {
                return invalid("authorization code does not exist");
            }
            Err(_) => return unavailable(),
        };
        let actual = token_redirect;
        let expected = loaded
            .grant
            .redirect_uri_provided_explicitly
            .then_some(&loaded.grant.redirect_uri);
        if actual.as_ref() != expected {
            return Failure::request(
                "redirect_uri did not match the one used when creating auth code",
            )
            .response(2);
        }
        let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
        if !equal(&challenge, &loaded.grant.code_challenge) {
            return invalid("incorrect code_verifier");
        }
        if params.get("resource").is_some_and(|v| {
            v != &s.settings.resource_server_url
                || loaded.grant.resource.as_ref().is_some_and(|g| g != v)
        }) {
            return Failure::new(
                "invalid_target",
                "Resource does not match the authorized grant",
                StatusCode::BAD_REQUEST,
            )
            .response(2);
        }
        match s.store.redeem_code(&code, &loaded.grant).await {
            Ok(p) => pair_response(p),
            Err(OAuthError::Denied(_)) => invalid("authorization code does not exist"),
            Err(_) => unavailable(),
        }
    } else if grant_type == "refresh_token" {
        let raw = match required(&params, "refresh_token").and_then(|v| {
            Secret::new(v.into()).map_err(|_| Failure::request("Invalid refresh token"))
        }) {
            Ok(v) => v,
            Err(e) => return e.response(2),
        };
        let loaded = match s.store.load_token(TokenKind::Refresh, &raw).await {
            Ok(v) if v.grant.client_id == client.client_id => v,
            Ok(_) | Err(OAuthError::Denied(_)) => return invalid("refresh token does not exist"),
            Err(_) => return unavailable(),
        };
        let scopes = params
            .get("scope")
            .filter(|v| !v.is_empty())
            .map(|v| v.split(' ').map(str::to_owned).collect::<Vec<_>>())
            .unwrap_or_else(|| loaded.grant.scopes.clone());
        if let Some(bad) = scopes.iter().find(|v| !loaded.grant.scopes.contains(v)) {
            return Failure::new(
                "invalid_scope",
                format!("cannot request scope `{bad}` not provided by refresh token"),
                StatusCode::BAD_REQUEST,
            )
            .response(2);
        }
        if params.get("resource").is_some_and(|v| {
            v != &s.settings.resource_server_url
                || loaded.grant.resource.as_ref().is_some_and(|g| g != v)
        }) {
            return Failure::new(
                "invalid_target",
                "Resource does not match the authorized grant",
                StatusCode::BAD_REQUEST,
            )
            .response(2);
        }
        match s.store.rotate_refresh(&raw, &loaded.grant, &scopes).await {
            Ok(p) => pair_response(p),
            Err(OAuthError::Denied(Denial::Scope) | OAuthError::InvalidInput) => Failure::new(
                "invalid_scope",
                "Requested scopes exceed durable grant bounds",
                StatusCode::BAD_REQUEST,
            )
            .response(2),
            Err(OAuthError::Denied(_)) => invalid("refresh token does not exist"),
            Err(_) => unavailable(),
        }
    } else {
        Failure::new(
            "unsupported_grant_type",
            "The JWT bearer grant is not supported by this authorization server",
            StatusCode::BAD_REQUEST,
        )
        .response(2)
    }
}
pub(super) async fn revoke(State(s): State<Arc<OAuthEndpoints>>, request: Request) -> Response {
    let (headers, params) = match form(request).await.map_err(|e| *e) {
        Ok(v) => v,
        Err(e) => return e,
    };
    let client = match authenticate(&s, &headers, &params).await.map_err(|e| *e) {
        Ok(c) => c,
        Err(mut e) => {
            if e.status() == StatusCode::UNAUTHORIZED {
                let body = to_bytes(e.into_body(), BODY_LIMIT)
                    .await
                    .unwrap_or_default();
                let mut v: Value = serde_json::from_slice(&body).unwrap_or(json!({}));
                v["error"] = json!("unauthorized_client");
                e = response(StatusCode::UNAUTHORIZED, v, 0);
            }
            return e;
        }
    };
    for k in ["token", "client_secret"] {
        if let Err(e) = required(&params, k) {
            return e.response(0);
        }
    }
    let hint = params.get("token_type_hint").map(String::as_str);
    if hint.is_some_and(|v| !matches!(v, "access_token" | "refresh_token")) {
        return Failure::request(
            "token_type_hint: Input should be 'access_token' or 'refresh_token'",
        )
        .response(0);
    }
    let token = match Secret::new(params["token"].clone()) {
        Ok(v) => v,
        Err(_) => return Failure::request("Invalid token").response(0),
    };
    let kinds = if hint == Some("refresh_token") {
        [TokenKind::Refresh, TokenKind::Access]
    } else {
        [TokenKind::Access, TokenKind::Refresh]
    };
    for kind in kinds {
        match s.store.load_token(kind, &token).await {
            Ok(grant) => {
                if grant.grant.client_id == client.client_id {
                    match s.store.revoke(kind, &token, &client.client_id).await {
                        Ok(()) | Err(OAuthError::Denied(_)) => (),
                        Err(_) => return unavailable(),
                    }
                }
                break;
            }
            Err(OAuthError::Denied(_)) => continue,
            Err(_) => return unavailable(),
        }
    }
    let mut r = Body::empty().into_response();
    r.headers_mut().insert(
        HeaderName::from_static("cache-control"),
        "no-store".parse().unwrap(),
    );
    r.headers_mut()
        .insert("pragma", "no-cache".parse().unwrap());
    r
}
