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
use sldr_core::presentation::Skeleton;
use sldr_core::slide::{Slide, SlideCollection, SlideMetadata};
use sldr_renderer::{HtmlRenderer, RenderConfig};

use crate::models::{
    BuildRequest, BuildResponse, CreateSkeletonRequest, CreateSlideRequest, FlavorsResponse,
    PreviewResponse, SkeletonsResponse, SlideDetail, SlideSummary, SlidesResponse,
    TemplateEditResponse, UpdateSlideRequest,
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
        .route("/skeletons", get(list_skeletons).post(create_skeleton))
        .route("/skeletons/{name}", put(update_skeleton))
        .route("/flavors", get(list_flavors))
        .route("/build", post(build_presentation))
        .route("/preview/{skeleton}", get(preview_skeleton))
        .route("/templates/{name}/edit", post(edit_template))
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

    let slide = slides
        .find(&name)
        .cloned()
        .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "Slide not found"))?;

    Ok(Json(SlideDetail {
        name: slide.name,
        relative_path: slide.relative_path,
        metadata: slide.metadata,
        content: slide.content,
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
        None => default_slide_template(&payload.name, payload.metadata.clone()),
    };

    fs::write(&path, content)
        .with_context(|| format!("Failed to write slide {}", path.display()))
        .map_err(to_api_error("Failed to write slide"))?;

    let slide = Slide::load_with_base(&path, &slide_dir)
        .map_err(to_api_error("Failed to load created slide"))?;

    Ok(Json(SlideDetail {
        name: slide.name,
        relative_path: slide.relative_path,
        metadata: slide.metadata,
        content: slide.content,
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

    let existing = slides
        .find(&name)
        .cloned()
        .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "Slide not found"))?;

    let updated_metadata = payload.metadata.unwrap_or(existing.metadata.clone());
    let updated_content = payload.content.unwrap_or(existing.content.clone());
    let file_content = build_slide_content(Some(updated_metadata.clone()), updated_content.clone());

    fs::write(&existing.path, file_content)
        .with_context(|| format!("Failed to update slide {}", existing.path.display()))
        .map_err(to_api_error("Failed to update slide"))?;

    Ok(Json(SlideDetail {
        name: existing.name,
        relative_path: existing.relative_path,
        metadata: updated_metadata,
        content: updated_content,
    }))
}

async fn list_skeletons(State(state): State<SldrState>) -> ApiResult<SkeletonsResponse> {
    let skeleton_dir = state.config.skeleton_dir();
    let mut skeletons = Vec::new();

    if skeleton_dir.exists() {
        for entry in fs::read_dir(&skeleton_dir)
            .with_context(|| {
                format!(
                    "Failed to read skeleton directory {}",
                    skeleton_dir.display()
                )
            })
            .map_err(to_api_error("Failed to read skeletons"))?
        {
            let entry = entry.map_err(to_api_error("Failed to read skeleton entry"))?;
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "toml") {
                match Skeleton::load(&path) {
                    Ok(skeleton) => skeletons.push(skeleton),
                    Err(err) => {
                        warn!("Failed to load skeleton {:?}: {}", path, err);
                    }
                }
            }
        }
    }

    Ok(Json(SkeletonsResponse { skeletons }))
}

async fn create_skeleton(
    State(state): State<SldrState>,
    Json(payload): Json<CreateSkeletonRequest>,
) -> ApiResult<serde_json::Value> {
    let skeleton_dir = state.config.skeleton_dir();
    let filename = format!("{}.toml", payload.name);
    let path = skeleton_dir.join(&filename);

    if path.exists() {
        return Err(ApiError::new(
            StatusCode::CONFLICT,
            format!("Skeleton already exists: {}", payload.name),
        ));
    }

    let skeleton = Skeleton {
        name: payload.name.clone(),
        title: payload.title,
        description: payload.description,
        slides: payload.slides,
        flavor: payload.flavor,
        slidev_config: payload.slidev_config,
    };

    skeleton
        .save(&path)
        .with_context(|| format!("Failed to save skeleton {}", path.display()))
        .map_err(to_api_error("Failed to save skeleton"))?;

    Ok(Json(json!({ "name": payload.name })))
}

async fn update_skeleton(
    State(state): State<SldrState>,
    AxumPath(name): AxumPath<String>,
    Json(payload): Json<CreateSkeletonRequest>,
) -> ApiResult<serde_json::Value> {
    let skeleton_dir = state.config.skeleton_dir();
    let path = skeleton_dir.join(format!("{name}.toml"));

    if !path.exists() {
        return Err(ApiError::new(StatusCode::NOT_FOUND, "Skeleton not found"));
    }

    let skeleton = Skeleton {
        name: name.clone(),
        title: payload.title,
        description: payload.description,
        slides: payload.slides,
        flavor: payload.flavor,
        slidev_config: payload.slidev_config,
    };

    skeleton
        .save(&path)
        .with_context(|| format!("Failed to update skeleton {}", path.display()))
        .map_err(to_api_error("Failed to update skeleton"))?;

    Ok(Json(json!({ "name": name })))
}

async fn list_flavors(State(state): State<SldrState>) -> ApiResult<FlavorsResponse> {
    let flavors = FlavorCollection::load_from_dir(&state.config.flavor_dir())
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
    let (name, output_dir, html_path) = build_html_from_skeleton(config, &payload)
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

async fn preview_skeleton(
    State(state): State<SldrState>,
    AxumPath(skeleton): AxumPath<String>,
    Query(query): Query<PreviewQuery>,
) -> ApiResult<PreviewResponse> {
    let payload = BuildRequest {
        skeleton: skeleton.clone(),
        flavor: query.flavor,
        output: None,
        pdf: false,
        pptx: false,
    };

    let (name, _output_dir, html_path) = build_html_from_skeleton(state.config.as_ref(), &payload)
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

async fn edit_template(
    State(state): State<SldrState>,
    AxumPath(name): AxumPath<String>,
) -> ApiResult<TemplateEditResponse> {
    let template_path = resolve_template_path(&state.config, &name)
        .map_err(to_api_error("Failed to resolve template"))?;

    // Create a temp dir and render the template as a single-slide presentation
    let temp_dir = tempfile::tempdir().map_err(to_api_error("Failed to create temp dir"))?;

    let content = fs::read_to_string(&template_path)
        .with_context(|| format!("Failed to read template {}", template_path.display()))
        .map_err(to_api_error("Failed to read template"))?;

    // Create a temporary slide from the template content
    let slide = Slide {
        name: name.clone(),
        path: template_path,
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
    renderer.add_slide(&slide);

    let html_path = temp_dir.path().join("index.html");
    renderer
        .render_to_file(&html_path)
        .map_err(to_api_error("Failed to render template preview"))?;

    let session = state
        .preview
        .spawn_preview_with_temp(html_path, temp_dir)
        .await
        .map_err(to_api_error("Failed to start template edit preview"))?;

    Ok(Json(TemplateEditResponse {
        session_id: session.id,
        url: session.url,
        port: session.port,
    }))
}

/// Build a self-contained HTML presentation from a skeleton
fn build_html_from_skeleton(
    config: &Config,
    payload: &BuildRequest,
) -> Result<(String, PathBuf, PathBuf)> {
    let skeleton_dir = config.skeleton_dir();
    let skeleton_path = skeleton_dir.join(format!("{}.toml", payload.skeleton));

    if !skeleton_path.exists() {
        anyhow::bail!("Skeleton not found: {}", payload.skeleton);
    }

    let skeleton = Skeleton::load(&skeleton_path)
        .with_context(|| format!("Failed to load skeleton {}", skeleton_path.display()))?;

    let flavor_name = payload
        .flavor
        .clone()
        .or_else(|| skeleton.flavor.clone())
        .unwrap_or_else(|| config.config.default_flavor.clone());

    let flavor = if let Ok(collection) = FlavorCollection::load_from_dir(&config.flavor_dir()) {
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
    for slide_ref in &skeleton.slides {
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
        .unwrap_or_else(|| config.output_dir().join(&skeleton.name));

    let title = skeleton
        .title
        .clone()
        .unwrap_or_else(|| skeleton.name.clone());

    let transition = skeleton
        .slidev_config
        .transition
        .clone()
        .unwrap_or_else(|| "fade".to_string());

    let aspect_ratio = skeleton
        .slidev_config
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
    renderer.add_slides(&resolved);

    fs::create_dir_all(&output_dir)?;
    let html_path = output_dir.join("index.html");
    renderer.render_to_file(&html_path)?;

    Ok((skeleton.name, output_dir, html_path))
}

fn build_slide_content(metadata: Option<SlideMetadata>, content: String) -> String {
    let metadata = metadata.unwrap_or_default();
    let yaml = serde_yaml_ng::to_string(&metadata).unwrap_or_default();
    format!("---\n{yaml}---\n\n{content}")
}

fn default_slide_template(name: &str, metadata: Option<SlideMetadata>) -> String {
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

fn resolve_template_path(config: &Config, name: &str) -> Result<PathBuf> {
    let template_dir = config.template_dir();
    let candidates = [
        template_dir.join(format!("{name}.md")),
        template_dir.join(name),
    ];

    for candidate in candidates {
        if candidate.exists() {
            return Ok(candidate);
        }
    }

    anyhow::bail!("Template not found: {name}");
}

fn to_api_error<E>(context: &'static str) -> impl FnOnce(E) -> ApiError
where
    E: std::fmt::Display,
{
    move |err| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, format!("{context}: {err}"))
}
