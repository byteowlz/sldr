//! API request/response models

use serde::{Deserialize, Serialize};
use sldr_core::flavor::Flavor;
use sldr_core::presentation::{Playlist, RenderOpts};
use sldr_core::slide::SlideMetadata;
use uuid::Uuid;

#[derive(Debug, Serialize)]
pub struct SlideSummary {
    pub name: String,
    pub relative_path: String,
    pub metadata: SlideMetadata,
}

#[derive(Debug, Serialize)]
pub struct SlideDetail {
    pub name: String,
    pub relative_path: String,
    pub metadata: SlideMetadata,
    pub content: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateSlideRequest {
    pub name: String,
    #[serde(default)]
    pub content: Option<String>,
    #[serde(default)]
    pub metadata: Option<SlideMetadata>,
    #[serde(default)]
    pub scaffold: Option<String>,
    #[serde(default)]
    pub subdir: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateSlideRequest {
    #[serde(default)]
    pub content: Option<String>,
    #[serde(default)]
    pub metadata: Option<SlideMetadata>,
}

#[derive(Debug, Serialize)]
pub struct SlidesResponse {
    pub slides: Vec<SlideSummary>,
}

#[derive(Debug, Serialize)]
pub struct LayoutSummary {
    pub name: String,
    pub category: Option<String>,
    /// True for a shipped layout, false for one in the user's library.
    pub builtin: bool,
    pub zone_count: usize,
}

#[derive(Debug, Serialize)]
pub struct LayoutsResponse {
    pub layouts: Vec<LayoutSummary>,
}

#[derive(Debug, Serialize)]
pub struct LayoutDetail {
    pub name: String,
    pub category: Option<String>,
    pub builtin: bool,
    /// The layout's HTML source (markup + scoped `<style>` + directives).
    pub source: String,
    /// Parsed PPTX-export zones (for the visual zone editor).
    pub zones: Vec<sldr_renderer::Zone>,
}

/// Overwrite a layout's whole source (the HTML/CSS editor save path).
#[derive(Debug, Deserialize)]
pub struct UpdateLayoutRequest {
    pub source: String,
}

/// Rewrite only a layout's zone directives, leaving markup/CSS untouched
/// (the visual zone editor save path).
#[derive(Debug, Deserialize)]
pub struct UpdateZonesRequest {
    pub zones: Vec<sldr_renderer::Zone>,
}

#[derive(Debug, Serialize)]
pub struct PlaylistsResponse {
    pub playlists: Vec<Playlist>,
}

#[derive(Debug, Deserialize)]
pub struct CreatePlaylistRequest {
    pub name: String,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub slides: Vec<String>,
    #[serde(default)]
    pub flavor: Option<String>,
    /// Rendering options (legacy `slidev_config` key still accepted)
    #[serde(default, alias = "slidev_config")]
    pub render: RenderOpts,
}

#[derive(Debug, Serialize)]
pub struct FlavorsResponse {
    pub flavors: Vec<Flavor>,
}

#[derive(Debug, Deserialize)]
pub struct BuildRequest {
    pub playlist: String,
    #[serde(default)]
    pub flavor: Option<String>,
    #[serde(default)]
    pub output: Option<String>,
    #[serde(default)]
    pub pdf: bool,
    #[serde(default)]
    pub pptx: bool,
}

#[derive(Debug, Serialize)]
pub struct BuildResponse {
    pub name: String,
    pub output_dir: String,
    pub html_path: String,
}

#[derive(Debug, Serialize)]
pub struct PreviewResponse {
    pub session_id: Uuid,
    pub url: String,
    pub port: u16,
}

#[derive(Debug, Deserialize)]
pub struct PreviewRequest {
    #[serde(default)]
    pub flavor: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ScaffoldEditResponse {
    pub session_id: Uuid,
    pub url: String,
    pub port: u16,
}
