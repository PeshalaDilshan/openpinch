// core/crates/tools/src/lib.rs
use openpinch_common::{ToolRequest, ToolResponse};
use openpinch_sandbox::FirecrackerSandbox;
use std::sync::Arc;

pub struct ToolExecutor {
    sandbox: Arc<FirecrackerSandbox>,
}

impl ToolExecutor {
    pub fn new(sandbox: Arc<FirecrackerSandbox>) -> Self {
        Self { sandbox }
    }

    pub async fn execute(&self, req: ToolRequest) -> ToolResponse {
        self.sandbox
            .execute(&req.tool_name, req.parameters)
            .await
            .unwrap_or_else(|e| ToolResponse {
                success: false,
                result: String::new(),
                error: Some(e.to_string()),
            })
    }
}