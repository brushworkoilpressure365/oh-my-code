use anyhow::Result;
use async_trait::async_trait;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::env;
use std::path::PathBuf;

use super::{Tool, ToolOutput};
use crate::search::ripgrep::{grep_search, GrepMatch, GrepOptions};

pub struct GrepTool;

#[async_trait]
impl Tool for GrepTool {
    fn name(&self) -> &str {
        "grep"
    }

    fn description(&self) -> &str {
        "Search for a pattern in files using ripgrep. Returns matching lines with file paths and line numbers. Supports regex patterns."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "pattern": {
                    "type": "string",
                    "description": "The regex pattern to search for."
                },
                "path": {
                    "type": "string",
                    "description": "Directory to search in. Defaults to current working directory."
                },
                "case_insensitive": {
                    "type": "boolean",
                    "description": "Whether to perform a case-insensitive search. Defaults to false."
                },
                "glob": {
                    "type": "string",
                    "description": "Glob pattern to filter which files are searched (e.g. '*.rs')."
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

        let path = input
            .get("path")
            .and_then(|v| v.as_str())
            .map(PathBuf::from)
            .unwrap_or_else(|| env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));

        let case_insensitive = input
            .get("case_insensitive")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let glob_filter = input
            .get("glob")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let max_results = input
            .get("max_results")
            .and_then(|v| v.as_u64())
            .map(|n| n as usize)
            .unwrap_or(500);

        let options = GrepOptions {
            pattern,
            path,
            case_insensitive,
            max_results,
            glob_filter,
            ignore_patterns: vec![
                "node_modules".to_string(),
                ".git".to_string(),
                "dist".to_string(),
                "build".to_string(),
                "target".to_string(),
            ],
        };

        match grep_search(&options) {
            Ok(matches) => Ok(ToolOutput::success(format_matches(&matches))),
            Err(e) => Ok(ToolOutput::error(format!("Search error: {}", e))),
        }
    }
}

pub fn format_matches(matches: &[GrepMatch]) -> String {
    if matches.is_empty() {
        return "0 match(es) found.".to_string();
    }

    // Group by file, preserving insertion order via BTreeMap for deterministic output
    let mut by_file: BTreeMap<String, Vec<&GrepMatch>> = BTreeMap::new();
    for m in matches {
        let key = m.file_path.to_string_lossy().to_string();
        by_file.entry(key).or_default().push(m);
    }

    let mut output = String::new();
    for (file, file_matches) in &by_file {
        output.push_str(file);
        output.push_str(":\n");
        for m in file_matches {
            output.push_str(&format!("    {} | {}\n", m.line_number, m.line_content));
        }
    }

    output.push_str(&format!("{} match(es) found.", matches.len()));
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::fs;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_grep_tool_missing_pattern() {
        let tool = GrepTool;
        let output = tool.execute(json!({})).await.unwrap();
        assert!(output.is_error);
        assert!(output.content.contains("pattern is required"));
    }

    #[tokio::test]
    async fn test_grep_tool_empty_pattern() {
        let tool = GrepTool;
        let output = tool.execute(json!({ "pattern": "" })).await.unwrap();
        assert!(output.is_error);
        assert!(output.content.contains("pattern must not be empty"));
    }

    #[tokio::test]
    async fn test_grep_tool_search() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("hello.txt"), "hello world\nfoo bar\n").unwrap();
        fs::write(dir.path().join("other.txt"), "no match here\n").unwrap();

        let tool = GrepTool;
        let output = tool
            .execute(json!({
                "pattern": "hello",
                "path": dir.path().to_str().unwrap()
            }))
            .await
            .unwrap();

        assert!(!output.is_error);
        assert!(output.content.contains("hello"));
        assert!(output.content.contains("1 match(es) found."));
    }

    #[test]
    fn test_format_matches() {
        use std::path::PathBuf;

        let matches = vec![
            GrepMatch {
                file_path: PathBuf::from("/tmp/a.txt"),
                line_number: 3,
                line_content: "hello world".to_string(),
            },
            GrepMatch {
                file_path: PathBuf::from("/tmp/a.txt"),
                line_number: 7,
                line_content: "hello again".to_string(),
            },
            GrepMatch {
                file_path: PathBuf::from("/tmp/b.txt"),
                line_number: 1,
                line_content: "say hello".to_string(),
            },
        ];

        let result = format_matches(&matches);
        assert!(result.contains("/tmp/a.txt:"));
        assert!(result.contains("    3 | hello world"));
        assert!(result.contains("    7 | hello again"));
        assert!(result.contains("/tmp/b.txt:"));
        assert!(result.contains("    1 | say hello"));
        assert!(result.ends_with("3 match(es) found."));
    }
}
