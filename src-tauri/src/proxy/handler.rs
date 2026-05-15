use axum::body::Body;
use axum::extract::State;
use axum::http::{Response, StatusCode, Uri};
use axum::response::IntoResponse;
use bytes::Bytes;
use futures_util::StreamExt;
use serde_json::{json, Value};
use std::time::Instant;

use crate::config::{Config, Provider};
use crate::proxy::convert;
use crate::store::{LogStore, RequestLog};
use crate::AppState;

pub async fn proxy_handler(
    State(state): State<AppState>,
    uri: Uri,
    body: Bytes,
) -> impl IntoResponse {
    let path = uri.path().to_string();
    let is_responses_api = path == "/v1/responses";
    let start = Instant::now();

    let body_value: Value = match serde_json::from_slice(&body) {
        Ok(v) => v,
        Err(e) => {
            return Response::builder()
                .status(StatusCode::BAD_REQUEST)
                .body(Body::from(json!({"error": e.to_string()}).to_string()))
                .unwrap();
        }
    };

    let model_in = body_value["model"].as_str().unwrap_or("").to_string();

    let config = state.config.lock().unwrap().clone();
    let (provider, model_out) = match resolve_provider(&config, &model_in) {
        Some(v) => v,
        None => {
            return Response::builder()
                .status(StatusCode::BAD_REQUEST)
                .body(Body::from(
                    json!({"error": format!("No mapping found for model: {}", model_in)}).to_string(),
                ))
                .unwrap();
        }
    };

    let mut chat_body = if is_responses_api {
        convert::responses_to_chat(&body_value)
    } else {
        body_value.clone()
    };
    chat_body["model"] = json!(model_out);

    let is_streaming = chat_body["stream"].as_bool().unwrap_or(false);
    let base = provider.base_url.trim_end_matches('/');
    let target_url = if base.ends_with("/anthropic") {
        // Anthropic-compatible endpoint: POST base_url/v1/messages
        format!("{}/v1/messages", base)
    } else {
        format!("{}/v1/chat/completions", base)
    };
    let is_anthropic_endpoint = base.ends_with("/anthropic");

    let client = &state.http_client;
    let mut req_builder = client.post(&target_url).header("Content-Type", "application/json");

    if is_anthropic_endpoint {
        // Anthropic format: use x-api-key header and anthropic-version
        req_builder = req_builder
            .header("x-api-key", &provider.api_key)
            .header("anthropic-version", "2023-06-01");
        // Convert chat format to Anthropic messages format
        let anthropic_body = convert_to_anthropic_format(&chat_body);
        req_builder = req_builder.body(serde_json::to_vec(&anthropic_body).unwrap());
    } else {
        req_builder = req_builder
            .header("Authorization", format!("Bearer {}", provider.api_key))
            .body(serde_json::to_vec(&chat_body).unwrap());
    }

    let upstream_resp = match req_builder.send().await {
        Ok(r) => r,
        Err(e) => {
            return Response::builder()
                .status(StatusCode::BAD_GATEWAY)
                .body(Body::from(json!({"error": e.to_string()}).to_string()))
                .unwrap();
        }
    };

    let status = upstream_resp.status().as_u16();

    if is_streaming {
        let stream = upstream_resp.bytes_stream();
        let converted_stream = stream.map(move |chunk_result| {
            match chunk_result {
                Ok(chunk) => {
                    let text = String::from_utf8_lossy(&chunk);
                    if is_responses_api {
                        let mut output = String::new();
                        for line in text.lines() {
                            if let Some(data) = line.strip_prefix("data: ") {
                                if data == "[DONE]" {
                                    output.push_str("event: response.completed\ndata: {\"type\":\"response.completed\"}\n\n");
                                    continue;
                                }
                                if let Ok(chunk_val) = serde_json::from_str::<Value>(data) {
                                    let events = convert::chat_stream_chunk_to_responses_events(&chunk_val);
                                    for (event_type, event_data) in events {
                                        output.push_str(&format!(
                                            "event: {}\ndata: {}\n\n",
                                            event_type,
                                            serde_json::to_string(&event_data).unwrap_or_default()
                                        ));
                                    }
                                }
                            }
                        }
                        Ok::<_, reqwest::Error>(Bytes::from(output))
                    } else {
                        Ok(chunk)
                    }
                }
                Err(e) => Err(e),
            }
        });

        // Log the streaming request
        let log_state = state.clone();
        let log_model_in = model_in.clone();
        let log_model_out = model_out.clone();
        let log_provider = provider.id.clone();
        let log_path = path.clone();
        let log_body = body_value.clone();
        tokio::spawn(async move {
            log_state.log_store.push(RequestLog {
                id: LogStore::new_id(),
                timestamp: chrono::Utc::now(),
                method: "POST".to_string(),
                path: log_path,
                model_in: log_model_in,
                model_out: log_model_out,
                provider: log_provider,
                status: 200,
                latency_ms: 0,
                prompt_tokens: None,
                completion_tokens: None,
                request_body: log_body,
                response_body: json!({"stream": true}),
            });
        });

        Response::builder()
            .status(status)
            .header("Content-Type", "text/event-stream")
            .header("Cache-Control", "no-cache")
            .header("Connection", "keep-alive")
            .body(Body::from_stream(converted_stream))
            .unwrap()
    } else {
        let resp_bytes = upstream_resp.bytes().await.unwrap_or_default();
        let resp_value: Value = serde_json::from_slice(&resp_bytes).unwrap_or(json!({}));
        let latency = start.elapsed().as_millis() as u64;

        let final_body = if is_responses_api && status == 200 {
            let request_id = format!("resp_{}", LogStore::new_id());
            convert::chat_to_responses(&resp_value, &request_id)
        } else {
            resp_value.clone()
        };

        let prompt_tokens = resp_value["usage"]["prompt_tokens"].as_u64();
        let completion_tokens = resp_value["usage"]["completion_tokens"].as_u64();

        state.log_store.push(RequestLog {
            id: LogStore::new_id(),
            timestamp: chrono::Utc::now(),
            method: "POST".to_string(),
            path,
            model_in: model_in.clone(),
            model_out: model_out.clone(),
            provider: provider.id.clone(),
            status,
            latency_ms: latency,
            prompt_tokens,
            completion_tokens,
            request_body: body_value,
            response_body: final_body.clone(),
        });

        Response::builder()
            .status(status)
            .header("Content-Type", "application/json")
            .body(Body::from(serde_json::to_vec(&final_body).unwrap()))
            .unwrap()
    }
}

pub async fn models_handler(State(state): State<AppState>) -> impl IntoResponse {
    let config = state.config.lock().unwrap().clone();
    let models: Vec<Value> = config
        .model_mappings
        .iter()
        .map(|m| {
            json!({
                "id": m.from,
                "object": "model",
                "owned_by": m.to_provider
            })
        })
        .collect();

    let resp = json!({
        "object": "list",
        "data": models
    });

    Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_vec(&resp).unwrap()))
        .unwrap()
}

fn resolve_provider(config: &Config, model: &str) -> Option<(Provider, String)> {
    if let Some(mapping) = config.find_mapping(model) {
        if let Some(provider) = config.find_provider(&mapping.to_provider) {
            return Some((provider.clone(), mapping.to_model.clone()));
        }
    }
    if let Some(provider) = config.providers.first() {
        let model = provider.models.first().cloned().unwrap_or_else(|| "default".to_string());
        return Some((provider.clone(), model));
    }
    None
}

fn convert_to_anthropic_format(chat_body: &Value) -> Value {
    let messages = chat_body["messages"].as_array().cloned().unwrap_or_default();
    let mut system = String::new();
    let mut anthropic_messages: Vec<Value> = vec![];

    for msg in &messages {
        let role = msg["role"].as_str().unwrap_or("user");
        if role == "system" {
            system = msg["content"].as_str().unwrap_or("").to_string();
        } else {
            anthropic_messages.push(json!({
                "role": role,
                "content": msg["content"]
            }));
        }
    }

    let mut body = json!({
        "model": chat_body["model"],
        "messages": anthropic_messages,
        "max_tokens": chat_body["max_tokens"].as_u64().unwrap_or(4096)
    });

    if !system.is_empty() {
        body["system"] = json!(system);
    }
    if let Some(stream) = chat_body["stream"].as_bool() {
        body["stream"] = json!(stream);
    }

    body
}
