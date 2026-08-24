use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub webhooks: Vec<WebhookConfig>,
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

    /// The example is the only committed copy of the config shape, so a schema
    /// change that forgets it would otherwise surface as a startup panic on a
    /// fresh checkout rather than a failing test.
    #[test]
    fn the_shipped_example_parses() {
        let contents = include_str!("../config.example.yaml");
        let config: Config =
            serde_yaml::from_str(contents).expect("config.example.yaml is invalid");

        assert!(!config.webhooks.is_empty());
        assert!(config
            .webhooks
            .iter()
            .all(|w| w.keywords.is_some() && w.asins.is_some()));
    }
}
