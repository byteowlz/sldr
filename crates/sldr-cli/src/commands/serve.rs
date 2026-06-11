//! `sldr serve` — long-running HTTP daemon for external agents.
//!
//! Promotes the flavor-builder server pattern into a general agent API.
//! External tools (web-to-slide pipelines, MCP servers, custom scripts)
//! drive sldr over HTTP instead of forking the CLI per call.
//!
//! Boundary: sldr handles slide/playlist/asset CRUD + rendering. It does
//! NOT fetch URLs, OCR, or summarize content — those are agent jobs.
//!
//! See `trx-jbpj.2` for design rationale.
//!
//! # Endpoints
//!
//! Discovery:
//! - `GET  /api/health` — liveness + version
//! - `GET  /api/sample` — bundled sample slide markdown sources (the agent's
//!   catalog of available layouts)
//! - `GET  /sample.html?flavor=<name>` — sample deck rendered against a flavor
//!
//! Flavors:
//! - `GET  /api/flavors` — list installed flavors
//! - `GET  /api/flavors/{name}` — full flavor as JSON (every token)
//!
//! Slides:
//! - `GET  /api/slides` — list slide names + frontmatter
//! - `GET  /api/slides/{name}` — fetch one slide (markdown + parsed metadata)
//! - `POST /api/slides` — create slides from a `SlideInputBatch` JSON spec
//!
//! Assets:
//! - `POST /api/assets` — accept base64'd image bytes, return a stable
//!   filename agents can reference in slide markdown
//!
//! Build:
//! - `POST /api/build/{playlist}` — render a presentation, returns output path

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use axum::extract::{Path as AxumPath, Query, State as AxumState};
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Json};
use axum::routing::{get, post};
use axum::Router;
use base64::Engine;
use colored::Colorize;
use serde::{Deserialize, Serialize};
use sldr_core::config::Config;
use sldr_core::flavor::{Flavor, FlavorCollection};
use sldr_core::slide::{Slide, SlideCollection, SlideInputBatch};
use tokio::net::TcpListener;

#[derive(Clone)]
struct AppState {
    config: Config,
}

pub fn run(port: u16, open_browser: bool) -> Result<()> {
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async { run_server(port, open_browser).await })
}

async fn run_server(port: u16, open_browser: bool) -> Result<()> {
    let config = Config::load()?;
    let shared = Arc::new(AppState { config });

    let app = Router::new()
        .route("/", get(handle_root))
        .route("/api/health", get(handle_health))
        .route("/api/sample", get(handle_sample_sources))
        .route("/sample.html", get(handle_sample_html))
        .route("/api/flavors", get(handle_list_flavors))
        .route("/api/flavors/{name}", get(handle_get_flavor))
        .route("/api/slides", get(handle_list_slides).post(handle_create_slides))
        .route("/api/slides/{name}", get(handle_get_slide))
        .route("/api/assets", post(handle_upload_asset))
        .route("/api/build/{playlist}", post(handle_build))
        .with_state(shared);

    let addr = format!("127.0.0.1:{port}");
    let listener = TcpListener::bind(&addr)
        .await
        .context("Failed to bind port")?;

    let url = format!("http://{addr}");

    println!(
        "\n  {} sldr serve running at {}\n",
        ">>".green().bold(),
        url.cyan().bold()
    );
    println!("  {} {}", "Try:".dimmed(), format!("curl {url}/api/health").bold());
    println!("  {} {}", "    ".dimmed(), format!("curl {url}/api/sample").bold());
    println!("  {} {}", "    ".dimmed(), format!("curl {url}/sample.html?flavor=editorial-serif").bold());
    println!("  {} Press {} to quit\n", "    ".dimmed(), "Ctrl+C".bold());

    if open_browser {
        let _ = std::process::Command::new("open").arg(&url).spawn();
    }

    axum::serve(listener, app).await?;

    Ok(())
}

// ============================================================================
// ROOT — minimal landing with API map
// ============================================================================

async fn handle_root() -> Html<&'static str> {
    Html(
        r#"<!DOCTYPE html><meta charset="UTF-8"><title>sldr serve</title>
<style>body{font:14px system-ui;max-width:680px;margin:4rem auto;padding:0 1rem;color:#1a1a1a}h1{font-size:1.4rem}code{background:#f3f4f6;padding:.15em .4em;border-radius:3px}li{margin:.4rem 0}</style>
<h1>sldr serve</h1>
<p>HTTP API for external agents. Boundary: sldr renders, agents bring content.</p>
<h2>Discovery</h2>
<ul>
<li><code>GET /api/health</code></li>
<li><code>GET /api/sample</code> — slide catalog (markdown sources)</li>
<li><code>GET /sample.html?flavor=NAME</code> — rendered sample deck</li>
</ul>
<h2>Flavors</h2>
<ul>
<li><code>GET /api/flavors</code></li>
<li><code>GET /api/flavors/{name}</code></li>
</ul>
<h2>Slides</h2>
<ul>
<li><code>GET /api/slides</code></li>
<li><code>GET /api/slides/{name}</code></li>
<li><code>POST /api/slides</code> — body: <code>SlideInputBatch</code></li>
</ul>
<h2>Assets &amp; build</h2>
<ul>
<li><code>POST /api/assets</code> — body: <code>{filename, mime, data_base64}</code></li>
<li><code>POST /api/build/{playlist}</code> — body: <code>{flavor?}</code></li>
</ul>
"#,
    )
}

// ============================================================================
// HEALTH
// ============================================================================

async fn handle_health() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "ok": true,
        "service": "sldr",
        "version": env!("CARGO_PKG_VERSION"),
    }))
}

// ============================================================================
// SAMPLE — agent catalog of available layouts
// ============================================================================

#[derive(Serialize)]
struct SampleSlideEntry {
    name: String,
    source: String,
}

async fn handle_sample_sources() -> Json<serde_json::Value> {
    let slides: Vec<SampleSlideEntry> = sldr_renderer::sample::SAMPLE_SLIDES
        .iter()
        .map(|s| SampleSlideEntry {
            name: s.name.to_string(),
            source: s.source.to_string(),
        })
        .collect();
    Json(serde_json::json!({ "slides": slides }))
}

#[derive(Deserialize)]
struct SampleHtmlQuery {
    flavor: Option<String>,
}

async fn handle_sample_html(
    AxumState(state): AxumState<Arc<AppState>>,
    Query(q): Query<SampleHtmlQuery>,
) -> Result<Html<String>, (StatusCode, String)> {
    let flavor_name = q
        .flavor
        .unwrap_or_else(|| state.config.config.default_flavor.clone());
    let flavor = resolve_flavor(&state.config, &flavor_name)
        .map_err(|e| (StatusCode::NOT_FOUND, e.to_string()))?;
    let html = sldr_renderer::sample::render_sample(flavor, &[])
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Html(html))
}

// ============================================================================
// FLAVORS
// ============================================================================

async fn handle_list_flavors(
    AxumState(state): AxumState<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let collection = FlavorCollection::load_from_dir(&state.config.flavor_dir())
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let flavors: Vec<serde_json::Value> = collection
        .flavors
        .iter()
        .map(|f| {
            serde_json::json!({
                "name": f.name,
                "display_name": f.display_name,
                "description": f.description,
            })
        })
        .collect();
    Ok(Json(serde_json::json!({ "flavors": flavors })))
}

async fn handle_get_flavor(
    AxumState(state): AxumState<Arc<AppState>>,
    AxumPath(name): AxumPath<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let flavor = resolve_flavor(&state.config, &name)
        .map_err(|e| (StatusCode::NOT_FOUND, e.to_string()))?;
    serde_json::to_value(&flavor)
        .map(Json)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))
}

fn resolve_flavor(config: &Config, name: &str) -> Result<Flavor> {
    let collection = FlavorCollection::load_from_dir(&config.flavor_dir())?;
    if let Some(f) = collection.find(name) {
        return Ok(f.clone());
    }
    if name == "default" {
        return Ok(Flavor::default());
    }
    anyhow::bail!("Flavor '{name}' not found")
}

// ============================================================================
// SLIDES
// ============================================================================

#[derive(Serialize)]
struct SlideSummary {
    name: String,
    relative_path: String,
    title: Option<String>,
    description: Option<String>,
    layout: Option<String>,
    tags: Vec<String>,
}

impl From<&Slide> for SlideSummary {
    fn from(s: &Slide) -> Self {
        Self {
            name: s.name.clone(),
            relative_path: s.relative_path.clone(),
            title: s.metadata.title.clone(),
            description: s.metadata.description.clone(),
            layout: s.metadata.layout.clone(),
            tags: s.metadata.tags.clone(),
        }
    }
}

async fn handle_list_slides(
    AxumState(state): AxumState<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let collection = SlideCollection::load_from_dir(&state.config.slide_dir())
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let slides: Vec<SlideSummary> = collection.slides.iter().map(SlideSummary::from).collect();
    Ok(Json(serde_json::json!({ "slides": slides })))
}

async fn handle_get_slide(
    AxumState(state): AxumState<Arc<AppState>>,
    AxumPath(name): AxumPath<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let collection = SlideCollection::load_from_dir(&state.config.slide_dir())
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let slide = collection
        .find(&name)
        .ok_or_else(|| (StatusCode::NOT_FOUND, format!("Slide '{name}' not found")))?;
    Ok(Json(serde_json::json!({
        "name": slide.name,
        "relative_path": slide.relative_path,
        "metadata": slide.metadata,
        "content": slide.content,
    })))
}

#[derive(Serialize)]
struct CreateSlidesResponse {
    created: Vec<String>,
    failed: Vec<FailedSlide>,
}

#[derive(Serialize)]
struct FailedSlide {
    name: String,
    error: String,
}

async fn handle_create_slides(
    AxumState(state): AxumState<Arc<AppState>>,
    Json(batch): Json<SlideInputBatch>,
) -> Result<Json<CreateSlidesResponse>, (StatusCode, String)> {
    let slide_dir = state.config.slide_dir();
    std::fs::create_dir_all(&slide_dir)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let mut created = Vec::new();
    let mut failed = Vec::new();

    for input in &batch.slides {
        let effective_dir = input.effective_directory(batch.directory.as_deref());
        let filename = format!("{}.md", input.name);
        let path = match effective_dir {
            Some(ref d) => slide_dir.join(d).join(&filename),
            None => slide_dir.join(&filename),
        };

        if let Some(parent) = path.parent() {
            if let Err(e) = std::fs::create_dir_all(parent) {
                failed.push(FailedSlide {
                    name: input.name.clone(),
                    error: format!("create dir: {e}"),
                });
                continue;
            }
        }

        match std::fs::write(&path, input.to_markdown()) {
            Ok(()) => created.push(path.display().to_string()),
            Err(e) => failed.push(FailedSlide {
                name: input.name.clone(),
                error: e.to_string(),
            }),
        }
    }

    Ok(Json(CreateSlidesResponse { created, failed }))
}

// ============================================================================
// ASSETS — image upload from base64'd bytes
// ============================================================================

#[derive(Deserialize)]
struct UploadAssetRequest {
    /// Original filename (e.g. "diagram.png"). Used to derive a stable name
    /// in the slide assets directory; if a file with this name already
    /// exists, a numeric suffix is appended.
    filename: String,
    /// MIME type — informational; the bytes are written as-is.
    #[serde(default)]
    mime: Option<String>,
    /// Base64-encoded bytes (no `data:` prefix).
    data_base64: String,
    /// Optional subdir under slide_dir. Defaults to "assets".
    #[serde(default)]
    subdir: Option<String>,
}

#[derive(Serialize)]
struct UploadAssetResponse {
    path: String,
    relative_path: String,
}

async fn handle_upload_asset(
    AxumState(state): AxumState<Arc<AppState>>,
    Json(req): Json<UploadAssetRequest>,
) -> Result<Json<UploadAssetResponse>, (StatusCode, String)> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(req.data_base64.as_bytes())
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("invalid base64: {e}")))?;

    let subdir = req.subdir.as_deref().unwrap_or("assets");
    let asset_dir = state.config.slide_dir().join(subdir);
    std::fs::create_dir_all(&asset_dir)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let safe = sanitize_filename(&req.filename);
    let path = unique_path(&asset_dir, &safe);
    std::fs::write(&path, &bytes)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let _ = req.mime; // bytes are written verbatim; mime is informational

    let relative = path
        .strip_prefix(state.config.slide_dir())
        .map(std::path::Path::to_path_buf)
        .unwrap_or_else(|_| PathBuf::from(&safe));

    Ok(Json(UploadAssetResponse {
        path: path.display().to_string(),
        relative_path: relative.display().to_string(),
    }))
}

fn sanitize_filename(name: &str) -> String {
    name.chars()
        .map(|c| match c {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '.' | '-' | '_' => c,
            _ => '_',
        })
        .collect()
}

fn unique_path(dir: &std::path::Path, filename: &str) -> PathBuf {
    let candidate = dir.join(filename);
    if !candidate.exists() {
        return candidate;
    }
    let (stem, ext) = match filename.rfind('.') {
        Some(i) => (&filename[..i], &filename[i..]),
        None => (filename, ""),
    };
    for n in 1..1000 {
        let try_name = format!("{stem}-{n}{ext}");
        let try_path = dir.join(&try_name);
        if !try_path.exists() {
            return try_path;
        }
    }
    candidate
}

// ============================================================================
// BUILD
// ============================================================================

#[derive(Deserialize, Default)]
struct BuildRequest {
    #[serde(default)]
    flavor: Option<String>,
}

#[derive(Serialize)]
struct BuildResponse {
    playlist: String,
    flavor: String,
    output_path: String,
}

async fn handle_build(
    AxumState(state): AxumState<Arc<AppState>>,
    AxumPath(playlist): AxumPath<String>,
    body: Option<Json<BuildRequest>>,
) -> Result<Json<BuildResponse>, (StatusCode, String)> {
    let req = body.map(|Json(r)| r).unwrap_or_default();
    let playlist_name = playlist.clone();
    let flavor_arg = req.flavor.clone();
    let output_dir = state.config.output_dir();
    let config_clone = state.config.clone();

    // build::run writes to the config's output_dir; run on a blocking thread
    // since the existing CLI logic is synchronous.
    let result = tokio::task::spawn_blocking(move || {
        run_build_sync(&config_clone, &playlist_name, flavor_arg)
    })
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let output_path = output_dir.join(&playlist).join("index.html");
    Ok(Json(BuildResponse {
        playlist: result.playlist,
        flavor: result.flavor,
        output_path: output_path.display().to_string(),
    }))
}

struct BuildOutcome {
    playlist: String,
    flavor: String,
}

fn run_build_sync(
    _config: &Config,
    playlist_name: &str,
    flavor_arg: Option<String>,
) -> Result<BuildOutcome> {
    crate::commands::build::run(
        playlist_name,
        flavor_arg.clone(),
        false,
        false,
        None,
        "embed",
    )?;
    Ok(BuildOutcome {
        playlist: playlist_name.to_string(),
        flavor: flavor_arg.unwrap_or_else(|| "default".to_string()),
    })
}

// silence unused-import lints when adding the module incrementally
#[allow(dead_code)]
fn _used() -> impl IntoResponse {
    StatusCode::OK
}
