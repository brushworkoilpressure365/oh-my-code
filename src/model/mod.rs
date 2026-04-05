pub mod types;
pub mod claude;
pub mod openai;

use anyhow::Result;
use async_trait::async_trait;
use futures::stream::BoxStream;
use types::{Message, ModelConfig, ModelInfo, StreamEvent, ToolDef};

#[async_trait]
pub trait Provider: Send + Sync {
    async fn send_message(
        &self,
        messages: &[Message],
        tools: &[ToolDef],
        config: &ModelConfig,
    ) -> Result<BoxStream<'static, StreamEvent>>;

    fn name(&self) -> &str;
    fn supported_models(&self) -> Vec<ModelInfo>;
}

pub fn create_provider(
    provider_name: &str,
    api_key: String,
    base_url: String,
) -> Result<Box<dyn Provider>> {
    match provider_name {
        "claude" => Ok(Box::new(claude::ClaudeProvider::new(api_key, base_url))),
        "openai" | "zhipu" | "minimax" => Ok(Box::new(openai::OpenAIProvider::new(api_key, base_url, provider_name.to_string()))),
        other => anyhow::bail!("unsupported provider: {}", other),
    }
}
