use dx_agent_core::{Result, TokenUsage};
use dx_agent_harness::{ApiClient, ApiRequest, AssistantEvent};
use serde_json::{json, Value};

/// Extract a tool call from text output.
/// Many local models output tool calls as JSON in their response text instead
/// of using the structured tool_calls field. This handles that pattern.
fn extract_tool_call_from_text(text: &str) -> Option<AssistantEvent> {
    // Strip markdown code fences if present
    let cleaned = text
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();

    // Try parsing as JSON with "name" and "arguments" fields
    if let Ok(parsed) = serde_json::from_str::<Value>(cleaned) {
        if let (Some(name), Some(args)) = (
            parsed.get("name").and_then(|v| v.as_str()),
            parsed.get("arguments"),
        ) {
            // Validate it's a known tool name
            let known_tools = [
                "read_file",
                "write_file",
                "edit_file",
                "bash",
                "glob",
                "grep",
                "git",
                "web_fetch",
                "repo_map",
                "Read",
                "Write",
                "Edit",
                "Glob",
                "Grep",
                "WebFetch",
            ];
            if known_tools.iter().any(|t| *t == name) {
                return Some(AssistantEvent::ToolUse {
                    id: format!("call_{}", rand_id()),
                    name: name.to_string(),
                    input: args.to_string(),
                });
            }
        }

        // Also handle {"name": "tool", "parameters": {...}} format
        if let (Some(name), Some(params)) = (
            parsed.get("name").and_then(|v| v.as_str()),
            parsed.get("parameters"),
        ) {
            let known_tools = [
                "read_file",
                "write_file",
                "edit_file",
                "bash",
                "glob",
                "grep",
                "git",
            ];
            if known_tools.iter().any(|t| *t == name) {
                return Some(AssistantEvent::ToolUse {
                    id: format!("call_{}", rand_id()),
                    name: name.to_string(),
                    input: params.to_string(),
                });
            }
        }
    }

    // Try to find JSON embedded in surrounding text
    if let Some(start) = cleaned.find('{') {
        if let Some(end) = cleaned.rfind('}') {
            if start < end {
                let json_str = &cleaned[start..=end];
                return extract_tool_call_from_text(json_str);
            }
        }
    }

    None
}

fn rand_id() -> u32 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos()
}

pub struct OllamaClient {
    model: String,
    base_url: String,
    api_key: Option<String>,
    http: reqwest::blocking::Client,
}

impl OllamaClient {
    pub fn new(model: String, base_url: Option<String>) -> Self {
        Self {
            model,
            base_url: base_url.unwrap_or_else(|| "http://127.0.0.1:11434".to_string()),
            api_key: None,
            http: reqwest::blocking::Client::builder()
                .timeout(std::time::Duration::from_secs(300))
                .build()
                .unwrap_or_else(|_| reqwest::blocking::Client::new()),
        }
    }

    /// Create with an API key for OpenAI-compatible services (OpenAI, OpenRouter, Together, etc.)
    pub fn new_with_key(model: String, base_url: Option<String>, api_key: Option<String>) -> Self {
        Self {
            model,
            base_url: base_url.unwrap_or_else(|| "https://api.openai.com".to_string()),
            api_key,
            http: reqwest::blocking::Client::builder()
                .timeout(std::time::Duration::from_secs(300))
                .build()
                .unwrap_or_else(|_| reqwest::blocking::Client::new()),
        }
    }

    fn build_http_request(&self, body: &Value) -> reqwest::blocking::RequestBuilder {
        let mut req = self
            .http
            .post(format!("{}/v1/chat/completions", self.base_url))
            .header("content-type", "application/json");
        if let Some(key) = &self.api_key {
            req = req.bearer_auth(key);
        }
        req.json(body)
    }

    fn build_request_body(&self, request: &ApiRequest) -> Value {
        // Ollama uses OpenAI-compatible /v1/chat/completions format
        let mut messages: Vec<Value> = Vec::new();

        // System prompt
        let system_text = request.system_prompt.join("\n\n");
        if !system_text.is_empty() {
            messages.push(json!({
                "role": "system",
                "content": system_text
            }));
        }

        // Conversation messages
        for msg in &request.messages {
            let role = match msg.role {
                dx_agent_core::MessageRole::User => "user",
                dx_agent_core::MessageRole::Assistant => "assistant",
                dx_agent_core::MessageRole::Tool => "tool",
                dx_agent_core::MessageRole::System => "system",
            };

            // For tool results, use the OpenAI format
            for block in &msg.blocks {
                match block {
                    dx_agent_core::ContentBlock::Text { text } => {
                        messages.push(json!({
                            "role": role,
                            "content": text
                        }));
                    }
                    dx_agent_core::ContentBlock::ToolUse { id, name, input } => {
                        let input_val: Value = serde_json::from_str(input).unwrap_or(json!(input));
                        messages.push(json!({
                            "role": "assistant",
                            "content": null,
                            "tool_calls": [{
                                "id": id,
                                "type": "function",
                                "function": {
                                    "name": name,
                                    "arguments": input_val.to_string()
                                }
                            }]
                        }));
                    }
                    dx_agent_core::ContentBlock::ToolResult {
                        tool_use_id,
                        output,
                        ..
                    } => {
                        messages.push(json!({
                            "role": "tool",
                            "tool_call_id": tool_use_id,
                            "content": output
                        }));
                    }
                }
            }
        }

        // Convert tools from Anthropic format to OpenAI function format
        let tools: Vec<Value> = request
            .tools
            .iter()
            .map(|tool| {
                json!({
                    "type": "function",
                    "function": {
                        "name": tool["name"],
                        "description": tool["description"],
                        "parameters": tool["input_schema"]
                    }
                })
            })
            .collect();

        let mut body = json!({
            "model": self.model,
            "messages": messages,
        });

        if !tools.is_empty() {
            body["tools"] = json!(tools);
        }

        body
    }
}

impl ApiClient for OllamaClient {
    fn stream_with_callback(
        &mut self,
        request: ApiRequest,
        on_token: &mut dyn FnMut(&str),
    ) -> Result<Vec<AssistantEvent>> {
        let mut body = self.build_request_body(&request);
        body["stream"] = json!(true);

        let response = self
            .build_http_request(&body)
            .send()
            .map_err(|e: reqwest::Error| {
                dx_agent_core::DxosError::Api(format!(
                    "Failed to connect to {}. Is the server running? {e}",
                    self.base_url
                ))
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().unwrap_or_default();
            return Err(dx_agent_core::DxosError::Api(format!(
                "HTTP {status}: {text}"
            )));
        }

        // TRUE streaming: read response body line-by-line as chunks arrive
        let mut events = Vec::new();
        let mut full_content = String::new();
        let mut tool_calls_json: Vec<Value> = Vec::new();
        let mut usage_event = None;

        let reader = std::io::BufReader::new(response);
        use std::io::BufRead;
        for line_result in reader.lines() {
            let line = match line_result {
                Ok(l) => l,
                Err(_) => break,
            };
            let line = line.trim().to_string();
            if line.is_empty() || line == "data: [DONE]" {
                continue;
            }
            let json_str = line.strip_prefix("data: ").unwrap_or(&line);
            let chunk: Value = match serde_json::from_str(json_str) {
                Ok(v) => v,
                Err(_) => continue,
            };

            // Extract delta content
            if let Some(choices) = chunk["choices"].as_array() {
                for choice in choices {
                    let delta = &choice["delta"];

                    // Stream incremental text tokens only from delta field
                    if let Some(content) = delta.get("content").and_then(|v| v.as_str()) {
                        if !content.is_empty() {
                            on_token(content);
                            full_content.push_str(content);
                        }
                    }

                    // Tool calls (streamed as deltas)
                    if let Some(tcs) = delta["tool_calls"].as_array() {
                        for tc in tcs {
                            let idx = tc["index"].as_u64().unwrap_or(0) as usize;
                            while tool_calls_json.len() <= idx {
                                tool_calls_json.push(json!({
                                    "id": "",
                                    "function": { "name": "", "arguments": "" }
                                }));
                            }
                            if let Some(id) = tc["id"].as_str() {
                                tool_calls_json[idx]["id"] = json!(id);
                            }
                            if let Some(name) = tc["function"]["name"].as_str() {
                                let existing = tool_calls_json[idx]["function"]["name"]
                                    .as_str()
                                    .unwrap_or("")
                                    .to_string();
                                tool_calls_json[idx]["function"]["name"] =
                                    json!(format!("{existing}{name}"));
                            }
                            if let Some(args) = tc["function"]["arguments"].as_str() {
                                let existing = tool_calls_json[idx]["function"]["arguments"]
                                    .as_str()
                                    .unwrap_or("")
                                    .to_string();
                                tool_calls_json[idx]["function"]["arguments"] =
                                    json!(format!("{existing}{args}"));
                            }
                        }
                    }
                }
            }

            // Usage (usually in last chunk)
            if let Some(usage) = chunk.get("usage") {
                usage_event = Some(AssistantEvent::Usage(TokenUsage {
                    input_tokens: usage["prompt_tokens"].as_u64().unwrap_or(0) as u32,
                    output_tokens: usage["completion_tokens"].as_u64().unwrap_or(0) as u32,
                    cache_creation_input_tokens: 0,
                    cache_read_input_tokens: 0,
                }));
            }
        }

        // Build final events from accumulated data
        if let Some(usage) = usage_event {
            events.push(usage);
        }

        // Process tool calls
        let mut has_tool_calls = false;
        for tc in &tool_calls_json {
            let name = tc["function"]["name"].as_str().unwrap_or("");
            if !name.is_empty() {
                has_tool_calls = true;
                events.push(AssistantEvent::ToolUse {
                    id: tc["id"].as_str().unwrap_or("call_1").to_string(),
                    name: name.to_string(),
                    input: tc["function"]["arguments"]
                        .as_str()
                        .unwrap_or("{}")
                        .to_string(),
                });
            }
        }

        // If no structured tool calls, check text for embedded tool calls
        if !has_tool_calls && !full_content.is_empty() {
            if let Some(extracted) = extract_tool_call_from_text(&full_content) {
                events.push(extracted);
            } else {
                // Text already streamed via on_token — add to events for conversation history only
                events.push(AssistantEvent::TextDelta(full_content));
            }
        } else if has_tool_calls && !full_content.is_empty() {
            events.push(AssistantEvent::TextDelta(full_content));
        }

        events.push(AssistantEvent::Stop);
        Ok(events)
    }

    fn stream(&mut self, request: ApiRequest) -> Result<Vec<AssistantEvent>> {
        let body = self.build_request_body(&request);

        let response = self
            .build_http_request(&body)
            .send()
            .map_err(|e: reqwest::Error| {
                dx_agent_core::DxosError::Api(format!(
                    "Failed to connect to {}. Is the server running? {e}",
                    self.base_url
                ))
            })?;

        let status = response.status();
        let text = response
            .text()
            .map_err(|e: reqwest::Error| dx_agent_core::DxosError::Api(e.to_string()))?;

        if !status.is_success() {
            return Err(dx_agent_core::DxosError::Api(format!(
                "Ollama HTTP {status}: {text}"
            )));
        }

        let resp: Value = serde_json::from_str(&text)
            .map_err(|e: serde_json::Error| dx_agent_core::DxosError::Api(e.to_string()))?;

        let mut events = Vec::new();

        // Parse usage
        if let Some(usage) = resp.get("usage") {
            events.push(AssistantEvent::Usage(TokenUsage {
                input_tokens: usage["prompt_tokens"].as_u64().unwrap_or(0) as u32,
                output_tokens: usage["completion_tokens"].as_u64().unwrap_or(0) as u32,
                cache_creation_input_tokens: 0,
                cache_read_input_tokens: 0,
            }));
        }

        // Parse choices
        let mut tool_call_count = 0u32;
        if let Some(choices) = resp["choices"].as_array() {
            for choice in choices {
                let message = &choice["message"];

                // Structured tool calls (OpenAI format)
                if let Some(tool_calls) = message["tool_calls"].as_array() {
                    for tc in tool_calls {
                        tool_call_count += 1;
                        events.push(AssistantEvent::ToolUse {
                            id: tc["id"].as_str().unwrap_or_else(|| "call_1").to_string(),
                            name: tc["function"]["name"].as_str().unwrap_or("").to_string(),
                            input: tc["function"]["arguments"]
                                .as_str()
                                .unwrap_or("{}")
                                .to_string(),
                        });
                    }
                }

                // Text content — also check for text-based tool calls
                // Many local models output tool calls as JSON in their text
                if let Some(content) = message["content"].as_str() {
                    if !content.is_empty() && tool_call_count == 0 {
                        // Try to extract tool calls from text
                        if let Some(extracted) = extract_tool_call_from_text(content) {
                            tool_call_count += 1;
                            events.push(extracted);
                        } else {
                            events.push(AssistantEvent::TextDelta(content.to_string()));
                        }
                    } else if !content.is_empty() {
                        events.push(AssistantEvent::TextDelta(content.to_string()));
                    }
                }
            }
        }

        events.push(AssistantEvent::Stop);
        Ok(events)
    }
}
