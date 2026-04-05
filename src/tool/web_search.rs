use anyhow::Result;
use async_trait::async_trait;
use serde_json::{json, Value};
use std::time::Duration;

use super::{Tool, ToolOutput};

pub struct WebSearchTool;

#[async_trait]
impl Tool for WebSearchTool {
    fn name(&self) -> &str {
        "web_search"
    }

    fn description(&self) -> &str {
        "Search the web using DuckDuckGo and return a list of results with titles, URLs, and snippets."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "The search query."
                },
                "max_results": {
                    "type": "integer",
                    "description": "Maximum number of results to return. Defaults to 10."
                }
            },
            "required": ["query"]
        })
    }

    fn is_read_only(&self) -> bool {
        true
    }

    async fn execute(&self, input: Value) -> Result<ToolOutput> {
        let query = match input.get("query").and_then(|v| v.as_str()) {
            Some(q) if !q.is_empty() => q.to_string(),
            Some(_) => return Ok(ToolOutput::error("query must not be empty")),
            None => return Ok(ToolOutput::error("query is required")),
        };

        let max_results = input
            .get("max_results")
            .and_then(|v| v.as_u64())
            .map(|n| n as usize)
            .unwrap_or(10);

        let encoded = urlencoding::encode(&query);
        let url = format!("https://html.duckduckgo.com/html/?q={}", encoded);

        let client = match reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .user_agent("Mozilla/5.0 (compatible; oh-my-code/0.1)")
            .build()
        {
            Ok(c) => c,
            Err(e) => return Ok(ToolOutput::error(format!("Failed to create HTTP client: {}", e))),
        };

        let response = match client.get(&url).send().await {
            Ok(r) => r,
            Err(e) => return Ok(ToolOutput::error(format!("Search request failed: {}", e))),
        };

        if !response.status().is_success() {
            return Ok(ToolOutput::error(format!(
                "HTTP error: {}",
                response.status()
            )));
        }

        let html = match response.text().await {
            Ok(t) => t,
            Err(e) => return Ok(ToolOutput::error(format!("Failed to read response body: {}", e))),
        };

        let results = parse_ddg_results(&html, max_results);

        if results.is_empty() {
            return Ok(ToolOutput::success("No results found."));
        }

        let mut output = String::new();
        for (i, (title, url, snippet)) in results.iter().enumerate() {
            output.push_str(&format!("{}. {}\n   {}\n   {}\n\n", i + 1, title, url, snippet));
        }

        Ok(ToolOutput::success(output.trim_end().to_string()))
    }
}

/// Parse DuckDuckGo HTML search results, returning up to `max` entries as (title, url, snippet).
pub fn parse_ddg_results(html: &str, max: usize) -> Vec<(String, String, String)> {
    let mut results = Vec::new();

    for chunk in html.split("class=\"result__a\"") {
        if results.len() >= max {
            break;
        }

        // Skip the first split which is before any result
        if chunk.trim_start().starts_with('<') || chunk.contains("result__snippet") {
            // This is not a useful leading chunk — skip
        }

        // Extract href for URL
        let url = extract_between(chunk, "href=\"", "\"")
            .unwrap_or_default()
            .trim()
            .to_string();

        // Extract title text (content between > and </a>)
        let title = extract_between(chunk, ">", "</a>")
            .map(|s| strip_html_tags(&s))
            .unwrap_or_default()
            .trim()
            .to_string();

        // Extract snippet from class="result__snippet"
        let snippet = if let Some(rest) = html
            .split("class=\"result__snippet\"")
            .nth(results.len() + 1)
        {
            extract_between(rest, ">", "</a>")
                .map(|s| strip_html_tags(&s))
                .unwrap_or_default()
                .trim()
                .to_string()
        } else {
            String::new()
        };

        if !title.is_empty() && !url.is_empty() {
            results.push((title, url, snippet));
        }
    }

    results
}

/// Extract text between `start` and `end` delimiters.
pub fn extract_between(text: &str, start: &str, end: &str) -> Option<String> {
    let start_pos = text.find(start)?;
    let after_start = &text[start_pos + start.len()..];
    let end_pos = after_start.find(end)?;
    Some(after_start[..end_pos].to_string())
}

/// Strip HTML tags from a string.
fn strip_html_tags(html: &str) -> String {
    let mut result = String::new();
    let mut in_tag = false;
    for ch in html.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => result.push(ch),
            _ => {}
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn test_web_search_missing_query() {
        let tool = WebSearchTool;
        let output = tool.execute(json!({})).await.unwrap();
        assert!(output.is_error);
        assert!(output.content.contains("query is required"));
    }

    #[test]
    fn test_extract_between() {
        let text = "hello <b>world</b> foo";
        let result = extract_between(text, "<b>", "</b>");
        assert_eq!(result, Some("world".to_string()));
    }

    #[test]
    fn test_extract_between_not_found() {
        let text = "no delimiters here";
        assert!(extract_between(text, "<b>", "</b>").is_none());
    }

    #[test]
    fn test_extract_between_missing_end() {
        let text = "starts <b> but no end";
        assert!(extract_between(text, "<b>", "</b>").is_none());
    }
}
