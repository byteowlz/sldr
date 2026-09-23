//! TLS for the standalone server (ADR-0009 as amended by ADR-0011).
//!
//! Browsers grant the APIs an editor needs (clipboard, crypto, service
//! workers, …) only to a *secure context*. `localhost` qualifies over plain
//! HTTP; a tailnet hostname does not — the studio hangs there. So standalone
//! serving needs HTTPS, and the zero-config path is a self-signed
//! certificate: generated once, persisted under the data dir so the one-time
//! browser acceptance sticks across restarts, and covering the machine's
//! hostnames plus `localhost`.
//!
//! `SLDR_TLS=1` uses (or creates) that self-signed pair; `SLDR_TLS_CERT` +
//! `SLDR_TLS_KEY` point at your own PEM files (a Tailscale cert, a real one).

use std::net::IpAddr;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use axum_server::tls_rustls::RustlsConfig;
use tracing::info;

pub enum TlsMode {
    /// Plain HTTP.
    Off,
    /// Self-signed pair under the sldr data dir, created on first use.
    SelfSigned,
    /// Caller-provided PEM files.
    Files { cert: PathBuf, key: PathBuf },
}

impl TlsMode {
    /// From `SLDR_TLS` / `SLDR_TLS_CERT` / `SLDR_TLS_KEY`.
    pub fn from_env() -> Self {
        let cert = std::env::var("SLDR_TLS_CERT").ok().filter(|s| !s.is_empty());
        let key = std::env::var("SLDR_TLS_KEY").ok().filter(|s| !s.is_empty());
        if let (Some(c), Some(k)) = (cert, key) {
            return Self::Files { cert: c.into(), key: k.into() };
        }
        match std::env::var("SLDR_TLS").ok().as_deref().map(str::trim) {
            Some("1" | "true" | "yes" | "self-signed" | "on") => Self::SelfSigned,
            _ => Self::Off,
        }
    }

    /// Resolve to a rustls config, generating the self-signed pair if needed.
    pub async fn config(&self, data_dir: &Path) -> Result<Option<RustlsConfig>> {
        if !matches!(self, Self::Off) {
            // One provider, chosen explicitly: rcgen and axum-server could
            // otherwise each pull a different one and rustls refuses to guess.
            let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();
        }
        match self {
            Self::Off => Ok(None),
            Self::Files { cert, key } => {
                info!("TLS from {} / {}", cert.display(), key.display());
                Ok(Some(RustlsConfig::from_pem_file(cert, key).await.context("Could not load TLS PEM files")?))
            }
            Self::SelfSigned => {
                let (cert, key) = self_signed_pair(data_dir)?;
                Ok(Some(RustlsConfig::from_pem_file(&cert, &key).await.context("Could not load generated TLS pair")?))
            }
        }
    }
}

/// `<data_dir>/tls/{cert,key}.pem`, created on first call.
pub fn self_signed_pair(data_dir: &Path) -> Result<(PathBuf, PathBuf)> {
    let dir = data_dir.join("tls");
    let cert = dir.join("cert.pem");
    let key = dir.join("key.pem");
    if cert.is_file() && key.is_file() {
        info!("TLS self-signed pair from {}", dir.display());
        return Ok((cert, key));
    }
    std::fs::create_dir_all(&dir).with_context(|| format!("Could not create {}", dir.display()))?;
    let names = subject_names();
    let generated = rcgen::generate_simple_self_signed(names.clone())
        .context("Could not generate a self-signed certificate")?;
    std::fs::write(&cert, generated.cert.pem())?;
    std::fs::write(&key, generated.signing_key.serialize_pem())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&key, std::fs::Permissions::from_mode(0o600));
    }
    info!(
        "TLS self-signed pair generated in {} for {} — accept it once per device",
        dir.display(),
        names.join(", ")
    );
    Ok((cert, key))
}

/// `localhost`, the machine's hostname, and the IPv4 it would use to reach
/// the tailnet and the LAN — found by *routing* a UDP socket, which sends
/// nothing. Enough for a MagicDNS name and its 100.x address.
fn subject_names() -> Vec<String> {
    let mut names = vec!["localhost".to_string()];
    let mut push = |n: String| {
        if !n.is_empty() && !names.contains(&n) {
            names.push(n);
        }
    };
    if let Ok(h) = std::fs::read_to_string("/etc/hostname") {
        push(h.trim().to_string());
    }
    if let Ok(h) = std::env::var("HOSTNAME") {
        push(h.trim().to_string());
    }
    for probe in ["100.100.100.100:53", "1.1.1.1:53"] {
        if let Ok(sock) = std::net::UdpSocket::bind("0.0.0.0:0") {
            if sock.connect(probe).is_ok() {
                if let Ok(std::net::SocketAddr::V4(local)) = sock.local_addr() {
                    let ip: IpAddr = (*local.ip()).into();
                    if !ip.is_loopback() {
                        push(ip.to_string());
                    }
                }
            }
        }
    }
    names
}
