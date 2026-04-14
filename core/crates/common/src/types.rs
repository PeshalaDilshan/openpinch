use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

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
    pub vector_memory_backend: String,
    pub encryption_state: String,
    pub audit_mode: String,
    pub attestation_state: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub target: String,
    pub arguments_json: String,
    pub allow_network: bool,
    #[serde(default)]
    pub priority: QueuePriority,
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "kebab-case")]
pub enum QueuePriority {
    #[default]
    Interactive,
    Connector,
    Autonomy,
    Background,
}

impl QueuePriority {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Interactive => "interactive",
            Self::Connector => "connector",
            Self::Autonomy => "autonomy",
            Self::Background => "background",
        }
    }

    pub fn weight(&self) -> u32 {
        match self {
            Self::Interactive => 100,
            Self::Connector => 80,
            Self::Autonomy => 40,
            Self::Background => 20,
        }
    }
}

impl std::str::FromStr for QueuePriority {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "interactive" => Ok(Self::Interactive),
            "connector" => Ok(Self::Connector),
            "autonomy" => Ok(Self::Autonomy),
            "background" => Ok(Self::Background),
            other => Err(anyhow::anyhow!("unsupported queue priority {other}")),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectorDescriptor {
    pub name: String,
    pub enabled: bool,
    pub implemented: bool,
    pub mode: String,
    pub health: String,
    pub allowlist: Vec<String>,
    pub details: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttestationReport {
    pub subject: String,
    pub status: String,
    pub platform: String,
    pub hardware_backed: bool,
    pub nonce: String,
    pub public_key: String,
    pub measurements: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEvent {
    pub id: String,
    pub category: String,
    pub severity: String,
    pub summary: String,
    pub anomaly_score: f64,
    pub payload_json: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryRecord {
    pub key: String,
    pub namespace: String,
    pub content: String,
    pub metadata_json: String,
    pub score: f64,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryQuery {
    pub namespace: String,
    pub query: String,
    pub limit: usize,
    pub filter_json: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrainRemember {
    pub kind: String,
    pub subtype: String,
    pub title: String,
    pub content: String,
    pub importance: f64,
    pub scope_json: String,
    pub links_json: String,
    pub source_ref: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrainRecallQuery {
    pub query: String,
    pub scope_json: String,
    pub limit: usize,
    pub include_archived: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrainSuggestQuery {
    pub scope_json: String,
    pub limit: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrainTaskListQuery {
    pub scope_json: String,
    pub statuses: Vec<String>,
    pub priorities: Vec<String>,
    pub due_before: Option<DateTime<Utc>>,
    pub limit: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrainTaskUpdate {
    pub task_id: String,
    pub status: String,
    pub priority: String,
    pub due_at: Option<DateTime<Utc>>,
    pub summary: String,
    pub links_json: String,
    pub source_ref: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrainForget {
    pub target_kind: String,
    pub target_id: String,
    pub mode: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrainEntityRecord {
    pub id: String,
    pub kind: String,
    pub subtype: String,
    pub title: String,
    pub content: String,
    pub scope_json: String,
    pub links_json: String,
    pub salience: f64,
    pub confidence: f64,
    pub archived: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrainFactRecord {
    pub id: String,
    pub entity_id: String,
    pub content: String,
    pub scope_json: String,
    pub salience: f64,
    pub confidence: f64,
    pub archived: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrainRelationRecord {
    pub id: String,
    pub kind: String,
    pub from_id: String,
    pub to_id: String,
    pub metadata_json: String,
    pub confidence: f64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrainTaskRecord {
    pub id: String,
    pub title: String,
    pub summary: String,
    pub status: String,
    pub priority: String,
    pub due_at: Option<DateTime<Utc>>,
    pub scope_json: String,
    pub links_json: String,
    pub salience: f64,
    pub confidence: f64,
    pub archived: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrainSuggestionRecord {
    pub id: String,
    pub task_id: String,
    pub summary: String,
    pub reason: String,
    pub score: f64,
    pub context_json: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrainRecallResult {
    pub summary: String,
    pub entities: Vec<BrainEntityRecord>,
    pub facts: Vec<BrainFactRecord>,
    pub relations: Vec<BrainRelationRecord>,
    pub tasks: Vec<BrainTaskRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrainSuggestResult {
    pub summary: String,
    pub suggestions: Vec<BrainSuggestionRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrainTaskListResult {
    pub summary: String,
    pub tasks: Vec<BrainTaskRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrainRememberResult {
    pub stored: bool,
    pub entity: Option<BrainEntityRecord>,
    pub task: Option<BrainTaskRecord>,
    pub digest: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrainForgetResult {
    pub forgotten: bool,
    pub mode: String,
    pub target_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentEnvelope {
    pub sender: String,
    pub recipient: String,
    pub body: String,
    pub metadata_json: String,
    pub encrypted_body: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtocolRunRequest {
    pub protocol_id: String,
    pub initiator: String,
    pub messages: Vec<AgentEnvelope>,
    pub policy_scope: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtocolRunResult {
    pub accepted: bool,
    pub protocol_id: String,
    pub transcript_json: String,
    pub findings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyReport {
    pub subject: String,
    pub allowed_capabilities: Vec<String>,
    pub denied_capabilities: Vec<String>,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueTask {
    pub task_id: String,
    pub task_type: String,
    pub target: String,
    pub arguments_json: String,
    pub priority: QueuePriority,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueReceipt {
    pub accepted: bool,
    pub task_id: String,
    pub queue: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoleBinding {
    pub subject: String,
    pub roles: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::QueuePriority;

    #[test]
    fn queue_priority_parses() {
        let value: QueuePriority = "connector".parse().expect("parse connector priority");
        assert_eq!(value, QueuePriority::Connector);
        assert_eq!(value.weight(), 80);
    }
}
