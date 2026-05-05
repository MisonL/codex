use std::path::Path;

use rustls_pki_types::CertificateDer;
use rustls_pki_types::pem::PemObject;
use rustls_pki_types::pem::SectionKind;
use rustls_pki_types::pem::{self};
use tracing::info;

type PemSection = (SectionKind, Vec<u8>);

pub(super) enum NormalizedPem {
    Standard(String),
    TrustedCertificate(String),
}

impl NormalizedPem {
    pub(super) fn from_pem_data(source_env: &'static str, path: &Path, pem_data: &[u8]) -> Self {
        let pem = String::from_utf8_lossy(pem_data);
        if pem.contains("TRUSTED CERTIFICATE") {
            info!(
                source_env,
                ca_path = %path.display(),
                "normalizing OpenSSL TRUSTED CERTIFICATE labels in custom CA bundle"
            );
            Self::TrustedCertificate(
                pem.replace("BEGIN TRUSTED CERTIFICATE", "BEGIN CERTIFICATE")
                    .replace("END TRUSTED CERTIFICATE", "END CERTIFICATE"),
            )
        } else {
            Self::Standard(pem.into_owned())
        }
    }

    pub(super) fn certificates(&self) -> Result<Vec<CertificateDer<'static>>, String> {
        let mut certificates = Vec::new();
        for section_result in self.sections() {
            let (section_kind, der) =
                section_result.map_err(|error| format!("failed to parse PEM file: {error}"))?;
            if section_kind == SectionKind::Certificate {
                let cert_der = self.certificate_der(&der).ok_or_else(|| {
                    "failed to extract certificate data from TRUSTED CERTIFICATE: invalid DER length"
                        .to_string()
                })?;
                certificates.push(CertificateDer::from(cert_der.to_vec()));
            }
        }
        Ok(certificates)
    }

    fn contents(&self) -> &str {
        match self {
            Self::Standard(contents) | Self::TrustedCertificate(contents) => contents,
        }
    }

    fn sections(&self) -> impl Iterator<Item = Result<PemSection, pem::Error>> + '_ {
        PemSection::pem_slice_iter(self.contents().as_bytes())
    }

    fn certificate_der<'a>(&self, der: &'a [u8]) -> Option<&'a [u8]> {
        match self {
            Self::Standard(_) => Some(der),
            Self::TrustedCertificate(_) => first_der_item(der),
        }
    }
}

fn first_der_item(der: &[u8]) -> Option<&[u8]> {
    der_item_length(der).map(|length| &der[..length])
}

fn der_item_length(der: &[u8]) -> Option<usize> {
    let &length_octet = der.get(1)?;
    if length_octet & 0x80 == 0 {
        return Some(2 + usize::from(length_octet)).filter(|length| *length <= der.len());
    }

    let length_octets = usize::from(length_octet & 0x7f);
    if length_octets == 0 {
        return None;
    }

    let length_start = 2usize;
    let length_end = length_start.checked_add(length_octets)?;
    let length_bytes = der.get(length_start..length_end)?;
    let mut content_length = 0usize;
    for &byte in length_bytes {
        content_length = content_length
            .checked_mul(256)?
            .checked_add(usize::from(byte))?;
    }

    length_end
        .checked_add(content_length)
        .filter(|length| *length <= der.len())
}
