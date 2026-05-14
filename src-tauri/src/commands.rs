use tauri::State;
use tokio::sync::watch;

use crate::config::{Config, Provider};
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
    Ok(state.log_store.get_recent_chrono(limit.unwrap_or(200)))
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
        https: config.proxy.https,
    })
}

#[derive(serde::Serialize)]
pub struct ProxyStatus {
    pub running: bool,
    pub host: String,
    pub port: u16,
    pub https: bool,
}

#[tauri::command]
pub async fn start_proxy(state: State<'_, AppState>) -> Result<ProxyInfo, String> {
    {
        let running = *state.proxy_running.lock().unwrap();
        if running {
            return Err("Proxy is already running".to_string());
        }
    }

    let config = state.config.lock().unwrap().clone();
    let addr = format!("{}:{}", config.proxy.host, config.proxy.port);
    let use_https = config.proxy.https;

    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .map_err(|e| format!("Failed to bind {}: {}", addr, e))?;

    let (tx, mut rx) = watch::channel(false);
    *state.shutdown_tx.lock().unwrap() = Some(tx);
    *state.proxy_running.lock().unwrap() = true;

    let app_state = (*state).clone();
    let host = config.proxy.host.clone();
    let port = config.proxy.port;
    let https_used = use_https;

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

    Ok(ProxyInfo {
        host,
        port,
        https: https_used,
    })
}

#[derive(serde::Serialize)]
pub struct ProxyInfo {
    pub host: String,
    pub port: u16,
    pub https: bool,
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

// ── test ─────────────────────────────────────────────────────────────────────

#[derive(serde::Serialize)]
pub struct TestResult {
    pub success: bool,
    pub message: String,
    pub latency_ms: u64,
}

#[derive(serde::Serialize)]
pub struct ModelTestResult {
    pub model: String,
    pub success: bool,
    pub latency_ms: u64,
    pub message: String,
}

#[tauri::command]
pub async fn test_provider_models(state: State<'_, AppState>, provider_id: String) -> Result<Vec<ModelTestResult>, String> {
    let config = state.config.lock().unwrap().clone();
    let provider = config.providers.iter().find(|p| p.id == provider_id)
        .ok_or("Provider not found")?;

    let models = if provider.models.is_empty() {
        vec![provider.models.first().cloned().unwrap_or_default()]
    } else {
        provider.models.clone()
    };

    let mut results = vec![];
    let client = &state.http_client;

    for model in &models {
        let url = format!("{}/v1/chat/completions", provider.base_url.trim_end_matches('/'));
        let body = serde_json::json!({
            "model": model,
            "messages": [{"role": "user", "content": "hi"}],
            "max_tokens": 5
        });

        let start = std::time::Instant::now();
        let result = client.post(&url)
            .header("Authorization", format!("Bearer {}", provider.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await;

        let latency = start.elapsed().as_millis() as u64;

        match result {
            Ok(resp) => {
                let status = resp.status().as_u16();
                let text = resp.text().await.unwrap_or_default();
                if status == 200 {
                    results.push(ModelTestResult {
                        model: model.clone(),
                        success: true,
                        latency_ms: latency,
                        message: "✓".to_string(),
                    });
                } else {
                    let msg = serde_json::from_str::<serde_json::Value>(&text)
                        .ok()
                        .and_then(|v| v["error"]["message"].as_str().map(|s| s.to_string()))
                        .unwrap_or(format!("HTTP {}", status));
                    results.push(ModelTestResult {
                        model: model.clone(),
                        success: false,
                        latency_ms: latency,
                        message: msg,
                    });
                }
            }
            Err(e) => {
                results.push(ModelTestResult {
                    model: model.clone(),
                    success: false,
                    latency_ms: 0,
                    message: e.to_string(),
                });
            }
        }
    }

    Ok(results)
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

// ── fetch models from provider API ────────────────────────────────────────────

#[tauri::command]
pub async fn fetch_provider_models(state: State<'_, AppState>, provider_id: String, api_key: String) -> Result<Vec<String>, String> {
    let config = state.config.lock().unwrap().clone();
    let provider = config.providers.iter().find(|p| p.id == provider_id)
        .ok_or("Provider not found")?;

    let key = if api_key.is_empty() { &provider.api_key } else { &api_key };
    let url = format!("{}/v1/models", provider.base_url.trim_end_matches('/'));

    let resp = state.http_client.get(&url)
        .header("Authorization", format!("Bearer {}", key))
        .send()
        .await
        .map_err(|e| format!("Failed to fetch models: {}", e))?;

    let status = resp.status().as_u16();
    let text = resp.text().await.unwrap_or_default();

    if status != 200 {
        return Err(format!("HTTP {}: {}", status, text));
    }

    let json: serde_json::Value = serde_json::from_str(&text)
        .map_err(|e| format!("Invalid response: {}", e))?;

    let models: Vec<String> = json["data"].as_array()
        .unwrap_or(&vec![])
        .iter()
        .filter_map(|m| m["id"].as_str().map(|s| s.to_string()))
        .collect();

    if models.is_empty() {
        return Err("No models found in API response".to_string());
    }

    Ok(models)
}

// ── generate self-signed cert ─────────────────────────────────────────────────

#[tauri::command]
pub fn generate_self_signed_cert(state: State<'_, AppState>) -> Result<CertInfo, String> {
    use rcgen::{CertificateParams, KeyPair, DistinguishedName, DnType, IsCa, BasicConstraints};
    use std::fs;

    let mut config = state.config.lock().unwrap().clone();
    let cert_dir = state.config_path.parent().unwrap_or(std::path::Path::new("."));

    let cert_path = cert_dir.join("cert.pem");
    let key_path = cert_dir.join("key.pem");

    // Only generate if not exists
    if cert_path.exists() && key_path.exists() {
        config.proxy.cert_path = cert_path.to_string_lossy().to_string();
        config.proxy.key_path = key_path.to_string_lossy().to_string();
        config.save(&state.config_path)?;
        *state.config.lock().unwrap() = config;
        return Ok(CertInfo {
            cert_path: cert_path.to_string_lossy().to_string(),
            key_path: key_path.to_string_lossy().to_string(),
            generated: false,
        });
    }

    let mut params = CertificateParams::default();
    let mut dn = DistinguishedName::new();
    dn.push(DnType::CommonName, "CC Proxy Local CA");
    dn.push(DnType::OrganizationName, "CC Proxy");
    params.distinguished_name = dn;

    params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
    params.key_usages = vec![
        rcgen::KeyUsagePurpose::DigitalSignature,
        rcgen::KeyUsagePurpose::KeyCertSign,
        rcgen::KeyUsagePurpose::CrlSign,
    ];

    let subject_alt_names = vec![
        "localhost",
        "127.0.0.1",
        "::1",
    ];
    params.subject_alt_names = subject_alt_names
        .into_iter()
        .filter_map(|s| rcgen::Ia5String::try_from(s).ok().map(|ia5| rcgen::SanType::DnsName(ia5)))
        .collect();

    let key_pair = KeyPair::generate().map_err(|e| format!("Failed to generate key: {}", e))?;
    let cert = params.self_signed(&key_pair).map_err(|e| format!("Failed to sign cert: {}", e))?;

    fs::write(&cert_path, cert.pem())
        .map_err(|e| format!("Failed to write cert: {}", e))?;
    fs::write(&key_path, key_pair.serialize_pem())
        .map_err(|e| format!("Failed to write key: {}", e))?;

    config.proxy.cert_path = cert_path.to_string_lossy().to_string();
    config.proxy.key_path = key_path.to_string_lossy().to_string();
    config.save(&state.config_path)?;
    *state.config.lock().unwrap() = config;

    Ok(CertInfo {
        cert_path: cert_path.to_string_lossy().to_string(),
        key_path: key_path.to_string_lossy().to_string(),
        generated: true,
    })
}

#[derive(serde::Serialize)]
pub struct CertInfo {
    pub cert_path: String,
    pub key_path: String,
    pub generated: bool,
}

// ── CC-Switch import ──────────────────────────────────────────────────────────

#[tauri::command]
pub fn import_cc_switch_config(state: State<'_, AppState>, json_str: String) -> Result<Config, String> {
    let cc_config: serde_json::Value = serde_json::from_str(&json_str)
        .map_err(|e| format!("Invalid JSON: {}", e))?;

    let mut config = Config::default();
    config.proxy.port = 9528;

    // Parse providers
    if let Some(providers) = cc_config["providers"].as_array() {
        for p in providers {
            let id = p["id"].as_str().unwrap_or("unknown").to_string();
            let name = p["name"].as_str().unwrap_or(&id).to_string();
            let base_url = p["base_url"].as_str().unwrap_or("").to_string();
            let api_key = p["api_key"].as_str().unwrap_or("").to_string();
            let models: Vec<String> = p["models"].as_array()
                .map(|arr| arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
                .unwrap_or_default();

            config.providers.push(Provider {
                id,
                name,
                base_url,
                api_key,
                models,
            });
        }
    }

    // Parse mappings
    if let Some(mappings) = cc_config["mappings"].as_array() {
        for m in mappings {
            config.model_mappings.push(crate::config::ModelMapping {
                from: m["from"].as_str().unwrap_or("").to_string(),
                to_provider: m["to_provider"].as_str().unwrap_or("").to_string(),
                to_model: m["to_model"].as_str().unwrap_or("").to_string(),
            });
        }
    }

    // Parse proxy settings
    if let Some(port) = cc_config["port"].as_u64() {
        config.proxy.port = port as u16;
    }
    if let Some(host) = cc_config["host"].as_str() {
        config.proxy.host = host.to_string();
    }

    // Save
    let path = state.config_path.clone();
    config.save(&path)?;
    *state.config.lock().unwrap() = config.clone();

    Ok(config)
}
