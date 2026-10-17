//! Real native transport for foundation readiness; feature routes fail explicitly.
pub mod projection;
use axum::{Json, Router, extract::State, http::StatusCode, routing::get};
use ssx_application::Runtime;
use ssx_wire::Readiness;
pub fn router(runtime: Runtime) -> Router {
    Router::new()
        .route("/healthz", get(readiness))
        .fallback(|| async {
            (
                StatusCode::NOT_IMPLEMENTED,
                "Rust foundation: application surface not implemented",
            )
        })
        .with_state(runtime)
}
async fn readiness(State(runtime): State<Runtime>) -> (StatusCode, Json<Readiness>) {
    if runtime.probe().await {
        (StatusCode::OK, Json(Readiness::foundation()))
    } else {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(Readiness {
                status: "unavailable".into(),
                mode: "foundation".into(),
            }),
        )
    }
}

pub async fn serve(
    listener: tokio::net::TcpListener,
    runtime: Runtime,
) -> Result<(), std::io::Error> {
    axum::serve(listener, router(runtime))
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use tower::ServiceExt;
    #[tokio::test]
    async fn injected_state_reaches_real_router_and_features_fail_closed() {
        let runtime = ssx_infra::foundation_runtime();
        let response = router(runtime.clone())
            .oneshot(
                axum::http::Request::builder()
                    .uri("/healthz")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(response.into_body(), 1024)
            .await
            .unwrap();
        assert_eq!(&bytes[..], b"{\"status\":\"ok\",\"mode\":\"foundation\"}");
        let mut failed = runtime;
        failed.store = Arc::new(ssx_infra::MemoryStore(ssx_domain::StoreRead::Unavailable));
        let response = router(failed)
            .oneshot(
                axum::http::Request::builder()
                    .uri("/healthz")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        let response = router(ssx_infra::foundation_runtime())
            .oneshot(
                axum::http::Request::builder()
                    .uri("/search?q=test")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_IMPLEMENTED);
    }
}

pub mod browser_identity;

pub mod formats;
pub mod formatter_metadata;
pub mod native;
pub use native::{HtmlRenderer, HttpState, router as native_router, serve as native_serve};

pub mod discovery;

pub mod html;
