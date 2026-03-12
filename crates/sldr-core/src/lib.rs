//! sldr-core - Core library for the sldr presentation manager
//!
//! This crate provides the core functionality for managing markdown-based
//! presentations rendered as self-contained HTML.

pub mod config;
pub mod error;
pub mod fuzzy;
pub mod slide;
pub mod presentation;
pub mod flavor;

pub use config::Config;
pub use error::{Error, Result};
pub use flavor::Flavor;
pub use presentation::{RenderOpts, Skeleton, SlidevConfig};
