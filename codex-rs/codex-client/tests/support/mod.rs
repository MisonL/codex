mod http;
mod probe;
mod tls;

use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

pub use http::spawn_plain_http_origin;
pub use probe::CODEX_CA_CERT_ENV;
pub use probe::SSL_CERT_FILE_ENV;
pub use probe::run_probe;
pub use probe::run_probe_posting_through_tls_intercepting_proxy;
pub use probe::run_probe_posting_to_tls13_server;
pub use tls::spawn_tls_intercepting_proxy;
pub use tls::spawn_tls13_test_server;

pub fn write_cert_file(temp_dir: &TempDir, name: &str, contents: &str) -> PathBuf {
    let path = temp_dir.path().join(name);
    fs::write(&path, contents).unwrap_or_else(|error| {
        panic!("write cert fixture failed for {}: {error}", path.display())
    });
    path
}
