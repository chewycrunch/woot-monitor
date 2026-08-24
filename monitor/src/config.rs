use config::{ConfigError, Environment, File, FileFormat};
use serde::Deserialize;

use crate::monitor::woot_api::DEFAULT_GRAPHQL_API_KEY;

/// Prefix for env vars that override config values, e.g. `WOOT_GRAPHQL_API_KEY`.
const ENV_PREFIX: &str = "WOOT";

fn default_graphql_api_key() -> String {
    DEFAULT_GRAPHQL_API_KEY.to_string()
}

#[derive(Debug, Deserialize)]
pub struct Config {
    /// Woot's public AppSync key, normally supplied by WOOT_GRAPHQL_API_KEY.
    /// Defaulted, so config.toml carries webhooks only.
    #[serde(default = "default_graphql_api_key")]
    pub graphql_api_key: String,

    pub webhooks: Vec<WebhookConfig>,
}

impl Config {
    /// Loads `path`, then lets `WOOT_*` env vars override what it contains.
    pub fn load(path: &str) -> Result<Self, ConfigError> {
        config::Config::builder()
            .add_source(File::new(path, FileFormat::Toml))
            .add_source(Environment::with_prefix(ENV_PREFIX))
            .build()?
            .try_deserialize()
    }
}

#[derive(Debug, Deserialize)]
pub struct WebhookConfig {
    pub name: String,

    pub junk_url: Option<String>,
    pub unfiltered_url: Option<String>,
    pub filtered_url: Option<String>,

    pub keywords: Option<Vec<String>>,
    pub asins: Option<Vec<String>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The example is the only committed copy of the config shape.
    #[test]
    fn the_shipped_example_parses() {
        let config: Config = config::Config::builder()
            .add_source(File::from_str(
                include_str!("../config.example.toml"),
                FileFormat::Toml,
            ))
            .build()
            .and_then(config::Config::try_deserialize)
            .expect("config.example.toml is invalid");

        assert_eq!(config.graphql_api_key, DEFAULT_GRAPHQL_API_KEY);
        assert!(!config.webhooks.is_empty());
        assert!(config
            .webhooks
            .iter()
            .all(|w| w.keywords.is_some() && w.asins.is_some()));
    }

    /// The point of the env layer: rotating the key needs a restart, not a build.
    #[test]
    fn an_env_var_overrides_the_key_from_the_file() {
        std::env::set_var("WOOT_GRAPHQL_API_KEY", "da2-fromenv");

        let config: Config = config::Config::builder()
            .add_source(File::from_str(
                include_str!("../config.example.toml"),
                FileFormat::Toml,
            ))
            .add_source(Environment::with_prefix(ENV_PREFIX))
            .build()
            .and_then(config::Config::try_deserialize)
            .expect("config with env override is invalid");

        std::env::remove_var("WOOT_GRAPHQL_API_KEY");

        assert_eq!(config.graphql_api_key, "da2-fromenv");
    }
}
