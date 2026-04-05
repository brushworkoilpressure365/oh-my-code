use anyhow::Result;
use async_trait::async_trait;

use crate::agent::plan::PlanModeState;
use super::{Tool, ToolOutput};

pub struct EnterPlanModeTool {
    state: PlanModeState,
}

impl EnterPlanModeTool {
    pub fn new(state: PlanModeState) -> Self {
        Self { state }
    }
}

#[async_trait]
impl Tool for EnterPlanModeTool {
    fn name(&self) -> &str {
        "enter_plan_mode"
    }

    fn description(&self) -> &str {
        "Switch to PLAN mode. In plan mode only read-only tools are permitted."
    }

    fn input_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {},
            "required": []
        })
    }

    fn is_read_only(&self) -> bool {
        true
    }

    async fn execute(&self, _input: serde_json::Value) -> Result<ToolOutput> {
        if self.state.is_enabled() {
            return Ok(ToolOutput::success("Already in plan mode."));
        }
        self.state.set(true);
        Ok(ToolOutput::success("Entered plan mode. Only read-only tools are now permitted."))
    }
}

pub struct ExitPlanModeTool {
    state: PlanModeState,
}

impl ExitPlanModeTool {
    pub fn new(state: PlanModeState) -> Self {
        Self { state }
    }
}

#[async_trait]
impl Tool for ExitPlanModeTool {
    fn name(&self) -> &str {
        "exit_plan_mode"
    }

    fn description(&self) -> &str {
        "Switch to ACT mode. In act mode all tools are permitted."
    }

    fn input_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {},
            "required": []
        })
    }

    fn is_read_only(&self) -> bool {
        true
    }

    async fn execute(&self, _input: serde_json::Value) -> Result<ToolOutput> {
        if !self.state.is_enabled() {
            return Ok(ToolOutput::success("Already in act mode."));
        }
        self.state.set(false);
        Ok(ToolOutput::success("Exited plan mode. All tools are now permitted."))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_enter_plan_mode() {
        let state = PlanModeState::new();
        let tool = EnterPlanModeTool::new(state.clone());
        let result = tool.execute(serde_json::json!({})).await.unwrap();
        assert!(state.is_enabled());
        assert!(!result.is_error);
        assert!(!result.content.contains("Already"));
    }

    #[tokio::test]
    async fn test_enter_plan_mode_already_enabled() {
        let state = PlanModeState::new();
        state.set(true);
        let tool = EnterPlanModeTool::new(state.clone());
        let result = tool.execute(serde_json::json!({})).await.unwrap();
        assert!(result.content.contains("Already"));
    }

    #[tokio::test]
    async fn test_exit_plan_mode() {
        let state = PlanModeState::new();
        state.set(true);
        let tool = ExitPlanModeTool::new(state.clone());
        let result = tool.execute(serde_json::json!({})).await.unwrap();
        assert!(!state.is_enabled());
        assert!(!result.is_error);
        assert!(!result.content.contains("Already"));
    }

    #[tokio::test]
    async fn test_exit_plan_mode_already_disabled() {
        let state = PlanModeState::new();
        let tool = ExitPlanModeTool::new(state.clone());
        let result = tool.execute(serde_json::json!({})).await.unwrap();
        assert!(result.content.contains("Already"));
    }
}
