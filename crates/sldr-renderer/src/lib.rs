//! sldr-renderer - Compile markdown slides into self-contained HTML
//!
//! This crate takes parsed slides, templates, and flavors from sldr-core
//! and compiles them into a single HTML file with all CSS and JS inlined.
//!
//! # Flavor embedding
//!
//! - Single flavor (default): one `<style data-flavor>` block, no selector UI
//! - Multi-flavor: multiple `<style data-flavor>` blocks, presenter shows dropdown
//!
//! # Architecture
//!
//! ```text
//! Slide .md -> parse_frontmatter -> pulldown-cmark -> syntect -> HTML fragment
//!                                                                     |
//! Flavor .toml -> to_css_variables() -> <style data-flavor>           |
//!                                                       \            |
//!                                                        +-> HtmlRenderer::render()
//!                                                       /            |
//! base.css + presenter.js (include_str!)  -------------+             |
//!                                                                     v
//!                                                          Single .html file
//! ```

pub mod media;
mod markdown;
pub mod pptx;
mod render;
mod template;

pub use media::ImageMode;
pub use render::{HtmlRenderer, RenderConfig};
