//! Gateway-owned native DNS. Ordinary Reqwest clients keep their resolver.
#![forbid(unsafe_code)]
mod helper;
#[cfg(unix)]
pub mod install;
#[cfg(windows)]
#[path = "install_windows.rs"]
pub mod install;
#[cfg(unix)]
#[path = "unix_process.rs"]
mod process;
#[cfg(windows)]
#[path = "windows_process.rs"]
mod process;
mod protocol;
mod runtime;
#[cfg(feature = "gateway-source-endpoints")]
mod source_adapter;
pub use protocol::{SourceResolveError, SourceResolveStage};
pub use runtime::{SourceCloseError, SourceResolverHelper, SourceRuntime};
use std::{sync::Arc, time::Duration};

pub fn helper_main() -> std::io::Result<()> {
    helper::main()
}
#[cfg(feature = "gateway-dns-fixtures")]
#[doc(hidden)]
pub fn fixture_main() -> std::io::Result<()> {
    helper::fixture_main()
}

/// The network owner retains this runtime independently of all lookup futures.
/// Request Drop cancels a registered lookup; only awaited close proves retirement.
#[derive(Clone)]
pub struct GatewayResolver {
    runtime: SourceRuntime,
    budget: Duration,
}
impl GatewayResolver {
    pub fn new(helper: SourceResolverHelper, budget: Duration) -> Arc<Self> {
        Arc::new(Self {
            runtime: SourceRuntime::new(helper),
            budget,
        })
    }
    pub async fn close(&self) -> Result<(), SourceCloseError> {
        self.runtime.close().await
    }
    #[cfg(feature = "gateway-dns-fixtures")]
    #[doc(hidden)]
    pub fn runtime(&self) -> &SourceRuntime {
        &self.runtime
    }
}
impl reqwest::dns::Resolve for GatewayResolver {
    #[cfg(feature = "gateway-source-endpoints")]
    fn resolve_source(&self, name: reqwest::dns::SourceName) -> reqwest::dns::SourceResolving {
        source_adapter::resolve(self.runtime.clone(), name)
    }

    fn resolve(&self, name: reqwest::dns::Name) -> reqwest::dns::Resolving {
        let runtime = self.runtime.clone();
        let host = name.as_str().to_owned();
        let budget = self.budget;
        Box::pin(async move {
            let addresses = runtime.resolve_dns(&host, budget).await?;
            Ok(Box::new(addresses.into_iter()) as reqwest::dns::Addrs)
        })
    }
}

#[cfg(all(windows, feature = "gateway-dns-fixtures"))]
mod windows_fixture;

#[cfg(all(windows, feature = "gateway-dns-fixtures"))]
#[doc(hidden)]
pub use super::stdio_input_windows::{
    WindowsInputControl, fixture_control as windows_input_fixture_control,
};

#[cfg(all(feature = "gateway-source-endpoints", feature = "gateway-dns-fixtures"))]
#[doc(hidden)]
pub async fn source_scope_fixture_controls(runtime: SourceRuntime) -> crate::Result<()> {
    runtime::fixture_controls(runtime).await
}
