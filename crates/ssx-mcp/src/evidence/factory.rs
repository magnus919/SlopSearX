//! Concrete attachment/factories for the existing caller-owned native service.
//! W09/T02 owns executable startup and shutdown ordering around these transports.
use super::{EvidenceInputs, EvidenceRuntime, authority::EvidenceAuthority};
use crate::{
    harness::ManagedRouter,
    oauth::NetworkAuth,
    search_surface::{SearchState, SearchSurface},
};
use ssx_infra::{search::SearchClock, valkey::Valkey};
use std::{sync::Arc, time::Duration};
use tokio_util::sync::CancellationToken;

#[derive(Clone)]
pub struct EvidenceFactory {
    surface: SearchSurface,
}
impl EvidenceFactory {
    /// All inputs are trusted owner capabilities, never request/config objects.
    /// The caller supplies its SAME backend, clock and effective-policy source.
    /// Both surfaces borrow the exact SearchState snapshot service. This creates
    /// no new SearchService, Valkey connection, scheduler or lifecycle owner.
    pub fn new(
        state: SearchState,
        backend: Arc<Valkey>,
        authority: Arc<dyn EvidenceAuthority>,
        clock: Arc<dyn SearchClock>,
    ) -> Result<Self, &'static str> {
        let evidence = Arc::new(EvidenceRuntime::new(EvidenceInputs {
            backend,
            snapshots: state.snapshots.clone(),
            authority,
            clock,
        }));
        Ok(Self {
            surface: SearchSurface::new(state).with_evidence(evidence)?,
        })
    }
    pub fn surface(&self) -> &SearchSurface {
        &self.surface
    }
    pub fn http(&self, auth: NetworkAuth, stop: CancellationToken) -> ManagedRouter {
        crate::harness::http_router_for(self.surface.clone(), self.surface.runtime_catalogs(), stop)
            .map(|router| auth.mount(router))
    }
    pub fn legacy(
        &self,
        auth: NetworkAuth,
        sse_path: &str,
        message_path: &str,
        ttl: Duration,
        stop: CancellationToken,
    ) -> crate::Result<axum::Router> {
        let surface = self.surface.clone();
        let legacy = crate::legacy::Legacy::with_factory(
            Arc::new(move |wire, client| surface.clone().bind_connection(wire, client)),
            self.surface.runtime_catalogs(),
            message_path,
            ttl,
            stop,
        )?;
        Ok(auth.mount(legacy.router(sse_path)?))
    }
    /// Trusted stdio uses the existing factory's exact original-request binding
    /// and waits for every owned handler before returning to the caller owner.
    pub async fn stdio(&self, stop: CancellationToken) -> crate::Result<()> {
        let surface = self.surface.clone();
        crate::opaque::stdio_with_factory_owned(
            move |wire| surface.bind_connection(wire, None),
            self.surface.runtime_catalogs(),
            stop,
        )
        .await
    }
}
