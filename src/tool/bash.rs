use anyhow::Result;
use async_trait::async_trait;
use serde_json::{json, Value};

use super::{Tool, ToolOutput};

pub struct BashTool;

#[async_trait]
impl Tool for BashTool {
    fn name(&self) -> &str {
        "bash"
    }

    fn description(&self) -> &str {
        "Execute a bash command and return its output. The command runs in the current working directory."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "The bash command to execute."
                },
                "timeout": {
                    "type": "integer",
                    "description": "Timeout in seconds. Defaults to 120."
                }
            },
            "required": ["command"]
        })
    }

    fn is_read_only(&self) -> bool {
        false
    }

    async fn execute(&self, input: Value) -> Result<ToolOutput> {
        let command = match input.get("command").and_then(|v| v.as_str()) {
            Some(c) if !c.is_empty() => c.to_string(),
            Some(_) => return Ok(ToolOutput::error("command must not be empty")),
            None => return Ok(ToolOutput::error("command is required")),
        };

        let result = tokio::task::spawn_blocking(move || {
            std::process::Command::new("bash")
                .arg("-c")
                .arg(&command)
                .output()
        })
        .await??;

        let stdout = String::from_utf8_lossy(&result.stdout).to_string();
        let stderr = String::from_utf8_lossy(&result.stderr).to_string();

        let mut output = stdout.clone();

        if !stderr.is_empty() {
            if !output.is_empty() {
                output.push('\n');
            }
            output.push_str("STDERR:\n");
            output.push_str(&stderr);
        }

        let exit_code = result.status.code().unwrap_or(-1);
        let is_error = !result.status.success();

        if is_error {
            if !output.is_empty() {
                output.push('\n');
            }
            output.push_str(&format!("Exit code: {}", exit_code));
        }

        if output.is_empty() {
            output = "(no output)".to_string();
        }

        if is_error {
            Ok(ToolOutput::error(output))
        } else {
            Ok(ToolOutput::success(output))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn test_bash_echo() {
        let tool = BashTool;
        let output = tool
            .execute(json!({ "command": "echo hello world" }))
            .await
            .unwrap();
        assert!(!output.is_error);
        assert!(output.content.contains("hello world"));
    }

    #[tokio::test]
    async fn test_bash_failing_command() {
        let tool = BashTool;
        let output = tool
            .execute(json!({ "command": "false" }))
            .await
            .unwrap();
        assert!(output.is_error);
        assert!(output.content.contains("Exit code"));
    }

    #[tokio::test]
    async fn test_bash_stderr() {
        let tool = BashTool;
        let output = tool
            .execute(json!({ "command": "echo error >&2" }))
            .await
            .unwrap();
        assert!(output.content.contains("STDERR"));
        assert!(output.content.contains("error"));
    }

    #[tokio::test]
    async fn test_bash_empty_command() {
        let tool = BashTool;
        let output = tool.execute(json!({})).await.unwrap();
        assert!(output.is_error);
    }

    #[tokio::test]
    async fn test_bash_pwd() {
        let tool = BashTool;
        let output = tool
            .execute(json!({ "command": "pwd" }))
            .await
            .unwrap();
        assert!(!output.is_error);
        assert!(!output.content.is_empty());
    }
}
