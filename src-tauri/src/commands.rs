use tauri::State;
use tokio::sync::watch;

use crate::config::Config;
use crate::proxy;
use crate::store::{LogStore, RequestLog};
use crate::AppState;

#[tauri::command]
pub fn get_config(state: State<'_, AppState>) -> Result<Config, String> {
    Ok(state.config.lock().unwrap().clone())
}

#[tauri::command]
pub fn save_config(state: State<'_, AppState>, config: Config) -> Result<(), String> {
    let path = state.config_path.clone();
    config.save(&path)?;
    *state.config.lock().unwrap() = config;
    Ok(())
}

#[tauri::command]
pub fn get_logs(state: State<'_, AppState>, limit: Option<usize>) -> Result<Vec<RequestLog>, String> {
    Ok(state.log_store.get_recent(limit.unwrap_or(100)))
}

#[tauri::command]
pub fn clear_logs(state: State<'_, AppState>) -> Result<(), String> {
    state.log_store.clear();
    Ok(())
}

#[tauri::command]
pub fn get_proxy_status(state: State<'_, AppState>) -> Result<ProxyStatus, String> {
    let config = state.config.lock().unwrap();
    let running = *state.proxy_running.lock().unwrap();
    Ok(ProxyStatus {
        running,
        host: config.proxy.host.clone(),
        port: config.proxy.port,
    })
}

#[tauri::command]
pub async fn start_proxy(state: State<'_, AppState>) -> Result<String, String> {
    {
        let running = *state.proxy_running.lock().unwrap();
        if running {
            return Err("Proxy is already running".to_string());
        }
    }

    let config = state.config.lock().unwrap().clone();
    let addr = format!("{}:{}", config.proxy.host, config.proxy.port);

    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .map_err(|e| format!("Failed to bind {}: {}", addr, e))?;

    let (tx, mut rx) = watch::channel(false);
    *state.shutdown_tx.lock().unwrap() = Some(tx);
    *state.proxy_running.lock().unwrap() = true;

    let app_state = (*state).clone();

    tokio::spawn(async move {
        let router = proxy::create_router(app_state.clone());
        axum::serve(listener, router)
            .with_graceful_shutdown(async move {
                rx.changed().await.ok();
            })
            .await
            .ok();
        *app_state.proxy_running.lock().unwrap() = false;
        *app_state.shutdown_tx.lock().unwrap() = None;
    });

    Ok(addr)
}

#[tauri::command]
pub fn stop_proxy(state: State<'_, AppState>) -> Result<(), String> {
    let running = *state.proxy_running.lock().unwrap();
    if !running {
        return Err("Proxy is not running".to_string());
    }
    if let Some(tx) = state.shutdown_tx.lock().unwrap().take() {
        tx.send(true).ok();
    }
    *state.proxy_running.lock().unwrap() = false;
    Ok(())
}

#[tauri::command]
pub fn add_op_log(state: State<'_, AppState>, action: String, detail: String) -> Result<(), String> {
    let log = RequestLog {
        id: LogStore::new_id(),
        timestamp: chrono::Utc::now(),
        method: "OP".to_string(),
        path: action,
        model_in: String::new(),
        model_out: String::new(),
        provider: String::new(),
        status: 200,
        latency_ms: 0,
        prompt_tokens: None,
        completion_tokens: None,
        request_body: serde_json::Value::Null,
        response_body: serde_json::Value::String(detail),
    };
    state.log_store.push(log);
    Ok(())
}

#[derive(serde::Serialize)]
pub struct ProxyStatus {
    pub running: bool,
    pub host: String,
    pub port: u16,
}

#[derive(serde::Serialize)]
pub struct TestResult {
    pub success: bool,
    pub message: String,
    pub latency_ms: u64,
}

#[tauri::command]
pub async fn test_provider(state: State<'_, AppState>, provider_id: String) -> Result<TestResult, String> {
    let config = state.config.lock().unwrap().clone();
    let provider = config.providers.iter().find(|p| p.id == provider_id)
        .ok_or("Provider not found")?;

    let url = format!("{}/v1/chat/completions", provider.base_url.trim_end_matches('/'));
    let body = serde_json::json!({
        "model": provider.default_model,
        "messages": [{"role": "user", "content": "hi"}],
        "max_tokens": 5
    });

    let start = std::time::Instant::now();
    let resp = state.http_client.post(&url)
        .header("Authorization", format!("Bearer {}", provider.api_key))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("Request failed: {}", e))?;

    let latency = start.elapsed().as_millis() as u64;
    let status = resp.status().as_u16();
    let text = resp.text().await.unwrap_or_default();

    if status == 200 {
        Ok(TestResult { success: true, message: "连接成功".to_string(), latency_ms: latency })
    } else {
        let msg = serde_json::from_str::<serde_json::Value>(&text)
            .ok()
            .and_then(|v| v["error"]["message"].as_str().map(|s| s.to_string()))
            .unwrap_or(format!("HTTP {}", status));
        Ok(TestResult { success: false, message: msg, latency_ms: latency })
    }
}

#[tauri::command]
pub async fn test_mapping(state: State<'_, AppState>, mapping_idx: usize) -> Result<TestResult, String> {
    let config = state.config.lock().unwrap().clone();
    let mapping = config.model_mappings.get(mapping_idx)
        .ok_or("Mapping not found")?;

    let provider = config.providers.iter().find(|p| p.id == mapping.to_provider)
        .ok_or(format!("Provider '{}' not found", mapping.to_provider))?;

    let url = format!("{}/v1/chat/completions", provider.base_url.trim_end_matches('/'));
    let body = serde_json::json!({
        "model": mapping.to_model,
        "messages": [{"role": "user", "content": "hi"}],
        "max_tokens": 5
    });

    let start = std::time::Instant::now();
    let resp = state.http_client.post(&url)
        .header("Authorization", format!("Bearer {}", provider.api_key))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("Request failed: {}", e))?;

    let latency = start.elapsed().as_millis() as u64;
    let status = resp.status().as_u16();
    let text = resp.text().await.unwrap_or_default();

    if status == 200 {
        let model_out = serde_json::from_str::<serde_json::Value>(&text)
            .ok()
            .and_then(|v| v["model"].as_str().map(|s| s.to_string()))
            .unwrap_or_default();
        Ok(TestResult {
            success: true,
            message: format!("{} → {} OK", mapping.from, model_out),
            latency_ms: latency,
        })
    } else {
        let msg = serde_json::from_str::<serde_json::Value>(&text)
            .ok()
            .and_then(|v| v["error"]["message"].as_str().map(|s| s.to_string()))
            .unwrap_or(format!("HTTP {}", status));
        Ok(TestResult { success: false, message: msg, latency_ms: latency })
    }
}
