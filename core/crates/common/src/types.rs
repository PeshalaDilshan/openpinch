// core/crates/common/src/types.rs
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct ToolRequest {
    pub tool_name: String,
    pub parameters: serde_json::Value,
    pub skill_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ToolResponse {
    pub success: bool,
    pub result: String,
    pub error: Option<String>,
}