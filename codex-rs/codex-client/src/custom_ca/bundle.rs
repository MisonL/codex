use std::env;
use std::fs;
use std::path::PathBuf;

use rustls_pki_types::CertificateDer;
use rustls_pki_types::pem::{self};
use tracing::info;
use tracing::warn;

use super::CODEX_CA_CERT_ENV;
use super::SSL_CERT_FILE_ENV;
use super::error::BuildCustomCaTransportError;
use super::pem::NormalizedPem;

pub(super) trait EnvSource {
    fn var(&self, key: &str) -> Option<String>;

    fn non_empty_path(&self, key: &str) -> Option<PathBuf> {
        self.var(key)
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
    }

    fn configured_ca_bundle(&self) -> Option<ConfiguredCaBundle> {
        self.non_empty_path(CODEX_CA_CERT_ENV)
            .map(|path| ConfiguredCaBundle {
                source_env: CODEX_CA_CERT_ENV,
                path,
            })
            .or_else(|| {
                self.non_empty_path(SSL_CERT_FILE_ENV)
                    .map(|path| ConfiguredCaBundle {
                        source_env: SSL_CERT_FILE_ENV,
                        path,
                    })
            })
    }
}

pub(super) struct ProcessEnv;

impl EnvSource for ProcessEnv {
    fn var(&self, key: &str) -> Option<String> {
        env::var(key).ok()
    }
}

pub(super) struct ConfiguredCaBundle {
    pub(super) source_env: &'static str,
    pub(super) path: PathBuf,
}

impl ConfiguredCaBundle {
    pub(super) fn load_certificates(
        &self,
    ) -> Result<Vec<CertificateDer<'static>>, BuildCustomCaTransportError> {
        match self.parse_certificates() {
            Ok(certificates) => {
                info!(
                    source_env = self.source_env,
                    ca_path = %self.path.display(),
                    certificate_count = certificates.len(),
                    "loaded certificates from custom CA bundle"
                );
                Ok(certificates)
            }
            Err(error) => {
                warn!(
                    source_env = self.source_env,
                    ca_path = %self.path.display(),
                    error = %error,
                    "failed to load custom CA bundle"
                );
                Err(error)
            }
        }
    }

    fn parse_certificates(
        &self,
    ) -> Result<Vec<CertificateDer<'static>>, BuildCustomCaTransportError> {
        let pem_data =
            fs::read(&self.path).map_err(|source| BuildCustomCaTransportError::ReadCaFile {
                source_env: self.source_env,
                path: self.path.clone(),
                source,
            })?;
        let certificates = NormalizedPem::from_pem_data(self.source_env, &self.path, &pem_data)
            .certificates()
            .map_err(|detail| self.invalid_ca_file(detail))?;
        if certificates.is_empty() {
            return Err(self.pem_parse_error(&pem::Error::NoItemsFound));
        }
        Ok(certificates)
    }

    fn pem_parse_error(&self, error: &pem::Error) -> BuildCustomCaTransportError {
        let detail = match error {
            pem::Error::NoItemsFound => "no certificates found in PEM file".to_string(),
            _ => format!("failed to parse PEM file: {error}"),
        };

        self.invalid_ca_file(detail)
    }

    fn invalid_ca_file(&self, detail: impl std::fmt::Display) -> BuildCustomCaTransportError {
        BuildCustomCaTransportError::InvalidCaFile {
            source_env: self.source_env,
            path: self.path.clone(),
            detail: detail.to_string(),
        }
    }
}
