use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub default: DefaultConfig,
    pub providers: HashMap<String, ProviderConfig>,
    pub search: SearchConfig,
    pub session: SessionConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DefaultConfig {
    pub provider: String,
    pub model: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum AuthStyle {
    #[default]
    XApiKey,
    Bearer,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub api_key_env: String,
    pub base_url: String,
    #[serde(default)]
    pub auth_style: AuthStyle,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchConfig {
    pub ignore_patterns: Vec<String>,
    pub max_results: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionConfig {
    pub storage_dir: String,
}

impl AppConfig {
    pub fn config_dir() -> Result<PathBuf> {
        let base = dirs::config_dir().ok_or_else(|| anyhow!("Could not determine config directory"))?;
        Ok(base.join("oh-my-code"))
    }

    pub fn config_path() -> Result<PathBuf> {
        Ok(Self::config_dir()?.join("config.toml"))
    }

    pub fn load() -> Result<Self> {
        let path = Self::config_path()?;
        if path.exists() {
            let content = std::fs::read_to_string(&path)
                .with_context(|| format!("Failed to read config file at {}", path.display()))?;
            Self::load_from_str(&content)
        } else {
            let config = Self::default_config();
            let dir = Self::config_dir()?;
            std::fs::create_dir_all(&dir)
                .with_context(|| format!("Failed to create config directory {}", dir.display()))?;
            let content = toml::to_string_pretty(&config)
                .context("Failed to serialize default config")?;
            std::fs::write(&path, &content)
                .with_context(|| format!("Failed to write default config to {}", path.display()))?;
            Ok(config)
        }
    }

    pub fn load_from_str(content: &str) -> Result<Self> {
        toml::from_str(content).context("Failed to parse config TOML")
    }

    pub fn default_config() -> Self {
        let mut providers = HashMap::new();
        providers.insert(
            "claude".to_string(),
            ProviderConfig {
                api_key_env: "ANTHROPIC_API_KEY".to_string(),
                base_url: "https://api.anthropic.com".to_string(),
                auth_style: AuthStyle::XApiKey,
            },
        );
        providers.insert(
            "openai".to_string(),
            ProviderConfig {
                api_key_env: "OPENAI_API_KEY".to_string(),
                base_url: "https://api.openai.com".to_string(),
                auth_style: AuthStyle::XApiKey,
            },
        );
        providers.insert(
            "zhipu".to_string(),
            ProviderConfig {
                api_key_env: "ZHIPU_API_KEY".to_string(),
                base_url: "https://open.bigmodel.cn/api/paas/v4".to_string(),
                auth_style: AuthStyle::XApiKey,
            },
        );
        providers.insert(
            "minimax".to_string(),
            ProviderConfig {
                api_key_env: "MINIMAX_API_KEY".to_string(),
                base_url: "https://api.minimax.chat/v1".to_string(),
                auth_style: AuthStyle::XApiKey,
            },
        );

        AppConfig {
            default: DefaultConfig {
                provider: "claude".to_string(),
                model: "claude-sonnet-4-20250514".to_string(),
            },
            providers,
            search: SearchConfig {
                ignore_patterns: vec![
                    "node_modules".to_string(),
                    ".git".to_string(),
                    "dist".to_string(),
                    "build".to_string(),
                    "target".to_string(),
                ],
                max_results: 500,
            },
            session: SessionConfig {
                storage_dir: "~/.config/oh-my-code/sessions".to_string(),
            },
        }
    }

    pub fn active_provider_config(&self) -> Result<&ProviderConfig> {
        let provider_name = &self.default.provider;
        self.providers
            .get(provider_name)
            .ok_or_else(|| anyhow!("Provider '{}' not found in config", provider_name))
    }

    pub fn resolve_api_key(&self) -> Result<String> {
        let provider = self.active_provider_config()?;
        std::env::var(&provider.api_key_env).with_context(|| {
            format!(
                "Environment variable '{}' not set for provider '{}'",
                provider.api_key_env, self.default.provider
            )
        })
    }

    pub fn resolved_session_dir(&self) -> PathBuf {
        let storage_dir = &self.session.storage_dir;
        if let Some(rest) = storage_dir.strip_prefix("~/") {
            if let Some(home) = dirs::home_dir() {
                return home.join(rest);
            }
        }
        PathBuf::from(storage_dir)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_default_config() {
        let content = include_str!("../config/default.toml");
        let config = AppConfig::load_from_str(content).expect("Should parse default.toml");
        assert_eq!(config.default.provider, "claude");
        assert_eq!(config.providers.len(), 4);
        assert_eq!(config.search.ignore_patterns.len(), 5);
    }

    #[test]
    fn test_active_provider_config() {
        let config = AppConfig::default_config();
        let provider = config.active_provider_config().expect("Should get active provider");
        assert_eq!(provider.api_key_env, "ANTHROPIC_API_KEY");
        assert_eq!(provider.base_url, "https://api.anthropic.com");
    }

    #[test]
    fn test_active_provider_missing() {
        let mut config = AppConfig::default_config();
        config.default.provider = "nonexistent".to_string();
        let result = config.active_provider_config();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("nonexistent"));
    }

    #[test]
    fn test_resolved_session_dir_tilde() {
        let config = AppConfig::default_config();
        let resolved = config.resolved_session_dir();
        let resolved_str = resolved.to_string_lossy();
        assert!(!resolved_str.starts_with("~/"), "Tilde should be expanded, got: {}", resolved_str);
        assert!(resolved_str.contains(".config/oh-my-code/sessions"));
    }

    #[test]
    fn test_default_config_serializes() {
        let config = AppConfig::default_config();
        let serialized = toml::to_string_pretty(&config).expect("Should serialize");
        let deserialized = AppConfig::load_from_str(&serialized).expect("Should deserialize");
        assert_eq!(deserialized.default.provider, config.default.provider);
        assert_eq!(deserialized.default.model, config.default.model);
        assert_eq!(deserialized.providers.len(), config.providers.len());
        assert_eq!(
            deserialized.search.ignore_patterns,
            config.search.ignore_patterns
        );
        assert_eq!(deserialized.session.storage_dir, config.session.storage_dir);
    }

    #[test]
    fn test_provider_config_auth_style_defaults_to_x_api_key() {
        // Legacy TOML without auth_style must still parse; default is XApiKey.
        let content = r#"
[default]
provider = "claude"
model = "claude-sonnet-4-20250514"

[providers.claude]
api_key_env = "ANTHROPIC_API_KEY"
base_url = "https://api.anthropic.com"

[search]
ignore_patterns = []
max_results = 100

[session]
storage_dir = "/tmp"
"#;
        let config = AppConfig::load_from_str(content).expect("Should parse");
        let p = config.providers.get("claude").expect("claude provider");
        assert_eq!(p.auth_style, AuthStyle::XApiKey);
    }

    #[test]
    fn test_provider_config_auth_style_bearer() {
        let content = r#"
[default]
provider = "x"
model = "y"

[providers.x]
api_key_env = "TOK"
base_url = "https://example.com"
auth_style = "bearer"

[search]
ignore_patterns = []
max_results = 100

[session]
storage_dir = "/tmp"
"#;
        let config = AppConfig::load_from_str(content).expect("Should parse");
        let p = config.providers.get("x").expect("x provider");
        assert_eq!(p.auth_style, AuthStyle::Bearer);
    }
}
