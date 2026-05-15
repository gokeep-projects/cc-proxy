#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use clap::Parser;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "cc-proxy", version = "2.0.0", about = "AI Model Proxy Router - Claude/Codex to OpenAI-compatible providers")]
struct Cli {
    /// Path to config file
    #[arg(long, short, default_value = "config.toml")]
    config: PathBuf,

    /// Run in headless mode (no GUI, no tray)
    #[arg(long)]
    headless: bool,

    /// Override proxy host
    #[arg(long)]
    host: Option<String>,

    /// Override proxy port
    #[arg(long, short)]
    port: Option<u16>,

    /// Override log capacity
    #[arg(long)]
    log_capacity: Option<usize>,
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    if cli.headless || std::env::var("CC_PROXY_HEADLESS").is_ok() {
        // Apply CLI overrides
        let mut config = cc_proxy_lib::config::Config::load(&cli.config).unwrap_or_default();
        if let Some(h) = cli.host { config.proxy.host = h; }
        if let Some(p) = cli.port { config.proxy.port = p; }
        if let Some(c) = cli.log_capacity { config.proxy.log_capacity = c; }

        let host = config.proxy.host.clone();
        let port = config.proxy.port;
        let log_store = cc_proxy_lib::store::LogStore::new(config.proxy.log_capacity);

        let state = cc_proxy_lib::AppState {
            config: std::sync::Arc::new(std::sync::Mutex::new(config)),
            config_path: cli.config,
            log_store,
            http_client: reqwest::Client::builder()
                .danger_accept_invalid_certs(true)
                .pool_max_idle_per_host(32)
                .build()
                .unwrap(),
            proxy_running: std::sync::Arc::new(std::sync::Mutex::new(true)),
            shutdown_tx: std::sync::Arc::new(std::sync::Mutex::new(None)),
        };

        let router = cc_proxy_lib::proxy::create_router(state);
        let addr = format!("{}:{}", host, port);
        println!("cc-proxy v2.0.0 running on http://{}", addr);

        let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
        axum::serve(listener, router).await.unwrap();
    } else {
        cc_proxy_lib::run();
    }
}
