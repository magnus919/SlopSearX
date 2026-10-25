//! Explicit opt-in source endpoint adapter; ordinary DNS remains unchanged.
#![forbid(unsafe_code)]
use super::{
    protocol,
    runtime::{SourceFlow, SourceRuntime},
};
use reqwest::dns::{
    SourceCandidate, SourceCandidateResolver, SourceCandidateResolving, SourceFamily, SourceName,
    SourceResolution, SourceResolutionFuture, SourceResolving, SourceRetiring,
};
use std::sync::{Arc, Mutex};
type Error = Box<dyn std::error::Error + Send + Sync>;
fn input_error() -> Error {
    Box::new(protocol::SourceResolveError::new(
        protocol::SourceResolveStage::InputContract,
    ))
}
struct Owner {
    name: SourceName,
    flow: SourceFlow,
    candidates: Mutex<Option<Vec<protocol::Candidate>>>,
}
impl Drop for Owner {
    fn drop(&mut self) {
        self.flow.cancel();
    }
}
// Exists BEFORE boxing/polling the first GAI future. Even dropping an unpolled
// phase immediately cancels this scope, while Runtime retains original joins.
pub(super) struct PreparationDrop(Option<SourceFlow>);
pub(super) fn preparation_guard(flow: SourceFlow) -> PreparationDrop {
    PreparationDrop(Some(flow))
}
impl Drop for PreparationDrop {
    fn drop(&mut self) {
        if let Some(flow) = &self.0 {
            flow.cancel();
        }
    }
}
impl SourceCandidateResolver for Owner {
    fn resolve_candidate(
        &self,
        name: SourceName,
        candidate: SourceCandidate,
    ) -> SourceCandidateResolving {
        let same = Arc::ptr_eq(&self.name.identity().0, &name.identity().0)
            && self.name.deadline() == name.deadline();
        let row = protocol::Candidate {
            family: match candidate.family {
                SourceFamily::V4 => protocol::Family::V4,
                SourceFamily::V6 => protocol::Family::V6,
            },
            native_host_text: candidate.native_host_text,
        };
        let allowed = self
            .candidates
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .as_ref()
            .is_some_and(|v| v.contains(&row));
        let flow = self.flow.clone();
        let decimal = self.name.origin_port().map(str::to_owned);
        Box::pin(async move {
            if !same || !allowed {
                return Err(input_error());
            }
            let decimal = decimal.ok_or_else(input_error)?;
            flow.resolve_candidate(&row, &decimal)
                .await
                .map_err(|e| Box::new(e) as Error)
        })
    }
    fn cancel_and_wait(&self) -> SourceRetiring {
        cancel_flow(self.flow.clone())
    }
}
pub(super) fn resolve(runtime: SourceRuntime, name: SourceName) -> SourceResolving {
    let flow = runtime.source_flow(name.deadline());
    let owner = Arc::new(Owner {
        name: name.clone(),
        flow: flow.clone(),
        candidates: Mutex::new(None),
    });
    let external: Arc<dyn SourceCandidateResolver> = owner.clone();
    let guard = preparation_guard(flow);
    let future: SourceResolutionFuture = Box::pin(async move {
        let mut guard = guard;
        let decimal = name.origin_port().ok_or_else(input_error)?;
        let rows = owner
            .flow
            .prepare(name.raw_host(), decimal)
            .await
            .map_err(|e| Box::new(e) as Error)?;
        *owner.candidates.lock().unwrap_or_else(|e| e.into_inner()) = Some(rows.clone());
        let candidates = rows
            .into_iter()
            .map(|r| SourceCandidate {
                family: match r.family {
                    protocol::Family::V4 => SourceFamily::V4,
                    protocol::Family::V6 => SourceFamily::V6,
                },
                native_host_text: r.native_host_text,
            })
            .collect();
        let resolver: Arc<dyn SourceCandidateResolver> = owner;
        guard.0 = None;
        Ok(SourceResolution {
            candidates,
            resolver,
        })
    });
    // Lower connector retains this SAME Arc outside its absolute-deadline phase
    // and always awaits it after winner/error/timeout, including first GAI.
    SourceResolving {
        future,
        owner: external,
    }
}

// Shared implementation of the trait's synchronous cancellation contract.
fn cancel_flow(flow: SourceFlow) -> SourceRetiring {
    flow.cancel();
    Box::pin(async move {
        flow.cancel_and_wait()
            .await
            .map_err(|e| Box::new(e) as Error)
    })
}
#[cfg(feature = "gateway-dns-fixtures")]
pub(super) async fn synchronous_cancel_control(runtime: SourceRuntime) {
    let flow =
        runtime.source_flow(tokio::time::Instant::now() + std::time::Duration::from_secs(20));
    let unpolled = cancel_flow(flow.clone());
    assert_eq!(
        flow.prepare("127.0.0.1", "80").await.unwrap_err().stage,
        protocol::SourceResolveStage::Cancelled,
        "trait cancellation implementation runs before its returned future is polled"
    );
    drop(unpolled);
    cancel_flow(flow).await.unwrap();
}
