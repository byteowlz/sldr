//! API request/response models

use serde::{Deserialize, Serialize};
use sldr_core::flavor::Flavor;
use sldr_core::presentation::{Skeleton, SlidevConfig};
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
    pub template: Option<String>,
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
pub struct SkeletonsResponse {
    pub skeletons: Vec<Skeleton>,
}

#[derive(Debug, Deserialize)]
pub struct CreateSkeletonRequest {
    pub name: String,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub slides: Vec<String>,
    #[serde(default)]
    pub flavor: Option<String>,
    /// Kept for backwards compatibility with existing API consumers
    #[serde(default)]
    pub slidev_config: SlidevConfig,
}

#[derive(Debug, Serialize)]
pub struct FlavorsResponse {
    pub flavors: Vec<Flavor>,
}

#[derive(Debug, Deserialize)]
pub struct BuildRequest {
    pub skeleton: String,
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
pub struct TemplateEditResponse {
    pub session_id: Uuid,
    pub url: String,
    pub port: u16,
}
