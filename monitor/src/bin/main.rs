use std::fs;
use std::sync::Arc;
use woot_monitor::config::Config;
use woot_monitor::monitor::instance::MonitorInstance;
use woot_monitor::proxy::manager::ProxyManager;
use woot_monitor::utils::{log, LogLevel};
use woot_monitor::webhook::WebhookManager;

fn load_config(path: &str) -> Config {
    let contents = fs::read_to_string(path).expect("Failed to read config file");
    serde_yaml::from_str(&contents).expect("Invalid YAML config")
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();
    log(
        LogLevel::Info,
        format!("Welcome to Woot Monitor {}", env!("CARGO_PKG_VERSION")),
    );

    let config = load_config("config.yaml");
    log(
        LogLevel::Info,
        format!("Loaded config: {:?} senders", config.webhooks.len()),
    );

    let webhook_proxy_manager = Arc::new(ProxyManager::new_from_file("proxies/proxies.txt"));
    let monitor_proxy_manager = Arc::new(ProxyManager::new_from_file("proxies/proxies.txt"));
    log(
        LogLevel::Info,
        format!(
            "Loaded {} proxies from {} for webhooks",
            webhook_proxy_manager.count(),
            webhook_proxy_manager
                .filename()
                .unwrap_or("unknown".to_string())
        ),
    );
    log(
        LogLevel::Info,
        format!(
            "Loaded {} proxies from {} for monitor",
            monitor_proxy_manager.count(),
            monitor_proxy_manager
                .filename()
                .unwrap_or("unknown".to_string())
        ),
    );

    let mut webhook_manager = WebhookManager::new(webhook_proxy_manager);
    webhook_manager.register_from_configs(config.webhooks);
    log(LogLevel::Info, "Created webhook manager");

    let mut monitor = MonitorInstance::new(webhook_manager, monitor_proxy_manager);
    monitor.start().await;

    Ok(())
}
