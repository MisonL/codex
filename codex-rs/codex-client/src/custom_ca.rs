//! Custom CA handling for Codex outbound HTTP clients.
//!
//! Codex needs to trust enterprise CA bundles when proxies or gateways intercept TLS. This module
//! centralizes the environment-variable contract so callers can start from their usual
//! `reqwest::ClientBuilder`, layer in custom roots, and fail early with a user-facing error when a
//! configured bundle is not usable.

mod bundle;
mod error;
mod pem;

use bundle::EnvSource;
use bundle::ProcessEnv;
use codex_utils_rustls_provider::ensure_rustls_crypto_provider;
pub use error::BuildCustomCaTransportError;
use tracing::info;
use tracing::warn;

pub const CODEX_CA_CERT_ENV: &str = "CODEX_CA_CERTIFICATE";
pub const SSL_CERT_FILE_ENV: &str = "SSL_CERT_FILE";

const CA_CERT_HINT: &str = "If you set CODEX_CA_CERTIFICATE or SSL_CERT_FILE, ensure it points to a PEM file containing one or more CERTIFICATE blocks, or unset it to use system roots.";

/// Builds a reqwest client that honors Codex custom CA environment variables.
///
/// `CODEX_CA_CERTIFICATE` takes precedence over `SSL_CERT_FILE`. Empty values are treated as
/// unset. When a custom CA bundle is configured, this forces reqwest onto rustls before adding
/// custom roots so TLS-inspecting proxies work consistently.
pub fn build_reqwest_client_with_custom_ca(
    builder: reqwest::ClientBuilder,
) -> Result<reqwest::Client, BuildCustomCaTransportError> {
    build_reqwest_client_with_env(&ProcessEnv, builder)
}

/// Builds a reqwest client for spawned subprocess tests that exercise CA behavior.
///
/// Production callers should use [`build_reqwest_client_with_custom_ca`]. This helper disables
/// proxy autodetection so tests depend only on the explicit environment set by the test.
pub fn build_reqwest_client_for_subprocess_tests(
    builder: reqwest::ClientBuilder,
) -> Result<reqwest::Client, BuildCustomCaTransportError> {
    build_reqwest_client_with_env(&ProcessEnv, builder.no_proxy())
}

fn build_reqwest_client_with_env(
    env_source: &dyn EnvSource,
    mut builder: reqwest::ClientBuilder,
) -> Result<reqwest::Client, BuildCustomCaTransportError> {
    if let Some(bundle) = env_source.configured_ca_bundle() {
        ensure_rustls_crypto_provider();
        info!(
            source_env = bundle.source_env,
            ca_path = %bundle.path.display(),
            "building HTTP client with rustls backend for custom CA bundle"
        );
        builder = builder.use_rustls_tls();

        let certificates = bundle.load_certificates()?;
        for (idx, cert) in certificates.iter().enumerate() {
            let certificate = match reqwest::Certificate::from_der(cert.as_ref()) {
                Ok(certificate) => certificate,
                Err(source) => {
                    warn!(
                        source_env = bundle.source_env,
                        ca_path = %bundle.path.display(),
                        certificate_index = idx + 1,
                        error = %source,
                        "failed to register CA certificate"
                    );
                    return Err(BuildCustomCaTransportError::RegisterCertificate {
                        source_env: bundle.source_env,
                        path: bundle.path.clone(),
                        certificate_index: idx + 1,
                        source,
                    });
                }
            };
            builder = builder.add_root_certificate(certificate);
        }

        return match builder.build() {
            Ok(client) => Ok(client),
            Err(source) => {
                warn!(
                    source_env = bundle.source_env,
                    ca_path = %bundle.path.display(),
                    error = %source,
                    "failed to build client after loading custom CA bundle"
                );
                Err(BuildCustomCaTransportError::BuildClientWithCustomCa {
                    source_env: bundle.source_env,
                    path: bundle.path.clone(),
                    source,
                })
            }
        };
    }

    match builder.build() {
        Ok(client) => Ok(client),
        Err(source) => {
            warn!(
                error = %source,
                "failed to build client while using system root certificates"
            );
            Err(BuildCustomCaTransportError::BuildClientWithSystemRoots(
                source,
            ))
        }
    }
}
