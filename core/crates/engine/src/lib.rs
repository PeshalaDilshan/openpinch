mod providers;

use anyhow::{Context, Result, bail};
use base64ct::{Base64, Encoding};
use chrono::{DateTime, Utc};
use openpinch_common::openpinch::engine_runtime_service_server::{
    EngineRuntimeService, EngineRuntimeServiceServer,
};
use openpinch_common::openpinch::{
    AgentProtocolRequest, AgentProtocolResponse, AttestationRequest, AttestationResponse,
    AuditEvent as ProtoAuditEvent, AuditExportRequest, AuditExportResponse, Empty,
    EngineMessageRequest, EngineSkillRequest, EngineToolRequest, ExecuteResponse, HealthResponse,
    MemoryQueryRequest, MemoryQueryResponse, MemoryRecord as ProtoMemoryRecord,
    MemoryUpsertRequest, MemoryUpsertResponse, PolicyReportRequest, PolicyReportResponse,
    QueueTaskRequest, QueueTaskResponse, StatusResponse, SubmitMessageResponse,
};
use openpinch_common::{
    AppConfig, AttestationReport, AuditEvent, EncryptedBlob, MemoryQuery, MemoryRecord,
    MessageEnvelope, OpenPinchPaths, PolicyReport, ProtocolRunRequest, ProtocolRunResult,
    QueuePriority, QueueReceipt, QueueTask, RoleBinding, RuntimeStatus, ScheduleRequest,
    SessionIdentity, SessionKeypair, ToolCall, ToolOutcome, decrypt_bytes,
    derive_key_from_material, encrypt_bytes,
};
use openpinch_sandbox::SandboxManager;
use openpinch_tools::{SkillManager, ToolExecutor};
use parking_lot::Mutex;
use providers::ProviderRegistry;
use rusqlite::{Connection, params};
use sha2::{Digest, Sha256};
use std::cmp::Ordering;
use std::collections::{BTreeMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering as AtomicOrdering};
#[cfg(not(unix))]
use tokio::net::TcpListener;
#[cfg(unix)]
use tokio::net::UnixListener;
use tokio::sync::{Notify, Semaphore};
use tokio::task::JoinHandle;
#[cfg(not(unix))]
use tokio_stream::wrappers::TcpListenerStream;
#[cfg(unix)]
use tokio_stream::wrappers::UnixListenerStream;
use tokio_util::sync::CancellationToken;
use tonic::{Request, Response, Status, transport::Server};
use tracing::warn;
use uuid::Uuid;

#[derive(Clone)]
pub struct EngineRuntime {
    inner: Arc<EngineInner>,
}

struct EngineInner {
    config: AppConfig,
    paths: OpenPinchPaths,
    state: StateStore,
    tools: ToolExecutor,
    _skills: SkillManager,
    orchestrator: Orchestrator,
    queue: QueueManager,
    encryption: EncryptionManager,
    started_at: DateTime<Utc>,
}

pub struct EngineHandle {
    pub endpoint: String,
    join: JoinHandle<Result<()>>,
    shutdown: CancellationToken,
}

impl EngineRuntime {
    pub async fn bootstrap(config: AppConfig, paths: OpenPinchPaths) -> Result<Self> {
        paths.ensure_all()?;
        let sandbox = SandboxManager::from_config(&config.sandbox, &paths)?;
        let skills = SkillManager::new(config.skills.clone(), paths.clone());
        let tools = ToolExecutor::new(
            config.clone(),
            paths.clone(),
            sandbox.clone(),
            skills.clone(),
        )?;
        let state = StateStore::open(&paths.database_file)?;
        let encryption = EncryptionManager::load_or_init(&config, &paths)?;
        let providers = ProviderRegistry::from_config(&config.models);
        let orchestrator =
            Orchestrator::new(config.clone(), state.clone(), providers, encryption.clone());
        let queue = QueueManager::new(
            config.clone(),
            state.clone(),
            tools.clone(),
            encryption.clone(),
        );

        Ok(Self {
            inner: Arc::new(EngineInner {
                config,
                paths,
                state,
                tools,
                _skills: skills,
                orchestrator,
                queue,
                encryption,
                started_at: Utc::now(),
            }),
        })
    }

    pub async fn start_private_rpc(&self, shutdown: CancellationToken) -> Result<EngineHandle> {
        #[cfg(unix)]
        {
            if self.inner.paths.runtime_socket.exists() {
                let _ = std::fs::remove_file(&self.inner.paths.runtime_socket);
            }

            let listener =
                UnixListener::bind(&self.inner.paths.runtime_socket).with_context(|| {
                    format!(
                        "failed to bind unix socket at {}",
                        self.inner.paths.runtime_socket.display()
                    )
                })?;
            let endpoint = format!("unix://{}", self.inner.paths.runtime_socket.display());
            let service = RuntimeRpcService {
                engine: self.clone(),
            };
            let incoming = UnixListenerStream::new(listener);
            let cancel = shutdown.clone();
            let socket_path = self.inner.paths.runtime_socket.clone();
            let join = tokio::spawn(async move {
                let result = Server::builder()
                    .add_service(EngineRuntimeServiceServer::new(service))
                    .serve_with_incoming_shutdown(incoming, cancel.cancelled())
                    .await
                    .context("engine RPC server failed");
                let _ = std::fs::remove_file(socket_path);
                result
            });

            Ok(EngineHandle {
                endpoint,
                join,
                shutdown,
            })
        }

        #[cfg(not(unix))]
        {
            let listener = TcpListener::bind("127.0.0.1:0")
                .await
                .context("failed to bind local engine TCP listener")?;
            let endpoint = format!("tcp://{}", listener.local_addr()?);
            let service = RuntimeRpcService {
                engine: self.clone(),
            };
            let incoming = TcpListenerStream::new(listener);
            let cancel = shutdown.clone();
            let join = tokio::spawn(async move {
                Server::builder()
                    .add_service(EngineRuntimeServiceServer::new(service))
                    .serve_with_incoming_shutdown(incoming, cancel.cancelled())
                    .await
                    .context("engine RPC server failed")
            });

            Ok(EngineHandle {
                endpoint,
                join,
                shutdown,
            })
        }
    }

    pub async fn execute_tool(&self, call: ToolCall) -> ToolOutcome {
        self.inner.tools.execute(call).await
    }

    pub async fn execute_skill(&self, skill_id: &str, arguments_json: &str) -> ToolOutcome {
        self.inner
            .tools
            .execute_skill(skill_id, arguments_json)
            .await
    }

    pub async fn handle_message(&self, message: MessageEnvelope) -> Result<String> {
        self.inner.state.record_message(&message)?;

        let prompt = format!(
            "You are OpenPinch, a local autonomous agent. Connector: {}. Sender: {}. Message: {}",
            message.connector, message.sender, message.body
        );

        match self
            .inner
            .orchestrator
            .generate(&prompt, QueuePriority::Connector)
            .await
        {
            Ok(result) => {
                self.record_audit(
                    "connector.message",
                    "info",
                    &format!("message handled via {}", result.provider),
                    0.05,
                    serde_json::json!({
                        "connector": message.connector,
                        "provider": result.provider,
                        "cache": result.cache_tier,
                    })
                    .to_string(),
                )?;
                Ok(result.response)
            }
            Err(error) => {
                self.record_audit(
                    "connector.message",
                    "warning",
                    "message handled without local model reply",
                    0.45,
                    serde_json::json!({ "error": error.to_string() }).to_string(),
                )?;
                Ok(format!(
                    "OpenPinch received the message but no local model backend produced a reply: {error}"
                ))
            }
        }
    }

    pub fn status(&self, gateway_endpoint: String) -> RuntimeStatus {
        RuntimeStatus {
            status: "running".to_owned(),
            version: env!("CARGO_PKG_VERSION").to_owned(),
            runtime_endpoint: if cfg!(unix) {
                format!("unix://{}", self.inner.paths.runtime_socket.display())
            } else {
                "tcp://127.0.0.1:0".to_owned()
            },
            gateway_endpoint,
            enabled_connectors: self
                .inner
                .config
                .connectors
                .iter()
                .filter(|(_, config)| config.enabled)
                .map(|(name, _)| name.clone())
                .collect(),
            available_model_backends: self.inner.orchestrator.enabled_names(),
            uptime_seconds: (Utc::now() - self.inner.started_at).num_seconds(),
            started_at: self.inner.started_at,
            data_dir: self.inner.paths.data_dir.display().to_string(),
            log_file: self.inner.paths.log_file.display().to_string(),
            vector_memory_backend: self.inner.orchestrator.vector_backend().to_owned(),
            encryption_state: self.inner.encryption.state().to_owned(),
            audit_mode: self.inner.config.security.audit.mode.clone(),
            attestation_state: self.inner.encryption.attestation_state().to_owned(),
        }
    }

    pub async fn health(&self) -> Result<HealthResponse> {
        let report = self.inner.tools.health().await;
        Ok(HealthResponse {
            status: if report.missing_prerequisites.is_empty() {
                "ready".to_owned()
            } else {
                "degraded".to_owned()
            },
            missing_prerequisites: report.missing_prerequisites,
            sandbox_backend: report.backend,
            vector_memory_backend: self.inner.orchestrator.vector_backend().to_owned(),
            encryption_state: self.inner.encryption.state().to_owned(),
            audit_mode: self.inner.config.security.audit.mode.clone(),
        })
    }

    pub fn write_runtime_state(
        &self,
        gateway_endpoint: &str,
        runtime_endpoint: &str,
    ) -> Result<()> {
        let mut status = self.status(gateway_endpoint.to_owned());
        status.runtime_endpoint = runtime_endpoint.to_owned();
        let raw = serde_json::to_vec_pretty(&status).context("failed to encode runtime status")?;
        std::fs::write(&self.inner.paths.runtime_state_file, raw).with_context(|| {
            format!(
                "failed to write {}",
                self.inner.paths.runtime_state_file.display()
            )
        })
    }

    pub fn config(&self) -> &AppConfig {
        &self.inner.config
    }

    pub fn paths(&self) -> &OpenPinchPaths {
        &self.inner.paths
    }

    pub fn state_file(&self) -> &Path {
        &self.inner.paths.runtime_state_file
    }

    pub fn schedules(&self) -> Result<Vec<ScheduleRequest>> {
        self.inner.state.list_jobs()
    }

    pub fn role_bindings(&self) -> Vec<RoleBinding> {
        self.inner
            .config
            .rbac
            .role_bindings
            .iter()
            .map(|(subject, roles)| RoleBinding {
                subject: subject.clone(),
                roles: roles.clone(),
            })
            .collect()
    }

    pub fn query_memory(&self, query: MemoryQuery) -> Result<(Vec<MemoryRecord>, String)> {
        Ok((
            self.inner
                .state
                .query_memory(&query, &self.inner.encryption)?,
            self.inner.orchestrator.vector_backend().to_owned(),
        ))
    }

    pub fn upsert_memory(
        &self,
        namespace: &str,
        key: &str,
        content: &str,
        metadata_json: &str,
    ) -> Result<String> {
        let digest = self.inner.state.upsert_memory(
            namespace,
            key,
            content,
            metadata_json,
            &self.inner.encryption,
        )?;
        self.record_audit(
            "memory.upsert",
            "info",
            "vector memory record stored",
            0.05,
            serde_json::json!({
                "namespace": namespace,
                "key": key,
                "digest": digest,
            })
            .to_string(),
        )?;
        Ok(digest)
    }

    pub fn attest_session(
        &self,
        subject: &str,
        nonce: &str,
        include_hardware: bool,
    ) -> Result<AttestationReport> {
        let report = self.inner.encryption.attest(
            subject,
            nonce,
            include_hardware,
            &self.inner.paths,
            &self.inner.config,
        )?;
        self.record_audit(
            "security.attestation",
            "info",
            "session attested",
            if report.hardware_backed { 0.03 } else { 0.18 },
            serde_json::to_string(&report).context("failed to encode attestation report")?,
        )?;
        Ok(report)
    }

    pub fn export_audit(&self, sink: &str, limit: usize) -> Result<Vec<AuditEvent>> {
        let events = self.inner.state.list_audit(limit)?;
        if sink == "ocsf-file"
            && self.inner.config.siem.enabled
            && !self.inner.config.siem.ocsf_file.is_empty()
        {
            let payload =
                serde_json::to_vec_pretty(&events).context("failed to serialize audit export")?;
            let output = resolve_optional_path(
                &self.inner.config.siem.ocsf_file,
                &self.inner.paths.state_dir,
            );
            if let Some(parent) = output.parent() {
                std::fs::create_dir_all(parent).with_context(|| {
                    format!(
                        "failed to create SIEM export directory {}",
                        parent.display()
                    )
                })?;
            }
            std::fs::write(&output, payload)
                .with_context(|| format!("failed to write {}", output.display()))?;
        }
        Ok(events)
    }

    pub fn run_agent_protocol(&self, request: ProtocolRunRequest) -> Result<ProtocolRunResult> {
        let result = validate_protocol(&request, &self.inner.encryption, &self.inner.config);
        self.inner.state.record_protocol_run(&result)?;
        self.record_audit(
            "agent.protocol",
            if result.accepted { "info" } else { "warning" },
            &format!("protocol {} executed", request.protocol_id),
            if result.accepted { 0.08 } else { 0.74 },
            result.transcript_json.clone(),
        )?;
        Ok(result)
    }

    pub async fn queue_task(&self, task: QueueTask) -> Result<QueueReceipt> {
        self.inner.queue.enqueue(task).await
    }

    pub fn policy_report(&self, subject: &str, capability: Option<&str>) -> PolicyReport {
        if let Some(capability) = capability {
            let mut report = self.inner.tools.policy_report(subject);
            report
                .allowed_capabilities
                .retain(|candidate| candidate == capability);
            report
                .denied_capabilities
                .retain(|candidate| candidate == capability);
            report
        } else {
            self.inner.tools.policy_report(subject)
        }
    }

    fn record_audit(
        &self,
        category: &str,
        severity: &str,
        summary: &str,
        anomaly_score: f64,
        payload_json: String,
    ) -> Result<()> {
        self.inner.state.record_audit(AuditEvent {
            id: format!("audit-{}", Uuid::new_v4()),
            category: category.to_owned(),
            severity: severity.to_owned(),
            summary: summary.to_owned(),
            anomaly_score,
            payload_json,
            created_at: Utc::now(),
        })
    }
}

impl EngineHandle {
    pub async fn shutdown(self) -> Result<()> {
        self.shutdown.cancel();
        self.join.await.context("engine RPC task join failed")?
    }
}

#[derive(Clone)]
struct RuntimeRpcService {
    engine: EngineRuntime,
}

#[tonic::async_trait]
impl EngineRuntimeService for RuntimeRpcService {
    async fn deliver_message(
        &self,
        request: Request<EngineMessageRequest>,
    ) -> Result<Response<SubmitMessageResponse>, Status> {
        let message = request
            .into_inner()
            .message
            .ok_or_else(|| Status::invalid_argument("message is required"))?;
        let envelope = MessageEnvelope {
            connector: message.connector,
            channel_id: message.channel_id,
            sender: message.sender,
            body: message.body,
            metadata_json: message.metadata_json,
        };
        let reply = self
            .engine
            .handle_message(envelope)
            .await
            .map_err(internal_status)?;

        Ok(Response::new(SubmitMessageResponse {
            accepted: true,
            message_id: format!("msg-{}", Utc::now().timestamp_millis()),
            reply,
        }))
    }

    async fn run_tool(
        &self,
        request: Request<EngineToolRequest>,
    ) -> Result<Response<ExecuteResponse>, Status> {
        let call = request.into_inner();
        let outcome = self
            .engine
            .execute_tool(ToolCall {
                target: call.tool,
                arguments_json: call.arguments_json,
                allow_network: call.allow_network,
                priority: parse_priority(&call.priority),
            })
            .await;
        Ok(Response::new(proto_outcome(outcome)))
    }

    async fn run_skill(
        &self,
        request: Request<EngineSkillRequest>,
    ) -> Result<Response<ExecuteResponse>, Status> {
        let call = request.into_inner();
        let outcome = self
            .engine
            .execute_skill(&call.skill_id, &call.arguments_json)
            .await;
        Ok(Response::new(proto_outcome(outcome)))
    }

    async fn health(&self, _request: Request<Empty>) -> Result<Response<HealthResponse>, Status> {
        let response = self.engine.health().await.map_err(internal_status)?;
        Ok(Response::new(response))
    }

    async fn query_memory(
        &self,
        request: Request<MemoryQueryRequest>,
    ) -> Result<Response<MemoryQueryResponse>, Status> {
        let request = request.into_inner();
        let (records, backend) = self
            .engine
            .query_memory(MemoryQuery {
                namespace: empty_to_default(
                    &request.namespace,
                    &self.engine.inner.config.vector_memory.default_namespace,
                ),
                query: request.query,
                limit: request.limit as usize,
                filter_json: request.filter_json,
            })
            .map_err(internal_status)?;

        Ok(Response::new(MemoryQueryResponse {
            records: records.into_iter().map(proto_memory_record).collect(),
            backend,
        }))
    }

    async fn upsert_memory(
        &self,
        request: Request<MemoryUpsertRequest>,
    ) -> Result<Response<MemoryUpsertResponse>, Status> {
        let request = request.into_inner();
        let namespace = empty_to_default(
            &request.namespace,
            &self.engine.inner.config.vector_memory.default_namespace,
        );
        let digest = self
            .engine
            .upsert_memory(
                &namespace,
                &request.key,
                &request.content,
                &request.metadata_json,
            )
            .map_err(internal_status)?;
        Ok(Response::new(MemoryUpsertResponse {
            stored: true,
            backend: self.engine.inner.orchestrator.vector_backend().to_owned(),
            digest,
        }))
    }

    async fn run_agent_protocol(
        &self,
        request: Request<AgentProtocolRequest>,
    ) -> Result<Response<AgentProtocolResponse>, Status> {
        let request = request.into_inner();
        let result = self
            .engine
            .run_agent_protocol(ProtocolRunRequest {
                protocol_id: request.protocol_id,
                initiator: request.initiator,
                messages: request
                    .messages
                    .into_iter()
                    .map(|message| openpinch_common::AgentEnvelope {
                        sender: message.sender,
                        recipient: message.recipient,
                        body: message.body,
                        metadata_json: message.metadata_json,
                        encrypted_body: if message.encrypted_body.is_empty() {
                            None
                        } else {
                            Some(message.encrypted_body)
                        },
                    })
                    .collect(),
                policy_scope: request.policy_scope,
            })
            .map_err(internal_status)?;
        Ok(Response::new(AgentProtocolResponse {
            accepted: result.accepted,
            protocol_id: result.protocol_id,
            transcript_json: result.transcript_json,
            findings: result.findings,
        }))
    }

    async fn attest_session(
        &self,
        request: Request<AttestationRequest>,
    ) -> Result<Response<AttestationResponse>, Status> {
        let request = request.into_inner();
        let report = self
            .engine
            .attest_session(
                &empty_to_default(&request.subject, "openpinch-session"),
                &request.nonce,
                request.include_hardware,
            )
            .map_err(internal_status)?;
        Ok(Response::new(proto_attestation(report)))
    }

    async fn export_audit(
        &self,
        request: Request<AuditExportRequest>,
    ) -> Result<Response<AuditExportResponse>, Status> {
        let request = request.into_inner();
        let limit = if request.limit == 0 {
            50
        } else {
            request.limit as usize
        };
        let events = self
            .engine
            .export_audit(&request.sink, limit)
            .map_err(internal_status)?;
        Ok(Response::new(AuditExportResponse {
            exported: true,
            format: if request.sink.is_empty() {
                "json".to_owned()
            } else {
                request.sink
            },
            events: events.into_iter().map(proto_audit_event).collect(),
        }))
    }

    async fn queue_task(
        &self,
        request: Request<QueueTaskRequest>,
    ) -> Result<Response<QueueTaskResponse>, Status> {
        let request = request.into_inner();
        let receipt = self
            .engine
            .queue_task(QueueTask {
                task_id: if request.task_id.is_empty() {
                    format!("task-{}", Uuid::new_v4())
                } else {
                    request.task_id
                },
                task_type: request.task_type,
                target: request.target,
                arguments_json: request.arguments_json,
                priority: parse_priority(&request.priority),
            })
            .await
            .map_err(internal_status)?;
        Ok(Response::new(QueueTaskResponse {
            accepted: receipt.accepted,
            task_id: receipt.task_id,
            queue: receipt.queue,
            message: receipt.message,
        }))
    }

    async fn get_policy_report(
        &self,
        request: Request<PolicyReportRequest>,
    ) -> Result<Response<PolicyReportResponse>, Status> {
        let request = request.into_inner();
        let report = self.engine.policy_report(
            &empty_to_default(&request.subject, "builtin.command"),
            if request.capability.is_empty() {
                None
            } else {
                Some(request.capability.as_str())
            },
        );
        Ok(Response::new(PolicyReportResponse {
            subject: report.subject,
            allowed_capabilities: report.allowed_capabilities,
            denied_capabilities: report.denied_capabilities,
            source: report.source,
        }))
    }
}

fn proto_outcome(outcome: ToolOutcome) -> ExecuteResponse {
    ExecuteResponse {
        success: outcome.success,
        summary: outcome.summary,
        data_json: outcome.data_json,
        error: outcome.error,
        logs: outcome.logs,
    }
}

fn proto_memory_record(record: MemoryRecord) -> ProtoMemoryRecord {
    ProtoMemoryRecord {
        key: record.key,
        namespace: record.namespace,
        content: record.content,
        metadata_json: record.metadata_json,
        score: record.score,
        created_at: record.created_at.to_rfc3339(),
    }
}

fn proto_audit_event(event: AuditEvent) -> ProtoAuditEvent {
    ProtoAuditEvent {
        id: event.id,
        category: event.category,
        severity: event.severity,
        summary: event.summary,
        anomaly_score: event.anomaly_score,
        payload_json: event.payload_json,
        created_at: event.created_at.to_rfc3339(),
    }
}

fn proto_attestation(report: AttestationReport) -> AttestationResponse {
    AttestationResponse {
        status: report.status,
        subject: report.subject,
        platform: report.platform,
        hardware_backed: report.hardware_backed,
        nonce: report.nonce,
        public_key: report.public_key,
        measurements: report.measurements.into_iter().collect(),
    }
}

fn empty_to_default(value: &str, default: &str) -> String {
    if value.is_empty() {
        default.to_owned()
    } else {
        value.to_owned()
    }
}

fn internal_status(error: anyhow::Error) -> Status {
    Status::internal(error.to_string())
}

#[derive(Clone)]
struct StateStore {
    connection: Arc<Mutex<Connection>>,
}

impl StateStore {
    fn open(path: &Path) -> Result<Self> {
        let connection = Connection::open(path)
            .with_context(|| format!("failed to open database {}", path.display()))?;
        connection.execute_batch(
            r#"
            create table if not exists messages (
                id integer primary key,
                connector text not null,
                channel_id text not null,
                sender text not null,
                body text not null,
                metadata_json text not null,
                created_at text not null
            );
            create table if not exists jobs (
                job_id text primary key,
                cron text not null,
                tool text not null,
                arguments_json text not null,
                created_at text not null
            );
            create table if not exists prompt_cache (
                prompt_hash text primary key,
                provider text not null,
                response text not null,
                created_at text not null
            );
            create table if not exists prefix_cache (
                prefix text primary key,
                provider text not null,
                response text not null,
                created_at text not null
            );
            create table if not exists vector_memory (
                namespace text not null,
                key text not null,
                encrypted_blob_json text not null,
                metadata_json text not null,
                fingerprint text not null,
                created_at text not null,
                primary key (namespace, key)
            );
            create table if not exists audit_events (
                id text primary key,
                category text not null,
                severity text not null,
                summary text not null,
                anomaly_score real not null,
                payload_json text not null,
                created_at text not null
            );
            create table if not exists queue_tasks (
                task_id text primary key,
                task_type text not null,
                target text not null,
                arguments_json text not null,
                priority text not null,
                status text not null,
                created_at text not null,
                updated_at text not null
            );
            create table if not exists protocol_runs (
                protocol_id text not null,
                transcript_json text not null,
                findings_json text not null,
                created_at text not null
            );
            "#,
        )?;

        Ok(Self {
            connection: Arc::new(Mutex::new(connection)),
        })
    }

    fn record_message(&self, message: &MessageEnvelope) -> Result<()> {
        let connection = self.connection.lock();
        connection.execute(
            "insert into messages (connector, channel_id, sender, body, metadata_json, created_at) values (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                message.connector,
                message.channel_id,
                message.sender,
                message.body,
                message.metadata_json,
                Utc::now().to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    fn list_jobs(&self) -> Result<Vec<ScheduleRequest>> {
        let connection = self.connection.lock();
        let mut statement = connection.prepare(
            "select job_id, cron, tool, arguments_json from jobs order by created_at asc",
        )?;
        let rows = statement.query_map([], |row| {
            Ok(ScheduleRequest {
                job_id: row.get(0)?,
                cron: row.get(1)?,
                tool: row.get(2)?,
                arguments_json: row.get(3)?,
            })
        })?;

        let mut jobs = Vec::new();
        for row in rows {
            jobs.push(row?);
        }
        Ok(jobs)
    }

    fn cache_exact_get(&self, prompt_hash: &str) -> Result<Option<(String, String)>> {
        let connection = self.connection.lock();
        let mut statement = connection
            .prepare("select provider, response from prompt_cache where prompt_hash = ?1")?;
        let mut rows = statement.query(params![prompt_hash])?;
        if let Some(row) = rows.next()? {
            Ok(Some((row.get(0)?, row.get(1)?)))
        } else {
            Ok(None)
        }
    }

    fn cache_exact_put(&self, prompt_hash: &str, provider: &str, response: &str) -> Result<()> {
        let connection = self.connection.lock();
        connection.execute(
            "insert into prompt_cache (prompt_hash, provider, response, created_at) values (?1, ?2, ?3, ?4)
             on conflict(prompt_hash) do update set provider = excluded.provider, response = excluded.response, created_at = excluded.created_at",
            params![prompt_hash, provider, response, Utc::now().to_rfc3339()],
        )?;
        Ok(())
    }

    fn cache_prefix_get(&self, prompt: &str) -> Result<Option<(String, String)>> {
        let connection = self.connection.lock();
        let mut statement = connection.prepare(
            "select provider, response from prefix_cache where ?1 like prefix || '%' order by length(prefix) desc limit 1",
        )?;
        let mut rows = statement.query(params![prompt])?;
        if let Some(row) = rows.next()? {
            Ok(Some((row.get(0)?, row.get(1)?)))
        } else {
            Ok(None)
        }
    }

    fn cache_prefix_put(&self, prefix: &str, provider: &str, response: &str) -> Result<()> {
        let connection = self.connection.lock();
        connection.execute(
            "insert into prefix_cache (prefix, provider, response, created_at) values (?1, ?2, ?3, ?4)
             on conflict(prefix) do update set provider = excluded.provider, response = excluded.response, created_at = excluded.created_at",
            params![prefix, provider, response, Utc::now().to_rfc3339()],
        )?;
        Ok(())
    }

    fn upsert_memory(
        &self,
        namespace: &str,
        key: &str,
        content: &str,
        metadata_json: &str,
        encryption: &EncryptionManager,
    ) -> Result<String> {
        let blob = encryption.encrypt_text(content)?;
        let blob_json =
            serde_json::to_string(&blob).context("failed to encode encrypted memory")?;
        let digest = prompt_hash(content);
        let fingerprint = fingerprint(content);
        let connection = self.connection.lock();
        connection.execute(
            "insert into vector_memory (namespace, key, encrypted_blob_json, metadata_json, fingerprint, created_at) values (?1, ?2, ?3, ?4, ?5, ?6)
             on conflict(namespace, key) do update set encrypted_blob_json = excluded.encrypted_blob_json, metadata_json = excluded.metadata_json, fingerprint = excluded.fingerprint, created_at = excluded.created_at",
            params![namespace, key, blob_json, metadata_json, fingerprint, Utc::now().to_rfc3339()],
        )?;
        Ok(digest)
    }

    fn query_memory(
        &self,
        query: &MemoryQuery,
        encryption: &EncryptionManager,
    ) -> Result<Vec<MemoryRecord>> {
        let connection = self.connection.lock();
        let mut statement = connection.prepare(
            "select key, namespace, encrypted_blob_json, metadata_json, fingerprint, created_at
             from vector_memory where namespace = ?1 order by created_at desc limit 256",
        )?;
        let rows = statement.query_map(params![query.namespace], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
            ))
        })?;

        let query_fingerprint = fingerprint(&query.query);
        let mut records = Vec::new();
        for row in rows {
            let (key, namespace, blob_json, metadata_json, entry_fingerprint, created_at) = row?;
            let score = similarity(&query_fingerprint, &entry_fingerprint);
            let blob = serde_json::from_str::<EncryptedBlob>(&blob_json)
                .context("failed to decode encrypted memory blob")?;
            let content = encryption.decrypt_text(&blob)?;
            records.push(MemoryRecord {
                key,
                namespace,
                content,
                metadata_json,
                score,
                created_at: DateTime::parse_from_rfc3339(&created_at)
                    .context("invalid stored memory timestamp")?
                    .with_timezone(&Utc),
            });
        }

        records.sort_by(|left, right| {
            right
                .score
                .partial_cmp(&left.score)
                .unwrap_or(Ordering::Equal)
        });
        records.truncate(query.limit.max(1));
        Ok(records)
    }

    fn record_audit(&self, event: AuditEvent) -> Result<()> {
        let connection = self.connection.lock();
        connection.execute(
            "insert into audit_events (id, category, severity, summary, anomaly_score, payload_json, created_at) values (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                event.id,
                event.category,
                event.severity,
                event.summary,
                event.anomaly_score,
                event.payload_json,
                event.created_at.to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    fn list_audit(&self, limit: usize) -> Result<Vec<AuditEvent>> {
        let connection = self.connection.lock();
        let mut statement = connection.prepare(
            "select id, category, severity, summary, anomaly_score, payload_json, created_at
             from audit_events order by created_at desc limit ?1",
        )?;
        let rows = statement.query_map(params![limit as i64], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, f64>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, String>(6)?,
            ))
        })?;

        let mut events = Vec::new();
        for row in rows {
            let (id, category, severity, summary, anomaly_score, payload_json, created_at) = row?;
            events.push(AuditEvent {
                id,
                category,
                severity,
                summary,
                anomaly_score,
                payload_json,
                created_at: DateTime::parse_from_rfc3339(&created_at)
                    .context("invalid stored audit timestamp")?
                    .with_timezone(&Utc),
            });
        }
        Ok(events)
    }

    fn store_queue_task(&self, task: &QueueTask, status: &str) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        let connection = self.connection.lock();
        connection.execute(
            "insert into queue_tasks (task_id, task_type, target, arguments_json, priority, status, created_at, updated_at) values (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7)
             on conflict(task_id) do update set task_type = excluded.task_type, target = excluded.target, arguments_json = excluded.arguments_json, priority = excluded.priority, status = excluded.status, updated_at = excluded.updated_at",
            params![task.task_id, task.task_type, task.target, task.arguments_json, task.priority.as_str(), status, now],
        )?;
        Ok(())
    }

    fn update_queue_status(&self, task_id: &str, status: &str) -> Result<()> {
        let connection = self.connection.lock();
        connection.execute(
            "update queue_tasks set status = ?2, updated_at = ?3 where task_id = ?1",
            params![task_id, status, Utc::now().to_rfc3339()],
        )?;
        Ok(())
    }

    fn record_protocol_run(&self, result: &ProtocolRunResult) -> Result<()> {
        let connection = self.connection.lock();
        connection.execute(
            "insert into protocol_runs (protocol_id, transcript_json, findings_json, created_at) values (?1, ?2, ?3, ?4)",
            params![
                result.protocol_id,
                result.transcript_json,
                serde_json::to_string(&result.findings).context("failed to encode findings")?,
                Utc::now().to_rfc3339(),
            ],
        )?;
        Ok(())
    }
}

struct OrchestrationResult {
    provider: String,
    response: String,
    cache_tier: String,
}

struct Orchestrator {
    config: AppConfig,
    state: StateStore,
    providers: ProviderRegistry,
    encryption: EncryptionManager,
    inflight: Arc<Semaphore>,
    vector_backend: String,
}

impl Orchestrator {
    fn new(
        config: AppConfig,
        state: StateStore,
        providers: ProviderRegistry,
        encryption: EncryptionManager,
    ) -> Self {
        let vector_backend = "sqlite-fallback".to_owned();
        Self {
            inflight: Arc::new(Semaphore::new(config.orchestration.max_inflight.max(1))),
            config,
            state,
            providers,
            encryption,
            vector_backend,
        }
    }

    fn enabled_names(&self) -> Vec<String> {
        self.providers.enabled_names()
    }

    fn vector_backend(&self) -> &str {
        &self.vector_backend
    }

    async fn generate(&self, prompt: &str, priority: QueuePriority) -> Result<OrchestrationResult> {
        let _permit = self
            .inflight
            .acquire()
            .await
            .context("orchestrator semaphore closed")?;

        let prompt_hash = prompt_hash(prompt);

        if self.config.orchestration.exact_cache_enabled {
            if let Some((provider, response)) = self.state.cache_exact_get(&prompt_hash)? {
                return Ok(OrchestrationResult {
                    provider,
                    response,
                    cache_tier: "exact".to_owned(),
                });
            }
        }

        if self.config.orchestration.prefix_cache_enabled {
            if let Some((provider, response)) = self.state.cache_prefix_get(prompt)? {
                return Ok(OrchestrationResult {
                    provider,
                    response,
                    cache_tier: "prefix".to_owned(),
                });
            }
        }

        if self.config.orchestration.semantic_cache_enabled {
            let cached = self.state.query_memory(
                &MemoryQuery {
                    namespace: "semantic-cache".to_owned(),
                    query: prompt.to_owned(),
                    limit: 1,
                    filter_json: "{}".to_owned(),
                },
                &self.encryption,
            )?;
            if let Some(entry) = cached.into_iter().next() {
                if entry.score >= 0.92 {
                    return Ok(OrchestrationResult {
                        provider: "semantic-cache".to_owned(),
                        response: entry.content,
                        cache_tier: "semantic".to_owned(),
                    });
                }
            }
        }

        let route = if priority == QueuePriority::Background {
            reverse_provider_order(&self.config.orchestration.provider_order)
        } else {
            self.config.orchestration.provider_order.clone()
        };
        let (provider, response, cache_tier) = self.generate_live(&route, prompt).await?;

        if self.config.orchestration.exact_cache_enabled {
            self.state
                .cache_exact_put(&prompt_hash, &provider, &response)?;
        }
        if self.config.orchestration.prefix_cache_enabled {
            let prefix = &prompt[..prompt.len().min(128)];
            self.state.cache_prefix_put(prefix, &provider, &response)?;
        }
        if self.config.orchestration.semantic_cache_enabled {
            let key = format!("semantic-{}", prompt_hash);
            let payload = serde_json::json!({
                "provider": provider,
                "response": response,
            })
            .to_string();
            let _ =
                self.state
                    .upsert_memory("semantic-cache", &key, &payload, "{}", &self.encryption);
        }

        Ok(OrchestrationResult {
            provider,
            response,
            cache_tier,
        })
    }

    async fn generate_live(
        &self,
        route: &[String],
        prompt: &str,
    ) -> Result<(String, String, String)> {
        if self.config.orchestration.speculative_enabled {
            let maybe_draft = route.iter().find(|name| {
                self.providers
                    .config_for(name)
                    .map(|config| config.speculative_enabled && !config.draft_model.is_empty())
                    .unwrap_or(false)
            });

            if let Some(draft_name) = maybe_draft {
                let draft_future = self.providers.generate_with_provider(draft_name, prompt);
                let target_future = self.providers.generate_with_routing(route, prompt);
                let (draft, target) = tokio::join!(draft_future, target_future);
                return match (draft, target) {
                    (_, Ok((provider, response))) => {
                        Ok((provider, response, "live+speculative".to_owned()))
                    }
                    (Ok(response), Err(_)) => Ok((
                        draft_name.clone(),
                        response,
                        "speculative-fallback".to_owned(),
                    )),
                    (_, Err(error)) => Err(error),
                };
            }
        }

        let (provider, response) = self.providers.generate_with_routing(route, prompt).await?;
        Ok((provider, response, "live".to_owned()))
    }
}

#[derive(Clone)]
struct QueueManager {
    inner: Arc<QueueInner>,
}

struct QueueInner {
    state: StateStore,
    tools: ToolExecutor,
    pending: Mutex<QueueBuckets>,
    notify: Notify,
    inflight: Arc<Semaphore>,
    queue_depth: AtomicUsize,
    max_pending: usize,
}

#[derive(Default)]
struct QueueBuckets {
    interactive: VecDeque<QueueTask>,
    connector: VecDeque<QueueTask>,
    autonomy: VecDeque<QueueTask>,
    background: VecDeque<QueueTask>,
}

impl QueueManager {
    fn new(
        config: AppConfig,
        state: StateStore,
        tools: ToolExecutor,
        _encryption: EncryptionManager,
    ) -> Self {
        let max_pending = config.orchestration.max_inflight.max(1) * 8;
        let inner = Arc::new(QueueInner {
            state,
            tools,
            pending: Mutex::new(QueueBuckets::default()),
            notify: Notify::new(),
            inflight: Arc::new(Semaphore::new(config.orchestration.max_inflight.max(1))),
            queue_depth: AtomicUsize::new(0),
            max_pending,
        });
        let worker = inner.clone();
        tokio::spawn(async move {
            run_queue_worker(worker).await;
        });
        Self { inner }
    }

    async fn enqueue(&self, task: QueueTask) -> Result<QueueReceipt> {
        let queued = self.inner.queue_depth.load(AtomicOrdering::Relaxed);
        if queued >= self.inner.max_pending {
            bail!("queue is full");
        }

        self.inner.state.store_queue_task(&task, "queued")?;
        {
            let mut pending = self.inner.pending.lock();
            match task.priority {
                QueuePriority::Interactive => pending.interactive.push_back(task.clone()),
                QueuePriority::Connector => pending.connector.push_back(task.clone()),
                QueuePriority::Autonomy => pending.autonomy.push_back(task.clone()),
                QueuePriority::Background => pending.background.push_back(task.clone()),
            }
        }
        self.inner.queue_depth.fetch_add(1, AtomicOrdering::Relaxed);
        self.inner.notify.notify_one();

        Ok(QueueReceipt {
            accepted: true,
            task_id: task.task_id,
            queue: task.priority.as_str().to_owned(),
            message: "task queued for asynchronous execution".to_owned(),
        })
    }
}

async fn run_queue_worker(inner: Arc<QueueInner>) {
    loop {
        inner.notify.notified().await;
        while let Some(task) = next_queued_task(&inner) {
            let permit = match inner.inflight.clone().acquire_owned().await {
                Ok(permit) => permit,
                Err(_) => return,
            };
            let worker = inner.clone();
            tokio::spawn(async move {
                let _permit = permit;
                let _ = worker.state.update_queue_status(&task.task_id, "running");
                let outcome = worker
                    .tools
                    .execute(ToolCall {
                        target: task.target.clone(),
                        arguments_json: task.arguments_json.clone(),
                        allow_network: false,
                        priority: task.priority.clone(),
                    })
                    .await;
                let status = if outcome.success {
                    "completed"
                } else {
                    "failed"
                };
                let _ = worker.state.update_queue_status(&task.task_id, status);
                let _ = worker.state.record_audit(AuditEvent {
                    id: format!("audit-{}", Uuid::new_v4()),
                    category: "queue.execution".to_owned(),
                    severity: if outcome.success {
                        "info".to_owned()
                    } else {
                        "warning".to_owned()
                    },
                    summary: format!("queued task {} {}", task.task_id, status),
                    anomaly_score: if outcome.success { 0.06 } else { 0.55 },
                    payload_json: serde_json::json!({
                        "task_id": task.task_id,
                        "task_type": task.task_type,
                        "target": task.target,
                        "summary": outcome.summary,
                    })
                    .to_string(),
                    created_at: Utc::now(),
                });
                worker.queue_depth.fetch_sub(1, AtomicOrdering::Relaxed);
            });
        }
    }
}

fn next_queued_task(inner: &QueueInner) -> Option<QueueTask> {
    let mut pending = inner.pending.lock();
    pending
        .interactive
        .pop_front()
        .or_else(|| pending.connector.pop_front())
        .or_else(|| pending.autonomy.pop_front())
        .or_else(|| pending.background.pop_front())
}

#[derive(Clone)]
struct EncryptionManager {
    enabled: bool,
    key: [u8; 32],
    key_path: PathBuf,
    session: Arc<SessionKeypair>,
    hardware_available: bool,
}

impl EncryptionManager {
    fn load_or_init(config: &AppConfig, paths: &OpenPinchPaths) -> Result<Self> {
        let key_path = if config.security.encryption.key_file.is_empty() {
            paths.data_dir.join("keys").join("runtime.key")
        } else {
            resolve_optional_path(&config.security.encryption.key_file, &paths.data_dir)
        };
        if let Some(parent) = key_path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }

        let key = if key_path.exists() {
            let raw = std::fs::read_to_string(&key_path)
                .with_context(|| format!("failed to read {}", key_path.display()))?;
            let decoded =
                Base64::decode_vec(raw.trim()).context("invalid stored encryption key")?;
            derive_key_from_material(&decoded)
        } else {
            let key = openpinch_common::generate_data_key();
            std::fs::write(&key_path, Base64::encode_string(&key))
                .with_context(|| format!("failed to write {}", key_path.display()))?;
            key
        };

        Ok(Self {
            enabled: config.security.encryption.enabled,
            session: Arc::new(SessionKeypair::generate()),
            hardware_available: detect_hardware_attestation(config),
            key,
            key_path,
        })
    }

    fn state(&self) -> &'static str {
        if self.enabled {
            "encrypted"
        } else {
            "disabled"
        }
    }

    fn attestation_state(&self) -> &'static str {
        if self.hardware_available {
            "hardware-available"
        } else {
            "software-attested"
        }
    }

    fn identity(&self) -> SessionIdentity {
        self.session.identity()
    }

    fn encrypt_text(&self, plaintext: &str) -> Result<EncryptedBlob> {
        if self.enabled {
            encrypt_bytes(&self.key, plaintext.as_bytes())
        } else {
            Ok(EncryptedBlob {
                nonce: String::new(),
                ciphertext: Base64::encode_string(plaintext.as_bytes()),
            })
        }
    }

    fn decrypt_text(&self, blob: &EncryptedBlob) -> Result<String> {
        let bytes = if self.enabled {
            decrypt_bytes(&self.key, blob)?
        } else {
            Base64::decode_vec(&blob.ciphertext).context("invalid plaintext blob")?
        };
        String::from_utf8(bytes).context("memory payload was not valid UTF-8")
    }

    fn attest(
        &self,
        subject: &str,
        nonce: &str,
        include_hardware: bool,
        paths: &OpenPinchPaths,
        config: &AppConfig,
    ) -> Result<AttestationReport> {
        let mut measurements = BTreeMap::new();
        measurements.insert(
            "config_sha256".to_owned(),
            hash_if_exists(&paths.config_file).unwrap_or_else(|| "missing".to_owned()),
        );
        measurements.insert(
            "policy_sha256".to_owned(),
            hash_if_exists(&resolve_optional_path(
                &config.sandbox.capabilities.matrix_path,
                &paths.data_dir,
            ))
            .unwrap_or_else(|| "missing".to_owned()),
        );
        measurements.insert(
            "key_sha256".to_owned(),
            hash_if_exists(&self.key_path).unwrap_or_else(|| "missing".to_owned()),
        );

        Ok(AttestationReport {
            subject: subject.to_owned(),
            status: if include_hardware
                && config.security.attestation.require_hardware
                && !self.hardware_available
            {
                "hardware-unavailable".to_owned()
            } else if include_hardware && self.hardware_available {
                "hardware-backed".to_owned()
            } else {
                "software-attested".to_owned()
            },
            platform: std::env::consts::OS.to_owned(),
            hardware_backed: include_hardware && self.hardware_available,
            nonce: nonce.to_owned(),
            public_key: self.identity().public_key,
            measurements,
        })
    }
}

fn validate_protocol(
    request: &ProtocolRunRequest,
    encryption: &EncryptionManager,
    config: &AppConfig,
) -> ProtocolRunResult {
    let mut findings = Vec::new();
    let mut transcript = Vec::new();

    if request.messages.is_empty() {
        findings.push("protocol run must include at least one message".to_owned());
    }

    for (index, message) in request.messages.iter().enumerate() {
        if index == 0 && message.sender != request.initiator {
            findings.push("first message sender must equal initiator".to_owned());
        }
        if message.sender == message.recipient {
            findings.push(format!(
                "message {} uses identical sender and recipient {}",
                index, message.sender
            ));
        }
        if config.security.encryption.encrypt_agent_channels
            && message.encrypted_body.is_none()
            && !message.body.is_empty()
        {
            findings.push(format!("message {} is missing encrypted_body", index));
        }
        let encrypted_body = message
            .encrypted_body
            .clone()
            .or_else(|| {
                encryption
                    .encrypt_text(&message.body)
                    .ok()
                    .and_then(|blob| serde_json::to_string(&blob).ok())
            })
            .unwrap_or_default();

        transcript.push(serde_json::json!({
            "sender": message.sender,
            "recipient": message.recipient,
            "metadata_json": message.metadata_json,
            "encrypted_body": encrypted_body,
        }));
    }

    if request.protocol_id == "handoff.v1" && request.messages.len() < 2 {
        findings.push("handoff.v1 expects at least two messages".to_owned());
    }

    ProtocolRunResult {
        accepted: findings.is_empty(),
        protocol_id: request.protocol_id.clone(),
        transcript_json: serde_json::to_string_pretty(&transcript)
            .unwrap_or_else(|_| "[]".to_owned()),
        findings,
    }
}

fn reverse_provider_order(order: &[String]) -> Vec<String> {
    let mut reversed = order.to_vec();
    reversed.reverse();
    reversed
}

fn parse_priority(raw: &str) -> QueuePriority {
    raw.parse().unwrap_or_default()
}

fn prompt_hash(prompt: &str) -> String {
    format!("{:x}", Sha256::digest(prompt.as_bytes()))
}

fn fingerprint(content: &str) -> String {
    let mut tokens = content
        .split(|character: char| !character.is_alphanumeric())
        .filter(|token| !token.is_empty())
        .map(|token| token.to_ascii_lowercase())
        .collect::<Vec<_>>();
    tokens.sort();
    tokens.dedup();
    tokens.join(" ")
}

fn similarity(left: &str, right: &str) -> f64 {
    let left_tokens = left.split_whitespace().collect::<Vec<_>>();
    let right_tokens = right.split_whitespace().collect::<Vec<_>>();
    if left_tokens.is_empty() || right_tokens.is_empty() {
        return 0.0;
    }
    let intersection = left_tokens
        .iter()
        .filter(|token| right_tokens.contains(token))
        .count();
    let union = left_tokens.len() + right_tokens.len() - intersection;
    intersection as f64 / union as f64
}

fn hash_if_exists(path: &Path) -> Option<String> {
    let bytes = std::fs::read(path).ok()?;
    Some(format!("{:x}", Sha256::digest(bytes)))
}

fn detect_hardware_attestation(config: &AppConfig) -> bool {
    if !config.security.attestation.enabled {
        return false;
    }

    if cfg!(target_os = "linux") {
        Path::new(&config.security.attestation.tpm_device).exists()
    } else {
        cfg!(target_os = "macos") || cfg!(target_os = "windows")
    }
}

fn resolve_optional_path(path: &str, base: &Path) -> PathBuf {
    let configured = PathBuf::from(path);
    if configured.is_absolute() {
        configured
    } else if path.is_empty() {
        base.to_path_buf()
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| base.to_path_buf())
            .join(configured)
    }
}

pub fn load_runtime_status(paths: &OpenPinchPaths) -> Result<RuntimeStatus> {
    let raw = std::fs::read(&paths.runtime_state_file).with_context(|| {
        format!(
            "failed to read runtime state {}",
            paths.runtime_state_file.display()
        )
    })?;
    serde_json::from_slice(&raw).context("failed to decode runtime state")
}

pub async fn fetch_gateway_status(gateway_endpoint: &str) -> Result<StatusResponse> {
    let endpoint = format!("http://{}", gateway_endpoint);
    let mut client =
        openpinch_common::openpinch::gateway_service_client::GatewayServiceClient::connect(
            endpoint,
        )
        .await
        .with_context(|| format!("failed to connect to gateway at {gateway_endpoint}"))?;
    let response = client
        .get_status(Empty {})
        .await
        .context("failed to fetch gateway status")?;
    Ok(response.into_inner())
}

pub async fn wait_for_gateway(gateway_endpoint: &str, timeout: std::time::Duration) -> Result<()> {
    let started = std::time::Instant::now();
    loop {
        match fetch_gateway_status(gateway_endpoint).await {
            Ok(_) => return Ok(()),
            Err(error) if started.elapsed() < timeout => {
                warn!("waiting for gateway: {error}");
                tokio::time::sleep(std::time::Duration::from_millis(250)).await;
            }
            Err(error) => return Err(error),
        }
    }
}

pub fn runtime_endpoint_hint(paths: &OpenPinchPaths) -> Result<String> {
    if cfg!(unix) {
        Ok(format!("unix://{}", paths.runtime_socket.display()))
    } else {
        bail!("runtime endpoint is allocated dynamically on non-unix platforms")
    }
}

#[cfg(test)]
mod tests {
    use super::{EncryptionManager, QueueBuckets, fingerprint, similarity};
    use openpinch_common::{AppConfig, OpenPinchPaths};

    #[test]
    fn fingerprint_similarity_prefers_shared_tokens() {
        let a = fingerprint("alpha beta gamma");
        let b = fingerprint("alpha beta delta");
        let c = fingerprint("zeta eta theta");
        assert!(similarity(&a, &b) > similarity(&a, &c));
    }

    #[test]
    fn queue_bucket_order_is_priority_first() {
        let mut buckets = QueueBuckets::default();
        buckets.background.push_back(openpinch_common::QueueTask {
            task_id: "background".to_owned(),
            task_type: "tool".to_owned(),
            target: "builtin.echo".to_owned(),
            arguments_json: "{}".to_owned(),
            priority: openpinch_common::QueuePriority::Background,
        });
        buckets.interactive.push_back(openpinch_common::QueueTask {
            task_id: "interactive".to_owned(),
            task_type: "tool".to_owned(),
            target: "builtin.echo".to_owned(),
            arguments_json: "{}".to_owned(),
            priority: openpinch_common::QueuePriority::Interactive,
        });
        let first = buckets.interactive.pop_front().expect("interactive item");
        assert_eq!(first.task_id, "interactive");
    }

    #[test]
    fn encryption_manager_reports_state() {
        let paths = OpenPinchPaths::discover().expect("discover paths");
        let config = AppConfig::default();
        let manager = EncryptionManager::load_or_init(&config, &paths).expect("manager");
        assert_eq!(manager.state(), "encrypted");
    }
}
