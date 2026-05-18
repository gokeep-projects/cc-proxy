use serde_json::{json, Value};

pub fn responses_to_chat(body: &Value, model: &str) -> Value {
    let mut messages: Vec<Value> = vec![];

    if let Some(instructions) = body["instructions"].as_str() {
        messages.push(json!({"role": "system", "content": instructions}));
    }

    if let Some(input) = body["input"].as_array() {
        for item in input {
            match item["type"].as_str() {
                Some("message") | None => {
                    let role = item["role"].as_str().unwrap_or("user");
                    let content = convert_input_content(&item["content"]);
                    messages.push(json!({"role": role, "content": content}));
                }
                _ => {}
            }
        }
    } else if let Some(input) = body["input"].as_str() {
        messages.push(json!({"role": "user", "content": input}));
    }

    let mut chat = json!({
        "model": body["model"],
        "messages": messages,
    });

    if let Some(stream) = body["stream"].as_bool() {
        chat["stream"] = json!(stream);
        if stream {
            chat["stream_options"] = json!({"include_usage": true});
        }
    }

    for key in &["temperature", "top_p", "max_tokens"] {
        if !body[*key].is_null() {
            chat[*key] = body[*key].clone();
        }
    }
    if !body["max_output_tokens"].is_null() {
        chat["max_tokens"] = body["max_output_tokens"].clone();
    }

    if let Some(tools) = body["tools"].as_array() {
        let converted_tools: Vec<Value> = tools
            .iter()
            .filter_map(|t| convert_tool_to_chat(t))
            .collect();
        if !converted_tools.is_empty() {
            chat["tools"] = json!(converted_tools);
        }
    }

    chat
}

fn convert_input_content(content: &Value) -> Value {
    if content.is_string() {
        return content.clone();
    }
    if let Some(arr) = content.as_array() {
        let parts: Vec<Value> = arr
            .iter()
            .filter_map(|part| match part["type"].as_str() {
                Some("input_text") => Some(json!({
                    "type": "text",
                    "text": part["text"]
                })),
                Some("text") => Some(part.clone()),
                _ => Some(part.clone()),
            })
            .collect();
        return json!(parts);
    }
    content.clone()
}

fn convert_tool_to_chat(tool: &Value) -> Option<Value> {
    let t_type = tool["type"].as_str()?;
    if t_type == "function" {
        return Some(tool.clone());
    }
    Some(json!({
        "type": "function",
        "function": {
            "name": tool["name"].as_str().unwrap_or("unknown"),
            "description": tool["description"].as_str().unwrap_or(""),
            "parameters": tool["parameters"].clone()
        }
    }))
}

pub fn chat_to_responses(chat_resp: &Value, request_id: &str) -> Value {
    let choice = &chat_resp["choices"][0];
    let message = &choice["message"];
    let content_text = message["content"].as_str().unwrap_or("");

    let mut output = vec![];

    if let Some(tool_calls) = message["tool_calls"].as_array() {
        for tc in tool_calls {
            output.push(json!({
                "type": "function_call",
                "id": tc["id"],
                "call_id": tc["id"],
                "name": tc["function"]["name"],
                "arguments": tc["function"]["arguments"],
                "status": "completed"
            }));
        }
    }

    if !content_text.is_empty() {
        output.push(json!({
            "type": "message",
            "role": "assistant",
            "content": [{"type": "output_text", "text": content_text}],
            "status": "completed"
        }));
    }

    let mut resp = json!({
        "id": request_id,
        "object": "response",
        "created_at": chat_resp["created"],
        "model": chat_resp["model"],
        "output": output,
        "status": "completed"
    });

    if !chat_resp["usage"].is_null() {
        resp["usage"] = json!({
            "input_tokens": chat_resp["usage"]["prompt_tokens"],
            "output_tokens": chat_resp["usage"]["completion_tokens"],
            "total_tokens": chat_resp["usage"]["total_tokens"]
        });
    }

    resp
}

pub fn chat_stream_chunk_to_responses_events(chunk: &Value) -> Vec<(String, Value)> {
    let mut events = vec![];

    if let Some(choices) = chunk["choices"].as_array() {
        if let Some(choice) = choices.first() {
            let delta = &choice["delta"];

            if let Some(text) = delta["content"].as_str() {
                if !text.is_empty() {
                    events.push((
                        "response.output_text.delta".to_string(),
                        json!({
                            "type": "response.output_text.delta",
                            "output_index": 0,
                            "content_index": 0,
                            "delta": text
                        }),
                    ));
                }
            }

            if let Some(tool_calls) = delta["tool_calls"].as_array() {
                for tc in tool_calls {
                    if let Some(args) = tc["function"]["arguments"].as_str() {
                        events.push((
                            "response.function_call_arguments.delta".to_string(),
                            json!({
                                "type": "response.function_call_arguments.delta",
                                "output_index": 0,
                                "delta": args
                            }),
                        ));
                    }
                }
            }

            if choice["finish_reason"].as_str() == Some("stop") {
                events.push((
                    "response.output_text.done".to_string(),
                    json!({"type": "response.output_text.done", "output_index": 0, "content_index": 0}),
                ));
                events.push((
                    "response.output_item.done".to_string(),
                    json!({"type": "response.output_item.done", "output_index": 0}),
                ));
                events.push((
                    "response.completed".to_string(),
                    json!({"type": "response.completed"}),
                ));
            }
        }
    }

    if !chunk["usage"].is_null() && chunk["usage"]["total_tokens"].as_u64().unwrap_or(0) > 0 {
        events.push((
            "response.usage".to_string(),
            json!({
                "type": "response.usage",
                "usage": {
                    "input_tokens": chunk["usage"]["prompt_tokens"],
                    "output_tokens": chunk["usage"]["completion_tokens"],
                    "total_tokens": chunk["usage"]["total_tokens"]
                }
            }),
        ));
    }

    events
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_responses_to_chat_basic() {
        let input = json!({
            "model": "gpt-4o",
            "input": [{"role": "user", "content": "hello"}],
            "instructions": "You are helpful",
            "max_output_tokens": 1000
        });

        let result = responses_to_chat(&input);
        let messages = result["messages"].as_array().unwrap();
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0]["role"], "system");
        assert_eq!(messages[0]["content"], "You are helpful");
        assert_eq!(messages[1]["role"], "user");
        assert_eq!(messages[1]["content"], "hello");
        assert_eq!(result["max_tokens"], 1000);
        assert_eq!(result["model"], "gpt-4o");
    }

    #[test]
    fn test_responses_to_chat_string_input() {
        let input = json!({
            "model": "test",
            "input": "just a string"
        });

        let result = responses_to_chat(&input);
        let messages = result["messages"].as_array().unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0]["role"], "user");
        assert_eq!(messages[0]["content"], "just a string");
    }

    #[test]
    fn test_chat_to_responses() {
        let chat_resp = json!({
            "id": "chatcmpl-123",
            "created": 1700000000,
            "model": "deepseek-chat",
            "choices": [{
                "message": {
                    "role": "assistant",
                    "content": "Hello! How can I help?"
                },
                "finish_reason": "stop"
            }],
            "usage": {
                "prompt_tokens": 10,
                "completion_tokens": 5,
                "total_tokens": 15
            }
        });

        let result = chat_to_responses(&chat_resp, "resp_test123");
        assert_eq!(result["id"], "resp_test123");
        assert_eq!(result["object"], "response");
        assert_eq!(result["status"], "completed");

        let output = result["output"].as_array().unwrap();
        assert_eq!(output.len(), 1);
        assert_eq!(output[0]["type"], "message");
        assert_eq!(output[0]["content"][0]["type"], "output_text");
        assert_eq!(output[0]["content"][0]["text"], "Hello! How can I help?");

        assert_eq!(result["usage"]["input_tokens"], 10);
        assert_eq!(result["usage"]["output_tokens"], 5);
    }

    #[test]
    fn test_stream_chunk_conversion() {
        let chunk = json!({
            "choices": [{
                "delta": {"content": "Hello"},
                "finish_reason": null
            }]
        });

        let events = chat_stream_chunk_to_responses_events(&chunk);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].0, "response.output_text.delta");
        assert_eq!(events[0].1["delta"], "Hello");
    }

    #[test]
    fn test_stream_chunk_finish() {
        let chunk = json!({
            "choices": [{
                "delta": {},
                "finish_reason": "stop"
            }]
        });

        let events = chat_stream_chunk_to_responses_events(&chunk);
        assert_eq!(events.len(), 3);
        assert_eq!(events[0].0, "response.output_text.done");
        assert_eq!(events[1].0, "response.output_item.done");
        assert_eq!(events[2].0, "response.completed");
    }
}

// Convert OpenAI chat stream chunk to Anthropic SSE events
pub fn chat_chunk_to_anthropic_event(chunk: &Value) -> String {
    let delta = &chunk["choices"][0]["delta"];
    let role = delta["role"].as_str();
    let content = delta["content"].as_str().unwrap_or("");
    let reasoning = delta["reasoning_content"].as_str().unwrap_or("");
    let text = if !content.is_empty() { content } else { reasoning };
    let finish_reason = chunk["choices"][0]["finish_reason"].as_str();
    let index = chunk["choices"][0]["index"].as_u64().unwrap_or(0);

    let mut out = String::new();

    // First chunk: emit message_start + content_block_start
    if role == Some("assistant") {
        out.push_str(&format!(
            "event: message_start\ndata: {}\n\n",
            json!({"type": "message_start", "message": {"id": chunk["id"], "type": "message", "role": "assistant", "model": chunk["model"], "content": [], "usage": null}})
        ));
        out.push_str(&format!(
            "event: content_block_start\ndata: {}\n\n",
            json!({"type": "content_block_start", "index": index, "content_block": {"type": "text", "text": ""}})
        ));
    }

    // Content delta
    if !text.is_empty() {
        out.push_str(&format!(
            "event: content_block_delta\ndata: {}\n\n",
            json!({"type": "content_block_delta", "index": index, "delta": {"type": "text_delta", "text": text}})
        ));
    }

    // Done - emit stop events
    if finish_reason == Some("stop") || finish_reason == Some("length") || finish_reason == Some("tool_calls") {
        out.push_str(&format!(
            "event: content_block_stop\ndata: {}\n\n",
            json!({"type": "content_block_stop", "index": index})
        ));
        out.push_str(&format!(
            "event: message_delta\ndata: {}\n\n",
            json!({"type": "message_delta", "delta": {"stop_reason": if finish_reason == Some("length") { "max_tokens" } else { "end_turn" }, "stop_sequence": null}, "usage": chunk.get("usage")})
        ));
        out.push_str(&format!(
            "event: message_stop\ndata: {}\n\n",
            json!({"type": "message_stop"})
        ));
    }

    out
}
