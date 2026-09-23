use std::net::SocketAddr;

use anyhow::{Context, Result};
use sldr_core::config::Config;
use sldr_server::{app, ServeOptions, SldrState};
use tracing::{info, warn};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("sldr_server=info")
        .init();

    let addr = std::env::var("SLDR_SERVER_ADDR").unwrap_or_else(|_| "127.0.0.1:4100".to_string());
    let addr: SocketAddr = addr
        .parse()
        .with_context(|| format!("Invalid SLDR_SERVER_ADDR: {addr}"))?;

    let config = Config::load().context("Failed to load sldr config")?;
    let state = SldrState::new(config);

    let token = std::env::var("SLDR_API_TOKEN").ok().filter(|t| !t.is_empty());
    if token.is_none() {
        warn!("SLDR_API_TOKEN unset — the API is UNAUTHENTICATED (local/dev only)");
    } else {
        info!("Bearer-token auth enabled");
    }
    let studio_dir = std::env::var("SLDR_STUDIO_DIR")
        .ok()
        .filter(|d| !d.is_empty())
        .map(std::path::PathBuf::from);
    match &studio_dir {
        Some(d) => info!("serving studio frontend from {}", d.display()),
        None => info!("no SLDR_STUDIO_DIR — serving API only (no studio UI)"),
    }
    let app = app(state, ServeOptions { token, studio_dir });

    // HTTPS is what makes the studio usable from another device: a tailnet
    // hostname over plain HTTP is not a secure context (ADR-0009/0011).
    let tls = sldr_server::tls::TlsMode::from_env()
        .config(&Config::data_dir())
        .await?;
    match tls {
        Some(rustls) => {
            info!("sldr-server listening on https://{}", addr);
            axum_server::bind_rustls(addr, rustls)
                .serve(app.into_make_service())
                .await
                .context("Server error")
        }
        None => {
            if !addr.ip().is_loopback() {
                warn!(
                    "serving plain HTTP on a non-loopback address — the studio needs a secure \
                     context; set SLDR_TLS=1 (self-signed) or SLDR_TLS_CERT/SLDR_TLS_KEY"
                );
            }
            info!("sldr-server listening on http://{}", addr);
            axum::serve(tokio::net::TcpListener::bind(addr).await?, app)
                .await
                .context("Server error")
        }
    }
}
