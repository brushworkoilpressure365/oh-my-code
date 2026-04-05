pub mod storage;

use anyhow::Result;
use std::path::PathBuf;

use crate::model::types::Message;
use storage::SessionData;

pub struct Session {
    pub data: SessionData,
    pub storage_dir: PathBuf,
}

impl Session {
    pub fn new(provider: &str, model: &str, storage_dir: PathBuf) -> Self {
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();

        Self {
            data: SessionData {
                id,
                created_at: now.clone(),
                updated_at: now,
                model: model.to_string(),
                provider: provider.to_string(),
                messages: Vec::new(),
            },
            storage_dir,
        }
    }

    pub fn from_data(data: SessionData, storage_dir: PathBuf) -> Self {
        Self { data, storage_dir }
    }

    pub fn save(&mut self) -> Result<()> {
        self.data.updated_at = chrono::Utc::now().to_rfc3339();
        storage::save_session(&self.storage_dir, &self.data)
    }

    pub fn update_messages(&mut self, messages: &[Message]) {
        self.data.messages = messages.to_vec();
    }
}
