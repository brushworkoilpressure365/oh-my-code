use anyhow::Result;
use async_trait::async_trait;
use futures::stream::{BoxStream, StreamExt};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::types::{ContentBlock, Message, ModelConfig, ModelInfo, Role, StreamEvent, ToolDef};
use super::Provider;

// --- Wire types for OpenAI Chat Completions API ---

#[derive(Debug, Serialize)]
struct OpenAIRequest {
    model: String,
    messages: Vec<OpenAIMessage>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    tools: Vec<OpenAITool>,
    max_tokens: u32,
    temperature: f32,
    stream: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct OpenAIMessage {
    role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    content: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_calls: Option<Vec<OpenAIToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_call_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct OpenAIToolCall {
    id: String,
    #[serde(rename = "type")]
    call_type: String,
    function: OpenAIFunctionCall,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct OpenAIFunctionCall {
    name: String,
    arguments: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct OpenAITool {
    #[serde(rename = "type")]
    tool_type: String,
    function: OpenAIFunctionDef,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct OpenAIFunctionDef {
    name: String,
    description: String,
    parameters: Value,
}

// --- SSE parsing types ---

#[derive(Debug, Deserialize)]
struct OpenAISseChunk {
    choices: Vec<OpenAISseChoice>,
}

#[derive(Debug, Deserialize)]
struct OpenAISseChoice {
    delta: OpenAISseDelta,
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OpenAISseDelta {
    content: Option<String>,
    tool_calls: Option<Vec<OpenAISseToolCall>>,
}

#[derive(Debug, Deserialize)]
struct OpenAISseToolCall {
    index: Option<u32>,
    id: Option<String>,
    function: Option<OpenAISseFunctionDelta>,
}

#[derive(Debug, Deserialize)]
struct OpenAISseFunctionDelta {
    name: Option<String>,
    arguments: Option<String>,
}

// --- OpenAIProvider ---

pub struct OpenAIProvider {
    api_key: String,
    base_url: String,
    client: Client,
    provider_name: String,
}

impl OpenAIProvider {
    pub fn new(api_key: String, base_url: String, provider_name: String) -> Self {
        Self {
            api_key,
            base_url,
            client: Client::new(),
            provider_name,
        }
    }
}

// --- Helper functions ---

/// Converts internal Messages to OpenAI wire format.
/// System messages become role "system".
/// User messages with ToolResult blocks become individual role "tool" messages.
/// User messages with text become role "user".
/// Assistant messages become role "assistant" with optional content + tool_calls.
pub fn to_openai_messages(messages: &[Message]) -> Vec<OpenAIMessage> {
    let mut result = Vec::new();

    for msg in messages {
        match msg.role {
            Role::System => {
                let text = msg.text_content();
                result.push(OpenAIMessage {
                    role: "system".to_string(),
                    content: Some(Value::String(text)),
                    tool_calls: None,
                    tool_call_id: None,
                });
            }
            Role::User => {
                // Check if this message contains only ToolResult blocks
                let has_tool_results = msg
                    .content
                    .iter()
                    .any(|b| matches!(b, ContentBlock::ToolResult { .. }));
                let has_text = msg
                    .content
                    .iter()
                    .any(|b| matches!(b, ContentBlock::Text { .. }));

                if has_tool_results && !has_text {
                    // Each ToolResult becomes its own "tool" role message
                    for block in &msg.content {
                        if let ContentBlock::ToolResult {
                            tool_use_id,
                            content,
                        } = block
                        {
                            result.push(OpenAIMessage {
                                role: "tool".to_string(),
                                content: Some(Value::String(content.clone())),
                                tool_calls: None,
                                tool_call_id: Some(tool_use_id.clone()),
                            });
                        }
                    }
                } else {
                    // Regular user text message
                    let text = msg.text_content();
                    result.push(OpenAIMessage {
                        role: "user".to_string(),
                        content: Some(Value::String(text)),
                        tool_calls: None,
                        tool_call_id: None,
                    });
                }
            }
            Role::Assistant => {
                // Collect text content
                let text = msg.text_content();
                let content = if text.is_empty() {
                    None
                } else {
                    Some(Value::String(text))
                };

                // Collect tool_calls from ToolUse blocks
                let tool_calls: Vec<OpenAIToolCall> = msg
                    .content
                    .iter()
                    .filter_map(|block| {
                        if let ContentBlock::ToolUse { id, name, input } = block {
                            Some(OpenAIToolCall {
                                id: id.clone(),
                                call_type: "function".to_string(),
                                function: OpenAIFunctionCall {
                                    name: name.clone(),
                                    arguments: input.to_string(),
                                },
                            })
                        } else {
                            None
                        }
                    })
                    .collect();

                let tool_calls = if tool_calls.is_empty() {
                    None
                } else {
                    Some(tool_calls)
                };

                result.push(OpenAIMessage {
                    role: "assistant".to_string(),
                    content,
                    tool_calls,
                    tool_call_id: None,
                });
            }
        }
    }

    result
}

/// Converts ToolDef list to OpenAI wire format.
pub fn to_openai_tools(tools: &[ToolDef]) -> Vec<OpenAITool> {
    tools
        .iter()
        .map(|t| OpenAITool {
            tool_type: "function".to_string(),
            function: OpenAIFunctionDef {
                name: t.name.clone(),
                description: t.description.clone(),
                parameters: t.input_schema.clone(),
            },
        })
        .collect()
}

/// Parses a single SSE data line from OpenAI format into zero or more StreamEvents.
/// Returns None for non-data or unparseable lines.
pub fn parse_openai_sse_line(line: &str) -> Option<Vec<StreamEvent>> {
    if !line.starts_with("data:") {
        return None;
    }

    let json_str = line["data:".len()..].trim();
    if json_str.is_empty() {
        return None;
    }

    if json_str == "[DONE]" {
        return Some(vec![StreamEvent::MessageEnd]);
    }

    let chunk: OpenAISseChunk = serde_json::from_str(json_str).ok()?;

    let mut events = Vec::new();

    for choice in &chunk.choices {
        // Check finish_reason — if present and non-null, emit MessageEnd
        let is_finished = choice
            .finish_reason
            .as_deref()
            .map(|r| !r.is_empty())
            .unwrap_or(false);

        // Text content delta
        if let Some(text) = &choice.delta.content {
            if !text.is_empty() {
                events.push(StreamEvent::Delta { text: text.clone() });
            }
        }

        // Tool call deltas
        if let Some(tool_calls) = &choice.delta.tool_calls {
            for tc in tool_calls {
                if let Some(id) = &tc.id {
                    // Tool call start: has id (and usually name)
                    let name = tc
                        .function
                        .as_ref()
                        .and_then(|f| f.name.clone())
                        .unwrap_or_default();
                    events.push(StreamEvent::ToolUseStart {
                        id: id.clone(),
                        name,
                    });
                } else if let Some(func) = &tc.function {
                    // Tool call delta: arguments fragment
                    if let Some(args) = &func.arguments {
                        if !args.is_empty() {
                            events.push(StreamEvent::ToolUseDelta {
                                partial_json: args.clone(),
                            });
                        }
                    }
                }
            }
        }

        if is_finished {
            events.push(StreamEvent::MessageEnd);
        }
    }

    if events.is_empty() {
        None
    } else {
        Some(events)
    }
}

// --- Provider trait implementation ---

#[async_trait]
impl Provider for OpenAIProvider {
    async fn send_message(
        &self,
        messages: &[Message],
        tools: &[ToolDef],
        config: &ModelConfig,
    ) -> Result<BoxStream<'static, StreamEvent>> {
        let openai_messages = to_openai_messages(messages);
        let openai_tools = to_openai_tools(tools);

        let request_body = OpenAIRequest {
            model: config.model_id.clone(),
            messages: openai_messages,
            tools: openai_tools,
            max_tokens: config.max_tokens,
            temperature: config.temperature,
            stream: true,
        };

        let response = self
            .client
            .post(format!("{}/v1/chat/completions", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("content-type", "application/json")
            .json(&request_body)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("OpenAI API error {}: {}", status, body);
        }

        let byte_stream = response.bytes_stream();

        let event_stream = byte_stream
            .map(|chunk_result| {
                chunk_result
                    .ok()
                    .map(|bytes| String::from_utf8_lossy(&bytes).to_string())
                    .unwrap_or_default()
            })
            .flat_map(|chunk| {
                let events: Vec<StreamEvent> = chunk
                    .lines()
                    .filter_map(parse_openai_sse_line)
                    .flatten()
                    .collect();
                futures::stream::iter(events)
            });

        Ok(Box::pin(event_stream))
    }

    fn name(&self) -> &str {
        &self.provider_name
    }

    fn supported_models(&self) -> Vec<ModelInfo> {
        match self.provider_name.as_str() {
            "openai" => vec![
                ModelInfo {
                    id: "gpt-4o".to_string(),
                    name: "GPT-4o".to_string(),
                },
                ModelInfo {
                    id: "gpt-4o-mini".to_string(),
                    name: "GPT-4o Mini".to_string(),
                },
            ],
            "zhipu" => vec![ModelInfo {
                id: "glm-4".to_string(),
                name: "GLM-4".to_string(),
            }],
            "minimax" => vec![ModelInfo {
                id: "abab6.5-chat".to_string(),
                name: "ABAB 6.5 Chat".to_string(),
            }],
            _ => vec![],
        }
    }
}

// --- Inline tests ---

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_to_openai_messages_system() {
        let messages = vec![Message::system("You are a helpful assistant.")];
        let result = to_openai_messages(&messages);

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].role, "system");
        assert_eq!(
            result[0].content,
            Some(Value::String("You are a helpful assistant.".to_string()))
        );
        assert!(result[0].tool_calls.is_none());
        assert!(result[0].tool_call_id.is_none());
    }

    #[test]
    fn test_to_openai_messages_user() {
        let messages = vec![Message::user("Hello!")];
        let result = to_openai_messages(&messages);

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].role, "user");
        assert_eq!(
            result[0].content,
            Some(Value::String("Hello!".to_string()))
        );
        assert!(result[0].tool_calls.is_none());
        assert!(result[0].tool_call_id.is_none());
    }

    #[test]
    fn test_to_openai_messages_tool_results() {
        let messages = vec![Message::tool_results(vec![
            ("call_001", "result one"),
            ("call_002", "result two"),
        ])];
        let result = to_openai_messages(&messages);

        // Each ToolResult becomes its own "tool" role message
        assert_eq!(result.len(), 2);

        assert_eq!(result[0].role, "tool");
        assert_eq!(
            result[0].content,
            Some(Value::String("result one".to_string()))
        );
        assert_eq!(result[0].tool_call_id, Some("call_001".to_string()));

        assert_eq!(result[1].role, "tool");
        assert_eq!(
            result[1].content,
            Some(Value::String("result two".to_string()))
        );
        assert_eq!(result[1].tool_call_id, Some("call_002".to_string()));
    }

    #[test]
    fn test_to_openai_messages_assistant_with_tool_calls() {
        let messages = vec![Message {
            role: Role::Assistant,
            content: vec![
                ContentBlock::Text {
                    text: "Let me check that.".to_string(),
                },
                ContentBlock::ToolUse {
                    id: "call_abc".to_string(),
                    name: "bash".to_string(),
                    input: json!({"cmd": "ls"}),
                },
            ],
        }];
        let result = to_openai_messages(&messages);

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].role, "assistant");
        assert_eq!(
            result[0].content,
            Some(Value::String("Let me check that.".to_string()))
        );

        let tool_calls = result[0].tool_calls.as_ref().expect("expected tool_calls");
        assert_eq!(tool_calls.len(), 1);
        assert_eq!(tool_calls[0].id, "call_abc");
        assert_eq!(tool_calls[0].call_type, "function");
        assert_eq!(tool_calls[0].function.name, "bash");
        // arguments is the JSON string representation of the input
        let args: Value = serde_json::from_str(&tool_calls[0].function.arguments).unwrap();
        assert_eq!(args["cmd"], "ls");
    }

    #[test]
    fn test_to_openai_tools() {
        let tools = vec![ToolDef {
            name: "read_file".to_string(),
            description: "Read a file from disk".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string"}
                },
                "required": ["path"]
            }),
        }];

        let result = to_openai_tools(&tools);

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].tool_type, "function");
        assert_eq!(result[0].function.name, "read_file");
        assert_eq!(result[0].function.description, "Read a file from disk");
        assert_eq!(result[0].function.parameters["properties"]["path"]["type"], "string");
    }

    #[test]
    fn test_parse_openai_sse_text_delta() {
        let line = r#"data: {"id":"chatcmpl-1","object":"chat.completion.chunk","choices":[{"index":0,"delta":{"content":"Hello"},"finish_reason":null}]}"#;
        let events = parse_openai_sse_line(line).expect("should parse");

        assert_eq!(events.len(), 1);
        match &events[0] {
            StreamEvent::Delta { text } => assert_eq!(text, "Hello"),
            other => panic!("unexpected event: {:?}", other),
        }
    }

    #[test]
    fn test_parse_openai_sse_done() {
        let line = "data: [DONE]";
        let events = parse_openai_sse_line(line).expect("should parse");

        assert_eq!(events.len(), 1);
        assert!(matches!(events[0], StreamEvent::MessageEnd));
    }

    #[test]
    fn test_parse_openai_sse_tool_call_start() {
        let line = r#"data: {"id":"chatcmpl-1","object":"chat.completion.chunk","choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"id":"call_xyz","type":"function","function":{"name":"bash","arguments":""}}]},"finish_reason":null}]}"#;
        let events = parse_openai_sse_line(line).expect("should parse");

        assert_eq!(events.len(), 1);
        match &events[0] {
            StreamEvent::ToolUseStart { id, name } => {
                assert_eq!(id, "call_xyz");
                assert_eq!(name, "bash");
            }
            other => panic!("unexpected event: {:?}", other),
        }
    }
}
