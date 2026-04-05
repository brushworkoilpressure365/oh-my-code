use anyhow::Result;
use async_trait::async_trait;
use serde_json::{json, Value};

use super::{Tool, ToolOutput};

pub struct ThinkTool;

#[async_trait]
impl Tool for ThinkTool {
    fn name(&self) -> &str {
        "think"
    }

    fn description(&self) -> &str {
        "Use this tool to think through a problem step-by-step. Your thoughts will not be shown to the user. Use it when you need to reason about complex tasks, plan your approach, or work through a problem before taking action."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "thought": {
                    "type": "string",
                    "description": "Your step-by-step thinking about the problem."
                }
            },
            "required": ["thought"]
        })
    }

    fn is_read_only(&self) -> bool {
        true
    }

    async fn execute(&self, input: Value) -> Result<ToolOutput> {
        let thought = input
            .get("thought")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let char_count = thought.chars().count();
        Ok(ToolOutput::success(format!(
            "Thought recorded ({} characters).",
            char_count
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn test_think_tool_basic() {
        let tool = ThinkTool;
        assert_eq!(tool.name(), "think");
        assert!(tool.is_read_only());

        let input = json!({ "thought": "I need to analyze the problem carefully." });
        let output = tool.execute(input).await.unwrap();
        assert!(!output.is_error);
        assert!(output.content.contains("40"));
    }

    #[tokio::test]
    async fn test_think_tool_empty() {
        let tool = ThinkTool;

        // Empty object — no "thought" key
        let output = tool.execute(json!({})).await.unwrap();
        assert!(!output.is_error);
        assert!(output.content.contains("0"));
    }
}
