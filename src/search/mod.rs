pub mod finder;
pub mod highlight;
pub mod ripgrep;

use anyhow::Result;
use ignore::overrides::{Override, OverrideBuilder};
use std::path::Path;

#[derive(Debug, Clone)]
pub struct SearchConfig {
    pub ignore_patterns: Vec<String>,
    pub max_results: usize,
}

pub struct SearchEngine {
    pub config: SearchConfig,
    pub highlighter: highlight::Highlighter,
}

impl SearchEngine {
    pub fn new(config: SearchConfig) -> Result<Self> {
        let highlighter = highlight::Highlighter::new()?;
        Ok(Self {
            config,
            highlighter,
        })
    }

    pub fn ignore_overrides(&self, base_path: &Path) -> Result<Override> {
        let mut builder = OverrideBuilder::new(base_path);
        for pattern in &self.config.ignore_patterns {
            builder.add(&format!("!{}", pattern))?;
        }
        Ok(builder.build()?)
    }
}

impl From<&crate::config::SearchConfig> for SearchConfig {
    fn from(cfg: &crate::config::SearchConfig) -> Self {
        Self {
            ignore_patterns: cfg.ignore_patterns.clone(),
            max_results: cfg.max_results,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_search_engine_new() {
        let config = SearchConfig {
            ignore_patterns: vec!["node_modules".to_string(), ".git".to_string()],
            max_results: 42,
        };
        let engine = SearchEngine::new(config).expect("Should create SearchEngine");
        assert_eq!(engine.config.max_results, 42);
        assert_eq!(engine.config.ignore_patterns.len(), 2);
    }
}
