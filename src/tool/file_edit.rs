use anyhow::Result;
use async_trait::async_trait;
use serde_json::{json, Value};
use std::fs;
use std::path::PathBuf;

use super::{Tool, ToolOutput};

pub struct FileEditTool;

#[async_trait]
impl Tool for FileEditTool {
    fn name(&self) -> &str {
        "file_edit"
    }

    fn description(&self) -> &str {
        "Edit a file by replacing a specific string with a new string. Use replace_all to replace all occurrences. By default, errors if the string is not found or found multiple times to prevent ambiguous edits."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Absolute path to the file to edit."
                },
                "old_string": {
                    "type": "string",
                    "description": "The exact string to find and replace."
                },
                "new_string": {
                    "type": "string",
                    "description": "The string to replace old_string with."
                },
                "replace_all": {
                    "type": "boolean",
                    "description": "If true, replace all occurrences of old_string. Defaults to false."
                }
            },
            "required": ["path", "old_string", "new_string"]
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

        let old_string = match input.get("old_string").and_then(|v| v.as_str()) {
            Some(s) => s.to_string(),
            None => return Ok(ToolOutput::error("old_string is required")),
        };

        let new_string = match input.get("new_string").and_then(|v| v.as_str()) {
            Some(s) => s.to_string(),
            None => return Ok(ToolOutput::error("new_string is required")),
        };

        let replace_all = input
            .get("replace_all")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        if !path.exists() {
            return Ok(ToolOutput::error(format!(
                "File not found: {}",
                path.display()
            )));
        }

        let content = match fs::read_to_string(&path) {
            Ok(c) => c,
            Err(e) => return Ok(ToolOutput::error(format!("Failed to read file: {}", e))),
        };

        let match_count = content.matches(old_string.as_str()).count();

        if replace_all {
            let new_content = content.replace(old_string.as_str(), new_string.as_str());
            if let Err(e) = fs::write(&path, &new_content) {
                return Ok(ToolOutput::error(format!("Failed to write file: {}", e)));
            }
            return Ok(ToolOutput::success(format!(
                "Replaced {} occurrence(s) of the string in {}",
                match_count,
                path.display()
            )));
        }

        // Not replace_all: require exactly one match
        if match_count == 0 {
            return Ok(ToolOutput::error(format!(
                "String not found in {}",
                path.display()
            )));
        }

        if match_count > 1 {
            return Ok(ToolOutput::error(format!(
                "String found {} times in {}. Provide more context to make it unique, or use replace_all.",
                match_count,
                path.display()
            )));
        }

        // Exactly one match - replace it
        let new_content = content.replacen(old_string.as_str(), new_string.as_str(), 1);
        if let Err(e) = fs::write(&path, &new_content) {
            return Ok(ToolOutput::error(format!("Failed to write file: {}", e)));
        }

        Ok(ToolOutput::success(format!(
            "Successfully edited {}",
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
    async fn test_file_edit_basic() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("test.txt");
        fs::write(&file_path, "hello world\nfoo bar\n").unwrap();

        let tool = FileEditTool;
        let output = tool
            .execute(json!({
                "path": file_path.to_str().unwrap(),
                "old_string": "hello world",
                "new_string": "goodbye world"
            }))
            .await
            .unwrap();

        assert!(!output.is_error);
        let content = fs::read_to_string(&file_path).unwrap();
        assert!(content.contains("goodbye world"));
        assert!(!content.contains("hello world"));
    }

    #[tokio::test]
    async fn test_file_edit_not_found_string() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("test.txt");
        fs::write(&file_path, "hello world\n").unwrap();

        let tool = FileEditTool;
        let output = tool
            .execute(json!({
                "path": file_path.to_str().unwrap(),
                "old_string": "nonexistent string",
                "new_string": "replacement"
            }))
            .await
            .unwrap();

        assert!(output.is_error);
        assert!(output.content.contains("not found"));
    }

    #[tokio::test]
    async fn test_file_edit_ambiguous() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("test.txt");
        fs::write(&file_path, "foo\nfoo\nbar\n").unwrap();

        let tool = FileEditTool;
        let output = tool
            .execute(json!({
                "path": file_path.to_str().unwrap(),
                "old_string": "foo",
                "new_string": "baz"
            }))
            .await
            .unwrap();

        assert!(output.is_error);
        assert!(output.content.contains("2 times"));
    }

    #[tokio::test]
    async fn test_file_edit_replace_all() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("test.txt");
        fs::write(&file_path, "foo\nfoo\nbar\nfoo\n").unwrap();

        let tool = FileEditTool;
        let output = tool
            .execute(json!({
                "path": file_path.to_str().unwrap(),
                "old_string": "foo",
                "new_string": "baz",
                "replace_all": true
            }))
            .await
            .unwrap();

        assert!(!output.is_error);
        assert!(output.content.contains("3 occurrence(s)"));
        let content = fs::read_to_string(&file_path).unwrap();
        assert!(!content.contains("foo"));
        assert_eq!(content.matches("baz").count(), 3);
    }
}
