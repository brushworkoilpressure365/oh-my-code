use anyhow::Result;
use async_trait::async_trait;
use serde_json::{json, Value};
use std::fs;
use std::path::PathBuf;

use super::{Tool, ToolOutput};

pub struct FileReadTool;

#[async_trait]
impl Tool for FileReadTool {
    fn name(&self) -> &str {
        "file_read"
    }

    fn description(&self) -> &str {
        "Read a file from the local filesystem. Returns file contents with line numbers. Supports optional offset and limit for reading specific portions of large files."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Absolute path to the file to read."
                },
                "offset": {
                    "type": "integer",
                    "description": "Line number to start reading from (0-indexed). Defaults to 0."
                },
                "limit": {
                    "type": "integer",
                    "description": "Maximum number of lines to read. Defaults to 2000."
                }
            },
            "required": ["path"]
        })
    }

    fn is_read_only(&self) -> bool {
        true
    }

    async fn execute(&self, input: Value) -> Result<ToolOutput> {
        let path = match input.get("path").and_then(|v| v.as_str()) {
            Some(p) if !p.is_empty() => PathBuf::from(p),
            Some(_) => return Ok(ToolOutput::error("path must not be empty")),
            None => return Ok(ToolOutput::error("path is required")),
        };

        if !path.exists() {
            return Ok(ToolOutput::error(format!(
                "File not found: {}",
                path.display()
            )));
        }

        if !path.is_file() {
            return Ok(ToolOutput::error(format!(
                "Path is not a file: {}",
                path.display()
            )));
        }

        let content = match fs::read_to_string(&path) {
            Ok(c) => c,
            Err(e) => return Ok(ToolOutput::error(format!("Failed to read file: {}", e))),
        };

        let offset = input
            .get("offset")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as usize;

        let limit = input
            .get("limit")
            .and_then(|v| v.as_u64())
            .unwrap_or(2000) as usize;

        let all_lines: Vec<&str> = content.lines().collect();
        let total_lines = all_lines.len();

        let start = offset.min(total_lines);
        let end = (start + limit).min(total_lines);
        let selected = &all_lines[start..end];

        let mut output = String::new();
        for (i, line) in selected.iter().enumerate() {
            let line_num = start + i + 1; // 1-indexed for display
            output.push_str(&format!("{:4} | {}\n", line_num, line));
        }

        if end < total_lines {
            output.push_str(&format!(
                "\n[Truncated: showing lines {}-{} of {}. Use offset and limit to read more.]\n",
                start + 1,
                end,
                total_lines
            ));
        }

        Ok(ToolOutput::success(output))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::fs;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_file_read_basic() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("test.txt");
        fs::write(&file_path, "line one\nline two\nline three\n").unwrap();

        let tool = FileReadTool;
        let output = tool
            .execute(json!({ "path": file_path.to_str().unwrap() }))
            .await
            .unwrap();

        assert!(!output.is_error);
        assert!(output.content.contains("1 | line one"));
        assert!(output.content.contains("2 | line two"));
        assert!(output.content.contains("3 | line three"));
    }

    #[tokio::test]
    async fn test_file_read_with_offset_and_limit() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("test.txt");
        let content = (1..=10)
            .map(|i| format!("line {}", i))
            .collect::<Vec<_>>()
            .join("\n");
        fs::write(&file_path, content).unwrap();

        let tool = FileReadTool;
        let output = tool
            .execute(json!({
                "path": file_path.to_str().unwrap(),
                "offset": 2,
                "limit": 3
            }))
            .await
            .unwrap();

        assert!(!output.is_error);
        // offset=2 means start from line index 2 (line 3), limit=3 means lines 3,4,5
        assert!(output.content.contains("3 | line 3"));
        assert!(output.content.contains("4 | line 4"));
        assert!(output.content.contains("5 | line 5"));
        assert!(!output.content.contains("line 1\n"));
        assert!(!output.content.contains("line 6"));
        // Should include truncation notice since only showing part of the file
        assert!(output.content.contains("Truncated"));
    }

    #[tokio::test]
    async fn test_file_read_not_found() {
        let tool = FileReadTool;
        let output = tool
            .execute(json!({ "path": "/nonexistent/path/file.txt" }))
            .await
            .unwrap();

        assert!(output.is_error);
        assert!(output.content.contains("File not found"));
    }

    #[tokio::test]
    async fn test_file_read_missing_path() {
        let tool = FileReadTool;
        let output = tool.execute(json!({})).await.unwrap();

        assert!(output.is_error);
        assert!(output.content.contains("path is required"));
    }
}
