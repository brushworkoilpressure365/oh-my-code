use futures::future::join_all;

use crate::tool::{ToolOutput, ToolRegistry};

/// Executes tool calls, running reads concurrently and writes sequentially.
pub struct ToolExecutor<'a> {
    pub registry: &'a ToolRegistry,
    pub plan_mode: bool,
}

impl<'a> ToolExecutor<'a> {
    pub fn new(registry: &'a ToolRegistry, plan_mode: bool) -> Self {
        Self {
            registry,
            plan_mode,
        }
    }

    /// Execute a batch of tool calls.
    ///
    /// Input: slice of (tool_use_id, tool_name, input_json)
    /// Output: Vec of (tool_use_id, content, is_error) in original order
    pub async fn execute_batch(
        &self,
        tool_calls: &[(&str, &str, &serde_json::Value)],
    ) -> Vec<(String, String, bool)> {
        // Partition into reads and writes, preserving original indices
        let mut reads: Vec<(usize, &str, &str, &serde_json::Value)> = Vec::new();
        let mut writes: Vec<(usize, &str, &str, &serde_json::Value)> = Vec::new();
        let mut unknown: Vec<(usize, &str)> = Vec::new();

        for (idx, (id, name, input)) in tool_calls.iter().enumerate() {
            match self.registry.get(name) {
                None => unknown.push((idx, id)),
                Some(tool) => {
                    if tool.is_read_only() {
                        reads.push((idx, id, name, input));
                    } else {
                        writes.push((idx, id, name, input));
                    }
                }
            }
        }

        let mut results: Vec<Option<(String, String, bool)>> =
            (0..tool_calls.len()).map(|_| None).collect();

        // Handle unknown tools
        for (idx, id) in unknown {
            let name = tool_calls[idx].1;
            results[idx] = Some((
                id.to_string(),
                format!("Unknown tool: {}", name),
                true,
            ));
        }

        // Execute reads concurrently
        let read_futures = reads.iter().map(|(idx, id, name, input)| {
            let tool = self.registry.get(name).unwrap();
            let input_owned = (*input).clone();
            let id_owned = id.to_string();
            let idx = *idx;
            async move {
                let output = match tool.execute(input_owned).await {
                    Ok(out) => out,
                    Err(e) => ToolOutput::error(e.to_string()),
                };
                (idx, id_owned, output.content, output.is_error)
            }
        });

        let read_results = join_all(read_futures).await;
        for (idx, id, content, is_error) in read_results {
            results[idx] = Some((id, content, is_error));
        }

        // Execute writes sequentially
        for (idx, id, name, input) in writes {
            let output = if self.plan_mode {
                ToolOutput::error(format!("Cannot execute write tool '{}' in plan mode", name))
            } else {
                let tool = self.registry.get(name).unwrap();
                match tool.execute((*input).clone()).await {
                    Ok(out) => out,
                    Err(e) => ToolOutput::error(e.to_string()),
                }
            };
            results[idx] = Some((id.to_string(), output.content, output.is_error));
        }

        results.into_iter().map(|r| r.unwrap()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::plan::PlanModeState;
    use crate::tool::create_default_registry;
    use serde_json::json;

    #[tokio::test]
    async fn test_executor_unknown_tool() {
        let registry = create_default_registry(PlanModeState::new());
        let executor = ToolExecutor::new(&registry, false);

        let input = json!({});
        let calls = vec![("id_001", "nonexistent_tool", &input)];
        let results = executor.execute_batch(&calls).await;

        assert_eq!(results.len(), 1);
        let (id, content, is_error) = &results[0];
        assert_eq!(id, "id_001");
        assert!(is_error);
        assert!(content.contains("Unknown tool"));
        assert!(content.contains("nonexistent_tool"));
    }

    #[tokio::test]
    async fn test_executor_think_tool() {
        let registry = create_default_registry(PlanModeState::new());
        let executor = ToolExecutor::new(&registry, false);

        let input = json!({ "thought": "Let me analyze this step by step." });
        let calls = vec![("id_002", "think", &input)];
        let results = executor.execute_batch(&calls).await;

        assert_eq!(results.len(), 1);
        let (id, content, is_error) = &results[0];
        assert_eq!(id, "id_002");
        assert!(!is_error);
        assert!(content.contains("Thought recorded"));
    }

    #[tokio::test]
    async fn test_executor_plan_mode_blocks_writes() {
        use crate::tool::{Tool, ToolOutput, ToolRegistry};
        use anyhow::Result;
        use async_trait::async_trait;

        // Create a mock write tool
        struct MockWriteTool;

        #[async_trait]
        impl Tool for MockWriteTool {
            fn name(&self) -> &str {
                "mock_write"
            }
            fn description(&self) -> &str {
                "A mock write tool"
            }
            fn input_schema(&self) -> serde_json::Value {
                json!({})
            }
            fn is_read_only(&self) -> bool {
                false
            }
            async fn execute(&self, _input: serde_json::Value) -> Result<ToolOutput> {
                Ok(ToolOutput::success("wrote something"))
            }
        }

        let mut registry = ToolRegistry::new();
        registry.register(Box::new(crate::tool::think::ThinkTool));
        registry.register(Box::new(MockWriteTool));

        let executor = ToolExecutor::new(&registry, true); // plan_mode = true

        let think_input = json!({ "thought": "Thinking in plan mode." });
        let write_input = json!({});
        let calls = vec![
            ("id_read", "think", &think_input),
            ("id_write", "mock_write", &write_input),
        ];
        let results = executor.execute_batch(&calls).await;

        assert_eq!(results.len(), 2);

        // think (read-only) should succeed even in plan mode
        let (id, content, is_error) = &results[0];
        assert_eq!(id, "id_read");
        assert!(!is_error, "think tool should succeed in plan mode");
        assert!(content.contains("Thought recorded"));

        // mock_write should be blocked
        let (id, content, is_error) = &results[1];
        assert_eq!(id, "id_write");
        assert!(is_error, "write tool should be blocked in plan mode");
        assert!(content.contains("Cannot execute write tool"));
        assert!(content.contains("mock_write"));
    }
}
