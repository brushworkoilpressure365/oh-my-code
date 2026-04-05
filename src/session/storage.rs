use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::model::types::Message;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionData {
    pub id: String,
    pub created_at: String,
    pub updated_at: String,
    pub model: String,
    pub provider: String,
    pub messages: Vec<Message>,
}

#[derive(Debug)]
pub struct SessionSummary {
    pub id: String,
    pub created_at: String,
    pub updated_at: String,
    pub model: String,
    pub message_count: usize,
}

pub fn save_session(dir: &Path, session: &SessionData) -> Result<()> {
    std::fs::create_dir_all(dir)
        .with_context(|| format!("Failed to create session directory {}", dir.display()))?;

    let file_path = dir.join(format!("{}.json", session.id));
    let json = serde_json::to_string_pretty(session)
        .context("Failed to serialize session data")?;
    std::fs::write(&file_path, json)
        .with_context(|| format!("Failed to write session file {}", file_path.display()))?;

    Ok(())
}

pub fn load_session(dir: &Path, id: &str) -> Result<SessionData> {
    let file_path = dir.join(format!("{}.json", id));
    let content = std::fs::read_to_string(&file_path)
        .with_context(|| format!("Failed to read session file {}", file_path.display()))?;
    let session: SessionData = serde_json::from_str(&content)
        .with_context(|| format!("Failed to parse session file {}", file_path.display()))?;
    Ok(session)
}

pub fn list_sessions(dir: &Path) -> Result<Vec<SessionSummary>> {
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut summaries: Vec<SessionSummary> = Vec::new();

    let entries = std::fs::read_dir(dir)
        .with_context(|| format!("Failed to read session directory {}", dir.display()))?;

    for entry in entries {
        let entry = entry.context("Failed to read directory entry")?;
        let path = entry.path();

        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }

        let content = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let data: SessionData = match serde_json::from_str(&content) {
            Ok(d) => d,
            Err(_) => continue,
        };

        summaries.push(SessionSummary {
            message_count: data.messages.len(),
            id: data.id,
            created_at: data.created_at,
            updated_at: data.updated_at,
            model: data.model,
        });
    }

    // Sort by updated_at descending (most recent first)
    summaries.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));

    Ok(summaries)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::types::Message;
    use tempfile::TempDir;

    fn make_session(id: &str, updated_at: &str) -> SessionData {
        SessionData {
            id: id.to_string(),
            created_at: "2026-01-01T00:00:00+00:00".to_string(),
            updated_at: updated_at.to_string(),
            model: "test-model".to_string(),
            provider: "test-provider".to_string(),
            messages: vec![
                Message::user("Hello"),
                Message::assistant("Hi there"),
            ],
        }
    }

    #[test]
    fn test_save_and_load_session() {
        let dir = TempDir::new().unwrap();
        let session = make_session("test-session-id", "2026-01-01T12:00:00+00:00");

        save_session(dir.path(), &session).expect("save should succeed");

        let loaded = load_session(dir.path(), "test-session-id").expect("load should succeed");

        assert_eq!(loaded.id, "test-session-id");
        assert_eq!(loaded.model, "test-model");
        assert_eq!(loaded.provider, "test-provider");
        assert_eq!(loaded.messages.len(), 2);
        assert_eq!(loaded.created_at, "2026-01-01T00:00:00+00:00");
        assert_eq!(loaded.updated_at, "2026-01-01T12:00:00+00:00");
    }

    #[test]
    fn test_list_sessions() {
        let dir = TempDir::new().unwrap();

        let session1 = make_session("session-one", "2026-01-01T10:00:00+00:00");
        let session2 = make_session("session-two", "2026-01-02T10:00:00+00:00");

        save_session(dir.path(), &session1).expect("save session1");
        save_session(dir.path(), &session2).expect("save session2");

        let summaries = list_sessions(dir.path()).expect("list should succeed");

        assert_eq!(summaries.len(), 2);
        // Most recent first: session2 has later updated_at
        assert_eq!(summaries[0].id, "session-two");
        assert_eq!(summaries[1].id, "session-one");
        assert_eq!(summaries[0].message_count, 2);
    }

    #[test]
    fn test_list_sessions_empty_dir() {
        let dir = TempDir::new().unwrap();

        let summaries = list_sessions(dir.path()).expect("list should succeed on empty dir");
        assert!(summaries.is_empty());
    }

    #[test]
    fn test_load_nonexistent_session() {
        let dir = TempDir::new().unwrap();

        let result = load_session(dir.path(), "nonexistent-id");
        assert!(result.is_err());
    }
}
