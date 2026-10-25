pub use self::body::Body;
pub use self::client::{Client, ClientBuilder};
pub use self::request::{Request, RequestBuilder};
pub use self::response::Response;
pub use self::upgrade::Upgraded;

pub mod body;
pub mod client;
pub mod h3_client;
#[cfg(feature = "multipart")]
pub mod multipart;
pub(crate) mod request;
mod response;
mod upgrade;

#[cfg(feature = "ssx-http1-source-pool")]
pub use self::{
    request::{SourceRequest, SourceRequestBuilder},
    response::SourceResponse,
};
