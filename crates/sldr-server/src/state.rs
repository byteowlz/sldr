use std::sync::Arc;

use anyhow::Result;
use sldr_core::config::Config;

use crate::slidev::SlidevManager;

#[derive(Clone)]
pub struct SldrState {
    pub config: Arc<Config>,
    pub slidev: SlidevManager,
}

impl SldrState {
    pub fn new(config: Config) -> Self {
        Self {
            config: Arc::new(config),
            slidev: SlidevManager::new(),
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
