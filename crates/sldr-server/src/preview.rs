//! Preview session manager - serves self-contained HTML presentations
//!
//! Replaces the old SlidevManager. Instead of spawning bun/node processes,
//! we serve the rendered HTML directly via axum.

use std::collections::HashMap;
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use tokio::sync::Mutex;
use tokio::time::sleep;
use tracing::info;
use uuid::Uuid;

/// A preview session - stores the path to a rendered HTML file and metadata
#[derive(Debug)]
pub struct PreviewSession {
    pub id: Uuid,
    pub port: u16,
    pub url: String,
    pub html_path: PathBuf,
    pub created_at: Instant,
    pub temp_dir: Option<tempfile::TempDir>,
    shutdown_tx: Option<tokio::sync::oneshot::Sender<()>>,
}

/// Manages preview sessions with auto-cleanup
#[derive(Clone)]
pub struct PreviewManager {
    sessions: Arc<Mutex<HashMap<Uuid, PreviewSession>>>,
    ttl: Duration,
    cleanup_interval: Duration,
}

impl PreviewManager {
    pub fn new() -> Self {
        let manager = Self {
            sessions: Arc::new(Mutex::new(HashMap::new())),
            ttl: Duration::from_secs(60 * 60),
            cleanup_interval: Duration::from_secs(30),
        };

        manager.spawn_cleanup_task();
        manager
    }

    fn spawn_cleanup_task(&self) {
        let sessions = Arc::clone(&self.sessions);
        let ttl = self.ttl;
        let interval = self.cleanup_interval;

        tokio::spawn(async move {
            loop {
                sleep(interval).await;
                let mut guard = sessions.lock().await;
                let now = Instant::now();
                let mut expired = Vec::new();

                for (id, session) in guard.iter() {
                    if now.duration_since(session.created_at) > ttl {
                        expired.push(*id);
                    }
                }

                for id in expired {
                    if let Some(session) = guard.remove(&id) {
                        // Send shutdown signal to the mini server
                        if let Some(tx) = session.shutdown_tx {
                            let _ = tx.send(());
                        }
                        info!("Cleaned up expired preview session {}", id);
                    }
                }
            }
        });
    }

    /// Spawn a preview session that serves an HTML file on a random port
    pub async fn spawn_preview(&self, html_path: PathBuf) -> Result<PreviewSessionInfo> {
        self.spawn_session(html_path, None).await
    }

    /// Spawn a preview session with a temp dir that gets cleaned up
    pub async fn spawn_preview_with_temp(
        &self,
        html_path: PathBuf,
        temp_dir: tempfile::TempDir,
    ) -> Result<PreviewSessionInfo> {
        self.spawn_session(html_path, Some(temp_dir)).await
    }

    async fn spawn_session(
        &self,
        html_path: PathBuf,
        temp_dir: Option<tempfile::TempDir>,
    ) -> Result<PreviewSessionInfo> {
        let port = allocate_port()?;
        let url = format!("http://127.0.0.1:{port}");
        let id = Uuid::new_v4();

        // Read the HTML content
        let html_content = tokio::fs::read_to_string(&html_path)
            .await
            .with_context(|| format!("Failed to read HTML file: {}", html_path.display()))?;

        // Spawn a tiny axum server on the allocated port
        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();

        let app = axum::Router::new().route(
            "/",
            axum::routing::get(move || {
                let content = html_content.clone();
                async move {
                    axum::response::Html(content)
                }
            }),
        );

        let addr = format!("127.0.0.1:{port}");
        let listener = tokio::net::TcpListener::bind(&addr)
            .await
            .with_context(|| format!("Failed to bind to {addr}"))?;

        tokio::spawn(async move {
            axum::serve(listener, app)
                .with_graceful_shutdown(async {
                    let _ = shutdown_rx.await;
                })
                .await
                .ok();
        });

        let session = PreviewSession {
            id,
            port,
            url: url.clone(),
            html_path,
            created_at: Instant::now(),
            temp_dir,
            shutdown_tx: Some(shutdown_tx),
        };

        let mut guard = self.sessions.lock().await;
        guard.insert(id, session);

        info!("Started preview session {} on {}", id, url);

        Ok(PreviewSessionInfo { id, url, port })
    }

    /// Stop a preview session
    pub async fn stop(&self, id: Uuid) -> Result<()> {
        let session = {
            let mut guard = self.sessions.lock().await;
            guard.remove(&id)
        };

        if let Some(session) = session {
            if let Some(tx) = session.shutdown_tx {
                let _ = tx.send(());
            }
            info!("Stopped preview session {}", id);
            return Ok(());
        }

        anyhow::bail!("Preview session not found: {id}");
    }
}

/// Information returned when a preview session is created
#[derive(Debug, Clone, serde::Serialize)]
pub struct PreviewSessionInfo {
    pub id: Uuid,
    pub url: String,
    pub port: u16,
}

/// Allocate an ephemeral port
fn allocate_port() -> Result<u16> {
    let listener = TcpListener::bind("127.0.0.1:0").context("Failed to bind to ephemeral port")?;
    let port = listener
        .local_addr()
        .context("Failed to read assigned port")?
        .port();
    Ok(port)
}
