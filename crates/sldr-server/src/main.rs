use std::net::SocketAddr;

use anyhow::{Context, Result};
use sldr_core::config::Config;
use sldr_server::{router, SldrState};
use tracing::info;

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

    let app = router(state);
    info!("sldr-server listening on {}", addr);

    axum::serve(tokio::net::TcpListener::bind(addr).await?, app)
        .await
        .context("Server error")
}
