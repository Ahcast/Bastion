use std::{net::SocketAddr, sync::Arc, time::Duration};

use bastion_mcp::{
    ClientMetadata, validate_client_identifier, validate_client_metadata,
    validate_public_dns_candidates,
};
use bytes::Bytes;
use http_body_util::{BodyExt, Empty};
use hyper::{
    Request, StatusCode,
    body::Body,
    client::conn::http1,
    header::{ACCEPT, CONNECTION, CONTENT_ENCODING, CONTENT_LENGTH, CONTENT_TYPE, HOST},
};
use hyper_util::rt::TokioIo;
use rustls::{ClientConfig, RootCertStore, pki_types::ServerName};
use tokio::{
    net::{TcpStream, lookup_host},
    task::JoinHandle,
};
use tokio_rustls::TlsConnector;
use url::{Host, Position, Url};

const MAX_BYTES: usize = 64 * 1024;

#[derive(Debug)]
pub enum MetadataFetchError {
    InvalidClient,
    UnsafeDestination,
    Unavailable,
    InvalidResponse,
}

/// Draft CIMD fetcher. It resolves and validates the entire answer set,
/// connects directly to a validated address, verifies TLS for the original
/// hostname and never uses proxy settings or follows redirects. Do not expose
/// it before network egress rules and adversarial end-to-end tests are in place.
pub struct DraftMetadataFetcher {
    tls: Arc<ClientConfig>,
    timeout: Duration,
}

impl DraftMetadataFetcher {
    pub fn new(timeout: Duration) -> Result<Self, MetadataFetchError> {
        if timeout.is_zero() || timeout > Duration::from_secs(30) {
            return Err(MetadataFetchError::InvalidClient);
        }
        let roots = RootCertStore::from_iter(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
        let provider = Arc::new(rustls::crypto::ring::default_provider());
        let tls = ClientConfig::builder_with_provider(provider)
            .with_safe_default_protocol_versions()
            .map_err(|_| MetadataFetchError::Unavailable)?
            .with_root_certificates(roots)
            .with_no_client_auth();
        Ok(Self {
            tls: Arc::new(tls),
            timeout,
        })
    }

    /// Never log `client_id` or response contents. Client metadata is returned
    /// for administrative review and does not itself approve a client.
    pub async fn fetch(&self, client_id: &str) -> Result<ClientMetadata, MetadataFetchError> {
        let url = validated_url(client_id)?;
        tokio::time::timeout(self.timeout, self.fetch_inner(client_id, &url))
            .await
            .map_err(|_| MetadataFetchError::Unavailable)?
    }

    async fn fetch_inner(
        &self,
        client_id: &str,
        url: &Url,
    ) -> Result<ClientMetadata, MetadataFetchError> {
        let Host::Domain(hostname) = url.host().ok_or(MetadataFetchError::InvalidClient)? else {
            return Err(MetadataFetchError::InvalidClient);
        };
        let addresses: Vec<SocketAddr> = lookup_host((hostname, 443))
            .await
            .map_err(|_| MetadataFetchError::Unavailable)?
            .take(17)
            .collect();
        let candidates = validate_public_dns_candidates(client_id, &addresses)
            .map_err(|_| MetadataFetchError::UnsafeDestination)?;
        self.fetch_pinned(
            client_id,
            url,
            candidates.hostname(),
            candidates.addresses(),
        )
        .await
    }

    async fn fetch_pinned(
        &self,
        client_id: &str,
        url: &Url,
        hostname: &str,
        addresses: &[SocketAddr],
    ) -> Result<ClientMetadata, MetadataFetchError> {
        let mut connected = None;
        for address in addresses {
            if let Ok(stream) = TcpStream::connect(address).await {
                connected = Some(stream);
                break;
            }
        }
        let stream = connected.ok_or(MetadataFetchError::Unavailable)?;
        let server_name = ServerName::try_from(hostname.to_owned())
            .map_err(|_| MetadataFetchError::InvalidClient)?;
        let tls = TlsConnector::from(self.tls.clone())
            .connect(server_name, stream)
            .await
            .map_err(|_| MetadataFetchError::Unavailable)?;
        let (mut sender, connection) = http1::Builder::new()
            .max_buf_size(16 * 1024)
            .handshake(TokioIo::new(tls))
            .await
            .map_err(|_| MetadataFetchError::Unavailable)?;
        let _guard = ConnectionGuard(tokio::spawn(async move {
            let _ = connection.await;
        }));
        let target = &url[Position::BeforePath..];
        let request = Request::builder()
            .method("GET")
            .uri(target)
            .header(HOST, hostname)
            .header(ACCEPT, "application/json")
            .header(CONNECTION, "close")
            .body(Empty::<Bytes>::new())
            .map_err(|_| MetadataFetchError::InvalidClient)?;
        let response = sender
            .send_request(request)
            .await
            .map_err(|_| MetadataFetchError::Unavailable)?;
        validate_response_headers(response.status(), response.headers())?;
        let bytes = read_body(response.into_body()).await?;
        validate_client_metadata(client_id, client_id, 200, &bytes)
            .map_err(|_| MetadataFetchError::InvalidResponse)
    }
}

struct ConnectionGuard(JoinHandle<()>);
impl Drop for ConnectionGuard {
    fn drop(&mut self) {
        self.0.abort();
    }
}

fn validated_url(client_id: &str) -> Result<Url, MetadataFetchError> {
    validate_client_identifier(client_id).map_err(|_| MetadataFetchError::InvalidClient)?;
    let url = Url::parse(client_id).map_err(|_| MetadataFetchError::InvalidClient)?;
    let Some(Host::Domain(hostname)) = url.host() else {
        return Err(MetadataFetchError::InvalidClient);
    };
    let bare = hostname.trim_end_matches('.');
    if url.as_str() != client_id
        || url.port_or_known_default() != Some(443)
        || bare.eq_ignore_ascii_case("localhost")
        || bare.ends_with(".localhost")
    {
        return Err(MetadataFetchError::InvalidClient);
    }
    Ok(url)
}

fn validate_response_headers(
    status: StatusCode,
    headers: &hyper::HeaderMap,
) -> Result<(), MetadataFetchError> {
    if status != StatusCode::OK
        || headers.get_all(CONTENT_TYPE).iter().count() != 1
        || headers.get_all(CONTENT_LENGTH).iter().count() > 1
        || headers.get_all(CONTENT_ENCODING).iter().count() > 1
    {
        return Err(MetadataFetchError::InvalidResponse);
    }
    let mime = headers
        .get(CONTENT_TYPE)
        .and_then(|h| h.to_str().ok())
        .and_then(|h| h.split(';').next())
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    if mime != "application/json" && !(mime.starts_with("application/") && mime.ends_with("+json"))
    {
        return Err(MetadataFetchError::InvalidResponse);
    }
    if headers.get(CONTENT_ENCODING).is_some_and(|v| {
        v.to_str()
            .ok()
            .is_none_or(|s| !s.eq_ignore_ascii_case("identity"))
    }) {
        return Err(MetadataFetchError::InvalidResponse);
    }
    if let Some(length) = headers.get(CONTENT_LENGTH) {
        if length
            .to_str()
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
            .is_none_or(|n| n == 0 || n > MAX_BYTES as u64)
        {
            return Err(MetadataFetchError::InvalidResponse);
        }
    }
    Ok(())
}

async fn read_body<B: Body<Data = Bytes> + Unpin>(
    mut body: B,
) -> Result<Vec<u8>, MetadataFetchError> {
    let mut output = Vec::new();
    while let Some(frame) = body.frame().await {
        let frame = frame.map_err(|_| MetadataFetchError::Unavailable)?;
        let data = frame
            .into_data()
            .map_err(|_| MetadataFetchError::InvalidResponse)?;
        if data.len() > MAX_BYTES.saturating_sub(output.len()) {
            return Err(MetadataFetchError::InvalidResponse);
        }
        output.extend_from_slice(&data);
    }
    if output.is_empty() {
        return Err(MetadataFetchError::InvalidResponse);
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_redirects_compression_oversize_and_noncanonical_identifiers() {
        let mut headers = hyper::HeaderMap::new();
        headers.insert(CONTENT_TYPE, "application/json".parse().unwrap());
        assert!(validate_response_headers(StatusCode::OK, &headers).is_ok());
        assert!(validate_response_headers(StatusCode::FOUND, &headers).is_err());
        headers.insert(CONTENT_ENCODING, "gzip".parse().unwrap());
        assert!(validate_response_headers(StatusCode::OK, &headers).is_err());
        headers.remove(CONTENT_ENCODING);
        headers.insert(CONTENT_LENGTH, "65537".parse().unwrap());
        assert!(validate_response_headers(StatusCode::OK, &headers).is_err());
        assert!(validated_url("https://client.example/metadata.json").is_ok());
        for url in [
            "http://client.example/doc",
            "https://localhost/doc",
            "https://127.0.0.1/doc",
            "https://CLIENT.example/doc",
            "https://client.example:8443/doc",
        ] {
            assert!(validated_url(url).is_err());
        }
    }
    #[tokio::test]
    async fn enforces_actual_body_length_even_without_content_length() {
        use http_body_util::Full;
        assert_eq!(
            read_body(Full::new(Bytes::from_static(b"{}")))
                .await
                .unwrap(),
            b"{}"
        );
        assert!(read_body(Full::new(Bytes::new())).await.is_err());
        assert!(
            read_body(Full::new(Bytes::from(vec![0u8; MAX_BYTES + 1])))
                .await
                .is_err()
        );
    }

    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn pinned_https_verifies_hostname_and_rejects_redirects() {
        use rustls::{
            ServerConfig,
            pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer},
        };
        use std::{
            path::PathBuf,
            process::{Command, Stdio},
        };
        use tokio::{
            io::{AsyncReadExt, AsyncWriteExt},
            net::TcpListener,
        };
        use tokio_rustls::TlsAcceptor;

        struct Temp(PathBuf);
        impl Drop for Temp {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
        let dir = std::env::temp_dir().join(format!(
            "bastion-mcp-tls-{}",
            bastion_mcp::Credential::generate().unwrap().expose()
        ));
        std::fs::create_dir(&dir).unwrap();
        let dir = Temp(dir);
        let run = |args: &[&str]| {
            let status = Command::new("openssl")
                .current_dir(&dir.0)
                .args(args)
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .unwrap();
            assert!(status.success());
        };
        run(&[
            "req",
            "-x509",
            "-newkey",
            "ec",
            "-pkeyopt",
            "ec_paramgen_curve:P-256",
            "-nodes",
            "-keyout",
            "key.pem",
            "-out",
            "cert.pem",
            "-days",
            "1",
            "-subj",
            "/CN=metadata.example",
            "-addext",
            "subjectAltName=DNS:metadata.example",
            "-addext",
            "basicConstraints=critical,CA:FALSE",
            "-addext",
            "keyUsage=critical,digitalSignature",
            "-addext",
            "extendedKeyUsage=serverAuth",
        ]);
        run(&[
            "x509", "-in", "cert.pem", "-outform", "DER", "-out", "cert.der",
        ]);
        run(&[
            "pkcs8", "-topk8", "-inform", "PEM", "-outform", "DER", "-nocrypt", "-in", "key.pem",
            "-out", "key.der",
        ]);
        let cert = CertificateDer::from(std::fs::read(dir.0.join("cert.der")).unwrap());
        let key = PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(
            std::fs::read(dir.0.join("key.der")).unwrap(),
        ));
        let provider = Arc::new(rustls::crypto::ring::default_provider());
        let server = ServerConfig::builder_with_provider(provider.clone())
            .with_safe_default_protocol_versions()
            .unwrap()
            .with_no_client_auth()
            .with_single_cert(vec![cert.clone()], key)
            .unwrap();
        let mut roots = RootCertStore::empty();
        roots.add(cert).unwrap();
        let client = ClientConfig::builder_with_provider(provider)
            .with_safe_default_protocol_versions()
            .unwrap()
            .with_root_certificates(roots)
            .with_no_client_auth();
        let fetcher = DraftMetadataFetcher {
            tls: Arc::new(client),
            timeout: Duration::from_secs(5),
        };
        let acceptor = TlsAcceptor::from(Arc::new(server));
        async fn serve(
            acceptor: TlsAcceptor,
            status: &'static str,
            body: String,
        ) -> (SocketAddr, JoinHandle<()>) {
            let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let address = listener.local_addr().unwrap();
            let handle = tokio::spawn(async move {
                let (stream, _) = listener.accept().await.unwrap();
                let Ok(mut tls) = acceptor.accept(stream).await else {
                    return;
                };
                let mut bytes = Vec::new();
                let mut chunk = [0u8; 1024];
                while !bytes.windows(4).any(|part| part == b"\r\n\r\n") {
                    let n = tls.read(&mut chunk).await.unwrap();
                    if n == 0 || bytes.len() + n > 8192 {
                        return;
                    }
                    bytes.extend_from_slice(&chunk[..n]);
                }
                assert!(
                    String::from_utf8_lossy(&bytes)
                        .to_ascii_lowercase()
                        .contains("host: metadata.example")
                );
                let response = format!(
                    "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                tls.write_all(response.as_bytes()).await.unwrap();
            });
            (address, handle)
        }
        let id = "https://metadata.example/doc";
        let url = validated_url(id).unwrap();
        let body=serde_json::json!({"client_id":id,"redirect_uris":["https://client.example/cb"],"token_endpoint_auth_method":"none"}).to_string();
        let (address, server) = serve(acceptor.clone(), "200 OK", body.clone()).await;
        let value = tokio::time::timeout(
            Duration::from_secs(5),
            fetcher.fetch_pinned(id, &url, "metadata.example", &[address]),
        )
        .await
        .unwrap()
        .unwrap();
        assert_eq!(value.client_id(), id);
        server.await.unwrap();
        let (address, server) = serve(acceptor.clone(), "302 Found", body.clone()).await;
        assert!(
            tokio::time::timeout(
                Duration::from_secs(5),
                fetcher.fetch_pinned(id, &url, "metadata.example", &[address])
            )
            .await
            .unwrap()
            .is_err()
        );
        server.await.unwrap();
        let (address, server) = serve(acceptor, "200 OK", body).await;
        assert!(
            tokio::time::timeout(
                Duration::from_secs(5),
                fetcher.fetch_pinned(id, &url, "wrong.example", &[address])
            )
            .await
            .unwrap()
            .is_err()
        );
        server.await.unwrap();
    }
}
