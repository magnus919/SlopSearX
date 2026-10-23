//! Owned loopback callback. Explicit shutdown joins; Drop only requests cleanup.
use axum::{
    Router,
    extract::{RawQuery, State},
    routing::get,
};
use std::{
    net::{Ipv4Addr, SocketAddr},
    time::Duration,
};
use tokio::{net::TcpListener, sync::watch, task::JoinHandle};
use tokio_util::sync::CancellationToken;
#[derive(Clone, PartialEq, Eq)]
pub struct AuthorizationCode {
    pub code: String,
    pub state: Option<String>,
}
impl std::fmt::Debug for AuthorizationCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("AuthorizationCode(<redacted>)")
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CallbackError {
    InvalidPort,
    Bind,
    Deadline,
    Closed,
    Server,
}
type CallbackState = watch::Sender<Option<AuthorizationCode>>;
pub struct LoopbackCallback {
    configured_port: u16,
    actual_address: SocketAddr,
    timeout: Duration,
    result: watch::Receiver<Option<AuthorizationCode>>,
    stop: CancellationToken,
    task: Option<JoinHandle<std::io::Result<()>>>,
}
impl std::fmt::Debug for LoopbackCallback {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LoopbackCallback")
            .field("address", &self.actual_address)
            .finish_non_exhaustive()
    }
}
async fn receive(State(state): State<CallbackState>, RawQuery(query): RawQuery) -> &'static str {
    // Starlette QueryParams.get selects the last repeated query parameter.
    let mut code = None;
    let mut nonce = None;
    for (key, value) in url::form_urlencoded::parse(query.as_deref().unwrap_or_default().as_bytes())
    {
        match key.as_ref() {
            "code" => code = Some(value.into_owned()),
            "state" => nonce = Some(value.into_owned()),
            _ => {}
        }
    }
    state.send_replace(Some(AuthorizationCode {
        code: code.unwrap_or_default(),
        state: nonce,
    }));
    "Authorization complete — you can close this tab and return to the gateway."
}
impl LoopbackCallback {
    /// Binds before returning, so callers cannot open the authorization URL
    /// before the callback listener exists. No browser is opened by this module.
    pub async fn start(port: &str, timeout: Duration) -> Result<Self, CallbackError> {
        let configured_port = port.parse().map_err(|_| CallbackError::InvalidPort)?;
        let listener = tokio::time::timeout(
            Duration::from_secs(15),
            TcpListener::bind((Ipv4Addr::LOCALHOST, configured_port)),
        )
        .await
        .map_err(|_| CallbackError::Deadline)?
        .map_err(|_| CallbackError::Bind)?;
        let actual_address = listener.local_addr().map_err(|_| CallbackError::Bind)?;
        let (state, result) = watch::channel(None);
        let stop = CancellationToken::new();
        let stopped = stop.clone();
        let app = Router::new()
            .route("/callback", get(receive))
            .with_state(state);
        let task = tokio::spawn(async move {
            axum::serve(listener, app)
                .with_graceful_shutdown(stopped.cancelled_owned())
                .await
        });
        Ok(Self {
            configured_port,
            actual_address,
            timeout,
            result,
            stop,
            task: Some(task),
        })
    }
    pub fn redirect_uri(&self) -> String {
        format!("http://127.0.0.1:{}/callback", self.configured_port)
    }
    /// Actual address is needed only by private port-zero qualification.
    pub fn address(&self) -> SocketAddr {
        self.actual_address
    }
    pub async fn wait(&mut self) -> Result<AuthorizationCode, CallbackError> {
        // Source checks its deadline before inspecting an already-received code.
        if self.timeout.is_zero() {
            return Err(CallbackError::Deadline);
        }
        let wait = async {
            loop {
                if let Some(value) = self.result.borrow_and_update().clone() {
                    return Ok(value);
                }
                self.result
                    .changed()
                    .await
                    .map_err(|_| CallbackError::Closed)?;
            }
        };
        tokio::select! {biased;
            _=self.stop.cancelled()=>Err(CallbackError::Closed),
            result=tokio::time::timeout(self.timeout,wait)=>result.map_err(|_|CallbackError::Deadline)?,
        }
    }
    pub async fn shutdown(&mut self) -> Result<(), CallbackError> {
        self.stop.cancel();
        let Some(task) = self.task.as_mut() else {
            return Ok(());
        };
        let result = match tokio::time::timeout(Duration::from_secs(5), &mut *task).await {
            Ok(result) => match result {
                Ok(result) => result.map_err(|_| CallbackError::Server),
                Err(_) => Err(CallbackError::Server),
            },
            Err(_) => {
                task.abort();
                let _ = task.await;
                Err(CallbackError::Deadline)
            }
        };
        self.task.take();
        result
    }
}
impl Drop for LoopbackCallback {
    fn drop(&mut self) {
        self.stop.cancel();
        if let Some(task) = &self.task {
            task.abort();
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn actual_loopback_callback_preserves_last_parameters_missing_empty_and_shutdown() {
        let mut callback = LoopbackCallback::start("0", Duration::from_secs(2))
            .await
            .unwrap();
        let address = callback.address();
        let client = reqwest::Client::builder()
            .no_proxy()
            .timeout(Duration::from_secs(2))
            .build()
            .unwrap();
        for (query, expected) in [
            (
                "code=first&code=last&state=old&state=",
                AuthorizationCode {
                    code: "last".into(),
                    state: Some("".into()),
                },
            ),
            (
                "other=ignored",
                AuthorizationCode {
                    code: "".into(),
                    state: None,
                },
            ),
            (
                "code=abc123&state=xyz",
                AuthorizationCode {
                    code: "abc123".into(),
                    state: Some("xyz".into()),
                },
            ),
        ] {
            let response = client
                .get(format!("http://{address}/callback?{query}"))
                .send()
                .await
                .unwrap();
            assert_eq!(response.status(), 200);
            assert_eq!(
                response.text().await.unwrap(),
                "Authorization complete — you can close this tab and return to the gateway."
            );
            assert_eq!(callback.wait().await.unwrap(), expected);
        }
        callback.shutdown().await.unwrap();
        assert!(callback.task.is_none());
        assert!(tokio::net::TcpStream::connect(address).await.is_err());
    }
    #[tokio::test]
    async fn actual_callback_deadline_joins_and_releases_listener() {
        let mut callback = LoopbackCallback::start("0", Duration::from_millis(10))
            .await
            .unwrap();
        let address = callback.address();
        assert_eq!(callback.wait().await, Err(CallbackError::Deadline));
        callback.shutdown().await.unwrap();
        assert!(callback.task.is_none());
        let rebound = TcpListener::bind(address).await.unwrap();
        drop(rebound);
    }
    #[tokio::test]
    async fn zero_timeout_rejects_received_and_unreceived_positive_wait_repeats() {
        let mut callback = LoopbackCallback::start("0", Duration::ZERO).await.unwrap();
        assert_eq!(callback.wait().await, Err(CallbackError::Deadline));
        let client = reqwest::Client::builder()
            .no_proxy()
            .timeout(Duration::from_secs(2))
            .build()
            .unwrap();
        let response = client
            .get(format!(
                "http://{}/callback?code=synthetic&state=nonce",
                callback.address()
            ))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 200);
        assert_eq!(
            response.text().await.unwrap(),
            "Authorization complete — you can close this tab and return to the gateway."
        );
        for _ in 0..2 {
            assert_eq!(callback.wait().await, Err(CallbackError::Deadline));
        }
        callback.timeout = Duration::from_secs(1);
        for _ in 0..2 {
            assert_eq!(
                callback.wait().await.unwrap(),
                AuthorizationCode {
                    code: "synthetic".into(),
                    state: Some("nonce".into())
                }
            );
        }
        callback.shutdown().await.unwrap();
        assert!(callback.task.is_none());
        assert!(
            tokio::net::TcpStream::connect(callback.address())
                .await
                .is_err()
        );
    }
}
