//! Server state - shared across all routes

use std::sync::Arc;

use anyhow::Result;
use sldr_core::config::Config;

use crate::preview::PreviewManager;

#[derive(Clone)]
pub struct SldrState {
    pub config: Arc<Config>,
    pub preview: PreviewManager,
}

impl SldrState {
    pub fn new(config: Config) -> Self {
        Self {
            config: Arc::new(config),
            preview: PreviewManager::new(),
        }
    }

    pub fn load() -> Result<Self> {
        let config = Config::load()?;
        Ok(Self::new(config))
    }

    #[must_use]
    pub fn load_or_default() -> Self {
        match Config::load() {
            Ok(config) => Self::new(config),
            Err(_) => Self::new(Config::default()),
        }
    }
}
