//! Native client-side gateway configuration, OAuth and owned startup.
pub mod settings;

pub mod token_storage;

pub mod callback;

pub mod io_owner;
pub mod token_json;

pub mod token_model;

pub mod client_model;

pub mod arguments;
pub mod browser;
pub mod network;
pub mod oauth;
pub mod oauth_policy;
pub mod redirect;
pub mod runtime;
pub mod source_projection;
pub mod title;

#[cfg(unix)]
pub(crate) mod stdio_input;

#[cfg(windows)]
pub(crate) mod stdio_input_windows;

#[cfg(any(unix, windows))]
pub mod resolver;

#[cfg(feature = "gateway-source-endpoints")]
pub(crate) mod source_discovery;
