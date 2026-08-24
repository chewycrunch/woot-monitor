use std::sync::Arc;
use tracing::info;
use woot_monitor::config::Config;
use woot_monitor::logging;
use woot_monitor::monitor::Monitor;
use woot_monitor::proxy::ProxyManager;
use woot_monitor::webhook::WebhookManager;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();
    logging::init();

    info!(
        version = env!("CARGO_PKG_VERSION"),
        "Welcome to Woot Monitor"
    );

    // Path is relative to the working directory (/app in the image), where a
    // missing bind mount is the usual cause.
    let config = Config::load("config.toml")
        .unwrap_or_else(|e| panic!("Failed to load config file config.toml: {e}"));
    let graphql_api_key = config.graphql_api_key.clone();
    info!(senders = config.webhooks.len(), "Loaded config");

    let webhook_proxy_manager = Arc::new(ProxyManager::new_from_file("proxies.txt"));
    let monitor_proxy_manager = Arc::new(ProxyManager::new_from_file("proxies.txt"));
    info!(
        count = webhook_proxy_manager.count(),
        file = webhook_proxy_manager
            .filename()
            .unwrap_or("unknown".to_string()),
        "Loaded proxies for webhooks"
    );
    info!(
        count = monitor_proxy_manager.count(),
        file = monitor_proxy_manager
            .filename()
            .unwrap_or("unknown".to_string()),
        "Loaded proxies for monitor"
    );

    let mut webhook_manager = WebhookManager::new(webhook_proxy_manager);
    webhook_manager.register_from_configs(config.webhooks);
    info!("Created webhook manager");

    let mut monitor = Monitor::new(webhook_manager, monitor_proxy_manager, graphql_api_key);
    monitor.start().await;

    Ok(())
}
