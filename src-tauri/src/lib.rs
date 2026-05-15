pub mod commands;
pub mod config;
pub mod proxy;
pub mod store;

use config::Config;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use store::LogStore;
use tauri::Manager;
use tokio::sync::watch;

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Mutex<Config>>,
    pub config_path: PathBuf,
    pub log_store: LogStore,
    pub http_client: reqwest::Client,
    pub proxy_running: Arc<Mutex<bool>>,
    pub shutdown_tx: Arc<Mutex<Option<watch::Sender<bool>>>>,
}

pub fn run() {
    let config_path = get_config_path();
    let config = Config::load(&config_path).unwrap_or_default();
    let log_store = LogStore::new(config.proxy.log_capacity);

    let state = AppState {
        config: Arc::new(Mutex::new(config)),
        config_path,
        log_store,
        http_client: reqwest::Client::builder()
            .danger_accept_invalid_certs(true)
            .pool_max_idle_per_host(32)
            .build()
            .unwrap(),
        proxy_running: Arc::new(Mutex::new(false)),
        shutdown_tx: Arc::new(Mutex::new(None)),
    };

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            commands::get_config,
            commands::save_config,
            commands::get_logs,
            commands::clear_logs,
            commands::get_proxy_status,
            commands::start_proxy,
            commands::stop_proxy,
            commands::add_op_log,
            commands::test_provider_models,
            commands::test_mapping,
            commands::fetch_provider_models,
            commands::generate_self_signed_cert,
            commands::import_cc_switch_config,
            commands::heartbeat_check,
        ])
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                window.hide().ok();
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

pub async fn run_headless(config_path: PathBuf, port: Option<u16>) {
    let config = Config::load(&config_path).unwrap_or_default();
    let actual_port = port.unwrap_or(config.proxy.port);
    let host = config.proxy.host.clone();
    let log_store = LogStore::new(config.proxy.log_capacity);

    let state = AppState {
        config: Arc::new(Mutex::new(config)),
        config_path,
        log_store,
        http_client: reqwest::Client::builder()
            .danger_accept_invalid_certs(true)
            .pool_max_idle_per_host(32)
            .build()
            .unwrap(),
        proxy_running: Arc::new(Mutex::new(true)),
        shutdown_tx: Arc::new(Mutex::new(None)),
    };

    let router = proxy::create_router(state);
    let addr = format!("{}:{}", host, actual_port);
    println!("cc-proxy running in headless mode on {}", addr);

    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, router).await.unwrap();
}

fn get_config_path() -> PathBuf {
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."));

    let config_in_exe_dir = exe_dir.join("config.toml");
    if config_in_exe_dir.exists() {
        return config_in_exe_dir;
    }

    let cwd_config = PathBuf::from("config.toml");
    if cwd_config.exists() {
        return cwd_config;
    }

    config_in_exe_dir
}
