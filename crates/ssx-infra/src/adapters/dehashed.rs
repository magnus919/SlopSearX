//! DeHashed source adapter. Shared policy remains the caller's acquisition gate.
use super::{clinicaltrials as c, source_json::Value, threat_intel as t};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use ssx_application::{PortFuture, SearchAdmission, SearchContext, SearchEngine};
use ssx_domain::{AdapterResponse, SearchRequest};
use ssx_registry::{EngineFactory, Registry, RegistryError};
use std::{borrow::Cow, sync::Arc, time::Instant};

pub const ENDPOINT: &str = "https://dehashed.com/api/v1";
/// Retains the accepted F06 scalar/Secret boundary, including bool source slices.
pub type DeHashedConfig = t::ThreatIntelConfig;
/// Proposed A13 application of the existing operation quota. Not an RSS bound.
const PROJECTION_BYTES: usize = c::ROW_BYTES;
pub struct DeHashedAdapter {
    http: Arc<crate::http::AdapterHttp>,
    admission: Arc<dyn SearchAdmission>,
    config: DeHashedConfig,
}
struct Authorization {
    value: reqwest::header::HeaderValue,
    // The independent header copy remains covered while the request is owned.
    _charge: ssx_owned_budget::Charge,
}
pub fn register_dehashed(
    registry: &mut Registry,
    http: Arc<crate::http::AdapterHttp>,
    admission: Arc<dyn SearchAdmission>,
    config: DeHashedConfig,
) -> Result<(), RegistryError> {
    registry.register(
        "dehashed",
        Arc::new(move || {
            Box::new(DeHashedAdapter::new(
                http.clone(),
                admission.clone(),
                config.clone(),
            )) as Box<dyn ssx_registry::Engine>
        }) as EngineFactory,
    )
}
impl DeHashedAdapter {
    pub fn new(
        http: Arc<crate::http::AdapterHttp>,
        admission: Arc<dyn SearchAdmission>,
        config: DeHashedConfig,
    ) -> Self {
        Self {
            http,
            admission,
            config,
        }
    }
    async fn authorize_header(
        &self,
        budget: &t::Budget,
        context: Option<Instant>,
    ) -> t::Result<Authorization> {
        let raw = self.config.key();
        let (email, key) = raw.split_once(':').unwrap_or(("", raw));
        let pair = t::join(budget, &[email, ":", key], "", context).await?;
        let encoded = base64::encoded_len(pair.len(), true)
            .ok_or_else(|| t::resource(ssx_owned_budget::Error::Overflow))?;
        let mut header = t::owned(budget, "Basic ", context).await?;
        header.try_reserve_exact(encoded).map_err(t::resource)?;
        // Use the accepted encoder with fixed stack output; no uncharged encode String.
        for (i, chunk) in pair.as_bytes().chunks(3).enumerate() {
            if i % 1024 == 0 {
                t::checkpoint(context).await?;
            }
            let mut bytes = [0u8; 4];
            let len = STANDARD
                .encode_slice(chunk, &mut bytes)
                .map_err(|_| t::error("DeHashed credential encoding failed"))?;
            header
                .try_push_str(std::str::from_utf8(&bytes[..len]).expect("Base64 ASCII"))
                .map_err(t::resource)?;
        }
        let charge = budget.reserve(header.len()).map_err(t::resource)?;
        let mut value = reqwest::header::HeaderValue::from_str(header.as_str())
            .map_err(|_| t::error("invalid DeHashed credential configuration"))?;
        value.set_sensitive(true);
        Ok(Authorization {
            value,
            _charge: charge,
        })
    }
    async fn execute<'b>(
        &self,
        q: &SearchRequest,
        context: Option<Instant>,
        budget: &'b t::Budget,
        auth: Authorization,
    ) -> t::Result<c::ProducedBatch<'b>> {
        let deadline = self.config.network(context)?;
        let limit = query_scalar(&self.config.max_results);
        let url = t::join(
            budget,
            &[&self.config.base_url, "/search"],
            "",
            Some(deadline),
        )
        .await?;
        // DeHashed does not supply a custom User-Agent in the source.
        let mut request = c::request(
            url.as_str(),
            &[("q", &q.query), ("size", &limit)],
            false,
            true,
        )
        .map_err(|_| t::error("invalid DeHashed request URL"))?;
        request
            .headers
            .insert(reqwest::header::AUTHORIZATION, auth.value);
        let body = t::fetch(
            &self.http,
            request,
            deadline,
            Some(deadline),
            "DeHashed",
            false,
        )
        .await?
        .expect("only GreyNoise permits the empty404 branch");
        let result = project(
            &body,
            &self.config.max_results,
            &q.query,
            budget,
            Some(deadline),
        )
        .await;
        drop(auth._charge);
        result
    }
    async fn run(&self, q: &SearchRequest, context: Option<Instant>) -> AdapterResponse {
        let budget = t::Budget::new(PROJECTION_BYTES).expect("existing positive native quota");
        // Admission and the named missing-key refusal precede the source timer.
        let early = t::admit(&self.admission, "dehashed", context)
            .await
            .and_then(|()| {
                if self.config.key().is_empty() {
                    Err(t::error(
                        "DeHashed API key not configured (set ENGINE_DEHASHED_API_KEY)",
                    ))
                } else {
                    Ok(())
                }
            });
        match early {
            Ok(()) => {}
            Err(error) => {
                return AdapterResponse {
                    status: error.status,
                    error_message: error.message,
                    ..Default::default()
                };
            }
        };
        let start = Instant::now();
        let result = async {
            let auth = self.authorize_header(&budget, context).await?;
            self.execute(q, context, &budget, auth).await
        }
        .await;
        t::finish(result, start).await
    }
}
impl SearchEngine for DeHashedAdapter {
    fn search<'a>(&'a self, q: &'a SearchRequest) -> PortFuture<'a, AdapterResponse> {
        Box::pin(self.run(q, None))
    }
    fn search_with_context<'a>(
        &'a self,
        q: &'a SearchRequest,
        context: SearchContext,
    ) -> PortFuture<'a, AdapterResponse> {
        Box::pin(self.run(q, Some(context.deadline)))
    }
}
fn query_scalar(value: &ssx_config::ConfigScalar) -> Cow<'_, str> {
    match value {
        ssx_config::ConfigScalar::Float(x) if x.is_nan() => "nan".into(),
        ssx_config::ConfigScalar::Float(x) if *x == f64::INFINITY => "inf".into(),
        ssx_config::ConfigScalar::Float(x) if *x == f64::NEG_INFINITY => "-inf".into(),
        ssx_config::ConfigScalar::Text(value) => Cow::Borrowed(value),
        _ => Cow::Owned(c::scalar_text(value)),
    }
}
fn field<'a>(value: &'a Value, key: &str, default: &'a Value) -> t::Result<&'a Value> {
    value
        .as_object()
        .map(|m| m.get(key).unwrap_or(default))
        .ok_or_else(|| t::error("DeHashed response element does not support field access"))
}
async fn content_part(
    parts: &mut t::OwnedVec<t::OwnedString>,
    budget: &t::Budget,
    label: &str,
    value: &Value,
    deadline: Option<Instant>,
) -> t::Result<()> {
    if t::truthy(value) {
        let text = t::stringify(budget, value, deadline).await?;
        let part = t::join(budget, &[label, text.as_str()], "", deadline).await?;
        parts.try_push(part).map_err(t::resource)?;
    }
    Ok(())
}
async fn project<'b>(
    body: &Value,
    max: &ssx_config::ConfigScalar,
    query: &str,
    budget: &'b t::Budget,
    deadline: Option<Instant>,
) -> t::Result<c::ProducedBatch<'b>> {
    let empty_array = Value::Array(Vec::new());
    let entries = field(body, "entries", &empty_array)?;
    let rows = match entries {
        Value::Array(rows) => &rows[..c::slice_end(max, rows.len())?],
        Value::String(text) if c::slice_end(max, text.len())? == 0 => &[],
        Value::String(_) => {
            return Err(t::error(
                "DeHashed response element does not support field access",
            ));
        }
        // No numeric conversion precedes the source's non-sequence subscription.
        Value::Object(_) => return Err(t::error("DeHashed entries mapping cannot be sliced")),
        _ => {
            return Err(t::error(
                "DeHashed entries collection does not support slicing",
            ));
        }
    };
    let mut batch =
        c::ProducedBatch::new(budget, rows.len(), deadline).map_err(c::projection_error)?;
    let empty = t::empty();
    for (i, entry) in rows.iter().enumerate() {
        t::checkpoint(deadline).await?;
        // Preserve source getter order before constructing title or content.
        let email = field(entry, "email", &empty)?;
        let username = field(entry, "username", &empty)?;
        let password = field(entry, "password", &empty)?;
        let hash = field(entry, "hashed_password", &empty)?;
        let ip = field(entry, "ip_address", &empty)?;
        let name = field(entry, "name", &empty)?;
        let database = field(entry, "database_name", &empty)?;
        let breach = field(entry, "obtained_from", &empty)?;
        let selected_title = [email, username, name].into_iter().find(|v| t::truthy(v));
        let fallback = if selected_title.is_none() {
            let index = Value::integer(
                i64::try_from(i).map_err(|_| t::resource(ssx_owned_budget::Error::Overflow))?,
            );
            let id = field(entry, "id", &index)?;
            let id = t::stringify(budget, id, deadline).await?;
            Some(t::join(budget, &["Entry #", id.as_str()], "", deadline).await?)
        } else {
            None
        };
        let mut parts = t::OwnedVec::with_capacity(budget, 7).map_err(t::resource)?;
        content_part(&mut parts, budget, "Email: ", email, deadline).await?;
        content_part(&mut parts, budget, "Username: ", username, deadline).await?;
        for (value, label, bound, suffix) in [
            (password, "Password: ", 4, "***"),
            (hash, "Hash: ", 12, "..."),
        ] {
            if t::truthy(value) {
                let selected = t::slice_format(budget, value, bound, deadline).await?;
                parts
                    .try_push(
                        t::join(budget, &[label, selected.as_str(), suffix], "", deadline).await?,
                    )
                    .map_err(t::resource)?;
            }
        }
        content_part(&mut parts, budget, "IP: ", ip, deadline).await?;
        content_part(&mut parts, budget, "DB: ", database, deadline).await?;
        content_part(&mut parts, budget, "Source: ", breach, deadline).await?;
        let mut refs = t::OwnedVec::with_capacity(budget, parts.len()).map_err(t::resource)?;
        for part in parts.as_slice() {
            refs.try_push(part.as_str()).map_err(t::resource)?;
        }
        let account = if t::truthy(email) {
            t::stringify(budget, email, deadline).await?
        } else {
            t::owned(budget, query, deadline).await?
        };
        let url = t::join(
            budget,
            &["https://dehashed.com/search?q=", account.as_str()],
            "",
            deadline,
        )
        .await?;
        let content = t::join(budget, refs.as_slice(), " | ", deadline).await?;
        // F05 typed title projection happens only after the source content operations.
        let title = match selected_title {
            Some(value) => t::text(value)?,
            None => fallback.as_ref().expect("fallback title prepared").as_str(),
        };
        t::push(
            &mut batch,
            "dehashed",
            t::Fields {
                url: url.as_str(),
                title,
                content: content.as_str(),
                score: 0.,
                position: i + 1,
                date: None,
            },
        )
        .await?;
    }
    Ok(batch)
}

#[cfg(test)]
mod tests {
    use super::super::source_payload::{BATCH_PHASE, BatchPhase};
    use super::*;
    use ssx_domain::EngineStatus;
    use std::time::Duration;
    #[tokio::test]
    async fn projection_drop_releases_backing_after_an_actual_stored_row() {
        let body = super::super::source_json::from_slice(
            br#"{"entries":[{"email":"one@example.test"},{"email":"two@example.test"}]}"#,
            None,
        )
        .await
        .unwrap();
        let budget = t::Budget::new(1024 * 1024).unwrap();
        let entered = Arc::new(tokio::sync::Notify::new());
        let notification = entered.clone();
        let observer: Arc<dyn Fn(BatchPhase) -> bool + Send + Sync> = Arc::new(move |phase| {
            if matches!(phase, BatchPhase::Stored(1)) {
                notification.notify_one();
                true
            } else {
                false
            }
        });
        BATCH_PHASE.scope(observer,async{
            let max=ssx_config::ConfigScalar::Integer(10);let mut pending=Box::pin(project(&body,&max,"query",&budget,None));
            tokio::time::timeout(Duration::from_secs(1),async{tokio::select!{_ =entered.notified()=>{},_ =&mut pending=>panic!("projection must suspend after its first actual stored row")}}).await.unwrap();
            assert!(budget.snapshot().unwrap().used>0);drop(pending);assert_eq!(budget.snapshot().unwrap().used,0);
        }).await;
        println!("A13_DEHASHED_UNIT_COMPLETE projection-stored-drop");
    }
    #[tokio::test]
    async fn late_failure_and_tiny_quota_leave_no_retained_partial_rows() {
        for (id, raw, limit) in [
            (
                "projection-late-failure",
                br#"{"entries":[{"email":"valid@example.test"},false]}"#.as_slice(),
                1024 * 1024,
            ),
            (
                "projection-atomic-quota",
                br#"{"entries":[{"email":"valid@example.test"}]}"#.as_slice(),
                64,
            ),
        ] {
            let body = super::super::source_json::from_slice(raw, None)
                .await
                .unwrap();
            let budget = t::Budget::new(limit).unwrap();
            let Err(error) = project(
                &body,
                &ssx_config::ConfigScalar::Integer(10),
                "query",
                &budget,
                None,
            )
            .await
            else {
                panic!("late or capacity failure must not return a partial batch")
            };
            assert_eq!(error.status, EngineStatus::Error);
            assert_eq!(budget.snapshot().unwrap().used, 0);
            println!("A13_DEHASHED_UNIT_COMPLETE {id}");
        }
    }
}
