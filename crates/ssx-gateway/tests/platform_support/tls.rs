use crate::platform_support::{BUDGET, rpc};
use serde_json::Value;
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt},
    sync::oneshot,
    task::{JoinHandle, JoinSet},
};
pub struct Ca {
    pub certificate: rcgen::Certificate,
    pub params: rcgen::CertificateParams,
    pub key: Arc<rcgen::KeyPair>,
}
impl Ca {
    pub fn new() -> Self {
        let key = Arc::new(rcgen::KeyPair::generate().unwrap());
        let mut params = rcgen::CertificateParams::new(Vec::<String>::new()).unwrap();
        params.is_ca = rcgen::IsCa::Ca(rcgen::BasicConstraints::Unconstrained);
        params.key_usages = vec![
            rcgen::KeyUsagePurpose::KeyCertSign,
            rcgen::KeyUsagePurpose::DigitalSignature,
            rcgen::KeyUsagePurpose::CrlSign,
        ];
        params.distinguished_name.push(
            rcgen::DnType::CommonName,
            format!("SSX ephemeral platform CA {}", std::process::id()),
        );
        let certificate = params.self_signed(key.as_ref()).unwrap();
        Self {
            certificate,
            params,
            key,
        }
    }
    fn config(&self, name: &str) -> rustls::ServerConfig {
        let key = rcgen::KeyPair::generate().unwrap();
        let params = rcgen::CertificateParams::new(vec![name.into()]).unwrap();
        let issuer = rcgen::Issuer::from_params(&self.params, self.key.as_ref());
        let leaf = params.signed_by(&key, &issuer).unwrap();
        rustls::ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(
                vec![leaf.der().clone(), self.certificate.der().clone()],
                rustls::pki_types::PrivatePkcs8KeyDer::from(key.serialize_der()).into(),
            )
            .unwrap()
    }
}
#[derive(Debug)]
pub struct Request {
    pub head: String,
    pub value: Option<Value>,
}
pub async fn request(stream: &mut (impl AsyncRead + Unpin)) -> std::io::Result<Option<Request>> {
    let mut bytes = Vec::new();
    let end = loop {
        let byte = match stream.read_u8().await {
            Ok(byte) => byte,
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(None),
            Err(e) => return Err(e),
        };
        bytes.push(byte);
        if bytes.len() > 65536 {
            return Err(std::io::ErrorKind::InvalidData.into());
        }
        if bytes.ends_with(b"\r\n\r\n") {
            break bytes.len();
        }
    };
    let head = String::from_utf8(bytes).map_err(|_| std::io::ErrorKind::InvalidData)?;
    let count = head
        .lines()
        .skip(1)
        .filter_map(|line| line.split_once(':'))
        .find(|(key, _)| key.eq_ignore_ascii_case("content-length"))
        .map(|(_, value)| value.trim().parse::<usize>())
        .transpose()
        .map_err(|_| std::io::ErrorKind::InvalidData)?
        .unwrap_or(0);
    if count > 65536 {
        return Err(std::io::ErrorKind::InvalidData.into());
    }
    let mut body = vec![0; count];
    stream.read_exact(&mut body).await?;
    let value = if count == 0 {
        None
    } else {
        Some(serde_json::from_slice(&body).map_err(|_| std::io::ErrorKind::InvalidData)?)
    };
    assert_eq!(head.len(), end);
    Ok(Some(Request { head, value }))
}
pub async fn respond(
    stream: &mut (impl AsyncWrite + Unpin),
    request: &Request,
    redirect: Option<&str>,
) -> std::io::Result<()> {
    let (status, extra, body) = if let Some(url) = redirect {
        (
            "307 Temporary Redirect",
            format!("Location: {url}\r\n"),
            String::new(),
        )
    } else if request.head.starts_with("DELETE ") {
        ("204 No Content", String::new(), String::new())
    } else if let Some(value) = request.value.as_ref().and_then(rpc) {
        (
            "200 OK",
            "Content-Type: application/json\r\nmcp-session-id: platform-session\r\n".into(),
            value.to_string(),
        )
    } else {
        ("202 Accepted", String::new(), String::new())
    };
    stream
        .write_all(
            format!(
                "HTTP/1.1 {status}\r\n{extra}Connection: close\r\nContent-Length: {}\r\n\r\n{body}",
                body.len()
            )
            .as_bytes(),
        )
        .await?;
    stream.shutdown().await
}
pub struct Fixture {
    pub url: String,
    pub observed: Arc<Mutex<Vec<Request>>>,
    stop: Option<oneshot::Sender<()>>,
    task: Option<JoinHandle<()>>,
}
impl Fixture {
    pub async fn tls(ca: &Ca, name: &str, redirect: Option<String>) -> Self {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("https://{}/mcp", listener.local_addr().unwrap());
        let acceptor = tokio_rustls::TlsAcceptor::from(Arc::new(ca.config(name)));
        let observed = Arc::new(Mutex::new(Vec::new()));
        let record = observed.clone();
        let (stop, mut stopping) = oneshot::channel();
        let task = tokio::spawn(async move {
            let mut connections = JoinSet::new();
            loop {
                tokio::select! {
                    _=&mut stopping=>break,
                    joined=connections.join_next(), if !connections.is_empty()=>{ joined.unwrap().unwrap(); },
                    next=listener.accept()=>{
                        let (stream,_)=next.unwrap(); let acceptor=acceptor.clone(); let record=record.clone(); let redirect=redirect.clone();
                        connections.spawn(async move {
                            // An untrusted/hostname-negative handshake is expected to abort.
                            if let Ok(Ok(mut stream))=tokio::time::timeout(BUDGET,acceptor.accept(stream)).await {
                                if let Some(request)=tokio::time::timeout(BUDGET,request(&mut stream)).await.unwrap().unwrap() {
                                    respond(&mut stream,&request,redirect.as_deref()).await.unwrap(); record.lock().unwrap().push(request);
                                }
                            }
                        });
                    }
                }
            }
            connections.abort_all();
            while let Some(joined) = connections.join_next().await {
                if let Err(error) = joined {
                    assert!(error.is_cancelled());
                }
            }
        });
        Self {
            url,
            observed,
            stop: Some(stop),
            task: Some(task),
        }
    }
    pub async fn close(mut self) {
        self.stop.take().unwrap().send(()).unwrap();
        tokio::time::timeout(Duration::from_secs(5), self.task.take().unwrap())
            .await
            .unwrap()
            .unwrap();
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        if let Some(task) = self.task.take() {
            task.abort();
        }
    }
}
