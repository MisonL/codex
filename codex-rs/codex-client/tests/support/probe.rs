use std::path::Path;
use std::process::Command;

use codex_utils_cargo_bin::cargo_bin;

pub const CODEX_CA_CERT_ENV: &str = "CODEX_CA_CERTIFICATE";
const PROBE_PROXY_ENV: &str = "CODEX_CUSTOM_CA_PROBE_PROXY";
const PROBE_TLS13_ENV: &str = "CODEX_CUSTOM_CA_PROBE_TLS13";
const PROBE_URL_ENV: &str = "CODEX_CUSTOM_CA_PROBE_URL";
pub const SSL_CERT_FILE_ENV: &str = "SSL_CERT_FILE";
const PROXY_ENV_VARS: &[&str] = &[
    "HTTP_PROXY",
    "http_proxy",
    "HTTPS_PROXY",
    "https_proxy",
    "ALL_PROXY",
    "all_proxy",
    "NO_PROXY",
    "no_proxy",
];

fn probe_command() -> Command {
    let mut cmd = Command::new(
        cargo_bin("custom_ca_probe")
            .unwrap_or_else(|error| panic!("failed to locate custom_ca_probe: {error}")),
    );
    cmd.env_remove(CODEX_CA_CERT_ENV);
    cmd.env_remove(PROBE_PROXY_ENV);
    cmd.env_remove(PROBE_TLS13_ENV);
    cmd.env_remove(PROBE_URL_ENV);
    cmd.env_remove(SSL_CERT_FILE_ENV);
    for env_var in PROXY_ENV_VARS {
        cmd.env_remove(env_var);
    }
    cmd
}

pub fn run_probe(envs: &[(&str, &Path)]) -> std::process::Output {
    let mut cmd = probe_command();
    for (key, value) in envs {
        cmd.env(key, value);
    }
    cmd.output()
        .unwrap_or_else(|error| panic!("failed to run custom_ca_probe: {error}"))
}

pub fn run_probe_posting_to_tls13_server(
    envs: &[(&str, &Path)],
    url: &str,
) -> std::process::Output {
    let mut cmd = probe_command();
    for (key, value) in envs {
        cmd.env(key, value);
    }
    cmd.env(PROBE_TLS13_ENV, "1");
    cmd.env(PROBE_URL_ENV, url);
    cmd.output()
        .unwrap_or_else(|error| panic!("failed to run custom_ca_probe: {error}"))
}

pub fn run_probe_posting_through_tls_intercepting_proxy(
    envs: &[(&str, &Path)],
    url: &str,
    proxy_url: &str,
) -> std::process::Output {
    let mut cmd = probe_command();
    for (key, value) in envs {
        cmd.env(key, value);
    }
    cmd.env(PROBE_PROXY_ENV, proxy_url);
    cmd.env(PROBE_TLS13_ENV, "1");
    cmd.env(PROBE_URL_ENV, url);
    cmd.output()
        .unwrap_or_else(|error| panic!("failed to run custom_ca_probe: {error}"))
}
