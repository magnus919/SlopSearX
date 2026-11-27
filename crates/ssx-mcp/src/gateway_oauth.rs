//! OAuth support for the installed MCP gateway.

mod authorizer;
mod callback;
mod protocol;
mod storage;

pub use authorizer::{AUTHORIZATION_TIMEOUT, GatewayOAuth};
