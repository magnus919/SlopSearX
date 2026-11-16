//! T05 protocol and network authorization over the consumer-owned T04 state port.
mod protocol;
use axum::{
    Router,
    body::Body,
    extract::{Request, State},
    http::{HeaderMap, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use serde_json::{Value, json};
use ssx_application::oauth::OAuthState;
use ssx_domain::oauth::{OAuthError, OAuthSettings, Secret, TokenKind};
use std::sync::Arc;
use subtle::ConstantTimeEq;
pub const BODY_LIMIT: usize = 65_536;
/// Established only by verified network OAuth middleware, never tool arguments.
#[derive(Clone)]
pub struct AuthenticatedClient(pub ssx_domain::TenantId);
const INVALID_TOKEN: &str = "Authentication failed. The provided bearer token is invalid, expired, or no longer recognized by the server. To resolve: clear authentication tokens in your MCP client and reconnect. Your client should automatically re-register and obtain new tokens.";
/// Explicit protocol configuration. Browser identity and tenant policy do not enter this authority.
#[derive(Clone)]
pub struct OAuthEndpoints {
    pub(crate) store: Arc<dyn OAuthState>,
    pub(crate) settings: OAuthSettings,
    pub(crate) subject: Option<String>,
    pub(crate) documentation: Option<String>,
}
impl OAuthEndpoints {
    pub fn new(store: Arc<dyn OAuthState>, settings: OAuthSettings) -> Result<Self, OAuthError> {
        let checked = OAuthSettings::new(&settings.issuer_url, settings.ttls)
            .map_err(|_| OAuthError::InvalidInput)?;
        let url = url::Url::parse(&settings.issuer_url).map_err(|_| OAuthError::InvalidInput)?;
        // T04 preserves a slash introduced by dot-segment normalization. Parsing
        // that already-normalized issuer again is not a stable string projection.
        let canonical = url.as_str();
        if checked.resource_server_url != settings.resource_server_url
            || (settings.issuer_url != canonical
                && format!("{}/", settings.issuer_url) != canonical)
        {
            return Err(OAuthError::InvalidInput);
        }
        // ADR005 permits cleartext credentials only at explicit loopback development endpoints.
        let loopback = match url.host() {
            Some(url::Host::Domain("localhost")) => true,
            Some(url::Host::Ipv4(ip)) => ip.is_loopback(),
            Some(url::Host::Ipv6(ip)) => ip.is_loopback(),
            _ => false,
        };
        if url.scheme() != "https" && !loopback {
            return Err(OAuthError::InvalidInput);
        }
        Ok(Self {
            store,
            settings,
            subject: Some("operator".into()),
            documentation: None,
        })
    }
    pub fn with_documentation(mut self, value: Option<String>) -> Result<Self, OAuthError> {
        if let Some(s) = &value {
            let u = url::Url::parse(s).map_err(|_| OAuthError::InvalidInput)?;
            if !matches!(u.scheme(), "http" | "https") {
                return Err(OAuthError::InvalidInput);
            }
        }
        self.documentation = value;
        Ok(self)
    }
    fn issuer(&self) -> String {
        url::Url::parse(&self.settings.issuer_url)
            .expect("validated settings")
            .into()
    }
    fn base(&self) -> &str {
        self.settings.issuer_url.trim_end_matches('/')
    }
    fn metadata_url(&self) -> String {
        format!("{}/.well-known/oauth-protected-resource/mcp", self.base())
    }
    fn routes(&self) -> Router {
        Router::new()
            .route(
                "/.well-known/openid-configuration",
                get(|| async { (StatusCode::NOT_FOUND, "Not Found") }),
            )
            .route(
                "/.well-known/oauth-authorization-server",
                get(metadata).options(preflight),
            )
            .route(
                "/.well-known/oauth-protected-resource/mcp",
                get(resource).options(preflight),
            )
            .route("/register", post(protocol::register).options(preflight))
            .route(
                "/authorize",
                get(protocol::authorize).post(protocol::authorize),
            )
            .route("/token", post(protocol::token).options(preflight))
            .route("/revoke", post(protocol::revoke).options(preflight))
            .layer(middleware::from_fn(cors))
            .with_state(Arc::new(self.clone()))
    }
}
/// CLI precedence is intentionally separate from an explicitly requested outer helper wrapper.
#[derive(Clone)]
pub enum NetworkAuth {
    Open,
    Static(Secret),
    OAuth(OAuthEndpoints),
    Both {
        oauth: OAuthEndpoints,
        outer_static: Secret,
    },
}
impl NetworkAuth {
    pub fn from_cli(oauth: Option<OAuthEndpoints>, static_token: Option<Secret>) -> Self {
        if let Some(oauth) = oauth {
            Self::OAuth(oauth)
        } else if let Some(s) = static_token {
            Self::Static(s)
        } else {
            Self::Open
        }
    }
    /// Compose only around network routers. Trusted stdio is never passed through this gate.
    /// Apply to the complete T01 streamable router or legacy SSE/messages pair, including session requests.
    pub fn mount(self, transport: Router) -> Router {
        match self {
            Self::Open => transport,
            Self::Static(token) => transport.layer(middleware::from_fn_with_state(
                Gate::Static(token),
                protected,
            )),
            Self::OAuth(oauth) => transport
                .layer(middleware::from_fn_with_state(
                    Gate::OAuth(oauth.clone()),
                    protected,
                ))
                .merge(oauth.routes()),
            Self::Both {
                oauth,
                outer_static,
            } => Self::OAuth(oauth)
                .mount(transport)
                .layer(middleware::from_fn_with_state(
                    Gate::Static(outer_static),
                    protected,
                )),
        }
    }
}
#[derive(Clone)]
enum Gate {
    Static(Secret),
    OAuth(OAuthEndpoints),
}
fn bearer(headers: &HeaderMap) -> Option<&str> {
    let mut values = headers.get_all("authorization").iter();
    let value = values.next()?.to_str().ok()?;
    if values.next().is_some() {
        return None;
    }
    let (scheme, token) = value.split_once(' ')?;
    (scheme.eq_ignore_ascii_case("bearer") && !token.is_empty()).then_some(token)
}
pub(crate) fn equal(a: &str, b: &str) -> bool {
    bool::from(a.as_bytes().ct_eq(b.as_bytes()))
}
async fn protected(State(gate): State<Gate>, mut request: Request, next: Next) -> Response {
    let credential = bearer(request.headers());
    match gate {
        Gate::Static(token) => {
            if credential.is_some_and(|v| equal(v, token.expose())) {
                next.run(request).await
            } else {
                (
                    StatusCode::UNAUTHORIZED,
                    [("content-type", "text/plain; charset=utf-8")],
                    "unauthorized",
                )
                    .into_response()
            }
        }
        Gate::OAuth(oauth) => {
            let Some(raw) = credential else {
                return challenge(&oauth, None);
            };
            let Ok(token) = Secret::new(raw.into()) else {
                return challenge(&oauth, Some(INVALID_TOKEN));
            };
            match oauth.store.load_token(TokenKind::Access, &token).await {
                Ok(grant)
                    if grant
                        .grant
                        .resource
                        .as_ref()
                        .is_none_or(|v| v == &oauth.settings.resource_server_url) =>
                {
                    let Ok(tenant) = ssx_domain::TenantId::new(grant.grant.client_id.clone())
                    else {
                        return challenge(&oauth, Some(INVALID_TOKEN));
                    };
                    request.extensions_mut().insert(AuthenticatedClient(tenant));
                    next.run(request).await
                }
                Ok(_) | Err(OAuthError::Denied(_)) => challenge(&oauth, Some(INVALID_TOKEN)),
                Err(_) => protocol::unavailable(),
            }
        }
    }
}
fn challenge(oauth: &OAuthEndpoints, error: Option<&str>) -> Response {
    let value = if let Some(description) = error {
        format!(
            "Bearer error=\"invalid_token\", error_description=\"{description}\", resource_metadata=\"{}\"",
            oauth.metadata_url()
        )
    } else {
        format!("Bearer resource_metadata=\"{}\"", oauth.metadata_url())
    };
    let mut response = if let Some(description) = error {
        axum::Json(json!({"error":"invalid_token","error_description":description})).into_response()
    } else {
        Body::empty().into_response()
    };
    *response.status_mut() = StatusCode::UNAUTHORIZED;
    response
        .headers_mut()
        .insert("www-authenticate", value.parse().expect("validated URL"));
    response
}
async fn metadata(State(s): State<Arc<OAuthEndpoints>>) -> Response {
    let mut body = json!({"issuer":s.issuer(),"authorization_endpoint":format!("{}/authorize",s.base()),"token_endpoint":format!("{}/token",s.base()),"registration_endpoint":format!("{}/register",s.base()),"response_types_supported":["code"],"grant_types_supported":["authorization_code","refresh_token"],"token_endpoint_auth_methods_supported":["client_secret_post","client_secret_basic"],"revocation_endpoint":format!("{}/revoke",s.base()),"revocation_endpoint_auth_methods_supported":["client_secret_post","client_secret_basic"],"code_challenge_methods_supported":["S256"]});
    if let Some(v) = &s.documentation {
        body["service_documentation"] = json!(v);
    }
    cached(body)
}
async fn resource(State(s): State<Arc<OAuthEndpoints>>) -> Response {
    cached(
        json!({"resource":s.settings.resource_server_url,"authorization_servers":[s.issuer()],"scopes_supported":[],"bearer_methods_supported":["header"]}),
    )
}
fn cached(value: Value) -> Response {
    let mut r = axum::Json(value).into_response();
    r.headers_mut()
        .insert("cache-control", "public, max-age=3600".parse().unwrap());
    r
}
async fn preflight(State(s): State<Arc<OAuthEndpoints>>, request: Request) -> Response {
    if !request.headers().contains_key("origin")
        || !request
            .headers()
            .contains_key("access-control-request-method")
    {
        return match request.uri().path() {
            "/register" => protocol::register(State(s), request).await,
            "/token" => protocol::token(State(s), request).await,
            "/revoke" => protocol::revoke(State(s), request).await,
            "/.well-known/oauth-protected-resource/mcp" => resource(State(s)).await,
            _ => metadata(State(s)).await,
        };
    }
    let method = if request.uri().path().starts_with("/.well-known/") {
        "GET"
    } else {
        "POST"
    };
    let requested = request
        .headers()
        .get("access-control-request-method")
        .and_then(|v| v.to_str().ok());
    let headers = request
        .headers()
        .get("access-control-request-headers")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    let allowed = headers.split(',').all(|v| {
        matches!(
            v.trim().to_ascii_lowercase().as_str(),
            "" | "accept"
                | "accept-language"
                | "content-language"
                | "content-type"
                | "mcp-protocol-version"
        )
    });
    let mut r = if requested.is_none_or(|v| v == method || v == "OPTIONS") && allowed {
        "OK".into_response()
    } else {
        (
            StatusCode::BAD_REQUEST,
            if !allowed {
                "Disallowed CORS headers"
            } else {
                "Disallowed CORS method"
            },
        )
            .into_response()
    };
    for (k, v) in [
        ("access-control-allow-origin", "*"),
        (
            "access-control-allow-methods",
            if method == "GET" {
                "GET, OPTIONS"
            } else {
                "POST, OPTIONS"
            },
        ),
        (
            "access-control-allow-headers",
            "Accept, Accept-Language, Content-Language, Content-Type, mcp-protocol-version",
        ),
        ("access-control-max-age", "600"),
    ] {
        r.headers_mut().insert(k, v.parse().unwrap());
    }
    r
}
async fn cors(request: Request, next: Next) -> Response {
    let allowed = !matches!(
        request.uri().path(),
        "/authorize" | "/.well-known/openid-configuration"
    );
    let origin = request.headers().contains_key("origin");
    let mut response = next.run(request).await;
    if allowed && origin {
        response
            .headers_mut()
            .insert("access-control-allow-origin", "*".parse().unwrap());
    }
    response
}
