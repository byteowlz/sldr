//! sldr-core - Core library for the sldr presentation manager
//!
//! This crate provides the core functionality for managing markdown-based
//! presentations powered by slidev.

pub mod config;
pub mod error;
pub mod fuzzy;
pub mod slide;
pub mod presentation;
pub mod flavor;

pub use config::Config;
pub use error::{Error, Result};
