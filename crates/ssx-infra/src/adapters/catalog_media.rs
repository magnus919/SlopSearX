//! A09-local request and owned projection mechanics. No alternate transport/policy.
use super::{clinicaltrials as c, source_json::Value, source_payload};
use crate::http::{AdapterHttp, Request};
use ssx_domain::{AdapterResponse, EngineStatus};
use std::time::Instant;
pub type Settings = c::BiomedicalConfig;
pub(super) struct Failure {
    pub status: EngineStatus,
    pub message: Option<String>,
}
impl From<c::Failure> for Failure {
    fn from(error: c::Failure) -> Self {
        let response = c::outcome(Err(error), Instant::now());
        Self {
            status: response.status,
            message: response.error_message,
        }
    }
}
pub(super) type Result<T> = std::result::Result<T, Failure>;
pub(super) fn error(message: impl Into<String>) -> Failure {
    Failure {
        status: EngineStatus::Error,
        message: Some(message.into()),
    }
}
pub(super) fn failure(status: EngineStatus, message: impl Into<String>) -> Failure {
    Failure {
        status,
        message: Some(message.into()),
    }
}
pub(super) struct Batch<'a> {
    pub rows: c::ProducedBatch<'a>,
    pub diagnostic: Option<String>,
}
impl<'a> Batch<'a> {
    pub fn new(budget: &'a c::Budget, capacity: usize, context: Option<Instant>) -> Result<Self> {
        Ok(Self {
            rows: c::ProducedBatch::new(budget, capacity, context).map_err(c::projection_error)?,
            diagnostic: None,
        })
    }
}
pub(super) async fn finish(result: Result<Batch<'_>>, start: Instant) -> AdapterResponse {
    let result = match result {
        Ok(batch) => batch
            .rows
            .into_adapter_response(source_payload::Outcome {
                status: EngineStatus::Ok,
                error_message: batch.diagnostic.as_deref(),
                latency_ms: start.elapsed().as_secs_f64() * 1000.,
                synthetic: false,
            })
            .await
            .map_err(|e| Failure::from(c::projection_error(e))),
        Err(e) => Err(e),
    };
    match result {
        Ok(response) => response,
        Err(e) => AdapterResponse {
            status: e.status,
            error_message: e.message,
            latency_ms: c::elapsed(start),
            ..Default::default()
        },
    }
}
pub(super) async fn fetch(
    http: &AdapterHttp,
    req: Request,
    deadline: Instant,
    context: Option<Instant>,
    wayback: bool,
    archive: bool,
    source: &str,
) -> Result<Value> {
    let mut session = c::session(http, deadline)?;
    c::check(deadline)?;
    let response = session.execute(req).await.map_err(|e| {
        let timeout = matches!(e, crate::http::Error::Deadline);
        let status = if timeout && !archive {
            EngineStatus::Timeout
        } else {
            EngineStatus::Error
        };
        Failure {
            status,
            message: if timeout && !wayback && !archive {
                None
            } else {
                Some(format!(
                    "{source} request {}",
                    if timeout { "timed out" } else { "failed" }
                ))
            },
        }
    })?;
    let code = response.status;
    if (wayback && code != 200) || !(200..300).contains(&code) {
        let status = if archive {
            EngineStatus::Error
        } else {
            match code {
                429 => EngineStatus::RateLimited,
                401 | 403 if wayback => EngineStatus::Blocked,
                408 if wayback => EngineStatus::Timeout,
                _ => EngineStatus::Error,
            }
        };
        return Err(Failure {
            status,
            message: if code == 429 && !wayback && !archive {
                None
            } else {
                Some(format!("{source} returned HTTP {code}"))
            },
        });
    }
    Ok(c::parse_body_observed(&response.body, context, None).await?)
}
pub(super) fn float(v: &Value) -> Result<f64> {
    Ok(super::regulatory::float(v)?)
}
pub(super) fn text(v: &Value) -> Result<&str> {
    Ok(c::borrowed_text(v)?)
}
pub(super) async fn owned(
    b: &c::Budget,
    s: &str,
    context: Option<Instant>,
) -> Result<c::OwnedString> {
    Ok(c::owned_join(b, &[s], "", None, context).await?)
}
pub(super) async fn join(
    b: &c::Budget,
    s: &[&str],
    cap: Option<usize>,
    context: Option<Instant>,
) -> Result<c::OwnedString> {
    Ok(c::owned_join(b, s, "", cap, context).await?)
}
macro_rules! adapter {
    ($name:ident, $config:ty, $engine:literal, $default:expr) => {
        pub struct $name { http:std::sync::Arc<crate::http::AdapterHttp>, admission:std::sync::Arc<dyn ssx_application::SearchAdmission>, config:$config }
        impl $name {
            pub fn new(http:std::sync::Arc<crate::http::AdapterHttp>,admission:std::sync::Arc<dyn ssx_application::SearchAdmission>,config:$config)->Self { Self{http,admission,config} }
            async fn run(&self,q:&ssx_domain::SearchRequest,context:Option<std::time::Instant>)->ssx_domain::AdapterResponse {
                let start=std::time::Instant::now();
                let budget=super::clinicaltrials::Budget::new(super::clinicaltrials::ROW_BYTES).expect("valid A09 quota");
                super::catalog_media::finish(self.execute(q,context,&budget).await,start).await
            }
        }
        #[cfg(test)]
        mod owned_operation_tests {
            use super::*;
            use crate::http::{Transport,ExchangeFuture,Request,Response,AdapterHttp};
            use std::sync::Arc;
            struct Large;
            impl Transport for Large {fn exchange(&self,req:Request)->ExchangeFuture<'_>{Box::pin(async move{
                let text="🙂".repeat(100000);
                let body=match $engine {
                    "openlibrary"=>serde_json::json!({"docs":[{"key":"OL1W","title":"first"},{"key":"OL2W","title":text,"cover_i":42}]}),
                    "musicbrainz"=>serde_json::json!({"artists":[{"name":"first","id":"id1"},{"name":text,"id":"id2"}]}),
                    "tmdb"=>serde_json::json!({"results":[{"media_type":"movie","title":"first","id":1},{"media_type":"movie","title":text,"overview":text,"id":2}]}),
                    "nominatim"=>serde_json::json!([{"display_name":"first","osm_type":"node","osm_id":1},{"display_name":text,"osm_type":"node","osm_id":text}]),
                    _=>serde_json::json!({"response":{"docs":[{"title":"first","identifier":"id1"},{"title":text,"identifier":"id2"}]}}),
                };
                Ok(Response{url:req.url,status:200,headers:Default::default(),body:serde_json::to_vec(&body).unwrap()})
            })}}
            fn engine()->($name,Arc<AdapterHttp>){
                let h=Arc::new(AdapterHttp::default());h.set_transport(Arc::new(Large)).unwrap();
                let mut config:$config=$default;
                // Configured credential is a synthetic local fixture.
                set_test_key(&mut config);
                ($name::new(h.clone(),Arc::new(super::super::AllowAllAdmission),config),h)
            }
            struct Mixed(std::sync::atomic::AtomicUsize);
            impl Transport for Mixed {fn exchange(&self,req:Request)->ExchangeFuture<'_>{Box::pin(async move{
                let first=self.0.fetch_add(1,std::sync::atomic::Ordering::SeqCst)==0;
                let good=match $engine {"openlibrary"=>serde_json::json!({"key":"OL1W","title":"valid"}),"musicbrainz"=>serde_json::json!({"id":"artist","name":"valid"}),"tmdb"=>serde_json::json!({"media_type":"movie","title":"valid"}),"nominatim"=>serde_json::json!({"display_name":"valid"}),_=>serde_json::json!({"identifier":"book","title":"valid"})};
                let mut bad=good.clone();bad[if $engine=="musicbrainz"{"name"}else if $engine=="nominatim"{"display_name"}else{"title"}]=serde_json::Value::Null;
                let rows=if first{vec![good,bad]}else{vec![good]};
                let body=match $engine {"openlibrary"=>serde_json::json!({"docs":rows}),"musicbrainz"=>serde_json::json!({"artists":rows}),"tmdb"=>serde_json::json!({"results":rows}),"nominatim"=>serde_json::json!(rows),_=>serde_json::json!({"response":{"docs":rows}})};
                Ok(Response{url:req.url,status:200,headers:Default::default(),body:serde_json::to_vec(&body).unwrap()})
            })}}
            #[tokio::test]
            async fn valid_then_invalid_is_atomic_releases_and_next_call_succeeds(){
                let http=Arc::new(AdapterHttp::default());http.set_transport(Arc::new(Mixed(std::sync::atomic::AtomicUsize::new(0)))).unwrap();let mut config:$config=$default;set_test_key(&mut config);
                let engine=$name::new(http.clone(),Arc::new(super::super::AllowAllAdmission),config);
                let budget=super::super::clinicaltrials::Budget::new(1024*1024).unwrap();let q=ssx_domain::SearchRequest{query:"test".into(),..Default::default()};
                let result=engine.execute(&q,None,&budget).await;assert!(result.is_err());let response=super::super::catalog_media::finish(result,std::time::Instant::now()).await;
                assert_eq!(response.status,ssx_domain::EngineStatus::Error);assert!(response.results.is_empty());assert_eq!(budget.snapshot().unwrap().used,0);
                let next=super::super::catalog_media::finish(engine.execute(&q,None,&budget).await,std::time::Instant::now()).await;
                assert_eq!(next.status,ssx_domain::EngineStatus::Ok);assert_eq!(next.results.len(),1);assert_eq!(budget.snapshot().unwrap().used,0);http.shutdown_and_wait().await;
                // Execute each exact captured valid-then-malformed input with this same owned seam.
                struct Exact(std::sync::Mutex<std::collections::VecDeque<Vec<u8>>>);
                impl Transport for Exact{fn exchange(&self,req:Request)->ExchangeFuture<'_>{Box::pin(async move{Ok(Response{url:req.url,status:200,headers:Default::default(),body:self.0.lock().unwrap().pop_front().expect("unexpected atomic source call")})})}}
                let source:serde_json::Value=serde_json::from_str(include_str!("../../../../docs/rust-rewrite/a09/source.json")).unwrap();
                for case in source["records"].as_array().unwrap().iter().filter(|c|c["engine"]==$engine&&c["id"].as_str().unwrap().contains("-atomic-")){
                    let wire=case["responses"][0]["wire_body"].as_str().unwrap();
                    let mut first_valid:serde_json::Value=serde_json::from_str(wire).unwrap();
                    let collection=match $engine{"openlibrary"=>&mut first_valid["docs"],"musicbrainz"=>&mut first_valid["artists"],"tmdb"=>&mut first_valid["results"],_=>&mut first_valid["response"]["docs"]};
                    assert_eq!(collection.as_array().unwrap().len(),2);collection.as_array_mut().unwrap().truncate(1);
                    let transport=Arc::new(Exact(std::sync::Mutex::new(std::collections::VecDeque::from([wire.as_bytes().to_vec(),serde_json::to_vec(&first_valid).unwrap()]))));
                    let http=Arc::new(AdapterHttp::default());http.set_transport(transport.clone()).unwrap();let mut cfg:$config=$default;set_test_key(&mut cfg);
                    let adapter=$name::new(http.clone(),Arc::new(super::super::AllowAllAdmission),cfg);let b=super::super::clinicaltrials::Budget::new(1024*1024).unwrap();
                    let query=ssx_domain::SearchRequest{query:case["query"].as_str().unwrap().into(),..Default::default()};
                    let bad=super::super::catalog_media::finish(adapter.execute(&query,None,&b).await,std::time::Instant::now()).await;
                    assert_eq!(bad.status,ssx_domain::EngineStatus::Error,"{}",case["id"]);assert!(bad.results.is_empty());assert_eq!(b.snapshot().unwrap().used,0);
                    let good=super::super::catalog_media::finish(adapter.execute(&query,None,&b).await,std::time::Instant::now()).await;
                    assert_eq!(good.status,ssx_domain::EngineStatus::Ok);assert_eq!(good.results.len(),1);assert_eq!(b.snapshot().unwrap().used,0);assert!(transport.0.lock().unwrap().is_empty());
                    http.shutdown_and_wait().await;println!("\nA09_OWNED_CASE {}",case["id"]);
                }
            }
            #[tokio::test]
            async fn actual_engine_budget_rejects_and_releases_all_operation_storage(){
                let (engine,http)=engine();let b=super::super::clinicaltrials::Budget::new(64).unwrap();
                let q=ssx_domain::SearchRequest{query:"test".into(),..Default::default()};
                let result=engine.execute(&q,None,&b).await;
                assert!(result.is_err());drop(result);assert_eq!(b.snapshot().unwrap().used,0);
                http.shutdown_and_wait().await;
            }
            #[tokio::test]
            async fn dropping_actual_projection_future_releases_charged_scratch_and_rows(){
                let (engine,http)=engine();let b=super::super::clinicaltrials::Budget::new(32*1024*1024).unwrap();
                let q=ssx_domain::SearchRequest{query:"test".into(),..Default::default()};
                use super::super::source_payload::{BatchPhase,BATCH_PHASE};
                let phases=Arc::new(std::sync::Mutex::new(Vec::new()));let observed=phases.clone();
                let stop=if $engine=="nominatim"{"url"}else{"title"};
                let observer:Arc<dyn Fn(BatchPhase)->bool+Send+Sync>=Arc::new(move|phase|{
                    let mut events=observed.lock().unwrap();
                    let block=phase==BatchPhase::FieldCopied{retained_rows:1,field:stop};
                    if block {assert!(events.contains(&BatchPhase::Stored(1)),"first produced row must already be retained");}
                    events.push(phase);block
                });
                let mut future=Box::pin(BATCH_PHASE.scope(observer,engine.execute(&q,None,&b)));
                let target=BatchPhase::FieldCopied{retained_rows:1,field:stop};
                let end=std::time::Instant::now()+std::time::Duration::from_secs(3);
                while !phases.lock().unwrap().contains(&target) {
                    tokio::select!{_= &mut future=>panic!("second large row must suspend inside actual field projection"),_=tokio::task::yield_now()=>{}}
                    assert!(std::time::Instant::now()<end);
                }
                assert!(phases.lock().unwrap().contains(&BatchPhase::Stored(1)));
                assert!(!phases.lock().unwrap().contains(&BatchPhase::Stored(2)));
                assert!(b.snapshot().unwrap().used>0);
                drop(future);assert_eq!(b.snapshot().unwrap().used,0);
                http.shutdown_and_wait().await;
            }
        }
        impl ssx_application::SearchEngine for $name {
            fn search<'a>(&'a self,q:&'a ssx_domain::SearchRequest)->ssx_application::PortFuture<'a,ssx_domain::AdapterResponse> {Box::pin(self.run(q,None))}
            fn search_with_context<'a>(&'a self,q:&'a ssx_domain::SearchRequest,context:ssx_application::SearchContext)->ssx_application::PortFuture<'a,ssx_domain::AdapterResponse> {Box::pin(self.run(q,Some(context.deadline)))}
        }
    };
}
pub(super) use adapter;
