mod support;

use pretty_assertions::assert_eq;
use std::time::Duration;
use support::CODEX_CA_CERT_ENV;
use support::SSL_CERT_FILE_ENV;
use support::run_probe;
use support::run_probe_posting_through_tls_intercepting_proxy;
use support::run_probe_posting_to_tls13_server;
use support::spawn_plain_http_origin;
use support::spawn_tls_intercepting_proxy;
use support::spawn_tls13_test_server;
use support::write_cert_file;
use tempfile::TempDir;

const TEST_CERT_1: &str = include_str!("fixtures/test-ca.pem");
const TEST_CERT_2: &str = include_str!("fixtures/test-intermediate.pem");
const TRUSTED_TEST_CERT: &str = include_str!("fixtures/test-ca-trusted.pem");

fn assert_token_exchange_request(request: &str) {
    assert!(
        request.starts_with("POST /oauth/token HTTP/1.1"),
        "unexpected request:\n{request}"
    );
    assert!(
        request.contains("grant_type=authorization_code&code=test"),
        "unexpected request body:\n{request}"
    );
}

#[test]
fn uses_codex_ca_cert_env() {
    let temp_dir = TempDir::new().expect("tempdir");
    let cert_path = write_cert_file(&temp_dir, "ca.pem", TEST_CERT_1);

    let output = run_probe(&[(CODEX_CA_CERT_ENV, cert_path.as_path())]);

    assert!(output.status.success());
}

#[test]
fn falls_back_to_ssl_cert_file() {
    let temp_dir = TempDir::new().expect("tempdir");
    let cert_path = write_cert_file(&temp_dir, "ssl.pem", TEST_CERT_1);

    let output = run_probe(&[(SSL_CERT_FILE_ENV, cert_path.as_path())]);

    assert!(output.status.success());
}

#[test]
fn prefers_codex_ca_cert_over_ssl_cert_file() {
    let temp_dir = TempDir::new().expect("tempdir");
    let cert_path = write_cert_file(&temp_dir, "ca.pem", TEST_CERT_1);
    let bad_path = write_cert_file(&temp_dir, "bad.pem", "");

    let output = run_probe(&[
        (CODEX_CA_CERT_ENV, cert_path.as_path()),
        (SSL_CERT_FILE_ENV, bad_path.as_path()),
    ]);

    assert!(output.status.success());
}

#[test]
fn handles_multi_certificate_bundle() {
    let temp_dir = TempDir::new().expect("tempdir");
    let bundle = format!("{TEST_CERT_1}\n{TEST_CERT_2}");
    let cert_path = write_cert_file(&temp_dir, "bundle.pem", &bundle);

    let output = run_probe(&[(CODEX_CA_CERT_ENV, cert_path.as_path())]);

    assert!(output.status.success());
}

#[test]
fn posts_to_tls13_server_using_custom_ca_bundle() {
    let temp_dir = TempDir::new().expect("tempdir");
    let server = spawn_tls13_test_server();
    let cert_path = write_cert_file(&temp_dir, "tls-ca.pem", &server.ca_cert_pem);

    let output =
        run_probe_posting_to_tls13_server(&[(CODEX_CA_CERT_ENV, cert_path.as_path())], &server.url);
    let server_result = server.request_rx.recv_timeout(Duration::from_secs(5));

    assert!(
        output.status.success(),
        "custom_ca_probe failed\nstdout:\n{}\nstderr:\n{}\nserver:\n{server_result:?}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let request = server_result
        .expect("TLS test server should report a request")
        .expect("TLS test server should accept the probe request");
    assert_token_exchange_request(&request);
}

#[test]
fn posts_to_token_origin_through_tls_intercepting_proxy_with_custom_ca_bundle() {
    let temp_dir = TempDir::new().expect("tempdir");
    let origin = spawn_plain_http_origin();
    let proxy = spawn_tls_intercepting_proxy();
    let cert_path = write_cert_file(&temp_dir, "proxy-ca.pem", &proxy.ca_cert_pem);

    let output = run_probe_posting_through_tls_intercepting_proxy(
        &[(CODEX_CA_CERT_ENV, cert_path.as_path())],
        &origin.url,
        &proxy.url,
    );
    let proxy_result = proxy.request_rx.recv_timeout(Duration::from_secs(5));
    let origin_result = origin.request_rx.recv_timeout(Duration::from_secs(5));

    assert!(
        output.status.success(),
        "custom_ca_probe failed\nstdout:\n{}\nstderr:\n{}\nproxy:\n{proxy_result:?}\norigin:\n{origin_result:?}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let proxy_request = proxy_result
        .expect("TLS intercepting proxy should report a request")
        .expect("TLS intercepting proxy should accept the probe request");
    let origin_request = origin_result
        .expect("plain HTTP origin should report a request")
        .expect("plain HTTP origin should accept the forwarded request");
    assert_token_exchange_request(&proxy_request);
    assert_token_exchange_request(&origin_request);
}

#[test]
fn rejects_empty_pem_file_with_hint() {
    let temp_dir = TempDir::new().expect("tempdir");
    let cert_path = write_cert_file(&temp_dir, "empty.pem", "");

    let output = run_probe(&[(CODEX_CA_CERT_ENV, cert_path.as_path())]);

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("no certificates found in PEM file"));
    assert!(stderr.contains("CODEX_CA_CERTIFICATE"));
    assert!(stderr.contains("SSL_CERT_FILE"));
}

#[test]
fn rejects_malformed_pem_with_hint() {
    let temp_dir = TempDir::new().expect("tempdir");
    let cert_path = write_cert_file(
        &temp_dir,
        "malformed.pem",
        "-----BEGIN CERTIFICATE-----\nMIIBroken",
    );

    let output = run_probe(&[(CODEX_CA_CERT_ENV, cert_path.as_path())]);

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("failed to parse PEM file"));
    assert!(stderr.contains("CODEX_CA_CERTIFICATE"));
    assert!(stderr.contains("SSL_CERT_FILE"));
}

#[test]
fn accepts_openssl_trusted_certificate() {
    let temp_dir = TempDir::new().expect("tempdir");
    let cert_path = write_cert_file(&temp_dir, "trusted.pem", TRUSTED_TEST_CERT);

    let output = run_probe(&[(CODEX_CA_CERT_ENV, cert_path.as_path())]);

    assert!(output.status.success());
}

#[test]
fn accepts_bundle_with_crl() {
    let temp_dir = TempDir::new().expect("tempdir");
    let crl = "-----BEGIN X509 CRL-----\nMIIC\n-----END X509 CRL-----";
    let bundle = format!("{TEST_CERT_1}\n{crl}");
    let cert_path = write_cert_file(&temp_dir, "bundle_crl.pem", &bundle);

    let output = run_probe(&[(CODEX_CA_CERT_ENV, cert_path.as_path())]);

    assert_eq!(output.status.success(), true);
}
