use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeStatus {
    pub status: String,
    pub version: String,
    pub runtime_endpoint: String,
    pub gateway_endpoint: String,
    pub enabled_connectors: Vec<String>,
    pub available_model_backends: Vec<String>,
    pub uptime_seconds: i64,
    pub started_at: DateTime<Utc>,
    pub data_dir: String,
    pub log_file: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub target: String,
    pub arguments_json: String,
    pub allow_network: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolOutcome {
    pub success: bool,
    pub summary: String,
    pub data_json: String,
    pub error: String,
    pub logs: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageEnvelope {
    pub connector: String,
    pub channel_id: String,
    pub sender: String,
    pub body: String,
    pub metadata_json: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduleRequest {
    pub job_id: String,
    pub cron: String,
    pub tool: String,
    pub arguments_json: String,
}
