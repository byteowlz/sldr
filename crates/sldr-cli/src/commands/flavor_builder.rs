//! Flavor Builder command - serves an interactive visual flavor editor

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use axum::extract::{Json, State as AxumState};
use axum::http::StatusCode;
use axum::response::Html;
use axum::routing::{get, post};
use axum::Router;
use colored::Colorize;
use serde::Deserialize;
use sldr_core::config::Config;
use tokio::net::TcpListener;

/// The flavor builder HTML page (embedded at compile time)
const BUILDER_HTML: &str = include_str!("../../../sldr-renderer/assets/flavor-builder.html");

/// Shared state for routes that need access to paths
#[derive(Clone)]
struct AppState {
    flavor_dir: PathBuf,
    initial_flavor_dir: PathBuf,
}

/// Request payload for saving a flavor
#[derive(Deserialize)]
struct SaveFlavorRequest {
    name: String,
    toml: String,
}

/// Request payload for uploading a logo (base64-encoded)
#[derive(Deserialize)]
struct UploadLogoRequest {
    /// Flavor name (determines which assets dir to save to)
    flavor_name: String,
    /// Original filename (e.g. "logo.png")
    filename: String,
    /// Base64-encoded file data (without data URI prefix)
    data: String,
    /// MIME type (e.g. "image/png", "image/svg+xml")
    mime: String,
}

pub fn run(flavor_name: Option<String>, port: u16) -> Result<()> {
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async { run_server(flavor_name, port).await })
}

async fn run_server(flavor_name: Option<String>, port: u16) -> Result<()> {
    let config = Config::load()?;
    let flavor_dir = config.flavor_dir();

    let initial_flavor_name = flavor_name.unwrap_or_else(|| config.config.default_flavor.clone());
    let initial_flavor_dir = flavor_dir.join(&initial_flavor_name);

    let shared = Arc::new(AppState {
        flavor_dir: flavor_dir.clone(),
        initial_flavor_dir: initial_flavor_dir.clone(),
    });

    let app = Router::new()
        .route("/", get(|| async { Html(BUILDER_HTML) }))
        .route("/api/flavor/current", get(handle_get_flavor))
        .route("/api/flavor/save", post(handle_save_flavor))
        .route("/api/logo/upload", post(handle_upload_logo))
        .route("/api/logo/list", get(handle_list_logos))
        .with_state(shared);

    let addr = format!("127.0.0.1:{port}");
    let listener = TcpListener::bind(&addr)
        .await
        .context("Failed to bind port")?;

    let url = format!("http://{addr}");

    println!(
        "\n  {} Flavor Builder running at {}\n",
        ">>".green().bold(),
        url.cyan().bold()
    );
    println!("  {} Press {} to randomize, {} to shuffle colors, {} to toggle light/dark", "Keys:".dimmed(), "R".bold(), "C".bold(), "D".bold());
    println!("  {} {} / {} to cycle templates", "     ".dimmed(), "Left".bold(), "Right".bold());
    println!("  {} Press {} to quit\n", "     ".dimmed(), "Ctrl+C".bold());

    // Open in browser
    if let Err(e) = std::process::Command::new("open").arg(&url).spawn() {
        tracing::warn!("Failed to open browser: {e}");
    }

    axum::serve(listener, app).await?;

    Ok(())
}

// ================================================================
// ROUTE HANDLERS
// ================================================================

async fn handle_get_flavor(
    AxumState(state): AxumState<Arc<AppState>>,
) -> (StatusCode, Json<serde_json::Value>) {
    match sldr_core::flavor::Flavor::load(&state.initial_flavor_dir) {
        Ok(flavor) => {
            let colors = serde_json::json!({
                "background": flavor.colors.background,
                "text": flavor.colors.text,
                "primary": flavor.colors.primary,
                "secondary": flavor.colors.secondary,
                "accent": flavor.colors.accent,
                "code_background": flavor.colors.code_background,
                "code_text": flavor.colors.code_text,
            });
            let dark_colors = flavor.dark_colors.as_ref().map(|dc| serde_json::json!({
                "background": dc.background,
                "text": dc.text,
                "primary": dc.primary,
                "secondary": dc.secondary,
                "accent": dc.accent,
                "code_background": dc.code_background,
                "code_text": dc.code_text,
            }));
            let typography = serde_json::json!({
                "heading_font": flavor.typography.heading_font,
                "body_font": flavor.typography.body_font,
                "code_font": flavor.typography.code_font,
            });

            // Build logo entries with data URIs for preview
            let assets_dir = state.initial_flavor_dir.join("assets");
            let logos: Vec<serde_json::Value> = flavor.logos.iter().map(|l| {
                let data_uri = logo_to_data_uri(&assets_dir, &l.file);
                serde_json::json!({
                    "file": l.file,
                    "position": l.position,
                    "x": l.x,
                    "y": l.y,
                    "width": l.width,
                    "opacity": l.opacity,
                    "templates": l.templates,
                    "dataUri": data_uri,
                })
            }).collect();

            let mut resp = serde_json::json!({
                "name": flavor.name,
                "colors": colors,
                "typography": typography,
                "logos": logos,
            });
            if let Some(dc) = dark_colors {
                resp["dark_colors"] = dc;
            }
            (StatusCode::OK, Json(resp))
        }
        Err(_) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "flavor not found"})),
        ),
    }
}

async fn handle_save_flavor(
    AxumState(state): AxumState<Arc<AppState>>,
    Json(payload): Json<SaveFlavorRequest>,
) -> (StatusCode, String) {
    let dest_dir = state.flavor_dir.join(&payload.name);
    if let Err(e) = std::fs::create_dir_all(&dest_dir) {
        return (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to create directory: {e}"));
    }
    let dest_file = dest_dir.join("flavor.toml");
    match std::fs::write(&dest_file, &payload.toml) {
        Ok(()) => (StatusCode::OK, format!("Saved to {}", dest_file.display())),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to write: {e}")),
    }
}

async fn handle_upload_logo(
    AxumState(state): AxumState<Arc<AppState>>,
    Json(payload): Json<UploadLogoRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    use base64::Engine;

    // Determine target assets directory
    let flavor_name = if payload.flavor_name.is_empty() {
        state.initial_flavor_dir
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("default")
            .to_string()
    } else {
        payload.flavor_name.clone()
    };
    let assets_dir = state.flavor_dir.join(&flavor_name).join("assets");

    if let Err(e) = std::fs::create_dir_all(&assets_dir) {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("Failed to create assets dir: {e}")})),
        );
    }

    // Decode base64 data
    let bytes = match base64::engine::general_purpose::STANDARD.decode(&payload.data) {
        Ok(b) => b,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": format!("Invalid base64: {e}")})),
            );
        }
    };

    // Sanitize filename
    let filename = payload.filename
        .replace(['/', '\\'], "")
        .replace("..", "")
        .trim()
        .to_string();
    let filename = if filename.is_empty() { "logo.png".to_string() } else { filename };

    let dest_path = assets_dir.join(&filename);

    if let Err(e) = std::fs::write(&dest_path, &bytes) {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("Failed to write file: {e}")})),
        );
    }

    // Build data URI for immediate preview
    let data_uri = format!(
        "data:{};base64,{}",
        payload.mime,
        payload.data,
    );

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "filename": filename,
            "path": dest_path.display().to_string(),
            "size": bytes.len(),
            "dataUri": data_uri,
        })),
    )
}

async fn handle_list_logos(
    AxumState(state): AxumState<Arc<AppState>>,
) -> (StatusCode, Json<serde_json::Value>) {
    let assets_dir = state.initial_flavor_dir.join("assets");

    if !assets_dir.exists() {
        return (StatusCode::OK, Json(serde_json::json!({"files": []})));
    }

    let mut files = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&assets_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                let name = path.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("")
                    .to_string();
                let ext = path.extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("")
                    .to_lowercase();

                if matches!(ext.as_str(), "svg" | "png" | "jpg" | "jpeg" | "webp" | "gif") {
                    let data_uri = logo_to_data_uri(&assets_dir, &name);
                    files.push(serde_json::json!({
                        "name": name,
                        "dataUri": data_uri,
                    }));
                }
            }
        }
    }

    (StatusCode::OK, Json(serde_json::json!({"files": files})))
}

/// Read a logo file and return a data URI for preview, or None
fn logo_to_data_uri(assets_dir: &std::path::Path, filename: &str) -> Option<String> {
    use base64::Engine;

    let path = assets_dir.join(filename);
    if !path.exists() {
        return None;
    }

    let ext = path.extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    let mime = match ext.as_str() {
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "gif" => "image/gif",
        _ => return None,
    };

    let bytes = std::fs::read(&path).ok()?;
    let encoded = base64::engine::general_purpose::STANDARD.encode(&bytes);
    Some(format!("data:{mime};base64,{encoded}"))
}
