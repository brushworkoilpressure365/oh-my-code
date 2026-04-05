use anyhow::Result;
use async_trait::async_trait;
use serde_json::{json, Value};
use std::fs;
use std::path::PathBuf;

use super::{Tool, ToolOutput};

pub struct FileWriteTool;

#[async_trait]
impl Tool for FileWriteTool {
    fn name(&self) -> &str {
        "file_write"
    }

    fn description(&self) -> &str {
        "Write content to a file on the local filesystem. Creates the file and any necessary parent directories if they do not exist. Overwrites existing files."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Absolute path to the file to write."
                },
                "content": {
                    "type": "string",
                    "description": "The content to write to the file."
                }
            },
            "required": ["path", "content"]
        })
    }

    fn is_read_only(&self) -> bool {
        false
    }

    async fn execute(&self, input: Value) -> Result<ToolOutput> {
        let path = match input.get("path").and_then(|v| v.as_str()) {
            Some(p) if !p.is_empty() => PathBuf::from(p),
            Some(_) => return Ok(ToolOutput::error("path must not be empty")),
            None => return Ok(ToolOutput::error("path is required")),
        };

        let content = match input.get("content").and_then(|v| v.as_str()) {
            Some(c) => c.to_string(),
            None => return Ok(ToolOutput::error("content is required")),
        };

        // Create parent directories if needed
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() && !parent.exists() {
                if let Err(e) = fs::create_dir_all(parent) {
                    return Ok(ToolOutput::error(format!(
                        "Failed to create directories: {}",
                        e
                    )));
                }
            }
        }

        let bytes = content.len();
        if let Err(e) = fs::write(&path, content.as_bytes()) {
            return Ok(ToolOutput::error(format!("Failed to write file: {}", e)));
        }

        Ok(ToolOutput::success(format!(
            "Successfully wrote {} bytes to {}",
            bytes,
            path.display()
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::fs;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_file_write_new_file() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("output.txt");

        let tool = FileWriteTool;
        let output = tool
            .execute(json!({
                "path": file_path.to_str().unwrap(),
                "content": "hello, world!\n"
            }))
            .await
            .unwrap();

        assert!(!output.is_error);
        assert!(output.content.contains("bytes"));
        let content = fs::read_to_string(&file_path).unwrap();
        assert_eq!(content, "hello, world!\n");
    }

    #[tokio::test]
    async fn test_file_write_creates_directories() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("a").join("b").join("c").join("file.txt");

        let tool = FileWriteTool;
        let output = tool
            .execute(json!({
                "path": file_path.to_str().unwrap(),
                "content": "nested content"
            }))
            .await
            .unwrap();

        assert!(!output.is_error);
        assert!(file_path.exists());
        let content = fs::read_to_string(&file_path).unwrap();
        assert_eq!(content, "nested content");
    }

    #[tokio::test]
    async fn test_file_write_overwrite() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("test.txt");
        fs::write(&file_path, "original content").unwrap();

        let tool = FileWriteTool;
        let output = tool
            .execute(json!({
                "path": file_path.to_str().unwrap(),
                "content": "new content"
            }))
            .await
            .unwrap();

        assert!(!output.is_error);
        let content = fs::read_to_string(&file_path).unwrap();
        assert_eq!(content, "new content");
        assert!(!content.contains("original"));
    }

    #[tokio::test]
    async fn test_file_write_missing_path() {
        let tool = FileWriteTool;
        let output = tool
            .execute(json!({ "content": "some content" }))
            .await
            .unwrap();

        assert!(output.is_error);
        assert!(output.content.contains("path is required"));
    }
}
