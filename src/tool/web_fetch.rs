use anyhow::Result;
use async_trait::async_trait;
use serde_json::{json, Value};
use std::time::Duration;

use super::{Tool, ToolOutput};

pub struct WebFetchTool;

#[async_trait]
impl Tool for WebFetchTool {
    fn name(&self) -> &str {
        "web_fetch"
    }

    fn description(&self) -> &str {
        "Fetch the content of a URL and return the response body as text."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "url": {
                    "type": "string",
                    "description": "The URL to fetch."
                },
                "max_length": {
                    "type": "integer",
                    "description": "Maximum number of characters to return. Defaults to 50000."
                }
            },
            "required": ["url"]
        })
    }

    fn is_read_only(&self) -> bool {
        true
    }

    async fn execute(&self, input: Value) -> Result<ToolOutput> {
        let url = match input.get("url").and_then(|v| v.as_str()) {
            Some(u) if !u.is_empty() => u.to_string(),
            Some(_) => return Ok(ToolOutput::error("url must not be empty")),
            None => return Ok(ToolOutput::error("url is required")),
        };

        let max_length = input
            .get("max_length")
            .and_then(|v| v.as_u64())
            .map(|n| n as usize)
            .unwrap_or(50000);

        let client = match reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
        {
            Ok(c) => c,
            Err(e) => return Ok(ToolOutput::error(format!("Failed to create HTTP client: {}", e))),
        };

        let response = match client.get(&url).send().await {
            Ok(r) => r,
            Err(e) => return Ok(ToolOutput::error(format!("Request failed: {}", e))),
        };

        if !response.status().is_success() {
            return Ok(ToolOutput::error(format!(
                "HTTP error: {}",
                response.status()
            )));
        }

        let body = match response.text().await {
            Ok(t) => t,
            Err(e) => return Ok(ToolOutput::error(format!("Failed to read response body: {}", e))),
        };

        if body.len() > max_length {
            let truncated = &body[..max_length];
            Ok(ToolOutput::success(format!(
                "{}\n\n[Content truncated at {} characters]",
                truncated, max_length
            )))
        } else {
            Ok(ToolOutput::success(body))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn test_web_fetch_missing_url() {
        let tool = WebFetchTool;
        let output = tool.execute(json!({})).await.unwrap();
        assert!(output.is_error);
        assert!(output.content.contains("url is required"));
    }

    #[tokio::test]
    async fn test_web_fetch_empty_url() {
        let tool = WebFetchTool;
        let output = tool.execute(json!({ "url": "" })).await.unwrap();
        assert!(output.is_error);
        assert!(output.content.contains("url must not be empty"));
    }
}
