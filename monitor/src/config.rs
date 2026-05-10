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
