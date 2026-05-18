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
        convert::responses_to_chat(&body_value, &model_out)
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

        // Convert Anthropic response to OpenAI format if needed
        let normalized_resp = if is_anthropic_endpoint && status == 200 {
            convert_anthropic_response_to_openai(&resp_value)
        } else {
            resp_value.clone()
        };

        let final_body = if is_responses_api && status == 200 {
            let request_id = format!("resp_{}", LogStore::new_id());
            convert::chat_to_responses(&normalized_resp, &request_id)
        } else {
            normalized_resp.clone()
        };

        let prompt_tokens = normalized_resp["usage"]["prompt_tokens"].as_u64();
        let completion_tokens = normalized_resp["usage"]["completion_tokens"].as_u64();

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

pub async fn responses_handler(
    State(state): State<AppState>,
    body: Bytes,
) -> impl IntoResponse {
    let path = "/v1/responses";
    let body_value: Value = serde_json::from_slice(&body).unwrap_or(json!({}));
    let model_in = body_value["model"].as_str().unwrap_or("unknown").to_string();

    let config = state.config.lock().unwrap().clone();
    let Some((provider, target_model)) = resolve_provider(&config, &model_in) else {
        return Response::builder()
            .status(StatusCode::BAD_GATEWAY)
            .body(Body::from(json!({"error": "No provider configured"}).to_string()))
            .unwrap();
    };

    let mut chat_body = convert::responses_to_chat(&body_value, &target_model);
    chat_body["model"] = json!(target_model);
    let is_streaming = chat_body["stream"].as_bool().unwrap_or(false);

    let base = provider.base_url.trim_end_matches('/');
    let target_url = if base.ends_with("/anthropic") {
        format!("{}/v1/messages", base)
    } else {
        format!("{}/v1/chat/completions", base)
    };
    let is_anthropic_endpoint = base.ends_with("/anthropic");

    let start = std::time::Instant::now();
    let mut req = state.http_client.post(&target_url).header("Content-Type", "application/json");

    if is_anthropic_endpoint {
        let anthropic_body = convert_to_anthropic_format(&chat_body);
        req = req.header("x-api-key", &provider.api_key).header("anthropic-version", "2023-06-01");
        req = req.body(serde_json::to_vec(&anthropic_body).unwrap());
    } else {
        req = req.header("Authorization", format!("Bearer {}", provider.api_key));
        req = req.body(serde_json::to_vec(&chat_body).unwrap());
    }

    let upstream_resp = match req.send().await {
        Ok(r) => r,
        Err(e) => {
            return Response::builder()
                .status(StatusCode::BAD_GATEWAY)
                .body(Body::from(json!({"error": e.to_string()}).to_string()))
                .unwrap();
        }
    };

    let status = upstream_resp.status().as_u16();
    let resp_bytes = upstream_resp.bytes().await.unwrap_or_default();
    let resp_value: Value = serde_json::from_slice(&resp_bytes).unwrap_or(json!({}));
    let latency = start.elapsed().as_millis() as u64;

    let normalized = if is_anthropic_endpoint && status == 200 {
        convert_anthropic_response_to_openai(&resp_value)
    } else {
        resp_value.clone()
    };

    let request_id = format!("resp_{}", LogStore::new_id());
    let final_body = if status == 200 {
        convert::chat_to_responses(&normalized, &request_id)
    } else {
        normalized.clone()
    };

    state.log_store.push(RequestLog {
        id: LogStore::new_id(), timestamp: chrono::Utc::now(),
        method: "POST".to_string(), path: path.to_string(),
        model_in: model_in.clone(), model_out: target_model.clone(),
        provider: provider.id.clone(), status, latency_ms: latency,
        prompt_tokens: normalized["usage"]["prompt_tokens"].as_u64(),
        completion_tokens: normalized["usage"]["completion_tokens"].as_u64(),
        request_body: body_value.clone(),
        response_body: final_body.clone(),
    });

    Response::builder()
        .status(status)
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_vec(&final_body).unwrap()))
        .unwrap()
}

pub async fn messages_handler(
    State(state): State<AppState>,
    body: Bytes,
) -> impl IntoResponse {
    let path = "/v1/messages";
    let body_value: Value = serde_json::from_slice(&body).unwrap_or(json!({}));
    let model_in = body_value["model"].as_str().unwrap_or("unknown").to_string();
    let is_streaming = body_value["stream"].as_bool().unwrap_or(false);

    let config = state.config.lock().unwrap().clone();
    let Some((provider, target_model)) = resolve_provider(&config, &model_in) else {
        return Response::builder()
            .status(StatusCode::BAD_GATEWAY)
            .body(Body::from(json!({"error": "No provider configured"}).to_string()))
            .unwrap();
    };

    // Claude Code sends Anthropic Messages format. Convert it to OpenAI chat format.
    let chat_body = convert_anthropic_to_openai_request(&body_value, &target_model);

    let base = provider.base_url.trim_end_matches('/');
    let is_anthropic_endpoint = base.ends_with("/anthropic");
    let target_url = if is_anthropic_endpoint {
        format!("{}/v1/messages", base)
    } else {
        format!("{}/v1/chat/completions", base)
    };

    let start = std::time::Instant::now();
    let mut req = state.http_client.post(&target_url).header("Content-Type", "application/json");

    if is_anthropic_endpoint {
        // Forward as Anthropic format directly
        req = req.header("x-api-key", &provider.api_key).header("anthropic-version", "2023-06-01");
        let mut fwd_body = body_value.clone();
        fwd_body["model"] = json!(target_model);
        req = req.body(serde_json::to_vec(&fwd_body).unwrap());
    } else {
        // Convert to OpenAI chat format for non-anthropic providers
        req = req.header("Authorization", format!("Bearer {}", provider.api_key));
        req = req.body(serde_json::to_vec(&chat_body).unwrap());
    }

    let upstream_resp = match req.send().await {
        Ok(r) => r,
        Err(e) => {
            return Response::builder()
                .status(StatusCode::BAD_GATEWAY)
                .body(Body::from(json!({"error": e.to_string()}).to_string()))
                .unwrap();
        }
    };

    let status = upstream_resp.status().as_u16();
    let latency = start.elapsed().as_millis() as u64;

    if is_streaming {
        // For streaming, pass through with Anthropic format conversion as needed
        let stream = upstream_resp.bytes_stream();
        let is_anthropic_upstream = is_anthropic_endpoint;
        let converted_stream = stream.map(move |chunk_result| {
            match chunk_result {
                Ok(chunk) => {
                    if is_anthropic_upstream {
                        Ok(chunk) // Pass through directly
                    } else {
                        // Convert SSE chat chunks to Anthropic event format
                        let text = String::from_utf8_lossy(&chunk);
                        let mut output = String::new();
                        for line in text.lines() {
                            if let Some(data) = line.strip_prefix("data: ") {
                                if data == "[DONE]" {
                                    output.push_str("event: message_stop\ndata: {\"type\":\"message_stop\"}\n\n");
                                    continue;
                                }
                                if let Ok(chunk_val) = serde_json::from_str::<Value>(data) {
                                    let delta = convert::chat_chunk_to_anthropic_event(&chunk_val);
                                    output.push_str(&delta);
                                }
                            }
                        }
                        Ok(Bytes::from(output))
                    }
                }
                Err(e) => Err(e),
            }
        });

        let log_state = state.clone();
        let log_model_in = model_in.clone();
        let log_model_out = target_model.clone();
        let log_provider = provider.id.clone();
        tokio::spawn(async move {
            log_state.log_store.push(RequestLog {
                id: LogStore::new_id(), timestamp: chrono::Utc::now(),
                method: "POST".to_string(), path: path.to_string(),
                model_in: log_model_in, model_out: log_model_out,
                provider: log_provider, status: 200, latency_ms: 0,
                prompt_tokens: None, completion_tokens: None,
                request_body: body_value,
                response_body: json!({"stream": true}),
            });
        });

        Response::builder()
            .status(200)
            .header("Content-Type", "text/event-stream")
            .header("Cache-Control", "no-cache")
            .body(Body::from_stream(converted_stream))
            .unwrap()
    } else {
        let resp_bytes = upstream_resp.bytes().await.unwrap_or_default();
        let resp_value: Value = serde_json::from_slice(&resp_bytes).unwrap_or(json!({}));

        // Convert response back to Anthropic format
        let final_body = if is_anthropic_endpoint {
            resp_value.clone() // Already Anthropic format
        } else {
            convert_openai_to_anthropic_response(&resp_value)
        };

        state.log_store.push(RequestLog {
            id: LogStore::new_id(), timestamp: chrono::Utc::now(),
            method: "POST".to_string(), path: path.to_string(),
            model_in: model_in.clone(), model_out: target_model.clone(),
            provider: provider.id.clone(), status, latency_ms: latency,
            prompt_tokens: resp_value["usage"]["prompt_tokens"].as_u64(),
            completion_tokens: resp_value["usage"]["completion_tokens"].as_u64(),
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

// Convert Anthropic Messages REQUEST to OpenAI Chat format
fn convert_anthropic_to_openai_request(msg_body: &Value, target_model: &str) -> Value {
    let messages = msg_body["messages"].as_array().cloned().unwrap_or_default();
    let system = msg_body["system"].as_str();
    let max_tokens = msg_body["max_tokens"].as_u64().unwrap_or(4096);
    let stream = msg_body["stream"].as_bool();

    let mut chat_messages: Vec<Value> = vec![];

    if let Some(sys) = system {
        if !sys.is_empty() {
            chat_messages.push(json!({"role": "system", "content": sys}));
        }
    }

    for msg in &messages {
        let role = msg["role"].as_str().unwrap_or("user");
        let content = if msg["content"].is_string() {
            msg["content"].as_str().unwrap_or("").to_string()
        } else if let Some(arr) = msg["content"].as_array() {
            // Handle content blocks
            let mut text = String::new();
            for block in arr {
                if block["type"].as_str() == Some("text") {
                    if let Some(t) = block["text"].as_str() {
                        text.push_str(t);
                    }
                }
            }
            text
        } else {
            String::new()
        };
        chat_messages.push(json!({"role": role, "content": content}));
    }

    let mut chat_body = json!({
        "model": target_model,
        "messages": chat_messages,
        "max_tokens": max_tokens,
    });

    if let Some(s) = stream {
        chat_body["stream"] = json!(s);
    }

    chat_body
}

// Convert OpenAI Chat RESPONSE back to Anthropic Messages format
fn convert_openai_to_anthropic_response(resp: &Value) -> Value {
    let content_text = resp["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("")
        .to_string();

    json!({
        "id": resp["id"],
        "type": "message",
        "role": "assistant",
        "model": resp["model"],
        "content": [{
            "type": "text",
            "text": content_text
        }],
        "stop_reason": resp["choices"][0]["finish_reason"].as_str().map(|r| match r {
            "stop" => "end_turn",
            "length" => "max_tokens",
            _ => "end_turn",
        }).unwrap_or("end_turn"),
        "usage": {
            "input_tokens": resp["usage"]["prompt_tokens"].as_u64().unwrap_or(0),
            "output_tokens": resp["usage"]["completion_tokens"].as_u64().unwrap_or(0),
        }
    })
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

fn convert_anthropic_response_to_openai(resp: &Value) -> Value {
    // Anthropic format: { id, type: "message", role, model, content: [{type: "text", text: "..."}], stop_reason, usage: {input_tokens, output_tokens} }
    // Convert to OpenAI format: { id, object: "chat.completion", model, choices: [{message: {role, content}, finish_reason}], usage: {prompt_tokens, completion_tokens, total_tokens} }

    let content_blocks = resp["content"].as_array();
    let mut text_content = String::new();

    if let Some(blocks) = content_blocks {
        for block in blocks {
            match block["type"].as_str() {
                Some("text") => {
                    if let Some(t) = block["text"].as_str() {
                        text_content.push_str(t);
                    }
                }
                Some("thinking") => {
                    // Skip thinking blocks or include as reasoning
                    if let Some(t) = block["thinking"].as_str() {
                        text_content.push_str(t);
                    }
                }
                _ => {}
            }
        }
    }

    let stop_reason = resp["stop_reason"].as_str().unwrap_or("stop");
    let finish_reason = match stop_reason {
        "end_turn" | "stop" => "stop",
        "max_tokens" => "length",
        _ => "stop",
    };

    let input_tokens = resp["usage"]["input_tokens"].as_u64().unwrap_or(0);
    let output_tokens = resp["usage"]["output_tokens"].as_u64().unwrap_or(0);

    json!({
        "id": resp["id"],
        "object": "chat.completion",
        "model": resp["model"],
        "choices": [{
            "index": 0,
            "message": {
                "role": "assistant",
                "content": text_content
            },
            "finish_reason": finish_reason
        }],
        "usage": {
            "prompt_tokens": input_tokens,
            "completion_tokens": output_tokens,
            "total_tokens": input_tokens + output_tokens
        }
    })
}
