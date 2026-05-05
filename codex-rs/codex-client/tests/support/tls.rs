use std::io;
use std::io::Read;
use std::io::Write;
use std::net::TcpListener;
use std::net::TcpStream;
use std::sync::Arc;
use std::sync::mpsc;
use std::sync::mpsc::Receiver;
use std::thread;

use codex_utils_rustls_provider::ensure_rustls_crypto_provider;
use rcgen::BasicConstraints;
use rcgen::CertificateParams;
use rcgen::DistinguishedName;
use rcgen::DnType;
use rcgen::ExtendedKeyUsagePurpose;
use rcgen::IsCa;
use rcgen::Issuer;
use rcgen::KeyPair;
use rcgen::KeyUsagePurpose;
use rcgen::PKCS_ECDSA_P256_SHA256;
use rustls::ServerConfig;
use rustls::ServerConnection;
use rustls::StreamOwned;
use rustls_pki_types::CertificateDer;
use rustls_pki_types::PrivateKeyDer;
use rustls_pki_types::pem::PemObject;

use super::http::read_http_request;
use super::http::write_ok_response;

pub struct Tls13TestServer {
    pub ca_cert_pem: String,
    pub request_rx: Receiver<Result<String, String>>,
    pub url: String,
}

pub struct TlsInterceptingProxy {
    pub ca_cert_pem: String,
    pub request_rx: Receiver<Result<String, String>>,
    pub url: String,
}

struct TestCa {
    cert_pem: String,
    issuer: Issuer<'static, KeyPair>,
}

pub fn spawn_tls13_test_server() -> Tls13TestServer {
    let ca = TestCa::new("codex-test-server-ca");
    let server_config = ca.server_config_for_localhost();
    let listener = TcpListener::bind(("127.0.0.1", 0))
        .unwrap_or_else(|error| panic!("bind TLS test server failed: {error}"));
    let port = listener
        .local_addr()
        .unwrap_or_else(|error| panic!("read TLS test server addr failed: {error}"))
        .port();
    let (request_tx, request_rx) = mpsc::channel();

    thread::spawn(move || {
        let result = accept_tls_http_request(listener, server_config);
        let _ = request_tx.send(result);
    });

    Tls13TestServer {
        ca_cert_pem: ca.cert_pem,
        request_rx,
        url: format!("https://localhost:{port}/oauth/token"),
    }
}

pub fn spawn_tls_intercepting_proxy() -> TlsInterceptingProxy {
    let ca = TestCa::new("codex-test-proxy-ca");
    let server_config = ca.server_config_for_host("token-origin.test");
    let listener = TcpListener::bind(("127.0.0.1", 0))
        .unwrap_or_else(|error| panic!("bind TLS proxy failed: {error}"));
    let port = listener
        .local_addr()
        .unwrap_or_else(|error| panic!("read TLS proxy addr failed: {error}"))
        .port();
    let (request_tx, request_rx) = mpsc::channel();

    thread::spawn(move || {
        let result = accept_connect_then_tls_http(listener, server_config);
        let _ = request_tx.send(result);
    });

    TlsInterceptingProxy {
        ca_cert_pem: ca.cert_pem,
        request_rx,
        url: format!("http://127.0.0.1:{port}"),
    }
}

impl TestCa {
    fn new(common_name: &str) -> Self {
        let mut params = CertificateParams::default();
        let mut distinguished_name = DistinguishedName::new();
        distinguished_name.push(DnType::CommonName, common_name);
        params.distinguished_name = distinguished_name;
        params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
        params.key_usages = vec![
            KeyUsagePurpose::DigitalSignature,
            KeyUsagePurpose::KeyCertSign,
            KeyUsagePurpose::CrlSign,
        ];

        let key = KeyPair::generate_for(&PKCS_ECDSA_P256_SHA256)
            .unwrap_or_else(|error| panic!("generate CA key failed: {error}"));
        let cert = params
            .self_signed(&key)
            .unwrap_or_else(|error| panic!("generate CA cert failed: {error}"));
        let cert_pem = cert.pem();
        let issuer = Issuer::new(params, key);
        Self { cert_pem, issuer }
    }

    fn server_config_for_localhost(&self) -> Arc<ServerConfig> {
        self.server_config_for_host("localhost")
    }

    fn server_config_for_host(&self, host: &str) -> Arc<ServerConfig> {
        let (cert_pem, key_pem) = self.issue_server_cert(host);
        Arc::new(
            server_config_from_pem(&cert_pem, &key_pem)
                .unwrap_or_else(|error| panic!("build server config failed: {error}")),
        )
    }

    fn issue_server_cert(&self, host: &str) -> (String, String) {
        let mut params = CertificateParams::new(vec![host.to_string()])
            .unwrap_or_else(|error| panic!("create cert params failed for {host}: {error}"));
        params.extended_key_usages = vec![ExtendedKeyUsagePurpose::ServerAuth];
        params.key_usages = vec![
            KeyUsagePurpose::DigitalSignature,
            KeyUsagePurpose::KeyEncipherment,
        ];

        let key = KeyPair::generate_for(&PKCS_ECDSA_P256_SHA256)
            .unwrap_or_else(|error| panic!("generate server key failed: {error}"));
        let cert = params
            .signed_by(&key, &self.issuer)
            .unwrap_or_else(|error| panic!("sign server cert failed for {host}: {error}"));
        (cert.pem(), key.serialize_pem())
    }
}

fn server_config_from_pem(cert_pem: &str, key_pem: &str) -> io::Result<ServerConfig> {
    ensure_rustls_crypto_provider();
    let cert = CertificateDer::from_pem_slice(cert_pem.as_bytes()).map_err(invalid_data)?;
    let key = PrivateKeyDer::from_pem_slice(key_pem.as_bytes()).map_err(invalid_data)?;
    ServerConfig::builder_with_protocol_versions(&[&rustls::version::TLS13])
        .with_no_client_auth()
        .with_single_cert(vec![cert], key)
        .map_err(invalid_data)
}

fn accept_tls_http_request(
    listener: TcpListener,
    config: Arc<ServerConfig>,
) -> Result<String, String> {
    let (tcp_stream, _) = listener
        .accept()
        .map_err(|error| format!("accept failed: {error}"))?;
    read_tls_http_request(tcp_stream, config)
}

fn accept_connect_then_tls_http(
    listener: TcpListener,
    config: Arc<ServerConfig>,
) -> Result<String, String> {
    let (mut tcp_stream, _) = listener
        .accept()
        .map_err(|error| format!("accept failed: {error}"))?;
    let connect = read_http_request(&mut tcp_stream)
        .map_err(|error| format!("read CONNECT failed: {error}"))?;
    let origin_port = origin_port_from_connect(&connect)?;
    std::io::Write::write_all(
        &mut tcp_stream,
        b"HTTP/1.1 200 Connection Established\r\n\r\n",
    )
    .map_err(|error| format!("write CONNECT response failed: {error}"))?;
    intercept_tls_and_forward(tcp_stream, config, origin_port)
}

fn origin_port_from_connect(connect: &str) -> Result<u16, String> {
    let request_target = connect
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .ok_or_else(|| format!("malformed CONNECT request:\n{connect}"))?;
    let port = request_target
        .strip_prefix("token-origin.test:")
        .ok_or_else(|| format!("unexpected CONNECT request:\n{connect}"))?;
    port.parse::<u16>()
        .map_err(|error| format!("invalid CONNECT port in {request_target}: {error}"))
}

fn intercept_tls_and_forward(
    tcp_stream: TcpStream,
    config: Arc<ServerConfig>,
    origin_port: u16,
) -> Result<String, String> {
    let connection =
        ServerConnection::new(config).map_err(|error| format!("TLS accept failed: {error}"))?;
    let mut client_stream = StreamOwned::new(connection, tcp_stream);
    let request = read_http_request(&mut client_stream)
        .map_err(|error| format!("TLS read failed: {error}"))?;
    let response = forward_request_to_origin(origin_port, &request)?;
    client_stream
        .write_all(&response)
        .map_err(|error| format!("TLS write failed: {error}"))?;
    Ok(request)
}

fn forward_request_to_origin(origin_port: u16, request: &str) -> Result<Vec<u8>, String> {
    let mut origin_stream = TcpStream::connect(("127.0.0.1", origin_port))
        .map_err(|error| format!("origin connect failed: {error}"))?;
    origin_stream
        .write_all(request.as_bytes())
        .map_err(|error| format!("origin write failed: {error}"))?;
    let mut response = Vec::new();
    origin_stream
        .read_to_end(&mut response)
        .map_err(|error| format!("origin read failed: {error}"))?;
    if response.is_empty() {
        return Err("origin returned an empty response".to_string());
    }
    Ok(response)
}

fn read_tls_http_request(
    tcp_stream: TcpStream,
    config: Arc<ServerConfig>,
) -> Result<String, String> {
    let connection =
        ServerConnection::new(config).map_err(|error| format!("TLS accept failed: {error}"))?;
    let mut stream = StreamOwned::new(connection, tcp_stream);
    let request =
        read_http_request(&mut stream).map_err(|error| format!("TLS read failed: {error}"))?;
    write_ok_response(&mut stream).map_err(|error| format!("TLS write failed: {error}"))?;
    Ok(request)
}

fn invalid_data(error: impl std::error::Error + Send + Sync + 'static) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, error)
}
