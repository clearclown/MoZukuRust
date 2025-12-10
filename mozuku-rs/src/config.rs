//! Configuration management for MoZuku LSP server
//!
//! Handles loading and parsing of `mozuku.toml` configuration file.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Main configuration structure
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    /// LLM provider settings
    #[serde(default)]
    pub llm: LlmConfig,

    /// Grammar checker settings
    #[serde(default)]
    pub checker: CheckerConfig,
}

/// LLM provider configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmConfig {
    /// LLM provider: "claude", "openai", or "none"
    #[serde(default = "default_provider")]
    pub provider: String,

    /// API key (can also be set via environment variable)
    #[serde(default)]
    pub api_key: Option<String>,

    /// Model name (e.g., "claude-3-5-sonnet-20241022", "gpt-4o")
    #[serde(default)]
    pub model: Option<String>,

    /// API base URL (for custom endpoints)
    #[serde(default)]
    pub base_url: Option<String>,

    /// Maximum tokens for response
    #[serde(default = "default_max_tokens")]
    pub max_tokens: u32,
}

impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            provider: default_provider(),
            api_key: None,
            model: None,
            base_url: None,
            max_tokens: default_max_tokens(),
        }
    }
}

/// Grammar checker configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckerConfig {
    /// Enable ら抜き言葉 detection
    #[serde(default = "default_true")]
    pub ra_nuki: bool,

    /// Enable い抜き言葉 detection
    #[serde(default = "default_true")]
    pub i_nuki: bool,

    /// Enable double particle detection
    #[serde(default = "default_true")]
    pub double_particle: bool,

    /// Enable double honorific detection
    #[serde(default = "default_true")]
    pub double_honorific: bool,

    /// Enable redundant expression detection
    #[serde(default = "default_true")]
    pub redundant_expression: bool,

    /// Enable consecutive sentence endings detection
    #[serde(default = "default_true")]
    pub consecutive_endings: bool,

    /// Enable incomplete たり parallel detection
    #[serde(default = "default_true")]
    pub tari_parallel: bool,

    /// Enable consecutive の detection
    #[serde(default = "default_true")]
    pub consecutive_no: bool,
}

impl Default for CheckerConfig {
    fn default() -> Self {
        Self {
            ra_nuki: true,
            i_nuki: true,
            double_particle: true,
            double_honorific: true,
            redundant_expression: true,
            consecutive_endings: true,
            tari_parallel: true,
            consecutive_no: true,
        }
    }
}

fn default_provider() -> String {
    "none".to_string()
}

fn default_max_tokens() -> u32 {
    1024
}

fn default_true() -> bool {
    true
}

impl Config {
    /// Load configuration from file
    pub fn load(path: &PathBuf) -> Result<Self> {
        if path.exists() {
            let content = std::fs::read_to_string(path)?;
            let config: Config = toml::from_str(&content)?;
            Ok(config)
        } else {
            Ok(Config::default())
        }
    }

    /// Get default config file path
    pub fn default_path() -> Option<PathBuf> {
        directories::ProjectDirs::from("", "", "mozuku").map(|dirs| dirs.config_dir().join("mozuku.toml"))
    }

    /// Load configuration from default path or workspace
    pub fn load_from_default() -> Self {
        // Try workspace path first
        let workspace_path = PathBuf::from("mozuku.toml");
        if workspace_path.exists() {
            if let Ok(config) = Self::load(&workspace_path) {
                return config;
            }
        }

        // Try user config directory
        if let Some(default_path) = Self::default_path() {
            if let Ok(config) = Self::load(&default_path) {
                return config;
            }
        }

        Config::default()
    }

    /// Get the effective API key (from config or environment)
    pub fn get_api_key(&self) -> Option<String> {
        // First check config file
        if let Some(ref key) = self.llm.api_key {
            if !key.is_empty() {
                return Some(key.clone());
            }
        }

        // Then check environment variables
        match self.llm.provider.as_str() {
            "claude" => std::env::var("ANTHROPIC_API_KEY").ok(),
            "openai" => std::env::var("OPENAI_API_KEY").ok(),
            _ => None,
        }
    }

    /// Get the effective model name
    pub fn get_model(&self) -> String {
        self.llm
            .model
            .clone()
            .unwrap_or_else(|| match self.llm.provider.as_str() {
                "claude" => "claude-3-5-sonnet-20241022".to_string(),
                "openai" => "gpt-4o".to_string(),
                _ => String::new(),
            })
    }

    /// Check if LLM integration is enabled
    pub fn is_llm_enabled(&self) -> bool {
        self.llm.provider != "none" && self.get_api_key().is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();

        assert_eq!(config.llm.provider, "none");
        assert!(config.llm.api_key.is_none());
        assert_eq!(config.llm.max_tokens, 1024);
        assert!(config.checker.ra_nuki);
        assert!(config.checker.double_honorific);
    }

    #[test]
    fn test_parse_minimal_toml() {
        let toml_str = r#"
[llm]
provider = "claude"
"#;
        let config: Config = toml::from_str(toml_str).unwrap();

        assert_eq!(config.llm.provider, "claude");
        assert!(config.llm.api_key.is_none());
        assert!(config.checker.ra_nuki); // defaults to true
    }

    #[test]
    fn test_parse_full_toml() {
        let toml_str = r#"
[llm]
provider = "openai"
api_key = "sk-test-key"
model = "gpt-4o-mini"
max_tokens = 2048

[checker]
ra_nuki = true
i_nuki = false
double_particle = true
double_honorific = true
redundant_expression = false
consecutive_endings = true
tari_parallel = true
consecutive_no = false
"#;
        let config: Config = toml::from_str(toml_str).unwrap();

        assert_eq!(config.llm.provider, "openai");
        assert_eq!(config.llm.api_key, Some("sk-test-key".to_string()));
        assert_eq!(config.llm.model, Some("gpt-4o-mini".to_string()));
        assert_eq!(config.llm.max_tokens, 2048);

        assert!(config.checker.ra_nuki);
        assert!(!config.checker.i_nuki);
        assert!(config.checker.double_particle);
        assert!(!config.checker.redundant_expression);
        assert!(!config.checker.consecutive_no);
    }

    #[test]
    fn test_get_model_defaults() {
        let mut config = Config::default();

        config.llm.provider = "claude".to_string();
        assert_eq!(config.get_model(), "claude-3-5-sonnet-20241022");

        config.llm.provider = "openai".to_string();
        assert_eq!(config.get_model(), "gpt-4o");

        config.llm.model = Some("custom-model".to_string());
        assert_eq!(config.get_model(), "custom-model");
    }

    #[test]
    fn test_is_llm_enabled() {
        let mut config = Config::default();

        // Default: disabled (provider = "none")
        assert!(!config.is_llm_enabled());

        // Provider set but no API key
        config.llm.provider = "claude".to_string();
        assert!(!config.is_llm_enabled());

        // Provider and API key set
        config.llm.api_key = Some("test-key".to_string());
        assert!(config.is_llm_enabled());
    }

    #[test]
    fn test_load_nonexistent_file() {
        let path = PathBuf::from("/nonexistent/path/mozuku.toml");
        let config = Config::load(&path).unwrap();

        // Should return default config
        assert_eq!(config.llm.provider, "none");
    }

    #[test]
    fn test_checker_config_all_enabled() {
        let config = CheckerConfig::default();

        assert!(config.ra_nuki);
        assert!(config.i_nuki);
        assert!(config.double_particle);
        assert!(config.double_honorific);
        assert!(config.redundant_expression);
        assert!(config.consecutive_endings);
        assert!(config.tari_parallel);
        assert!(config.consecutive_no);
    }

    #[test]
    fn test_serialize_config() {
        let config = Config::default();
        let toml_str = toml::to_string(&config).unwrap();

        assert!(toml_str.contains("[llm]"));
        assert!(toml_str.contains("[checker]"));
    }
}
