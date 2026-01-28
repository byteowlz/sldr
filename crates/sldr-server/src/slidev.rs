use std::collections::HashMap;
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use tokio::process::Command;
use tokio::sync::Mutex;
use tokio::time::sleep;
use tracing::{info, warn};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SlidevSessionKind {
    Preview,
    TemplateEdit,
}

#[derive(Debug)]
pub struct SlidevSession {
    pub id: Uuid,
    pub kind: SlidevSessionKind,
    pub port: u16,
    pub url: String,
    pub working_dir: PathBuf,
    pub source_path: Option<PathBuf>,
    pub created_at: Instant,
    pub child: tokio::process::Child,
    pub temp_dir: Option<tempfile::TempDir>,
    pub watcher: Option<RecommendedWatcher>,
}

impl SlidevSession {
    async fn stop(mut self) {
        if let Err(err) = self.child.kill().await {
            warn!("Failed to stop slidev process {}: {}", self.id, err);
            return;
        }
        if let Err(err) = self.child.wait().await {
            warn!("Failed to wait for slidev process {}: {}", self.id, err);
        }
    }
}

#[derive(Clone)]
pub struct SlidevManager {
    sessions: Arc<Mutex<HashMap<Uuid, SlidevSession>>>,
    ttl: Duration,
    cleanup_interval: Duration,
}

impl SlidevManager {
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

                for (id, session) in guard.iter_mut() {
                    if now.duration_since(session.created_at) > ttl {
                        expired.push(*id);
                        continue;
                    }

                    if let Ok(Some(_status)) = session.child.try_wait() {
                        expired.push(*id);
                    }
                }

                for id in expired {
                    if let Some(session) = guard.remove(&id) {
                        tokio::spawn(session.stop());
                    }
                }
            }
        });
    }

    pub async fn spawn_preview(&self, working_dir: PathBuf) -> Result<SlidevSessionInfo> {
        self.spawn_session(working_dir, SlidevSessionKind::Preview, None)
            .await
    }

    pub async fn spawn_template_edit(
        &self,
        working_dir: PathBuf,
        source_path: PathBuf,
        temp_dir: tempfile::TempDir,
    ) -> Result<SlidevSessionInfo> {
        self.spawn_session(
            working_dir,
            SlidevSessionKind::TemplateEdit,
            Some((source_path, temp_dir)),
        )
        .await
    }

    async fn spawn_session(
        &self,
        working_dir: PathBuf,
        kind: SlidevSessionKind,
        template_source: Option<(PathBuf, tempfile::TempDir)>,
    ) -> Result<SlidevSessionInfo> {
        ensure_slidev_project(&working_dir).await?;
        let port = allocate_port()?;
        let url = format!("http://127.0.0.1:{port}");

        let mut command = Command::new("bun");
        command
            .arg("run")
            .arg("dev")
            .arg("--")
            .arg("--port")
            .arg(port.to_string())
            .arg("--host")
            .arg("127.0.0.1")
            .current_dir(&working_dir)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .env("BROWSER", "none")
            .env("CI", "1");

        let child = command
            .spawn()
            .context("Failed to spawn slidev dev server")?;

        let id = Uuid::new_v4();

        let (source_path, temp_dir, watcher) = if let Some((source_path, temp_dir)) = template_source
        {
            let working_copy = working_dir.join("slides.md");
            let watcher = Some(start_dragpos_watcher(&working_copy, &source_path)?);
            (Some(source_path), Some(temp_dir), watcher)
        } else {
            (None, None, None)
        };

        let session = SlidevSession {
            id,
            kind,
            port,
            url: url.clone(),
            working_dir,
            source_path,
            created_at: Instant::now(),
            child,
            temp_dir,
            watcher,
        };

        let mut guard = self.sessions.lock().await;
        guard.insert(id, session);

        Ok(SlidevSessionInfo { id, url, port })
    }

    pub async fn stop(&self, id: Uuid) -> Result<()> {
        let session = {
            let mut guard = self.sessions.lock().await;
            guard.remove(&id)
        };

        if let Some(session) = session {
            session.stop().await;
            return Ok(());
        }

        anyhow::bail!("slidev session not found");
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SlidevSessionInfo {
    pub id: Uuid,
    pub url: String,
    pub port: u16,
}

fn allocate_port() -> Result<u16> {
    let listener = TcpListener::bind("127.0.0.1:0").context("Failed to bind to ephemeral port")?;
    let port = listener
        .local_addr()
        .context("Failed to read assigned port")?
        .port();
    Ok(port)
}

async fn ensure_slidev_project(dir: &Path) -> Result<()> {
    let slides_path = dir.join("slides.md");
    if !slides_path.exists() {
        anyhow::bail!("slides.md not found in {}", dir.display());
    }

    let package_json = dir.join("package.json");
    if !package_json.exists() {
        let content = default_package_json()?;
        tokio::fs::write(&package_json, content)
            .await
            .context("Failed to write package.json")?;
    }

    let node_modules = dir.join("node_modules");
    if !node_modules.exists() {
        info!("Installing slidev dependencies in {}", dir.display());
        let status = Command::new("bun")
            .arg("install")
            .current_dir(dir)
            .status()
            .await
            .context("Failed to run bun install")?;

        if !status.success() {
            anyhow::bail!("bun install failed in {}", dir.display());
        }
    }

    Ok(())
}

fn default_package_json() -> Result<String> {
    let value = serde_json::json!({
        "name": "sldr-preview",
        "type": "module",
        "private": true,
        "scripts": {
            "dev": "slidev --open=false",
            "build": "slidev build",
            "export": "slidev export",
            "export-pdf": "slidev export --format pdf",
            "export-pptx": "slidev export --format pptx"
        },
        "dependencies": {
            "@slidev/cli": "^52.0.0",
            "@slidev/theme-default": "latest",
            "@slidev/theme-seriph": "latest",
            "vue": "^3.5.0"
        }
    });

    serde_json::to_string_pretty(&value).context("Failed to serialize package.json")
}

fn start_dragpos_watcher(working_copy: &Path, source_path: &Path) -> Result<RecommendedWatcher> {
    let source_path = source_path.to_path_buf();
    let working_copy = working_copy.to_path_buf();
    let watch_path = working_copy.clone();

    let mut watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
        match res {
            Ok(event) => {
                if event.kind.is_modify() {
                    if let Err(err) = sync_template(&working_copy, &source_path) {
                        warn!(
                            "Failed to sync template {}: {}",
                            source_path.display(),
                            err
                        );
                    }
                }
            }
            Err(err) => {
                warn!("File watcher error: {}", err);
            }
        }
    })?;

    watcher.watch(watch_path.as_path(), RecursiveMode::NonRecursive)?;
    Ok(watcher)
}

fn sync_template(working_copy: &Path, source_path: &Path) -> Result<()> {
    let content = std::fs::read_to_string(working_copy).context("Failed to read working copy")?;
    std::fs::write(source_path, content).context("Failed to write template source")?;
    Ok(())
}
