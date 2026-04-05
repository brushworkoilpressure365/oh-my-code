pub mod bash;
pub mod file_edit;
pub mod file_read;
pub mod file_write;
pub mod glob;
pub mod grep;
pub mod plan_mode;
pub mod think;
pub mod web_fetch;
pub mod web_search;

use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::model::types::ToolDef;
use bash::BashTool;
use file_edit::FileEditTool;
use file_read::FileReadTool;
use file_write::FileWriteTool;
use glob::GlobTool;
use grep::GrepTool;
use think::ThinkTool;

/// Output returned by a tool execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolOutput {
    pub content: String,
    pub is_error: bool,
}

impl ToolOutput {
    pub fn success(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            is_error: false,
        }
    }

    pub fn error(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            is_error: true,
        }
    }
}

/// Trait that every tool must implement.
#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn input_schema(&self) -> serde_json::Value;
    fn is_read_only(&self) -> bool;
    async fn execute(&self, input: serde_json::Value) -> Result<ToolOutput>;
}

/// Registry that holds all available tools.
pub struct ToolRegistry {
    tools: HashMap<String, Box<dyn Tool>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }

    pub fn register(&mut self, tool: Box<dyn Tool>) {
        self.tools.insert(tool.name().to_string(), tool);
    }

    pub fn get(&self, name: &str) -> Option<&dyn Tool> {
        self.tools.get(name).map(|t| t.as_ref())
    }

    pub fn tool_defs(&self) -> Vec<ToolDef> {
        self.tools
            .values()
            .map(|t| ToolDef {
                name: t.name().to_string(),
                description: t.description().to_string(),
                input_schema: t.input_schema(),
            })
            .collect()
    }

    pub fn names(&self) -> Vec<&str> {
        self.tools.keys().map(|k| k.as_str()).collect()
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Build the default registry with all bundled tools registered.
pub fn create_default_registry(plan_state: crate::agent::plan::PlanModeState) -> ToolRegistry {
    let mut registry = ToolRegistry::new();
    registry.register(Box::new(ThinkTool));
    registry.register(Box::new(GrepTool));
    registry.register(Box::new(GlobTool));
    registry.register(Box::new(FileReadTool));
    registry.register(Box::new(FileEditTool));
    registry.register(Box::new(FileWriteTool));
    registry.register(Box::new(BashTool));
    registry.register(Box::new(plan_mode::EnterPlanModeTool::new(plan_state.clone())));
    registry.register(Box::new(plan_mode::ExitPlanModeTool::new(plan_state)));
    registry.register(Box::new(web_fetch::WebFetchTool));
    registry.register(Box::new(web_search::WebSearchTool));
    registry
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_output_success() {
        let out = ToolOutput::success("all good");
        assert_eq!(out.content, "all good");
        assert!(!out.is_error);
    }

    #[test]
    fn test_tool_output_error() {
        let out = ToolOutput::error("something went wrong");
        assert_eq!(out.content, "something went wrong");
        assert!(out.is_error);
    }

    #[test]
    fn test_registry_register_and_get() {
        use crate::agent::plan::PlanModeState;
        let registry = create_default_registry(PlanModeState::new());
        assert!(registry.get("think").is_some());
        assert!(registry.get("nonexistent").is_none());
    }

    #[test]
    fn test_registry_tool_defs() {
        use crate::agent::plan::PlanModeState;
        let registry = create_default_registry(PlanModeState::new());
        let defs = registry.tool_defs();
        assert!(defs.iter().any(|d| d.name == "think"));
    }

    #[test]
    fn test_all_tools_registered() {
        use crate::agent::plan::PlanModeState;
        let registry = create_default_registry(PlanModeState::new());
        let names = registry.names();
        assert_eq!(names.len(), 11);

        let expected = [
            "think", "grep", "glob", "file_read", "file_edit", "file_write",
            "bash", "enter_plan_mode", "exit_plan_mode", "web_fetch", "web_search",
        ];
        for name in &expected {
            assert!(
                registry.get(name).is_some(),
                "Tool '{}' not registered",
                name
            );
        }
    }
}
