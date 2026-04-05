use anyhow::Result;
use async_trait::async_trait;
use serde_json::{json, Value};
use std::env;
use std::path::PathBuf;

use super::{Tool, ToolOutput};
use crate::search::finder::{find_files, FindOptions};

pub struct GlobTool;

#[async_trait]
impl Tool for GlobTool {
    fn name(&self) -> &str {
        "glob"
    }

    fn description(&self) -> &str {
        "Find files matching a glob pattern. Returns file paths sorted by modification time (most recent first)."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "pattern": {
                    "type": "string",
                    "description": "The glob pattern to match files against (e.g. '**/*.rs', '*.toml')."
                },
                "path": {
                    "type": "string",
                    "description": "Directory to search in. Defaults to current working directory."
                },
                "max_results": {
                    "type": "integer",
                    "description": "Maximum number of results to return. Defaults to 500."
                }
            },
            "required": ["pattern"]
        })
    }

    fn is_read_only(&self) -> bool {
        true
    }

    async fn execute(&self, input: Value) -> Result<ToolOutput> {
        let pattern = match input.get("pattern").and_then(|v| v.as_str()) {
            Some(p) if !p.is_empty() => p.to_string(),
            Some(_) => return Ok(ToolOutput::error("pattern must not be empty")),
            None => return Ok(ToolOutput::error("pattern is required")),
        };

        let base_path = input
            .get("path")
            .and_then(|v| v.as_str())
            .map(PathBuf::from)
            .unwrap_or_else(|| env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));

        let max_results = input
            .get("max_results")
            .and_then(|v| v.as_u64())
            .map(|n| n as usize)
            .unwrap_or(500);

        let options = FindOptions {
            pattern,
            path: base_path.clone(),
            max_results,
            ignore_patterns: vec![
                "node_modules".to_string(),
                ".git".to_string(),
                "dist".to_string(),
                "build".to_string(),
                "target".to_string(),
            ],
        };

        match find_files(&options) {
            Ok(paths) => {
                let mut output = String::new();
                for p in &paths {
                    // Display as relative path when possible
                    let display = p
                        .strip_prefix(&base_path)
                        .map(|rel| rel.to_string_lossy().to_string())
                        .unwrap_or_else(|_| p.to_string_lossy().to_string());
                    output.push_str(&display);
                    output.push('\n');
                }
                output.push_str(&format!("{} file(s) found.", paths.len()));
                Ok(ToolOutput::success(output))
            }
            Err(e) => Ok(ToolOutput::error(format!("Search error: {}", e))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::fs;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_glob_tool_missing_pattern() {
        let tool = GlobTool;
        let output = tool.execute(json!({})).await.unwrap();
        assert!(output.is_error);
        assert!(output.content.contains("pattern is required"));
    }

    #[tokio::test]
    async fn test_glob_tool_find_rs_files() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("main.rs"), "fn main() {}").unwrap();
        fs::write(dir.path().join("lib.rs"), "pub fn foo() {}").unwrap();
        fs::write(dir.path().join("config.toml"), "[section]").unwrap();

        let tool = GlobTool;
        let output = tool
            .execute(json!({
                "pattern": "*.rs",
                "path": dir.path().to_str().unwrap()
            }))
            .await
            .unwrap();

        assert!(!output.is_error);
        assert!(output.content.contains("2 file(s) found."));
        assert!(output.content.contains(".rs"));
        assert!(!output.content.contains("config.toml"));
    }
}
