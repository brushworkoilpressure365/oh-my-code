use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Role of a message participant.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    System,
    User,
    Assistant,
}

/// A block of content within a message.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlock {
    Text {
        text: String,
    },
    ToolUse {
        id: String,
        name: String,
        input: Value,
    },
    ToolResult {
        tool_use_id: String,
        content: String,
    },
}

/// A message in a conversation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Message {
    pub role: Role,
    pub content: Vec<ContentBlock>,
}

/// Events emitted during streaming from the model.
#[derive(Debug)]
pub enum StreamEvent {
    Delta {
        text: String,
    },
    ToolUseStart {
        id: String,
        name: String,
    },
    ToolUseDelta {
        partial_json: String,
    },
    InputJsonComplete {
        id: String,
        name: String,
        input: Value,
    },
    MessageEnd,
}

/// Definition of a tool that can be called by the model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDef {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
}

/// Configuration for a model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelConfig {
    pub model_id: String,
    pub max_tokens: u32,
    pub temperature: f32,
}

/// Information about an available model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    pub id: String,
    pub name: String,
}

// --- Message convenience constructors ---

impl Message {
    /// Create a user message with a single text block.
    pub fn user(text: impl Into<String>) -> Self {
        Self {
            role: Role::User,
            content: vec![ContentBlock::Text { text: text.into() }],
        }
    }

    /// Create an assistant message with a single text block.
    pub fn assistant(text: impl Into<String>) -> Self {
        Self {
            role: Role::Assistant,
            content: vec![ContentBlock::Text { text: text.into() }],
        }
    }

    /// Create a system message with a single text block.
    pub fn system(text: impl Into<String>) -> Self {
        Self {
            role: Role::System,
            content: vec![ContentBlock::Text { text: text.into() }],
        }
    }

    /// Create a user message containing tool results.
    /// Each item is (tool_use_id, result_content).
    pub fn tool_results(results: Vec<(impl Into<String>, impl Into<String>)>) -> Self {
        let content = results
            .into_iter()
            .map(|(id, content)| ContentBlock::ToolResult {
                tool_use_id: id.into(),
                content: content.into(),
            })
            .collect();
        Self {
            role: Role::User,
            content,
        }
    }

    // --- Message helper methods ---

    /// Extract all ToolUse blocks as (id, name, input) tuples.
    pub fn tool_use_blocks(&self) -> Vec<(&str, &str, &Value)> {
        self.content
            .iter()
            .filter_map(|block| {
                if let ContentBlock::ToolUse { id, name, input } = block {
                    Some((id.as_str(), name.as_str(), input))
                } else {
                    None
                }
            })
            .collect()
    }

    /// Concatenate all Text block content into a single string.
    pub fn text_content(&self) -> String {
        self.content
            .iter()
            .filter_map(|block| {
                if let ContentBlock::Text { text } = block {
                    Some(text.as_str())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
            .join("")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_message_user_constructor() {
        let msg = Message::user("Hello, world!");
        assert_eq!(msg.role, Role::User);
        assert_eq!(msg.content.len(), 1);
        assert_eq!(
            msg.content[0],
            ContentBlock::Text {
                text: "Hello, world!".to_string()
            }
        );
        assert_eq!(msg.text_content(), "Hello, world!");
    }

    #[test]
    fn test_message_assistant_constructor() {
        let msg = Message::assistant("I can help with that.");
        assert_eq!(msg.role, Role::Assistant);
        assert_eq!(msg.content.len(), 1);
        assert_eq!(
            msg.content[0],
            ContentBlock::Text {
                text: "I can help with that.".to_string()
            }
        );
        assert_eq!(msg.text_content(), "I can help with that.");
    }

    #[test]
    fn test_tool_use_blocks_extraction() {
        let input = json!({"path": "/tmp/foo.txt"});
        let msg = Message {
            role: Role::Assistant,
            content: vec![
                ContentBlock::Text {
                    text: "Let me read that file.".to_string(),
                },
                ContentBlock::ToolUse {
                    id: "tu_001".to_string(),
                    name: "read_file".to_string(),
                    input: input.clone(),
                },
            ],
        };

        let blocks = msg.tool_use_blocks();
        assert_eq!(blocks.len(), 1);
        let (id, name, got_input) = blocks[0];
        assert_eq!(id, "tu_001");
        assert_eq!(name, "read_file");
        assert_eq!(*got_input, input);
    }

    #[test]
    fn test_tool_results_message() {
        let msg = Message::tool_results(vec![
            ("tu_001", "file content here"),
            ("tu_002", "another result"),
        ]);

        assert_eq!(msg.role, Role::User);
        assert_eq!(msg.content.len(), 2);
        assert_eq!(
            msg.content[0],
            ContentBlock::ToolResult {
                tool_use_id: "tu_001".to_string(),
                content: "file content here".to_string(),
            }
        );
        assert_eq!(
            msg.content[1],
            ContentBlock::ToolResult {
                tool_use_id: "tu_002".to_string(),
                content: "another result".to_string(),
            }
        );
    }

    #[test]
    fn test_content_block_serialization() {
        // Text block
        let text_block = ContentBlock::Text {
            text: "hello".to_string(),
        };
        let json = serde_json::to_string(&text_block).unwrap();
        assert!(json.contains(r#""type":"text""#));
        assert!(json.contains(r#""text":"hello""#));
        let roundtrip: ContentBlock = serde_json::from_str(&json).unwrap();
        assert_eq!(roundtrip, text_block);

        // ToolUse block
        let tool_block = ContentBlock::ToolUse {
            id: "tu_abc".to_string(),
            name: "bash".to_string(),
            input: json!({"cmd": "ls"}),
        };
        let json = serde_json::to_string(&tool_block).unwrap();
        assert!(json.contains(r#""type":"tool_use""#));
        let roundtrip: ContentBlock = serde_json::from_str(&json).unwrap();
        assert_eq!(roundtrip, tool_block);

        // ToolResult block
        let result_block = ContentBlock::ToolResult {
            tool_use_id: "tu_abc".to_string(),
            content: "output".to_string(),
        };
        let json = serde_json::to_string(&result_block).unwrap();
        assert!(json.contains(r#""type":"tool_result""#));
        let roundtrip: ContentBlock = serde_json::from_str(&json).unwrap();
        assert_eq!(roundtrip, result_block);

        // Role serialization
        let role = Role::Assistant;
        let json = serde_json::to_string(&role).unwrap();
        assert_eq!(json, r#""assistant""#);
        let roundtrip: Role = serde_json::from_str(&json).unwrap();
        assert_eq!(roundtrip, role);
    }
}
