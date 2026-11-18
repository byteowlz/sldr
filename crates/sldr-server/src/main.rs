use std::io::Write;
use std::net::SocketAddr;
use std::process::ExitCode;

use anyhow::{Context, Result};
use sldr_core::config::Config;
use sldr_server::{app, ServeOptions, SldrState};
use tracing::{info, warn};

const VERSION: &str = concat!(
    env!("CARGO_PKG_VERSION"),
    " (git ",
    env!("SLDR_GIT_HASH"),
    "; target ",
    env!("SLDR_BUILD_TARGET"),
    ")"
);

const HELP: &str = "HTTP API for sldr

Usage: sldr-server [OPTIONS]

Options:
  -h, --help     Print help and exit without loading server state
  -V, --version  Print version and build provenance, then exit

With no arguments, start the server using the existing environment configuration:
  SLDR_SERVER_ADDR       Listen address (default: 127.0.0.1:4100)
  SLDR_API_TOKEN         Optional bearer token; unset means unauthenticated
  SLDR_STUDIO_DIR        Optional prebuilt Studio frontend directory
  SLDR_TLS               Enable persisted self-signed TLS with 1
  SLDR_TLS_CERT          Existing TLS certificate PEM (requires SLDR_TLS_KEY)
  SLDR_TLS_KEY           Existing TLS private-key PEM (requires SLDR_TLS_CERT)

Config: $XDG_CONFIG_HOME/sldr/config.toml (default: ~/.config/sldr/config.toml)
Self-signed TLS: $XDG_DATA_HOME/sldr/tls (default: ~/.local/share/sldr/tls)
Help, version and invalid arguments never load configuration or start the server.";

// Deliberately before even constructing the async runtime: informational and
// invalid invocations must not reach logging, config, library, TLS or networking.
fn main() -> Result<ExitCode> {
    let mut arguments = std::env::args_os().skip(1);
    let Some(argument) = arguments.next() else {
        run()?;
        return Ok(ExitCode::SUCCESS);
    };
    if let Some(extra) = arguments.next() {
        writeln!(
            std::io::stderr(),
            "error: unexpected argument {extra:?}\n\nUsage: sldr-server [OPTIONS]"
        )?;
        return Ok(ExitCode::from(2));
    }
    match argument.to_str() {
        Some("--help" | "-h") => writeln!(std::io::stdout(), "{HELP}")?,
        Some("--version" | "-V") => writeln!(std::io::stdout(), "sldr-server {VERSION}")?,
        _ => {
            writeln!(
                std::io::stderr(),
                "error: unexpected argument {argument:?}\n\nUsage: sldr-server [OPTIONS]"
            )?;
            return Ok(ExitCode::from(2));
        }
    }
    Ok(ExitCode::SUCCESS)
}

#[tokio::main]
async fn run() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("sldr_server=info")
        .init();

    let addr = std::env::var("SLDR_SERVER_ADDR").unwrap_or_else(|_| "127.0.0.1:4100".to_string());
    let addr: SocketAddr = addr
        .parse()
        .with_context(|| format!("Invalid SLDR_SERVER_ADDR: {addr}"))?;

    let config = Config::load().context("Failed to load sldr config")?;
    let state = SldrState::new(config);

    let token = std::env::var("SLDR_API_TOKEN")
        .ok()
        .filter(|t| !t.is_empty());
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
