pub mod executor;
pub mod plan;

use anyhow::Result;
use futures::StreamExt;

use crate::model::{
    types::{ContentBlock, Message, ModelConfig, Role},
    Provider,
};
use crate::tool::ToolRegistry;

use executor::ToolExecutor;

/// The main agent that manages conversation history and orchestrates tool use.
pub struct Agent {
    pub messages: Vec<Message>,
    pub model_config: ModelConfig,
    pub plan_mode: plan::PlanModeState,
    pub max_turns: u32,
}

impl Agent {
    pub fn new(model_config: ModelConfig) -> Self {
        Self {
            messages: Vec::new(),
            model_config,
            plan_mode: plan::PlanModeState::new(),
            max_turns: 50,
        }
    }

    pub fn messages(&self) -> &[Message] {
        &self.messages
    }

    pub fn set_plan_mode(&mut self, enabled: bool) {
        self.plan_mode.set(enabled);
    }

    pub fn plan_mode(&self) -> bool {
        self.plan_mode.is_enabled()
    }

    pub fn plan_mode_state(&self) -> plan::PlanModeState {
        self.plan_mode.clone()
    }

    pub fn clear_history(&mut self) {
        self.messages.clear();
    }

    pub fn set_model_config(&mut self, config: ModelConfig) {
        self.model_config = config;
    }

    /// Build the system prompt incorporating environment info, mode, and available tools.
    pub fn build_system_prompt(&self, registry: &ToolRegistry) -> String {
        let cwd = std::env::current_dir()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|_| "unknown".to_string());

        let platform = std::env::consts::OS;

        let date = chrono::Local::now().format("%Y-%m-%d").to_string();

        let mode = if self.plan_mode.is_enabled() { "PLAN" } else { "ACT" };

        let tool_names: Vec<&str> = registry.names();
        let tool_list = tool_names.join(", ");

        format!(
            "You are oh-my-code, a Rust-native terminal coding assistant.\n\
             \n\
             Environment:\n\
             - Working directory: {cwd}\n\
             - Platform: {platform}\n\
             - Date: {date}\n\
             - Mode: {mode}\n\
             \n\
             Available tools: {tool_list}\n\
             \n\
             {mode_instructions}",
            cwd = cwd,
            platform = platform,
            date = date,
            mode = mode,
            tool_list = tool_list,
            mode_instructions = if self.plan_mode.is_enabled() {
                "You are in PLAN mode. You may use read-only tools to gather information, but write operations are not permitted. Produce a detailed plan for the user to review."
            } else {
                "You are in ACT mode. You may use all available tools to complete the user's request."
            }
        )
    }

    /// Run one turn of the agent loop.
    ///
    /// Pushes the user message, calls the provider, handles tool use in a loop,
    /// and returns when the assistant produces a response with no tool calls.
    pub async fn run_turn(
        &mut self,
        user_input: &str,
        provider: &dyn Provider,
        registry: &ToolRegistry,
    ) -> Result<()> {
        // 1. Push user message to history
        self.messages.push(Message::user(user_input));

        let tool_defs = registry.tool_defs();
        let executor = ToolExecutor::new(registry, self.plan_mode.is_enabled());

        for _turn in 0..self.max_turns {
            // 2. Build messages with system prompt prepended
            let system_prompt = self.build_system_prompt(registry);
            let mut messages_to_send = vec![Message::system(&system_prompt)];
            messages_to_send.extend(self.messages.iter().cloned());

            // 3. Call provider and get stream
            let mut stream = provider
                .send_message(&messages_to_send, &tool_defs, &self.model_config)
                .await?;

            // 4. Consume the stream
            let mut accumulated_text = String::new();
            let mut tool_use_blocks: Vec<ContentBlock> = Vec::new();

            // State for current in-progress tool use
            struct PendingTool {
                id: String,
                name: String,
                partial_json: String,
            }
            let mut pending: Option<PendingTool> = None;

            while let Some(event) = stream.next().await {
                use crate::model::types::StreamEvent;
                match event {
                    StreamEvent::Delta { text } => {
                        print!("{}", text);
                        use std::io::Write;
                        let _ = std::io::stdout().flush();
                        accumulated_text.push_str(&text);
                    }
                    StreamEvent::ToolUseStart { id, name } => {
                        // Flush any pending tool first
                        if let Some(p) = pending.take() {
                            let input = serde_json::from_str(&p.partial_json)
                                .unwrap_or(serde_json::Value::Null);
                            tool_use_blocks.push(ContentBlock::ToolUse {
                                id: p.id,
                                name: p.name,
                                input,
                            });
                        }
                        println!("\n[tool: {}]", name);
                        pending = Some(PendingTool {
                            id,
                            name,
                            partial_json: String::new(),
                        });
                    }
                    StreamEvent::ToolUseDelta { partial_json } => {
                        if let Some(ref mut p) = pending {
                            p.partial_json.push_str(&partial_json);
                        }
                    }
                    StreamEvent::InputJsonComplete { id, name, input } => {
                        // Replace/complete the pending tool with the complete input
                        pending = None;
                        tool_use_blocks.push(ContentBlock::ToolUse { id, name, input });
                    }
                    StreamEvent::MessageEnd => {
                        // Flush last pending tool if any
                        if let Some(p) = pending.take() {
                            let input = serde_json::from_str(&p.partial_json)
                                .unwrap_or(serde_json::Value::Null);
                            tool_use_blocks.push(ContentBlock::ToolUse {
                                id: p.id,
                                name: p.name,
                                input,
                            });
                        }
                        break;
                    }
                }
            }

            // Print newline after assistant output if there was text
            if !accumulated_text.is_empty() {
                println!();
            }

            // 5. Build assistant message and push to history
            let mut assistant_content: Vec<ContentBlock> = Vec::new();
            if !accumulated_text.is_empty() {
                assistant_content.push(ContentBlock::Text {
                    text: accumulated_text,
                });
            }
            assistant_content.extend(tool_use_blocks.iter().cloned());

            self.messages.push(Message {
                role: Role::Assistant,
                content: assistant_content,
            });

            // 6. If no tool_use blocks, we're done
            if tool_use_blocks.is_empty() {
                break;
            }

            // 7. Execute tools and collect results
            let calls: Vec<(&str, &str, &serde_json::Value)> = tool_use_blocks
                .iter()
                .filter_map(|block| {
                    if let ContentBlock::ToolUse { id, name, input } = block {
                        Some((id.as_str(), name.as_str(), input))
                    } else {
                        None
                    }
                })
                .collect();

            let tool_results = executor.execute_batch(&calls).await;

            // Print tool results
            for (id, content, is_error) in &tool_results {
                if *is_error {
                    println!("[tool result error: {}] {}", id, content);
                } else {
                    println!("[tool result: {}] {}", id, content);
                }
            }

            // 8. Push tool_results message to history
            let result_tuples: Vec<(String, String)> = tool_results
                .into_iter()
                .map(|(id, content, _)| (id, content))
                .collect();

            self.messages.push(Message::tool_results(result_tuples));

            // 9. Loop back to send updated history
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::types::ModelConfig;

    fn test_config() -> ModelConfig {
        ModelConfig {
            model_id: "test-model".to_string(),
            max_tokens: 1024,
            temperature: 0.0,
        }
    }

    #[test]
    fn test_agent_new() {
        let agent = Agent::new(test_config());
        assert!(agent.messages().is_empty());
        assert!(!agent.plan_mode());
        assert_eq!(agent.max_turns, 50);
    }

    #[test]
    fn test_agent_plan_mode_toggle() {
        let mut agent = Agent::new(test_config());
        assert!(!agent.plan_mode());

        agent.set_plan_mode(true);
        assert!(agent.plan_mode());

        agent.set_plan_mode(false);
        assert!(!agent.plan_mode());
    }

    #[test]
    fn test_agent_clear_history() {
        let mut agent = Agent::new(test_config());
        agent.messages.push(Message::user("Hello"));
        assert_eq!(agent.messages().len(), 1);

        agent.clear_history();
        assert!(agent.messages().is_empty());
    }
}
