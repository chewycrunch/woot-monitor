use std::fs;
use std::sync::Arc;
use tracing::info;
use woot_monitor::config::Config;
use woot_monitor::logging;
use woot_monitor::monitor::instance::MonitorInstance;
use woot_monitor::proxy::ProxyManager;
use woot_monitor::webhook::WebhookManager;

fn load_config(path: &str) -> Config {
    let contents = fs::read_to_string(path).expect("Failed to read config file");
    serde_yaml::from_str(&contents).expect("Invalid YAML config")
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();
    logging::init();

    info!(
        version = env!("CARGO_PKG_VERSION"),
        "Welcome to Woot Monitor"
    );

    let config = load_config("config.yaml");
    info!(senders = config.webhooks.len(), "Loaded config");

    let webhook_proxy_manager = Arc::new(ProxyManager::new_from_file("proxies/proxies.txt"));
    let monitor_proxy_manager = Arc::new(ProxyManager::new_from_file("proxies/proxies.txt"));
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

    let mut monitor = MonitorInstance::new(webhook_manager, monitor_proxy_manager);
    monitor.start().await;

    Ok(())
}
