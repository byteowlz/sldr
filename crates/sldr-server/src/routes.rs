//! HTTP API routes for sldr-server

use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use axum::extract::{Path as AxumPath, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post, put};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::json;
use tracing::{info, warn};

use sldr_core::config::Config;
use sldr_core::flavor::FlavorCollection;
use sldr_core::fuzzy::{ResolveResult, SldrMatcher};
use sldr_core::presentation::Playlist;
use sldr_core::slide::{Slide, SlideCollection, SlideMetadata};
use sldr_renderer::{HtmlRenderer, RenderConfig};

use crate::models::{
    BuildRequest, BuildResponse, CreatePlaylistRequest, CreateSlideRequest, FlavorsResponse,
    PreviewResponse, PlaylistsResponse, SlideDetail, SlideSummary, SlidesResponse,
    FlavorDetail, LayoutDetail, LayoutSummary, LayoutsResponse, ScaffoldEditResponse,
    UpdateFlavorRequest, UpdateLayoutRequest, UpdateSlideRequest, UpdateZonesRequest,
};
use crate::state::SldrState;

#[derive(Debug)]
struct ApiError {
    status: StatusCode,
    message: String,
    details: Option<serde_json::Value>,
}

impl ApiError {
    fn new(status: StatusCode, message: impl Into<String>) -> Self {
        Self {
            status,
            message: message.into(),
            details: None,
        }
    }

    #[allow(dead_code)]
    fn with_details(mut self, details: serde_json::Value) -> Self {
        self.details = Some(details);
        self
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let body = json!({
            "error": self.message,
            "details": self.details,
        });
        (self.status, Json(body)).into_response()
    }
}

type ApiResult<T> = std::result::Result<Json<T>, ApiError>;

pub fn router(state: SldrState) -> Router {
    Router::new()
        .route("/slides", get(list_slides).post(create_slide))
        .route("/slides/{name}", get(get_slide).put(update_slide))
        .route("/playlists", get(list_playlists).post(create_playlist))
        .route("/playlists/{name}", put(update_playlist))
        .route("/flavors", get(list_flavors))
        .route("/flavors/{name}", get(get_flavor).put(update_flavor))
        .route("/layouts", get(list_layouts))
        .route("/layouts/{name}", get(get_layout).put(update_layout))
        .route("/layouts/{name}/zones", put(update_layout_zones))
        .route("/build", post(build_presentation))
        .route("/preview/sample", get(preview_sample))
        .route("/preview/slide", get(preview_slide))
        .route("/preview/deck", get(preview_deck))
        .route("/preview/layout", get(preview_layout))
        .route("/preview/{playlist}", get(preview_playlist))
        .route("/scaffolds/{name}/edit", post(edit_scaffold))
        .with_state(state)
}

async fn list_slides(State(state): State<SldrState>) -> ApiResult<SlidesResponse> {
    let slides = SlideCollection::load_from_dir(&state.config.slide_dir())
        .map_err(to_api_error("Failed to load slides"))?;

    let items = slides
        .slides
        .into_iter()
        .map(|slide| SlideSummary {
            name: slide.name,
            relative_path: slide.relative_path,
            metadata: slide.metadata,
        })
        .collect();

    Ok(Json(SlidesResponse { slides: items }))
}

async fn get_slide(
    State(state): State<SldrState>,
    AxumPath(name): AxumPath<String>,
) -> ApiResult<SlideDetail> {
    let slides = SlideCollection::load_from_dir(&state.config.slide_dir())
        .map_err(to_api_error("Failed to load slides"))?;

    let slide = resolve_slide_ref(&state.config, &slides, &name)?;

    let raw = fs::read_to_string(&slide.path)
        .map_err(to_api_error("Failed to read slide file"))?;
    Ok(Json(SlideDetail {
        name: slide.name,
        relative_path: slide.relative_path,
        metadata: slide.metadata,
        content: slide.content,
        raw,
    }))
}

async fn create_slide(
    State(state): State<SldrState>,
    Json(payload): Json<CreateSlideRequest>,
) -> ApiResult<SlideDetail> {
    let slide_dir = state.config.slide_dir();
    let mut target_dir = slide_dir.clone();

    if let Some(subdir) = payload.subdir.as_ref() {
        target_dir = target_dir.join(subdir);
    }

    let filename = ensure_md_extension(&payload.name);
    let path = target_dir.join(&filename);

    if path.exists() {
        return Err(ApiError::new(
            StatusCode::CONFLICT,
            format!("Slide already exists: {}", path.display()),
        ));
    }

    fs::create_dir_all(&target_dir)
        .with_context(|| format!("Failed to create slide directory {}", target_dir.display()))
        .map_err(to_api_error("Failed to create slide"))?;

    let content = match payload.content {
        Some(content) => build_slide_content(payload.metadata.clone(), content),
        None => default_slide_scaffold(&payload.name, payload.metadata.clone()),
    };

    fs::write(&path, content)
        .with_context(|| format!("Failed to write slide {}", path.display()))
        .map_err(to_api_error("Failed to write slide"))?;

    let slide = Slide::load_with_base(&path, &slide_dir)
        .map_err(to_api_error("Failed to load created slide"))?;

    let raw = fs::read_to_string(&slide.path).unwrap_or_default();
    Ok(Json(SlideDetail {
        name: slide.name,
        relative_path: slide.relative_path,
        metadata: slide.metadata,
        content: slide.content,
        raw,
    }))
}

async fn update_slide(
    State(state): State<SldrState>,
    AxumPath(name): AxumPath<String>,
    Json(payload): Json<UpdateSlideRequest>,
) -> ApiResult<SlideDetail> {
    let slide_dir = state.config.slide_dir();
    let slides = SlideCollection::load_from_dir(&slide_dir)
        .map_err(to_api_error("Failed to load slides"))?;

    let existing = resolve_slide_ref(&state.config, &slides, &name)?;

    // Raw wins: the source drawer writes the whole file verbatim. Otherwise
    // rebuild the file from (possibly partial) metadata + content.
    let file_content = match &payload.raw {
        Some(raw) => raw.clone(),
        None => {
            let updated_metadata = payload.metadata.unwrap_or(existing.metadata.clone());
            let updated_content = payload.content.unwrap_or(existing.content.clone());
            build_slide_content(Some(updated_metadata), updated_content)
        }
    };

    fs::write(&existing.path, &file_content)
        .with_context(|| format!("Failed to update slide {}", existing.path.display()))
        .map_err(to_api_error("Failed to update slide"))?;

    // Re-parse what we wrote so the response reflects the file's truth.
    let parsed = Slide::from_str(existing.name.clone(), existing.path.clone(), &file_content);
    Ok(Json(SlideDetail {
        name: existing.name,
        relative_path: existing.relative_path,
        metadata: parsed.metadata,
        content: parsed.content,
        raw: file_content,
    }))
}

async fn list_playlists(State(state): State<SldrState>) -> ApiResult<PlaylistsResponse> {
    let playlist_dir = state.config.playlist_dir();
    let mut playlists = Vec::new();

    if playlist_dir.exists() {
        for entry in fs::read_dir(&playlist_dir)
            .with_context(|| {
                format!(
                    "Failed to read playlist directory {}",
                    playlist_dir.display()
                )
            })
            .map_err(to_api_error("Failed to read playlists"))?
        {
            let entry = entry.map_err(to_api_error("Failed to read playlist entry"))?;
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "toml") {
                match Playlist::load(&path) {
                    Ok(playlist) => playlists.push(playlist),
                    Err(err) => {
                        warn!("Failed to load playlist {:?}: {}", path, err);
                    }
                }
            }
        }
    }

    Ok(Json(PlaylistsResponse { playlists }))
}

async fn create_playlist(
    State(state): State<SldrState>,
    Json(payload): Json<CreatePlaylistRequest>,
) -> ApiResult<serde_json::Value> {
    let playlist_dir = state.config.playlist_dir();
    let filename = format!("{}.toml", payload.name);
    let path = playlist_dir.join(&filename);

    if path.exists() {
        return Err(ApiError::new(
            StatusCode::CONFLICT,
            format!("Playlist already exists: {}", payload.name),
        ));
    }

    let playlist = Playlist {
        name: payload.name.clone(),
        title: payload.title,
        description: payload.description,
        slides: payload.slides,
        flavor: payload.flavor,
        default_lang: None,
        render: payload.render,
    };

    playlist
        .save(&path)
        .with_context(|| format!("Failed to save playlist {}", path.display()))
        .map_err(to_api_error("Failed to save playlist"))?;

    Ok(Json(json!({ "name": payload.name })))
}

async fn update_playlist(
    State(state): State<SldrState>,
    AxumPath(name): AxumPath<String>,
    Json(payload): Json<CreatePlaylistRequest>,
) -> ApiResult<serde_json::Value> {
    let playlist_dir = state.config.playlist_dir();
    let path = playlist_dir.join(format!("{name}.toml"));

    if !path.exists() {
        return Err(ApiError::new(StatusCode::NOT_FOUND, "Playlist not found"));
    }

    let playlist = Playlist {
        name: name.clone(),
        title: payload.title,
        description: payload.description,
        slides: payload.slides,
        flavor: payload.flavor,
        default_lang: None,
        render: payload.render,
    };

    playlist
        .save(&path)
        .with_context(|| format!("Failed to update playlist {}", path.display()))
        .map_err(to_api_error("Failed to update playlist"))?;

    Ok(Json(json!({ "name": name })))
}

async fn list_flavors(State(state): State<SldrState>) -> ApiResult<FlavorsResponse> {
    let flavors = FlavorCollection::load_from_dirs(&state.config.flavor_dirs())
        .map_err(to_api_error("Failed to load flavors"))?;

    Ok(Json(FlavorsResponse {
        flavors: flavors.flavors,
    }))
}

async fn build_presentation(
    State(state): State<SldrState>,
    Json(payload): Json<BuildRequest>,
) -> ApiResult<BuildResponse> {
    let config = state.config.as_ref();
    let (name, output_dir, html_path) = build_html_from_playlist(config, &payload)
        .map_err(to_api_error("Build failed"))?;

    Ok(Json(BuildResponse {
        name,
        output_dir: output_dir.to_string_lossy().to_string(),
        html_path: html_path.to_string_lossy().to_string(),
    }))
}

#[derive(Deserialize)]
struct PreviewQuery {
    #[serde(default)]
    flavor: Option<String>,
}

async fn preview_playlist(
    State(state): State<SldrState>,
    AxumPath(playlist): AxumPath<String>,
    Query(query): Query<PreviewQuery>,
) -> ApiResult<PreviewResponse> {
    let payload = BuildRequest {
        playlist: playlist.clone(),
        flavor: query.flavor,
        output: None,
        pdf: false,
        pptx: false,
    };

    let (name, _output_dir, html_path) = build_html_from_playlist(state.config.as_ref(), &payload)
        .map_err(to_api_error("Build failed"))?;

    info!("Preview build complete for {}", name);

    let session = state
        .preview
        .spawn_preview(html_path)
        .await
        .map_err(to_api_error("Failed to start preview"))?;

    Ok(Json(PreviewResponse {
        session_id: session.id,
        url: session.url,
        port: session.port,
    }))
}

async fn edit_scaffold(
    State(state): State<SldrState>,
    AxumPath(name): AxumPath<String>,
) -> ApiResult<ScaffoldEditResponse> {
    let scaffold_path = resolve_scaffold_path(&state.config, &name)
        .map_err(to_api_error("Failed to resolve scaffold"))?;

    // Create a temp dir and render the scaffold as a single-slide presentation
    let temp_dir = tempfile::tempdir().map_err(to_api_error("Failed to create temp dir"))?;

    let content = fs::read_to_string(&scaffold_path)
        .with_context(|| format!("Failed to read scaffold {}", scaffold_path.display()))
        .map_err(to_api_error("Failed to read scaffold"))?;

    // Create a temporary slide from the scaffold content
    let slide = Slide {
        name: name.clone(),
        path: scaffold_path,
        relative_path: format!("{name}.md"),
        metadata: SlideMetadata::default(),
        content,
    };

    let render_config = RenderConfig {
        title: format!("Edit: {name}"),
        transition: "none".to_string(),
        ..Default::default()
    };

    let mut renderer = HtmlRenderer::new(render_config)
        .add_flavor(sldr_core::flavor::Flavor::default());
    renderer.add_slide(&slide).map_err(to_api_error("Failed to lay out slide"))?;

    let html_path = temp_dir.path().join("index.html");
    renderer
        .render_to_file(&html_path)
        .map_err(to_api_error("Failed to render scaffold preview"))?;

    let session = state
        .preview
        .spawn_preview_with_temp(html_path, temp_dir)
        .await
        .map_err(to_api_error("Failed to start scaffold edit preview"))?;

    Ok(Json(ScaffoldEditResponse {
        session_id: session.id,
        url: session.url,
        port: session.port,
    }))
}

/// Build a self-contained HTML presentation from a playlist
fn build_html_from_playlist(
    config: &Config,
    payload: &BuildRequest,
) -> Result<(String, PathBuf, PathBuf)> {
    let playlist_dir = config.playlist_dir();
    let playlist_path = playlist_dir.join(format!("{}.toml", payload.playlist));

    if !playlist_path.exists() {
        anyhow::bail!("Playlist not found: {}", payload.playlist);
    }

    let playlist = Playlist::load(&playlist_path)
        .with_context(|| format!("Failed to load playlist {}", playlist_path.display()))?;

    let flavor_name = payload
        .flavor
        .clone()
        .or_else(|| playlist.flavor.clone())
        .unwrap_or_else(|| config.config.default_flavor.clone());

    let flavor = if let Ok(collection) = FlavorCollection::load_from_dirs(&config.flavor_dirs()) {
        if collection.flavors.is_empty() {
            sldr_core::flavor::Flavor::default()
        } else {
            collection
                .flavors
                .iter()
                .find(|f| f.name == flavor_name)
                .cloned()
                .unwrap_or_else(sldr_core::flavor::Flavor::default)
        }
    } else {
        sldr_core::flavor::Flavor::default()
    };

    let slides = SlideCollection::load_from_dir(&config.slide_dir())
        .context("Failed to load slides")?;
    let matcher = SldrMatcher::new(config.matching.clone());

    let mut resolved = Vec::new();
    for slide_ref in &playlist.slides {
        match matcher.resolve(slide_ref, &slides.names()) {
            ResolveResult::Found(result) => {
                let slide = slides
                    .find(&result.value)
                    .cloned()
                    .with_context(|| format!("Slide not found: {}", result.value))?;
                resolved.push(slide);
            }
            ResolveResult::NotFound => {
                anyhow::bail!("Slide not found: {slide_ref}");
            }
            ResolveResult::Multiple(matches) => {
                let suggestions: Vec<String> = matches.into_iter().map(|m| m.value).collect();
                anyhow::bail!(
                    "Multiple slides match '{}': {}",
                    slide_ref,
                    suggestions.join(", ")
                );
            }
        }
    }

    let output_dir = payload
        .output
        .as_ref()
        .map(|path| Config::expand_path(path))
        .unwrap_or_else(|| config.output_dir().join(&playlist.name));

    let title = playlist
        .title
        .clone()
        .unwrap_or_else(|| playlist.name.clone());

    let transition = playlist
        .render
        .transition
        .clone()
        .unwrap_or_else(|| "fade".to_string());

    let aspect_ratio = playlist
        .render
        .aspect_ratio
        .clone()
        .unwrap_or_else(|| "16/9".to_string());

    let render_config = RenderConfig {
        title,
        transition,
        aspect_ratio,
        speaker_notes: true,
        ..Default::default()
    };

    let mut renderer = HtmlRenderer::new(render_config).add_flavor(flavor);
    for dir in config.layout_dirs() {
        renderer
            .load_layouts(&dir)
            .context("Failed to load user layouts")?;
    }
    renderer.add_slides(&resolved).context("Failed to lay out slides")?;

    fs::create_dir_all(&output_dir)?;
    let html_path = output_dir.join("index.html");
    renderer.render_to_file(&html_path)?;

    Ok((playlist.name, output_dir, html_path))
}

fn build_slide_content(metadata: Option<SlideMetadata>, content: String) -> String {
    let metadata = metadata.unwrap_or_default();
    let yaml = serde_yaml_ng::to_string(&metadata).unwrap_or_default();
    format!("---\n{yaml}---\n\n{content}")
}

fn default_slide_scaffold(name: &str, metadata: Option<SlideMetadata>) -> String {
    let title = name.trim_end_matches(".md").replace(['_', '-'], " ");
    let mut metadata = metadata.unwrap_or_default();
    if metadata.title.is_none() {
        metadata.title = Some(title.clone());
    }
    if metadata.layout.is_none() {
        metadata.layout = Some("default".to_string());
    }

    let content = format!("# {title}\n\n<!-- Your slide content here -->\n");
    build_slide_content(Some(metadata), content)
}

fn ensure_md_extension(name: &str) -> String {
    if name.ends_with(".md") {
        name.to_string()
    } else {
        format!("{name}.md")
    }
}

fn resolve_scaffold_path(config: &Config, name: &str) -> Result<PathBuf> {
    let scaffold_dir = config.scaffold_dir();
    let candidates = [
        scaffold_dir.join(format!("{name}.md")),
        scaffold_dir.join(name),
    ];

    for candidate in candidates {
        if candidate.exists() {
            return Ok(candidate);
        }
    }

    anyhow::bail!("Scaffold not found: {name}");
}

/// Render a single library slide to self-contained HTML — the thumbnail for the
/// slide library (`?slide=NAME&flavor=NAME`). Auto-fits to the iframe.
async fn preview_slide(
    State(state): State<SldrState>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> std::result::Result<axum::response::Html<String>, ApiError> {
    let slide_name = params
        .get("slide")
        .ok_or_else(|| ApiError::new(StatusCode::BAD_REQUEST, "slide param required"))?;
    let slides = SlideCollection::load_from_dir(&state.config.slide_dir())
        .map_err(to_api_error("Failed to load slides"))?;
    // Previews are an iframe surface: an unresolved ref renders as a legible
    // warning tile (matching build's fail-loud), not a JSON blob.
    let slide = match resolve_slide_ref(&state.config, &slides, slide_name) {
        Ok(s) => s,
        Err(e) => {
            let msg = html_escape_min(&e.message);
            return Ok(axum::response::Html(format!(
                r#"<!doctype html><html><body style="margin:0;height:100vh;display:grid;place-items:center;background:#16191b;color:#8b949e;font:500 26px/1.5 ui-monospace,monospace"><div style="text-align:center;padding:0 6%"><div style="color:#e0b84a;font-size:52px">⚠</div>{msg}<div style="font-size:18px;margin-top:12px;color:#6e7681">fix the reference in the playlist, or re-add the slide</div></div></body></html>"#
            )));
        }
    };

    let flavor_name = params.get("flavor").map(String::as_str).unwrap_or("default");
    let flavors = FlavorCollection::load_from_dirs(&state.config.flavor_dirs())
        .map_err(to_api_error("Failed to load flavors"))?;
    let flavor = flavors.find(flavor_name).cloned().unwrap_or_default();

    let cfg = RenderConfig {
        transition: "none".to_string(),
        ..Default::default()
    };
    let mut renderer = HtmlRenderer::new(cfg).add_flavor(flavor);
    renderer
        .add_slide(&slide)
        .map_err(to_api_error("Failed to lay out slide"))?;
    let html = renderer
        .render()
        .map_err(to_api_error("Failed to render slide"))?;
    Ok(axum::response::Html(html))
}

/// Render a whole playlist to self-contained HTML in memory — the full-deck
/// preview for the studio composer (`?playlist=NAME&flavor=NAME`). Unlike
/// `/build` it writes nothing to disk.
async fn preview_deck(
    State(state): State<SldrState>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> std::result::Result<axum::response::Html<String>, ApiError> {
    let config = state.config.as_ref();
    let name = params
        .get("playlist")
        .ok_or_else(|| ApiError::new(StatusCode::BAD_REQUEST, "playlist param required"))?;
    let playlist_path = config.playlist_dir().join(format!("{name}.toml"));
    if !playlist_path.exists() {
        return Err(ApiError::new(StatusCode::NOT_FOUND, "Playlist not found"));
    }
    let playlist = Playlist::load(&playlist_path)
        .map_err(to_api_error("Failed to load playlist"))?;

    let flavor_name = params
        .get("flavor")
        .cloned()
        .filter(|f| !f.is_empty())
        .or_else(|| playlist.flavor.clone())
        .unwrap_or_else(|| config.config.default_flavor.clone());
    let flavor = FlavorCollection::load_from_dirs(&config.flavor_dirs())
        .ok()
        .and_then(|c| c.find(&flavor_name).cloned())
        .unwrap_or_default();

    let slides = SlideCollection::load_from_dir(&config.slide_dir())
        .map_err(to_api_error("Failed to load slides"))?;
    let matcher = SldrMatcher::new(config.matching.clone());
    let mut resolved = Vec::new();
    for slide_ref in &playlist.slides {
        match matcher.resolve(slide_ref, &slides.names()) {
            ResolveResult::Found(result) => {
                if let Some(slide) = slides.find(&result.value) {
                    resolved.push(slide.clone());
                }
            }
            _ => {
                return Err(ApiError::new(
                    StatusCode::UNPROCESSABLE_ENTITY,
                    format!("Cannot resolve slide '{slide_ref}'"),
                ))
            }
        }
    }

    let cfg = RenderConfig {
        title: playlist.title.clone().unwrap_or_else(|| playlist.name.clone()),
        speaker_notes: true,
        ..Default::default()
    };
    let mut renderer = HtmlRenderer::new(cfg).add_flavor(flavor);
    for dir in config.layout_dirs() {
        let _ = renderer.load_layouts(&dir);
    }
    renderer
        .add_slides(&resolved)
        .map_err(to_api_error("Failed to lay out slides"))?;
    let html = renderer.render().map_err(to_api_error("Failed to render deck"))?;
    Ok(axum::response::Html(html))
}

/// Render a synthetic sample slide for one layout — the stage background of the
/// visual layout editor (`?layout=NAME&flavor=NAME`). The body is shaped to the
/// layout's expectations (columns / image / plain) so its regions are visible.
async fn preview_layout(
    State(state): State<SldrState>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> std::result::Result<axum::response::Html<String>, ApiError> {
    let layout_name = params
        .get("layout")
        .ok_or_else(|| ApiError::new(StatusCode::BAD_REQUEST, "layout param required"))?;
    let mut registry = sldr_renderer::LayoutRegistry::builtin();
    for dir in state.config.layout_dirs() {
        let _ = registry.load_dir(&dir);
    }
    let def = registry
        .get(layout_name)
        .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "Layout not found"))?;

    // A neutral inline SVG placeholder for image slots (base64 data URI — no
    // assets, and no characters that trip the markdown link parser).
    const IMG: &str = "data:image/svg+xml;base64,PHN2ZyB4bWxucz0naHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmcnIHZpZXdCb3g9JzAgMCAzMjAgMjAwJz48cmVjdCB3aWR0aD0nMzIwJyBoZWlnaHQ9JzIwMCcgZmlsbD0nIzMzNDE1NScvPjxjaXJjbGUgY3g9JzI1MCcgY3k9JzU1JyByPScyOCcgZmlsbD0nI2VhYjMwOCcvPjxwYXRoIGQ9J00wIDE2MCBMMTEwIDgwIEwxOTAgMTQwIEwyNTAgMTAwIEwzMjAgMTUwIFYyMDAgSDAgWicgZmlsbD0nIzQ3NTU2OScvPjwvc3ZnPg==";
    let body = if def.expects_columns() {
        "::left::\n### Left column\n\n- First point\n- Second point\n\n::right::\n### Right column\n\n- Another point\n- And one more\n".to_string()
    } else if def.expects_image() {
        format!("::content::\n### Content\n\n- First point\n- Second point\n\n::image::\n![Sample]({IMG})\n")
    } else {
        format!("# Sample heading\n\nA short paragraph of body text.\n\n- First point\n- Second point\n\n![Sample]({IMG})\n")
    };

    let mut metadata = SlideMetadata::default();
    metadata.layout = Some(layout_name.clone());
    metadata.title = Some("Sample headline".to_string());
    metadata.subtitle = Some("Sample subheadline".to_string());
    metadata.footer = Some("Footer line".to_string());
    metadata.source = Some("Sample source".to_string());

    let slide = Slide {
        name: format!("layout-preview-{layout_name}"),
        path: PathBuf::from("layout-preview.md"),
        relative_path: "layout-preview.md".to_string(),
        metadata,
        content: body,
    };

    let flavor_name = params.get("flavor").map(String::as_str).unwrap_or("default");
    let flavor = FlavorCollection::load_from_dirs(&state.config.flavor_dirs())
        .ok()
        .and_then(|c| c.find(flavor_name).cloned())
        .unwrap_or_default();

    let cfg = RenderConfig {
        transition: "none".to_string(),
        ..Default::default()
    };
    let mut renderer = HtmlRenderer::new(cfg).add_flavor(flavor);
    for dir in state.config.layout_dirs() {
        let _ = renderer.load_layouts(&dir);
    }
    renderer
        .add_slide(&slide)
        .map_err(to_api_error("Failed to lay out sample slide"))?;
    let html = renderer
        .render()
        .map_err(to_api_error("Failed to render layout preview"))?;
    Ok(axum::response::Html(html))
}

/// Render the bundled sample deck with a given flavor — live preview for the
/// flavor editor. Returns the full self-contained HTML for an iframe.
async fn preview_sample(
    State(state): State<SldrState>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> std::result::Result<axum::response::Html<String>, ApiError> {
    let name = params.get("flavor").map(String::as_str).unwrap_or("default");
    let flavors = FlavorCollection::load_from_dirs(&state.config.flavor_dirs())
        .map_err(to_api_error("Failed to load flavors"))?;
    let flavor = flavors.find(name).cloned().unwrap_or_default();
    let html = sldr_renderer::render_sample(flavor, &[])
        .map_err(to_api_error("Failed to render sample"))?;
    Ok(axum::response::Html(html))
}

// --- Flavors: get-one + save (trx-3f4w, for the studio flavor editor). ---

/// A name must be a bare directory stem — no path traversal.
fn validate_name(name: &str, kind: &str) -> Result<(), ApiError> {
    if name.is_empty() || name.contains('/') || name.contains('\\') || name.contains("..") {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            format!("Invalid {kind} name"),
        ));
    }
    Ok(())
}

async fn get_flavor(
    State(state): State<SldrState>,
    AxumPath(name): AxumPath<String>,
) -> ApiResult<FlavorDetail> {
    let flavors = FlavorCollection::load_from_dirs(&state.config.flavor_dirs())
        .map_err(to_api_error("Failed to load flavors"))?;
    let flavor = flavors
        .find(&name)
        .cloned()
        .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "Flavor not found"))?;
    // `custom_css` is `#[serde(skip)]`, so lift the css into its own field.
    let css = flavor.custom_css.clone();
    Ok(Json(FlavorDetail { flavor, css }))
}

/// Save a flavor: typed tokens → `flavor.toml`, css → `flavor.css` (removed when
/// empty). Writes into the library flavor dir; only canonical files (ADR-0009).
async fn update_flavor(
    State(state): State<SldrState>,
    AxumPath(name): AxumPath<String>,
    Json(payload): Json<UpdateFlavorRequest>,
) -> ApiResult<FlavorDetail> {
    validate_name(&name, "flavor")?;
    let dir = state.config.flavor_dir().join(&name);
    fs::create_dir_all(&dir)
        .with_context(|| format!("Failed to create flavor dir {}", dir.display()))
        .map_err(to_api_error("Failed to write flavor"))?;

    let toml_str = toml::to_string_pretty(&payload.flavor)
        .map_err(to_api_error("Failed to serialize flavor toml"))?;
    fs::write(dir.join("flavor.toml"), &toml_str)
        .with_context(|| format!("Failed to write {}/flavor.toml", dir.display()))
        .map_err(to_api_error("Failed to write flavor"))?;

    let css_path = dir.join("flavor.css");
    match payload.css.as_deref() {
        Some(css) if !css.trim().is_empty() => {
            fs::write(&css_path, css).map_err(to_api_error("Failed to write flavor.css"))?;
        }
        _ => {
            let _ = fs::remove_file(&css_path);
        }
    }
    info!("Saved flavor: {name}");
    Ok(Json(FlavorDetail {
        flavor: payload.flavor,
        css: payload.css,
    }))
}

// --- Layouts (trx-3f4w.11): canonical layout .html CRUD + zone parse/emit. ---

/// Extract a layout's `<!-- sldr:category NAME -->` value.
fn layout_category(source: &str) -> Option<String> {
    let pat = "<!-- sldr:category ";
    let start = source.find(pat)? + pat.len();
    let rest = &source[start..];
    let end = rest.find("-->")?;
    Some(rest[..end].trim().to_string())
}

/// Resolve a layout's source: a library file overrides the built-in. Returns
/// `(source, is_builtin)`.
fn resolve_layout_source(state: &SldrState, name: &str) -> Option<(String, bool)> {
    for dir in state.config.layout_dirs() {
        let path = dir.join(format!("{name}.html"));
        if path.is_file() {
            if let Ok(src) = fs::read_to_string(&path) {
                return Some((src, false));
            }
        }
    }
    sldr_renderer::builtin_layout_source(name).map(|s| (s.to_string(), true))
}

/// Write a layout's source into the (writable) library layout dir.
fn write_layout(state: &SldrState, name: &str, source: &str) -> Result<PathBuf, ApiError> {
    validate_name(name, "layout")?;
    let dir = state.config.layout_dir();
    fs::create_dir_all(&dir)
        .with_context(|| format!("Failed to create layout dir {}", dir.display()))
        .map_err(to_api_error("Failed to write layout"))?;
    let path = dir.join(format!("{name}.html"));
    fs::write(&path, source)
        .with_context(|| format!("Failed to write layout {}", path.display()))
        .map_err(to_api_error("Failed to write layout"))?;
    Ok(path)
}

fn layout_detail(name: String, source: String, builtin: bool) -> LayoutDetail {
    let zones = sldr_renderer::parse_zones(&source);
    let category = layout_category(&source);
    LayoutDetail {
        name,
        category,
        builtin,
        source,
        zones,
    }
}

async fn list_layouts(State(state): State<SldrState>) -> ApiResult<LayoutsResponse> {
    use std::collections::BTreeMap;
    // name -> is_builtin (a library file flips it to false / editable).
    let mut names: BTreeMap<String, bool> = BTreeMap::new();
    for n in sldr_renderer::builtin_layout_names() {
        names.insert(n.to_string(), true);
    }
    for dir in state.config.layout_dirs() {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("html") {
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    names.insert(stem.to_string(), false);
                }
            }
        }
    }

    let layouts = names
        .into_iter()
        .map(|(name, builtin)| {
            let (source, _) =
                resolve_layout_source(&state, &name).unwrap_or_else(|| (String::new(), builtin));
            LayoutSummary {
                category: layout_category(&source),
                zone_count: sldr_renderer::parse_zones(&source).len(),
                name,
                builtin,
            }
        })
        .collect();
    Ok(Json(LayoutsResponse { layouts }))
}

async fn get_layout(
    State(state): State<SldrState>,
    AxumPath(name): AxumPath<String>,
) -> ApiResult<LayoutDetail> {
    let (source, builtin) = resolve_layout_source(&state, &name)
        .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "Layout not found"))?;
    Ok(Json(layout_detail(name, source, builtin)))
}

async fn update_layout(
    State(state): State<SldrState>,
    AxumPath(name): AxumPath<String>,
    Json(payload): Json<UpdateLayoutRequest>,
) -> ApiResult<LayoutDetail> {
    write_layout(&state, &name, &payload.source)?;
    info!("Updated layout source: {name}");
    Ok(Json(layout_detail(name, payload.source, false)))
}

/// Rewrite only the zone directives (the visual zone editor). Editing a
/// built-in's zones writes a library override; markup and CSS are untouched.
async fn update_layout_zones(
    State(state): State<SldrState>,
    AxumPath(name): AxumPath<String>,
    Json(payload): Json<UpdateZonesRequest>,
) -> ApiResult<LayoutDetail> {
    let (source, _) = resolve_layout_source(&state, &name)
        .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "Layout not found"))?;
    let new_source = sldr_renderer::replace_zone_block(&source, &payload.zones);
    write_layout(&state, &name, &new_source)?;
    info!("Updated {} zone(s) on layout: {name}", payload.zones.len());
    Ok(Json(layout_detail(name, new_source, false)))
}

fn to_api_error<E>(context: &'static str) -> impl FnOnce(E) -> ApiError
where
    E: std::fmt::Display,
{
    move |err| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, format!("{context}: {err}"))
}

fn html_escape_min(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

/// Resolve a slide reference the way the CLI does: exact name first, then the
/// configured fuzzy matcher (playlists store fuzzy refs — same contract as
/// `sldr build`). Ambiguity fails loudly rather than guessing.
fn resolve_slide_ref(
    config: &Config,
    slides: &SlideCollection,
    name: &str,
) -> Result<Slide, ApiError> {
    if let Some(s) = slides.find(name) {
        return Ok(s.clone());
    }
    let matcher = SldrMatcher::new(config.matching.clone());
    match matcher.resolve(name, &slides.names()) {
        ResolveResult::Found(r) => slides
            .find(&r.value)
            .cloned()
            .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "Slide not found")),
        ResolveResult::Multiple(m) => Err(ApiError::new(
            StatusCode::UNPROCESSABLE_ENTITY,
            format!(
                "Ambiguous slide '{name}': {}",
                m.into_iter().map(|x| x.value).take(5).collect::<Vec<_>>().join(", ")
            ),
        )),
        ResolveResult::NotFound => Err(ApiError::new(
            StatusCode::NOT_FOUND,
            format!("Slide not found: {name}"),
        )),
    }
}
