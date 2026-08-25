use config::{ConfigError, Environment, File, FileFormat};
use serde::Deserialize;

use crate::monitor::tls_api::{DEFAULT_TLS_API_KEY, DEFAULT_TLS_API_URL};
use crate::monitor::woot_api::DEFAULT_GRAPHQL_API_KEY;

/// Prefix for env vars that override config values, e.g. `WOOT_GRAPHQL_API_KEY`.
const ENV_PREFIX: &str = "WOOT";

fn default_graphql_api_key() -> String {
    DEFAULT_GRAPHQL_API_KEY.to_string()
}

/// Woot lists offers far slower than this, so the interval is about how quickly
/// a new one is noticed, traded against request volume.
const DEFAULT_DELAY_MS: u64 = 5_000;

fn default_delay_ms() -> u64 {
    DEFAULT_DELAY_MS
}

fn default_tls_api_url() -> String {
    DEFAULT_TLS_API_URL.to_string()
}

fn default_tls_api_key() -> String {
    DEFAULT_TLS_API_KEY.to_string()
}

#[derive(Debug, Deserialize)]
pub struct Config {
    /// Woot's public AppSync key, normally supplied by WOOT_GRAPHQL_API_KEY.
    /// Defaulted, so config.toml carries webhooks only.
    #[serde(default = "default_graphql_api_key")]
    pub graphql_api_key: String,

    /// Wait between polls, normally supplied by WOOT_DELAY_MS.
    #[serde(default = "default_delay_ms")]
    pub delay_ms: u64,

    /// Where the tls-client sidecar listens, supplied by WOOT_TLS_API_URL.
    #[serde(default = "default_tls_api_url")]
    pub tls_api_url: String,

    /// Must match the sidecar's API_AUTH_KEYS, supplied by WOOT_TLS_API_KEY.
    #[serde(default = "default_tls_api_key")]
    pub tls_api_key: String,

    pub webhooks: Vec<WebhookConfig>,
}

impl Config {
    // @spec CONFIG-001, CONFIG-002, CONFIG-003, CONFIG-006, CONFIG-011, CONFIG-012, CONFIG-021, CONFIG-022
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

    /// Offers the ASIN and review-count scrape could not vouch for, including
    /// ones where the scrape itself came back empty.
    #[serde(alias = "junk_url")]
    pub unverified_url: Option<String>,

    /// Offers with an ASIN and at least [`MIN_REVIEWS`] reviews.
    #[serde(alias = "unfiltered_url")]
    pub verified_url: Option<String>,

    /// Offers matching `keywords` or `asins` below.
    #[serde(alias = "filtered_url")]
    pub watchlist_url: Option<String>,

    pub keywords: Option<Vec<String>>,
    pub asins: Option<Vec<String>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    // @spec CONFIG-010, CONFIG-012
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
        assert_eq!(config.delay_ms, DEFAULT_DELAY_MS);
        assert_eq!(config.tls_api_url, DEFAULT_TLS_API_URL);
        assert_eq!(config.tls_api_key, DEFAULT_TLS_API_KEY);
        assert!(!config.webhooks.is_empty());
        assert!(config
            .webhooks
            .iter()
            .all(|w| w.keywords.is_some() && w.asins.is_some()));
        assert!(config.webhooks.iter().all(|w| w.verified_url.is_some()
            && w.unverified_url.is_some()
            && w.watchlist_url.is_some()));
    }

    // @spec CONFIG-022
    /// A deployed config.toml still uses the old key names, so the aliases have
    /// to keep routing each one to the same channel it always meant.
    #[test]
    fn the_old_webhook_key_names_still_load() {
        let config: Config = config::Config::builder()
            .add_source(File::from_str(
                r#"
                [[webhooks]]
                name = "legacy"
                junk_url = "https://example.invalid/junk"
                unfiltered_url = "https://example.invalid/unfiltered"
                filtered_url = "https://example.invalid/filtered"
                "#,
                FileFormat::Toml,
            ))
            .build()
            .and_then(config::Config::try_deserialize)
            .expect("legacy webhook keys should still parse");

        let hook = &config.webhooks[0];
        assert_eq!(
            hook.unverified_url.as_deref(),
            Some("https://example.invalid/junk")
        );
        assert_eq!(
            hook.verified_url.as_deref(),
            Some("https://example.invalid/unfiltered")
        );
        assert_eq!(
            hook.watchlist_url.as_deref(),
            Some("https://example.invalid/filtered")
        );
    }

    // @spec CONFIG-002, CONFIG-003
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

    // @spec CONFIG-002
    /// The delay drives request volume: ~50 paged requests per poll makes the
    /// interval the main lever on how hard the proxies are worked.
    #[test]
    fn an_env_var_overrides_the_delay() {
        std::env::set_var("WOOT_DELAY_MS", "30000");

        let config: Config = config::Config::builder()
            .add_source(File::from_str(
                include_str!("../config.example.toml"),
                FileFormat::Toml,
            ))
            .add_source(Environment::with_prefix(ENV_PREFIX))
            .build()
            .and_then(config::Config::try_deserialize)
            .expect("config with env override is invalid");

        std::env::remove_var("WOOT_DELAY_MS");

        assert_eq!(config.delay_ms, 30_000);
    }

    // @spec CONFIG-002
    /// The sidecar takes the same key through its own API_AUTH_KEYS, so one
    /// stack-level value has to reach both containers.
    #[test]
    fn env_vars_override_the_tls_api_settings() {
        std::env::set_var("WOOT_TLS_API_URL", "http://tls-client:8080");
        std::env::set_var("WOOT_TLS_API_KEY", "fromenv");

        let config: Config = config::Config::builder()
            .add_source(File::from_str(
                include_str!("../config.example.toml"),
                FileFormat::Toml,
            ))
            .add_source(Environment::with_prefix(ENV_PREFIX))
            .build()
            .and_then(config::Config::try_deserialize)
            .expect("config with env override is invalid");

        std::env::remove_var("WOOT_TLS_API_URL");
        std::env::remove_var("WOOT_TLS_API_KEY");

        assert_eq!(config.tls_api_url, "http://tls-client:8080");
        assert_eq!(config.tls_api_key, "fromenv");
    }
}
