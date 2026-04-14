mod providers;

use anyhow::{Context, Result, bail};
use base64ct::{Base64, Encoding};
use chrono::{DateTime, Duration, Utc};
use openpinch_common::openpinch::engine_runtime_service_server::{
    EngineRuntimeService, EngineRuntimeServiceServer,
};
use openpinch_common::openpinch::{
    AgentProtocolRequest, AgentProtocolResponse, AttestationRequest, AttestationResponse,
    AuditEvent as ProtoAuditEvent, AuditExportRequest, AuditExportResponse, BrainEntity, BrainFact,
    BrainForgetRequest, BrainForgetResponse, BrainRecallRequest, BrainRecallResponse,
    BrainRelation, BrainRememberRequest, BrainRememberResponse, BrainSuggestRequest,
    BrainSuggestResponse, BrainSuggestion, BrainTask, BrainTaskListRequest, BrainTaskListResponse,
    BrainTaskUpdateRequest, BrainTaskUpdateResponse, Empty, EngineMessageRequest,
    EngineSkillRequest, EngineToolRequest, ExecuteResponse, HealthResponse, MemoryQueryRequest,
    MemoryQueryResponse, MemoryRecord as ProtoMemoryRecord, MemoryUpsertRequest,
    MemoryUpsertResponse, PolicyReportRequest, PolicyReportResponse, QueueTaskRequest,
    QueueTaskResponse, StatusResponse, SubmitMessageResponse,
};
use openpinch_common::{
    AppConfig, AttestationReport, AuditEvent, BrainConfig, BrainEntityRecord, BrainFactRecord,
    BrainForget, BrainForgetResult, BrainRecallQuery, BrainRecallResult, BrainRelationRecord,
    BrainRemember, BrainRememberResult, BrainSuggestQuery, BrainSuggestResult,
    BrainSuggestionRecord, BrainTaskListQuery, BrainTaskListResult, BrainTaskRecord,
    BrainTaskUpdate, EncryptedBlob, MemoryQuery, MemoryRecord, MessageEnvelope, OpenPinchPaths,
    PolicyReport, ProtocolRunRequest, ProtocolRunResult, QueuePriority, QueueReceipt, QueueTask,
    RoleBinding, RuntimeStatus, ScheduleRequest, SessionIdentity, SessionKeypair, ToolCall,
    ToolOutcome, decrypt_bytes, derive_key_from_material, encrypt_bytes,
};
use openpinch_sandbox::SandboxManager;
use openpinch_tools::{SkillManager, ToolExecutor};
use parking_lot::Mutex;
use providers::ProviderRegistry;
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
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
    brain: BrainManager,
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
        let brain = BrainManager::new(&config, &paths, state.clone(), encryption.clone())?;
        let queue = QueueManager::new(config.clone(), state.clone(), tools.clone(), brain.clone());

        Ok(Self {
            inner: Arc::new(EngineInner {
                config,
                paths,
                state,
                tools,
                _skills: skills,
                orchestrator,
                brain,
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
        let outcome = self.inner.tools.execute(call.clone()).await;
        if outcome.success {
            let _ = self.inner.brain.ingest_tool_result(&call, &outcome);
        }
        outcome
    }

    pub async fn execute_skill(&self, skill_id: &str, arguments_json: &str) -> ToolOutcome {
        self.inner
            .tools
            .execute_skill(skill_id, arguments_json)
            .await
    }

    pub async fn handle_message(&self, message: MessageEnvelope) -> Result<String> {
        self.inner.state.record_message(&message)?;
        let _ = self.inner.brain.ingest_message(&message);
        let scope_json = self.inner.brain.scope_for_message(&message);
        let context_pack = self
            .inner
            .brain
            .build_context_pack(&message.body, &scope_json)
            .unwrap_or_default();

        let prompt = if context_pack.is_empty() {
            format!(
                "You are OpenPinch, a local autonomous agent. Connector: {}. Sender: {}. Message: {}",
                message.connector, message.sender, message.body
            )
        } else {
            format!(
                "You are OpenPinch, a local autonomous agent.\nConnector: {}\nSender: {}\nRelevant brain context:\n{}\nUser message: {}",
                message.connector, message.sender, context_pack, message.body
            )
        };

        match self
            .inner
            .orchestrator
            .generate(&prompt, QueuePriority::Connector)
            .await
        {
            Ok(result) => {
                let mut reply = result.response.clone();
                let _ = self.inner.brain.ingest_assistant_reply(&message, &reply);
                if self.inner.config.brain.inline_suggestions_in_replies {
                    let suggestions = self
                        .inner
                        .brain
                        .inline_suggestions(
                            &scope_json,
                            self.inner.config.brain.max_inline_suggestions,
                        )
                        .unwrap_or_default();
                    if !suggestions.is_empty() {
                        let suffix = suggestions
                            .iter()
                            .take(self.inner.config.brain.max_inline_suggestions)
                            .map(|suggestion| {
                                format!("- {} ({})", suggestion.summary, suggestion.reason)
                            })
                            .collect::<Vec<_>>()
                            .join("\n");
                        reply = format!("{reply}\n\nNext actions:\n{suffix}");
                    }
                }
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
                Ok(reply)
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

    pub fn remember_brain(&self, request: BrainRemember) -> Result<BrainRememberResult> {
        let result = self.inner.brain.remember(request)?;
        self.record_audit(
            "brain.remember",
            "info",
            "brain record stored",
            0.05,
            json!({ "digest": result.digest }).to_string(),
        )?;
        Ok(result)
    }

    pub fn recall_brain(&self, query: BrainRecallQuery) -> Result<BrainRecallResult> {
        self.inner.brain.recall(query)
    }

    pub fn suggest_brain(&self, query: BrainSuggestQuery) -> Result<BrainSuggestResult> {
        self.inner.brain.suggest(query)
    }

    pub fn list_brain_tasks(&self, query: BrainTaskListQuery) -> Result<BrainTaskListResult> {
        self.inner.brain.list_task_records(query)
    }

    pub fn update_brain_task(&self, request: BrainTaskUpdate) -> Result<BrainTaskRecord> {
        let task = self.inner.brain.update_task_record(request)?;
        self.record_audit(
            "brain.task",
            "info",
            "brain task updated",
            0.05,
            json!({ "task_id": task.id, "status": task.status }).to_string(),
        )?;
        Ok(task)
    }

    pub fn forget_brain(&self, request: BrainForget) -> Result<BrainForgetResult> {
        let result = self.inner.brain.forget(request)?;
        self.record_audit(
            "brain.forget",
            "info",
            "brain record forgotten",
            0.08,
            json!({ "target_id": result.target_id, "mode": result.mode }).to_string(),
        )?;
        Ok(result)
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

    async fn remember_brain(
        &self,
        request: Request<BrainRememberRequest>,
    ) -> Result<Response<BrainRememberResponse>, Status> {
        let request = request.into_inner();
        let result = self
            .engine
            .remember_brain(BrainRemember {
                kind: request.kind,
                subtype: request.subtype,
                title: request.title,
                content: request.content,
                importance: request.importance,
                scope_json: request.scope_json,
                links_json: request.links_json,
                source_ref: request.source_ref,
            })
            .map_err(internal_status)?;
        Ok(Response::new(BrainRememberResponse {
            stored: result.stored,
            entity: result.entity.map(proto_brain_entity),
            task: result.task.map(proto_brain_task),
            digest: result.digest,
        }))
    }

    async fn recall_brain(
        &self,
        request: Request<BrainRecallRequest>,
    ) -> Result<Response<BrainRecallResponse>, Status> {
        let request = request.into_inner();
        let result = self
            .engine
            .recall_brain(BrainRecallQuery {
                query: request.query,
                scope_json: request.scope_json,
                limit: request.limit as usize,
                include_archived: request.include_archived,
            })
            .map_err(internal_status)?;
        Ok(Response::new(BrainRecallResponse {
            summary: result.summary,
            entities: result
                .entities
                .into_iter()
                .map(proto_brain_entity)
                .collect(),
            facts: result.facts.into_iter().map(proto_brain_fact).collect(),
            relations: result
                .relations
                .into_iter()
                .map(proto_brain_relation)
                .collect(),
            tasks: result.tasks.into_iter().map(proto_brain_task).collect(),
        }))
    }

    async fn suggest_brain(
        &self,
        request: Request<BrainSuggestRequest>,
    ) -> Result<Response<BrainSuggestResponse>, Status> {
        let request = request.into_inner();
        let result = self
            .engine
            .suggest_brain(BrainSuggestQuery {
                scope_json: request.scope_json,
                limit: request.limit as usize,
            })
            .map_err(internal_status)?;
        Ok(Response::new(BrainSuggestResponse {
            summary: result.summary,
            suggestions: result
                .suggestions
                .into_iter()
                .map(proto_brain_suggestion)
                .collect(),
        }))
    }

    async fn list_brain_tasks(
        &self,
        request: Request<BrainTaskListRequest>,
    ) -> Result<Response<BrainTaskListResponse>, Status> {
        let request = request.into_inner();
        let result = self
            .engine
            .list_brain_tasks(BrainTaskListQuery {
                scope_json: request.scope_json,
                statuses: request.statuses,
                priorities: request.priorities,
                due_before: if request.due_before.is_empty() {
                    None
                } else {
                    Some(parse_timestamp(&request.due_before).map_err(internal_status)?)
                },
                limit: request.limit as usize,
            })
            .map_err(internal_status)?;
        Ok(Response::new(BrainTaskListResponse {
            summary: result.summary,
            tasks: result.tasks.into_iter().map(proto_brain_task).collect(),
        }))
    }

    async fn update_brain_task(
        &self,
        request: Request<BrainTaskUpdateRequest>,
    ) -> Result<Response<BrainTaskUpdateResponse>, Status> {
        let request = request.into_inner();
        let task = self
            .engine
            .update_brain_task(BrainTaskUpdate {
                task_id: request.task_id,
                status: request.status,
                priority: request.priority,
                due_at: if request.due_at.is_empty() {
                    None
                } else {
                    Some(parse_timestamp(&request.due_at).map_err(internal_status)?)
                },
                summary: request.summary,
                links_json: request.links_json,
                source_ref: request.source_ref,
            })
            .map_err(internal_status)?;
        Ok(Response::new(BrainTaskUpdateResponse {
            updated: true,
            task: Some(proto_brain_task(task)),
        }))
    }

    async fn forget_brain(
        &self,
        request: Request<BrainForgetRequest>,
    ) -> Result<Response<BrainForgetResponse>, Status> {
        let request = request.into_inner();
        let result = self
            .engine
            .forget_brain(BrainForget {
                target_kind: request.target_kind,
                target_id: request.target_id,
                mode: request.mode,
                reason: request.reason,
            })
            .map_err(internal_status)?;
        Ok(Response::new(BrainForgetResponse {
            forgotten: result.forgotten,
            mode: result.mode,
            target_id: result.target_id,
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

fn proto_brain_entity(record: BrainEntityRecord) -> BrainEntity {
    BrainEntity {
        id: record.id,
        kind: record.kind,
        subtype: record.subtype,
        title: record.title,
        content: record.content,
        scope_json: record.scope_json,
        links_json: record.links_json,
        salience: record.salience,
        confidence: record.confidence,
        archived: record.archived,
        created_at: record.created_at.to_rfc3339(),
        updated_at: record.updated_at.to_rfc3339(),
    }
}

fn proto_brain_fact(record: BrainFactRecord) -> BrainFact {
    BrainFact {
        id: record.id,
        entity_id: record.entity_id,
        content: record.content,
        scope_json: record.scope_json,
        salience: record.salience,
        confidence: record.confidence,
        archived: record.archived,
        created_at: record.created_at.to_rfc3339(),
        updated_at: record.updated_at.to_rfc3339(),
    }
}

fn proto_brain_relation(record: BrainRelationRecord) -> BrainRelation {
    BrainRelation {
        id: record.id,
        kind: record.kind,
        from_id: record.from_id,
        to_id: record.to_id,
        metadata_json: record.metadata_json,
        confidence: record.confidence,
        created_at: record.created_at.to_rfc3339(),
        updated_at: record.updated_at.to_rfc3339(),
    }
}

fn proto_brain_task(record: BrainTaskRecord) -> BrainTask {
    BrainTask {
        id: record.id,
        title: record.title,
        summary: record.summary,
        status: record.status,
        priority: record.priority,
        due_at: record
            .due_at
            .map(|value| value.to_rfc3339())
            .unwrap_or_default(),
        scope_json: record.scope_json,
        links_json: record.links_json,
        salience: record.salience,
        confidence: record.confidence,
        archived: record.archived,
        created_at: record.created_at.to_rfc3339(),
        updated_at: record.updated_at.to_rfc3339(),
    }
}

fn proto_brain_suggestion(record: BrainSuggestionRecord) -> BrainSuggestion {
    BrainSuggestion {
        id: record.id,
        task_id: record.task_id,
        summary: record.summary,
        reason: record.reason,
        score: record.score,
        context_json: record.context_json,
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
            create table if not exists brain_entities (
                entity_id text primary key,
                entity_kind text not null,
                subtype text not null,
                encrypted_blob_json text not null,
                salience real not null,
                confidence real not null,
                fingerprint text not null,
                archived integer not null,
                source_ref text not null,
                created_at text not null,
                updated_at text not null
            );
            create table if not exists brain_facts (
                fact_id text primary key,
                entity_id text not null,
                encrypted_blob_json text not null,
                salience real not null,
                confidence real not null,
                fingerprint text not null,
                archived integer not null,
                source_ref text not null,
                created_at text not null,
                updated_at text not null
            );
            create table if not exists brain_relations (
                relation_id text primary key,
                relation_kind text not null,
                from_entity_id text not null,
                to_entity_id text not null,
                metadata_json text not null,
                confidence real not null,
                archived integer not null,
                created_at text not null,
                updated_at text not null,
                unique (relation_kind, from_entity_id, to_entity_id)
            );
            create table if not exists brain_tasks (
                task_id text primary key,
                status text not null,
                priority text not null,
                due_at text,
                encrypted_blob_json text not null,
                salience real not null,
                confidence real not null,
                fingerprint text not null,
                archived integer not null,
                source_ref text not null,
                created_at text not null,
                updated_at text not null
            );
            create table if not exists brain_ingest_log (
                source_ref text primary key,
                source_kind text not null,
                fingerprint text not null,
                target_json text not null,
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

#[derive(Clone)]
struct BrainManager {
    inner: Arc<BrainInner>,
}

struct BrainInner {
    config: BrainConfig,
    paths: OpenPinchPaths,
    state: StateStore,
    encryption: EncryptionManager,
    owner_id: String,
    runtime_env_id: String,
    workspace_env_id: String,
    project_env_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BrainEntityPayload {
    title: String,
    content: String,
    scope_json: String,
    links_json: String,
    source_ref: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BrainFactPayload {
    content: String,
    scope_json: String,
    source_ref: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BrainTaskPayload {
    title: String,
    summary: String,
    scope_json: String,
    links_json: String,
    source_ref: String,
}

#[derive(Debug, Clone)]
struct ProjectionHit {
    key: String,
}

impl BrainManager {
    fn new(
        config: &AppConfig,
        paths: &OpenPinchPaths,
        state: StateStore,
        encryption: EncryptionManager,
    ) -> Result<Self> {
        let workspace_root = std::env::current_dir().unwrap_or_else(|_| paths.data_dir.clone());
        let workspace_raw = workspace_root.display().to_string();
        let workspace_name = workspace_root
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("workspace")
            .to_owned();
        let inner = Arc::new(BrainInner {
            config: config.brain.clone(),
            paths: paths.clone(),
            state,
            encryption,
            owner_id: "person:owner:local".to_owned(),
            runtime_env_id: format!("environment:runtime:{}", std::env::consts::OS),
            workspace_env_id: format!("environment:workspace:{}", prompt_hash(&workspace_raw)),
            project_env_id: format!("environment:project:{}", prompt_hash(&workspace_name)),
        });
        let manager = Self { inner };
        manager.ensure_baseline_context()?;
        Ok(manager)
    }

    fn enabled(&self) -> bool {
        self.inner.config.enabled
    }

    fn remember(&self, request: BrainRemember) -> Result<BrainRememberResult> {
        if !self.enabled() {
            bail!("brain is disabled");
        }

        self.apply_retention()?;

        let source_ref = default_source_ref(
            &request.source_ref,
            &format!(
                "{}:{}:{}:{}",
                request.kind, request.subtype, request.title, request.content
            ),
            "remember",
        );
        let digest = prompt_hash(&format!(
            "{}:{}:{}:{}:{}",
            request.kind, request.subtype, request.title, request.content, source_ref
        ));

        if request.kind == "task" {
            let task = self.upsert_or_merge_task(
                None,
                preferred_title(&request.title, &request.content),
                non_empty_or_default(&request.content, &request.title),
                if request.scope_json.is_empty() {
                    "{}".to_owned()
                } else {
                    request.scope_json.clone()
                },
                request.links_json.clone(),
                default_source_ref(&source_ref, &request.title, "task"),
                "open".to_owned(),
                task_priority_from_text(&request.content),
                parse_due_at(&request.content),
                request.importance.max(0.45),
                0.92,
            )?;
            self.record_ingest(
                &source_ref,
                "remember",
                &digest,
                &json!({ "task_id": task.id }).to_string(),
            )?;
            return Ok(BrainRememberResult {
                stored: true,
                entity: None,
                task: Some(task),
                digest,
            });
        }

        validate_entity_kind(&request.kind)?;
        if request.kind == "environment" && !request.subtype.is_empty() {
            validate_environment_subtype(&request.subtype)?;
        }
        let scope_json = if request.scope_json.is_empty() {
            "{}".to_owned()
        } else {
            request.scope_json.clone()
        };
        let links_json = if request.links_json.is_empty() {
            "[]".to_owned()
        } else {
            request.links_json.clone()
        };
        let entity_id = format!(
            "{}:{}:{}",
            request.kind,
            request.subtype,
            prompt_hash(&format!("{}:{}", request.title, scope_json))
        );
        let entity = self.upsert_entity(
            &entity_id,
            &request.kind,
            &request.subtype,
            &preferred_title(&request.title, &request.content),
            &request.content,
            &scope_json,
            &links_json,
            request.importance.max(0.4),
            0.95,
            &source_ref,
        )?;
        let fact_id = format!(
            "fact:{}:{}",
            entity.id,
            prompt_hash(&format!("{}:{}", source_ref, request.content))
        );
        let _ = self.upsert_fact(
            &fact_id,
            &entity.id,
            &request.content,
            &scope_json,
            request.importance.max(0.35),
            0.9,
            &source_ref,
        )?;
        self.link_record(&entity.id, &request.kind, &links_json, &source_ref)?;
        self.record_ingest(
            &source_ref,
            "remember",
            &digest,
            &json!({ "entity_id": entity.id }).to_string(),
        )?;
        Ok(BrainRememberResult {
            stored: true,
            entity: Some(entity),
            task: None,
            digest,
        })
    }

    fn recall(&self, query: BrainRecallQuery) -> Result<BrainRecallResult> {
        if !self.enabled() {
            bail!("brain is disabled");
        }

        self.apply_retention()?;

        let limit = query.limit.max(1);
        let entities = self.list_entities(query.include_archived)?;
        let facts = self.list_facts(query.include_archived)?;
        let tasks = self.list_tasks(false)?;
        let relations = self.list_relations(query.include_archived)?;

        let entity_hits = self.query_projection_hits("brain-entities", &query.query, limit * 3)?;
        let fact_hits = self.query_projection_hits("brain-facts", &query.query, limit * 3)?;
        let task_hits = self.query_projection_hits("brain-tasks", &query.query, limit * 3)?;

        let entity_map = entities
            .iter()
            .cloned()
            .map(|record| (record.id.clone(), record))
            .collect::<BTreeMap<_, _>>();
        let fact_map = facts
            .iter()
            .cloned()
            .map(|record| (record.id.clone(), record))
            .collect::<BTreeMap<_, _>>();
        let task_map = tasks
            .iter()
            .cloned()
            .map(|record| (record.id.clone(), record))
            .collect::<BTreeMap<_, _>>();

        let mut matched_entities = Vec::new();
        let mut matched_facts = Vec::new();
        let mut matched_tasks = Vec::new();
        let mut selected_ids = BTreeSet::new();

        for hit in entity_hits {
            if matched_entities.len() >= limit {
                break;
            }
            if let Some(entity) = entity_map.get(&hit.key) {
                if scope_matches(&entity.scope_json, &query.scope_json) {
                    matched_entities.push(entity.clone());
                    selected_ids.insert(entity.id.clone());
                }
            }
        }

        for hit in fact_hits {
            if matched_facts.len() >= limit {
                break;
            }
            if let Some(fact) = fact_map.get(&hit.key) {
                if scope_matches(&fact.scope_json, &query.scope_json) {
                    matched_facts.push(fact.clone());
                    selected_ids.insert(fact.entity_id.clone());
                    if let Some(entity) = entity_map.get(&fact.entity_id) {
                        if !matched_entities.iter().any(|item| item.id == entity.id)
                            && matched_entities.len() < limit
                        {
                            matched_entities.push(entity.clone());
                        }
                    }
                }
            }
        }

        for hit in task_hits {
            if matched_tasks.len() >= limit {
                break;
            }
            if let Some(task) = task_map.get(&hit.key) {
                if scope_matches(&task.scope_json, &query.scope_json) {
                    matched_tasks.push(task.clone());
                    selected_ids.insert(task.id.clone());
                    for linked in parse_link_ids(&task.links_json) {
                        selected_ids.insert(linked);
                    }
                }
            }
        }

        for relation in &relations {
            if selected_ids.contains(&relation.from_id) || selected_ids.contains(&relation.to_id) {
                if let Some(entity) = entity_map.get(&relation.from_id) {
                    if !matched_entities.iter().any(|item| item.id == entity.id)
                        && matched_entities.len() < limit
                    {
                        matched_entities.push(entity.clone());
                    }
                }
                if let Some(entity) = entity_map.get(&relation.to_id) {
                    if !matched_entities.iter().any(|item| item.id == entity.id)
                        && matched_entities.len() < limit
                    {
                        matched_entities.push(entity.clone());
                    }
                }
            }
        }

        let matched_relations = relations
            .into_iter()
            .filter(|relation| {
                selected_ids.contains(&relation.from_id) || selected_ids.contains(&relation.to_id)
            })
            .take(limit * 2)
            .collect::<Vec<_>>();

        Ok(BrainRecallResult {
            summary: build_recall_summary(&matched_entities, &matched_facts, &matched_tasks),
            entities: matched_entities,
            facts: matched_facts,
            relations: matched_relations,
            tasks: matched_tasks,
        })
    }

    fn suggest(&self, query: BrainSuggestQuery) -> Result<BrainSuggestResult> {
        if !self.enabled() {
            bail!("brain is disabled");
        }

        self.apply_retention()?;

        let limit = query.limit.max(1);
        let tasks = self
            .list_tasks(false)?
            .into_iter()
            .filter(|task| matches!(task.status.as_str(), "open" | "in_progress" | "blocked"))
            .filter(|task| scope_matches(&task.scope_json, &query.scope_json))
            .collect::<Vec<_>>();
        let entities = self
            .list_entities(false)?
            .into_iter()
            .map(|entity| (entity.id.clone(), entity))
            .collect::<BTreeMap<_, _>>();

        let now = Utc::now();
        let mut suggestions = tasks
            .into_iter()
            .map(|task| {
                let stale =
                    (now - task.updated_at).num_hours() >= self.inner.config.stale_task_hours;
                let due_bonus = task
                    .due_at
                    .map(|due| {
                        if due <= now {
                            0.35
                        } else if due <= now + Duration::hours(24) {
                            0.2
                        } else {
                            0.0
                        }
                    })
                    .unwrap_or(0.0);
                let stale_bonus = if stale { 0.1 } else { 0.0 };
                let blocked_bonus = if task.status == "blocked" { 0.15 } else { 0.0 };
                let score = (task.salience * 0.4)
                    + (task.confidence * 0.2)
                    + (priority_weight(&task.priority) * 0.25)
                    + due_bonus
                    + stale_bonus
                    + blocked_bonus;
                let linked_titles = parse_link_ids(&task.links_json)
                    .into_iter()
                    .filter_map(|id| entities.get(&id))
                    .map(|record| record.title.clone())
                    .collect::<Vec<_>>();
                let reason = suggestion_reason(&task, stale, due_bonus, blocked_bonus);
                let context_json = json!({
                    "task_status": task.status,
                    "priority": task.priority,
                    "due_at": task.due_at.map(|due| due.to_rfc3339()),
                    "linked_entities": linked_titles,
                })
                .to_string();
                BrainSuggestionRecord {
                    id: format!("suggestion:{}", task.id),
                    task_id: task.id.clone(),
                    summary: task.summary.clone(),
                    reason,
                    score,
                    context_json,
                }
            })
            .collect::<Vec<_>>();

        suggestions.sort_by(|left, right| {
            right
                .score
                .partial_cmp(&left.score)
                .unwrap_or(Ordering::Equal)
        });
        suggestions.truncate(limit);

        for suggestion in &suggestions {
            let projection = format!(
                "{} {} {}",
                suggestion.summary, suggestion.reason, suggestion.context_json
            );
            let _ = self.upsert_projection(
                "brain-suggestions",
                &suggestion.id,
                &projection,
                &json!({ "task_id": suggestion.task_id, "score": suggestion.score }).to_string(),
            );
        }

        Ok(BrainSuggestResult {
            summary: format!("{} next actions suggested", suggestions.len()),
            suggestions,
        })
    }

    fn list_task_records(&self, query: BrainTaskListQuery) -> Result<BrainTaskListResult> {
        if !self.enabled() {
            bail!("brain is disabled");
        }

        self.apply_retention()?;

        let limit = query.limit.max(1);
        let mut tasks = self
            .list_tasks(false)?
            .into_iter()
            .filter(|task| scope_matches(&task.scope_json, &query.scope_json))
            .filter(|task| {
                query.statuses.is_empty()
                    || query.statuses.iter().any(|status| status == &task.status)
            })
            .filter(|task| {
                query.priorities.is_empty()
                    || query
                        .priorities
                        .iter()
                        .any(|priority| priority == &task.priority)
            })
            .filter(|task| {
                query
                    .due_before
                    .map(|due_before| task.due_at.map(|due| due <= due_before).unwrap_or(false))
                    .unwrap_or(true)
            })
            .collect::<Vec<_>>();
        tasks.sort_by(|left, right| right.updated_at.cmp(&left.updated_at));
        tasks.truncate(limit);
        Ok(BrainTaskListResult {
            summary: format!("{} tasks matched", tasks.len()),
            tasks,
        })
    }

    fn update_task_record(&self, request: BrainTaskUpdate) -> Result<BrainTaskRecord> {
        if !self.enabled() {
            bail!("brain is disabled");
        }

        validate_task_status(&request.status)?;
        let priority = if request.priority.is_empty() {
            "normal".to_owned()
        } else {
            request.priority
        };
        let task_id = if request.task_id.is_empty() {
            format!("task:{}", prompt_hash(&request.summary))
        } else {
            request.task_id
        };
        let existing = self
            .list_tasks(true)?
            .into_iter()
            .find(|task| task.id == task_id);
        let scope_json = existing
            .as_ref()
            .map(|task| task.scope_json.clone())
            .unwrap_or_else(|| "{}".to_owned());
        let links_json = if request.links_json.is_empty() {
            existing
                .as_ref()
                .map(|task| task.links_json.clone())
                .unwrap_or_else(|| "[]".to_owned())
        } else {
            request.links_json
        };
        let summary = non_empty_or_default(
            &request.summary,
            &existing
                .as_ref()
                .map(|task| task.summary.clone())
                .unwrap_or_else(|| task_id.clone()),
        );
        let task = self.upsert_task(
            &task_id,
            &preferred_title(&summary, &summary),
            &summary,
            &request.status,
            &priority,
            request
                .due_at
                .or_else(|| existing.as_ref().and_then(|task| task.due_at)),
            &scope_json,
            &links_json,
            0.8,
            0.95,
            &default_source_ref(&request.source_ref, &task_id, "task-update"),
        )?;
        self.link_record(
            &task.id,
            "task",
            &links_json,
            &default_source_ref(&request.source_ref, &task.id, "task-update"),
        )?;
        Ok(task)
    }

    fn forget(&self, request: BrainForget) -> Result<BrainForgetResult> {
        if !self.enabled() {
            bail!("brain is disabled");
        }

        let mode = if request.mode.is_empty() {
            "archive".to_owned()
        } else {
            request.mode
        };
        let target_kind = normalize_forget_target_kind(&request.target_kind);
        let connection = self.inner.state.connection.lock();
        match (target_kind.as_str(), mode.as_str()) {
            ("entity", "archive") => {
                connection.execute(
                    "update brain_entities set archived = 1, updated_at = ?2 where entity_id = ?1",
                    params![request.target_id, Utc::now().to_rfc3339()],
                )?;
                connection.execute(
                    "update brain_facts set archived = 1, updated_at = ?2 where entity_id = ?1",
                    params![request.target_id, Utc::now().to_rfc3339()],
                )?;
                connection.execute(
                    "update brain_relations set archived = 1, updated_at = ?2 where from_entity_id = ?1 or to_entity_id = ?1",
                    params![request.target_id, Utc::now().to_rfc3339()],
                )?;
            }
            ("task", "archive") => {
                connection.execute(
                    "update brain_tasks set archived = 1, updated_at = ?2 where task_id = ?1",
                    params![request.target_id, Utc::now().to_rfc3339()],
                )?;
            }
            ("fact", "archive") => {
                connection.execute(
                    "update brain_facts set archived = 1, updated_at = ?2 where fact_id = ?1",
                    params![request.target_id, Utc::now().to_rfc3339()],
                )?;
            }
            ("relation", "archive") => {
                connection.execute(
                    "update brain_relations set archived = 1, updated_at = ?2 where relation_id = ?1",
                    params![request.target_id, Utc::now().to_rfc3339()],
                )?;
            }
            ("entity", "delete") => {
                connection.execute(
                    "delete from brain_facts where entity_id = ?1",
                    params![request.target_id],
                )?;
                connection.execute(
                    "delete from brain_relations where from_entity_id = ?1 or to_entity_id = ?1",
                    params![request.target_id],
                )?;
                connection.execute(
                    "delete from brain_entities where entity_id = ?1",
                    params![request.target_id],
                )?;
            }
            ("task", "delete") => {
                connection.execute(
                    "delete from brain_tasks where task_id = ?1",
                    params![request.target_id],
                )?;
                connection.execute(
                    "delete from brain_relations where from_entity_id = ?1 or to_entity_id = ?1",
                    params![request.target_id],
                )?;
            }
            ("fact", "delete") => {
                connection.execute(
                    "delete from brain_facts where fact_id = ?1",
                    params![request.target_id],
                )?;
            }
            ("relation", "delete") => {
                connection.execute(
                    "delete from brain_relations where relation_id = ?1",
                    params![request.target_id],
                )?;
            }
            _ => bail!("unsupported forget mode {} for {}", mode, target_kind),
        }
        drop(connection);

        if mode == "delete" {
            let _ = self.delete_projection("brain-entities", &request.target_id);
            let _ = self.delete_projection("brain-facts", &request.target_id);
            let _ = self.delete_projection("brain-tasks", &request.target_id);
            let _ = self.delete_projection(
                "brain-suggestions",
                &format!("suggestion:{}", request.target_id),
            );
        }

        Ok(BrainForgetResult {
            forgotten: true,
            mode,
            target_id: request.target_id,
        })
    }

    fn ingest_message(&self, message: &MessageEnvelope) -> Result<()> {
        if !self.enabled() || !self.inner.config.auto_ingest_messages {
            return Ok(());
        }

        let source_ref = format!(
            "message:{}:{}:{}:{}",
            message.connector,
            message.channel_id,
            prompt_hash(&message.sender),
            prompt_hash(&message.body)
        );
        let scope_json = self.scope_for_message(message);
        let contact = self.upsert_entity(
            &format!(
                "person:contact:{}:{}",
                message.connector,
                prompt_hash(&message.sender)
            ),
            "person",
            "contact",
            &message.sender,
            &format!(
                "Observed through {} in channel {}",
                message.connector, message.channel_id
            ),
            &scope_json,
            "[]",
            0.58,
            0.96,
            &source_ref,
        )?;
        self.upsert_relation(
            "knows",
            &self.inner.owner_id,
            &contact.id,
            &json!({ "connector": message.connector, "channel_id": message.channel_id })
                .to_string(),
            0.92,
            false,
        )?;
        self.upsert_relation(
            "mentioned_with",
            &contact.id,
            &self.inner.workspace_env_id,
            &json!({ "source_ref": source_ref }).to_string(),
            0.65,
            false,
        )?;
        let _ = self.upsert_fact(
            &format!("fact:{}:{}", contact.id, prompt_hash(&source_ref)),
            &contact.id,
            &message.body,
            &scope_json,
            0.62,
            0.84,
            &source_ref,
        )?;

        let mut linked_ids = vec![contact.id.clone()];
        for artifact in extract_artifacts(&message.body) {
            let entity = self.upsert_entity(
                &format!("artifact:{}", prompt_hash(&artifact)),
                "artifact",
                "reference",
                &artifact,
                &format!("Referenced in inbound {} message", message.connector),
                &scope_json,
                "[]",
                0.56,
                0.85,
                &source_ref,
            )?;
            self.upsert_relation(
                "mentioned_with",
                &contact.id,
                &entity.id,
                &json!({ "source_ref": source_ref }).to_string(),
                0.74,
                false,
            )?;
            linked_ids.push(entity.id);
        }

        for (subtype, title) in extract_environment_mentions(&message.body) {
            let entity = self.upsert_entity(
                &format!("environment:{}:{}", subtype, prompt_hash(&title)),
                "environment",
                &subtype,
                &title,
                &format!("Mentioned {} context", subtype),
                &scope_json,
                "[]",
                0.52,
                0.78,
                &source_ref,
            )?;
            self.upsert_relation(
                "mentioned_with",
                &contact.id,
                &entity.id,
                &json!({ "source_ref": source_ref }).to_string(),
                0.7,
                false,
            )?;
            linked_ids.push(entity.id);
        }

        if let Some(summary) = extract_task_summary(&message.body) {
            let task = self.upsert_or_merge_task(
                None,
                preferred_title(&summary, &summary),
                summary,
                scope_json.clone(),
                serde_json::to_string(&linked_ids).unwrap_or_else(|_| "[]".to_owned()),
                source_ref.clone(),
                "open".to_owned(),
                task_priority_from_text(&message.body),
                parse_due_at(&message.body),
                0.84,
                0.88,
            )?;
            self.upsert_relation(
                "assigned_to",
                &task.id,
                &self.inner.owner_id,
                &json!({ "source_ref": source_ref }).to_string(),
                0.86,
                false,
            )?;
            self.upsert_relation(
                "about",
                &task.id,
                &contact.id,
                &json!({ "source_ref": source_ref }).to_string(),
                0.78,
                false,
            )?;
            linked_ids.push(task.id);
        }

        self.record_ingest(
            &source_ref,
            "message",
            &fingerprint(&message.body),
            &json!({ "linked_ids": linked_ids }).to_string(),
        )
    }

    fn ingest_tool_result(&self, call: &ToolCall, outcome: &ToolOutcome) -> Result<()> {
        if !self.enabled() || !self.inner.config.auto_ingest_tool_results || !outcome.success {
            return Ok(());
        }

        let source_ref = format!(
            "tool:{}:{}",
            call.target,
            prompt_hash(&format!("{}:{}", outcome.summary, outcome.data_json))
        );
        let scope_json = json!({
            "source": "tool",
            "target": call.target,
            "priority": call.priority.as_str(),
        })
        .to_string();
        let result_summary = non_empty_or_default(&outcome.summary, &call.target);
        let artifact = self.upsert_entity(
            &format!("artifact:tool:{}", prompt_hash(&call.target)),
            "artifact",
            "tool-result",
            &call.target,
            &format!("Successful tool execution: {}", result_summary),
            &scope_json,
            "[]",
            0.68,
            0.92,
            &source_ref,
        )?;
        let _ = self.upsert_fact(
            &format!("fact:{}:{}", artifact.id, prompt_hash(&source_ref)),
            &artifact.id,
            &truncate_with_ellipsis(&format!("{} {}", result_summary, outcome.data_json), 400),
            &scope_json,
            0.7,
            0.9,
            &source_ref,
        )?;
        let _ = self.upsert_fact(
            &format!(
                "fact:{}:{}",
                self.inner.workspace_env_id,
                prompt_hash(&call.target)
            ),
            &self.inner.workspace_env_id,
            &format!("Tool {} succeeded with {}", call.target, result_summary),
            &scope_json,
            0.58,
            0.86,
            &source_ref,
        )?;

        if let Some(task) = self.find_related_tool_task(&call.target, &result_summary) {
            let _ = self.upsert_task(
                &task.id,
                &task.title,
                &task.summary,
                "done",
                &task.priority,
                task.due_at,
                &task.scope_json,
                &task.links_json,
                task.salience,
                0.96,
                &source_ref,
            )?;
        }

        self.record_ingest(
            &source_ref,
            "tool",
            &fingerprint(&format!("{} {}", outcome.summary, outcome.data_json)),
            &json!({ "artifact_id": artifact.id }).to_string(),
        )
    }

    fn ingest_assistant_reply(&self, message: &MessageEnvelope, reply: &str) -> Result<()> {
        if !self.enabled() || !self.inner.config.auto_ingest_assistant_commitments {
            return Ok(());
        }

        let source_ref = format!(
            "assistant:{}:{}:{}",
            message.connector,
            message.channel_id,
            prompt_hash(reply)
        );
        let scope_json = self.scope_for_message(message);
        if let Some(summary) = extract_assistant_commitment(reply) {
            let task = self.upsert_or_merge_task(
                None,
                preferred_title(&summary, reply),
                summary.clone(),
                scope_json.clone(),
                serde_json::to_string(&vec![self.inner.owner_id.clone()])
                    .unwrap_or_else(|_| "[]".to_owned()),
                source_ref.clone(),
                "in_progress".to_owned(),
                task_priority_from_text(reply),
                parse_due_at(reply),
                0.74,
                0.72,
            )?;
            self.upsert_relation(
                "assigned_to",
                &task.id,
                &self.inner.owner_id,
                &json!({ "source_ref": source_ref }).to_string(),
                0.78,
                false,
            )?;
            self.record_ingest(
                &source_ref,
                "assistant",
                &fingerprint(reply),
                &json!({ "task_id": task.id }).to_string(),
            )?;
        }
        Ok(())
    }

    fn build_context_pack(&self, query: &str, scope_json: &str) -> Result<String> {
        if !self.enabled() {
            return Ok(String::new());
        }

        let recall = self.recall(BrainRecallQuery {
            query: query.to_owned(),
            scope_json: scope_json.to_owned(),
            limit: 3,
            include_archived: false,
        })?;
        let suggestions = self.suggest(BrainSuggestQuery {
            scope_json: scope_json.to_owned(),
            limit: self.inner.config.max_inline_suggestions,
        })?;

        let mut lines = Vec::new();
        if !recall.summary.is_empty() {
            lines.push(format!("summary: {}", recall.summary));
        }
        for entity in recall.entities.iter().take(2) {
            lines.push(format!(
                "entity: {} ({}) - {}",
                entity.title, entity.kind, entity.content
            ));
        }
        for task in recall.tasks.iter().take(2) {
            lines.push(format!(
                "task: {} [{} / {}]",
                task.summary, task.status, task.priority
            ));
        }
        for suggestion in suggestions
            .suggestions
            .iter()
            .take(self.inner.config.max_inline_suggestions)
        {
            lines.push(format!(
                "suggestion: {} ({})",
                suggestion.summary, suggestion.reason
            ));
        }

        let mut content = lines.join("\n");
        if content.len() > self.inner.config.context_budget_chars {
            content = truncate_with_ellipsis(&content, self.inner.config.context_budget_chars);
        }
        Ok(content)
    }

    fn inline_suggestions(
        &self,
        scope_json: &str,
        limit: usize,
    ) -> Result<Vec<BrainSuggestionRecord>> {
        Ok(self
            .suggest(BrainSuggestQuery {
                scope_json: scope_json.to_owned(),
                limit,
            })?
            .suggestions
            .into_iter()
            .filter(|suggestion| suggestion.score >= 0.55)
            .take(limit)
            .collect())
    }

    fn ensure_baseline_context(&self) -> Result<()> {
        let workspace_root =
            std::env::current_dir().unwrap_or_else(|_| self.inner.paths.data_dir.clone());
        let workspace_title = workspace_root
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("workspace")
            .to_owned();
        let runtime_scope = json!({
            "scope": "runtime",
            "os": std::env::consts::OS,
        })
        .to_string();
        let workspace_scope = json!({
            "scope": "workspace",
            "path": workspace_root.display().to_string(),
        })
        .to_string();
        let project_scope = json!({
            "scope": "project",
            "workspace": workspace_title,
        })
        .to_string();

        let _ = self.upsert_entity(
            &self.inner.owner_id,
            "person",
            "owner",
            "Local Owner",
            "Primary local OpenPinch operator",
            "{}",
            "[]",
            0.95,
            0.99,
            "brain:baseline",
        )?;
        let _ = self.upsert_entity(
            &self.inner.runtime_env_id,
            "environment",
            "runtime",
            &format!("{} runtime", std::env::consts::OS),
            &format!("OpenPinch runtime on {}", std::env::consts::OS),
            &runtime_scope,
            "[]",
            0.82,
            0.98,
            "brain:baseline",
        )?;
        let _ = self.upsert_entity(
            &self.inner.workspace_env_id,
            "environment",
            "workspace",
            &workspace_title,
            &format!("Workspace rooted at {}", workspace_root.display()),
            &workspace_scope,
            "[]",
            0.84,
            0.98,
            "brain:baseline",
        )?;
        let _ = self.upsert_entity(
            &self.inner.project_env_id,
            "environment",
            "project",
            &workspace_title,
            &format!("Project context for {}", workspace_title),
            &project_scope,
            "[]",
            0.8,
            0.94,
            "brain:baseline",
        )?;
        self.upsert_relation(
            "owns",
            &self.inner.owner_id,
            &self.inner.workspace_env_id,
            &json!({ "source_ref": "brain:baseline" }).to_string(),
            0.96,
            false,
        )?;
        self.upsert_relation(
            "belongs_to",
            &self.inner.workspace_env_id,
            &self.inner.project_env_id,
            &json!({ "source_ref": "brain:baseline" }).to_string(),
            0.94,
            false,
        )?;
        self.upsert_relation(
            "runs_in",
            &self.inner.project_env_id,
            &self.inner.runtime_env_id,
            &json!({ "source_ref": "brain:baseline" }).to_string(),
            0.93,
            false,
        )?;
        Ok(())
    }

    fn apply_retention(&self) -> Result<()> {
        let cutoff =
            (Utc::now() - Duration::days(self.inner.config.archive_decay_days)).to_rfc3339();
        let connection = self.inner.state.connection.lock();
        connection.execute(
            "update brain_entities set archived = 1, updated_at = ?2
             where archived = 0 and salience < 0.35 and updated_at < ?1",
            params![cutoff, Utc::now().to_rfc3339()],
        )?;
        connection.execute(
            "update brain_facts set archived = 1, updated_at = ?2
             where archived = 0 and salience < 0.35 and updated_at < ?1",
            params![cutoff, Utc::now().to_rfc3339()],
        )?;
        connection.execute(
            "update brain_tasks set archived = 1, updated_at = ?2
             where archived = 0 and salience < 0.5 and status in ('done', 'cancelled') and updated_at < ?1",
            params![cutoff, Utc::now().to_rfc3339()],
        )?;
        Ok(())
    }

    fn scope_for_message(&self, message: &MessageEnvelope) -> String {
        json!({
            "connector": message.connector,
            "channel_id": message.channel_id,
            "sender": message.sender,
        })
        .to_string()
    }

    fn record_ingest(
        &self,
        source_ref: &str,
        source_kind: &str,
        fingerprint_value: &str,
        target_json: &str,
    ) -> Result<()> {
        let connection = self.inner.state.connection.lock();
        connection.execute(
            "insert into brain_ingest_log (source_ref, source_kind, fingerprint, target_json, created_at) values (?1, ?2, ?3, ?4, ?5)
             on conflict(source_ref) do update set source_kind = excluded.source_kind, fingerprint = excluded.fingerprint, target_json = excluded.target_json, created_at = excluded.created_at",
            params![source_ref, source_kind, fingerprint_value, target_json, Utc::now().to_rfc3339()],
        )?;
        Ok(())
    }

    fn upsert_entity(
        &self,
        entity_id: &str,
        kind: &str,
        subtype: &str,
        title: &str,
        content: &str,
        scope_json: &str,
        links_json: &str,
        salience: f64,
        confidence: f64,
        source_ref: &str,
    ) -> Result<BrainEntityRecord> {
        validate_entity_kind(kind)?;
        if kind == "environment" && !subtype.is_empty() {
            validate_environment_subtype(subtype)?;
        }
        let payload = BrainEntityPayload {
            title: title.to_owned(),
            content: content.to_owned(),
            scope_json: scope_json.to_owned(),
            links_json: links_json.to_owned(),
            source_ref: source_ref.to_owned(),
        };
        let encrypted_blob_json = self.encrypt_payload(&payload)?;
        let now = Utc::now();
        let connection = self.inner.state.connection.lock();
        connection.execute(
            "insert into brain_entities (entity_id, entity_kind, subtype, encrypted_blob_json, salience, confidence, fingerprint, archived, source_ref, created_at, updated_at)
             values (?1, ?2, ?3, ?4, ?5, ?6, ?7, 0, ?8, ?9, ?9)
             on conflict(entity_id) do update set entity_kind = excluded.entity_kind, subtype = excluded.subtype, encrypted_blob_json = excluded.encrypted_blob_json, salience = excluded.salience, confidence = excluded.confidence, fingerprint = excluded.fingerprint, archived = 0, source_ref = excluded.source_ref, updated_at = excluded.updated_at",
            params![
                entity_id,
                kind,
                subtype,
                encrypted_blob_json,
                salience,
                confidence,
                fingerprint(&format!("{} {} {}", title, content, scope_json)),
                source_ref,
                now.to_rfc3339(),
            ],
        )?;
        drop(connection);

        let record = BrainEntityRecord {
            id: entity_id.to_owned(),
            kind: kind.to_owned(),
            subtype: subtype.to_owned(),
            title: title.to_owned(),
            content: content.to_owned(),
            scope_json: scope_json.to_owned(),
            links_json: links_json.to_owned(),
            salience,
            confidence,
            archived: false,
            created_at: now,
            updated_at: now,
        };
        self.upsert_projection(
            "brain-entities",
            entity_id,
            &entity_projection(&record),
            &json!({ "kind": kind, "subtype": subtype }).to_string(),
        )?;
        Ok(record)
    }

    fn upsert_fact(
        &self,
        fact_id: &str,
        entity_id: &str,
        content: &str,
        scope_json: &str,
        salience: f64,
        confidence: f64,
        source_ref: &str,
    ) -> Result<BrainFactRecord> {
        let payload = BrainFactPayload {
            content: content.to_owned(),
            scope_json: scope_json.to_owned(),
            source_ref: source_ref.to_owned(),
        };
        let encrypted_blob_json = self.encrypt_payload(&payload)?;
        let now = Utc::now();
        let connection = self.inner.state.connection.lock();
        connection.execute(
            "insert into brain_facts (fact_id, entity_id, encrypted_blob_json, salience, confidence, fingerprint, archived, source_ref, created_at, updated_at)
             values (?1, ?2, ?3, ?4, ?5, ?6, 0, ?7, ?8, ?8)
             on conflict(fact_id) do update set entity_id = excluded.entity_id, encrypted_blob_json = excluded.encrypted_blob_json, salience = excluded.salience, confidence = excluded.confidence, fingerprint = excluded.fingerprint, archived = 0, source_ref = excluded.source_ref, updated_at = excluded.updated_at",
            params![
                fact_id,
                entity_id,
                encrypted_blob_json,
                salience,
                confidence,
                fingerprint(content),
                source_ref,
                now.to_rfc3339(),
            ],
        )?;
        drop(connection);

        let record = BrainFactRecord {
            id: fact_id.to_owned(),
            entity_id: entity_id.to_owned(),
            content: content.to_owned(),
            scope_json: scope_json.to_owned(),
            salience,
            confidence,
            archived: false,
            created_at: now,
            updated_at: now,
        };
        self.upsert_projection(
            "brain-facts",
            fact_id,
            &fact_projection(&record),
            &json!({ "entity_id": entity_id }).to_string(),
        )?;
        Ok(record)
    }

    fn upsert_relation(
        &self,
        kind: &str,
        from_id: &str,
        to_id: &str,
        metadata_json: &str,
        confidence: f64,
        archived: bool,
    ) -> Result<BrainRelationRecord> {
        validate_relation_kind(kind)?;
        let relation_id = format!(
            "relation:{}",
            prompt_hash(&format!("{}:{}:{}", kind, from_id, to_id))
        );
        let now = Utc::now();
        let connection = self.inner.state.connection.lock();
        connection.execute(
            "insert into brain_relations (relation_id, relation_kind, from_entity_id, to_entity_id, metadata_json, confidence, archived, created_at, updated_at)
             values (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8)
             on conflict(relation_kind, from_entity_id, to_entity_id) do update set metadata_json = excluded.metadata_json, confidence = excluded.confidence, archived = excluded.archived, updated_at = excluded.updated_at",
            params![
                relation_id,
                kind,
                from_id,
                to_id,
                metadata_json,
                confidence,
                if archived { 1 } else { 0 },
                now.to_rfc3339(),
            ],
        )?;
        Ok(BrainRelationRecord {
            id: relation_id,
            kind: kind.to_owned(),
            from_id: from_id.to_owned(),
            to_id: to_id.to_owned(),
            metadata_json: metadata_json.to_owned(),
            confidence,
            created_at: now,
            updated_at: now,
        })
    }

    fn upsert_task(
        &self,
        task_id: &str,
        title: &str,
        summary: &str,
        status: &str,
        priority: &str,
        due_at: Option<DateTime<Utc>>,
        scope_json: &str,
        links_json: &str,
        salience: f64,
        confidence: f64,
        source_ref: &str,
    ) -> Result<BrainTaskRecord> {
        validate_task_status(status)?;
        let payload = BrainTaskPayload {
            title: title.to_owned(),
            summary: summary.to_owned(),
            scope_json: scope_json.to_owned(),
            links_json: links_json.to_owned(),
            source_ref: source_ref.to_owned(),
        };
        let encrypted_blob_json = self.encrypt_payload(&payload)?;
        let now = Utc::now();
        let connection = self.inner.state.connection.lock();
        connection.execute(
            "insert into brain_tasks (task_id, status, priority, due_at, encrypted_blob_json, salience, confidence, fingerprint, archived, source_ref, created_at, updated_at)
             values (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 0, ?9, ?10, ?10)
             on conflict(task_id) do update set status = excluded.status, priority = excluded.priority, due_at = excluded.due_at, encrypted_blob_json = excluded.encrypted_blob_json, salience = excluded.salience, confidence = excluded.confidence, fingerprint = excluded.fingerprint, archived = 0, source_ref = excluded.source_ref, updated_at = excluded.updated_at",
            params![
                task_id,
                status,
                priority,
                due_at.map(|value| value.to_rfc3339()),
                encrypted_blob_json,
                salience,
                confidence,
                fingerprint(&format!("{} {}", title, summary)),
                source_ref,
                now.to_rfc3339(),
            ],
        )?;
        drop(connection);

        let record = BrainTaskRecord {
            id: task_id.to_owned(),
            title: title.to_owned(),
            summary: summary.to_owned(),
            status: status.to_owned(),
            priority: priority.to_owned(),
            due_at,
            scope_json: scope_json.to_owned(),
            links_json: links_json.to_owned(),
            salience,
            confidence,
            archived: false,
            created_at: now,
            updated_at: now,
        };
        self.upsert_projection(
            "brain-tasks",
            task_id,
            &task_projection(&record),
            &json!({ "status": status, "priority": priority }).to_string(),
        )?;
        self.upsert_projection(
            "brain-suggestions",
            &format!("suggestion:{}", task_id),
            &format!("{} {}", record.summary, record.priority),
            &json!({ "task_id": task_id, "status": status }).to_string(),
        )?;
        Ok(record)
    }

    fn upsert_or_merge_task(
        &self,
        task_id: Option<String>,
        title: String,
        summary: String,
        scope_json: String,
        links_json: String,
        source_ref: String,
        status: String,
        priority: String,
        due_at: Option<DateTime<Utc>>,
        salience: f64,
        confidence: f64,
    ) -> Result<BrainTaskRecord> {
        let target_id = task_id
            .or_else(|| {
                self.find_similar_task(&summary, &scope_json)
                    .map(|task| task.id)
            })
            .unwrap_or_else(|| {
                format!(
                    "task:{}",
                    prompt_hash(&format!("{}:{}", summary, scope_json))
                )
            });
        self.upsert_task(
            &target_id,
            &title,
            &summary,
            &status,
            &priority,
            due_at,
            &scope_json,
            &links_json,
            salience,
            confidence,
            &source_ref,
        )
    }

    fn link_record(
        &self,
        record_id: &str,
        record_kind: &str,
        links_json: &str,
        source_ref: &str,
    ) -> Result<()> {
        for linked_id in parse_link_ids(links_json) {
            let relation_kind = if record_kind == "task" {
                "about"
            } else {
                "mentioned_with"
            };
            self.upsert_relation(
                relation_kind,
                record_id,
                &linked_id,
                &json!({ "source_ref": source_ref }).to_string(),
                0.74,
                false,
            )?;
        }
        Ok(())
    }

    fn list_entities(&self, include_archived: bool) -> Result<Vec<BrainEntityRecord>> {
        let connection = self.inner.state.connection.lock();
        let mut statement = connection.prepare(
            "select entity_id, entity_kind, subtype, encrypted_blob_json, salience, confidence, archived, created_at, updated_at from brain_entities order by updated_at desc limit 512",
        )?;
        let rows = statement.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, f64>(4)?,
                row.get::<_, f64>(5)?,
                row.get::<_, i64>(6)?,
                row.get::<_, String>(7)?,
                row.get::<_, String>(8)?,
            ))
        })?;
        let mut records = Vec::new();
        for row in rows {
            let (
                id,
                kind,
                subtype,
                blob_json,
                salience,
                confidence,
                archived,
                created_at,
                updated_at,
            ) = row?;
            if archived != 0 && !include_archived {
                continue;
            }
            let payload = self.decrypt_payload::<BrainEntityPayload>(&blob_json)?;
            records.push(BrainEntityRecord {
                id,
                kind,
                subtype,
                title: payload.title,
                content: payload.content,
                scope_json: payload.scope_json,
                links_json: payload.links_json,
                salience,
                confidence,
                archived: archived != 0,
                created_at: parse_timestamp(&created_at)?,
                updated_at: parse_timestamp(&updated_at)?,
            });
        }
        Ok(records)
    }

    fn list_facts(&self, include_archived: bool) -> Result<Vec<BrainFactRecord>> {
        let connection = self.inner.state.connection.lock();
        let mut statement = connection.prepare(
            "select fact_id, entity_id, encrypted_blob_json, salience, confidence, archived, created_at, updated_at from brain_facts order by updated_at desc limit 512",
        )?;
        let rows = statement.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, f64>(3)?,
                row.get::<_, f64>(4)?,
                row.get::<_, i64>(5)?,
                row.get::<_, String>(6)?,
                row.get::<_, String>(7)?,
            ))
        })?;
        let mut records = Vec::new();
        for row in rows {
            let (id, entity_id, blob_json, salience, confidence, archived, created_at, updated_at) =
                row?;
            if archived != 0 && !include_archived {
                continue;
            }
            let payload = self.decrypt_payload::<BrainFactPayload>(&blob_json)?;
            records.push(BrainFactRecord {
                id,
                entity_id,
                content: payload.content,
                scope_json: payload.scope_json,
                salience,
                confidence,
                archived: archived != 0,
                created_at: parse_timestamp(&created_at)?,
                updated_at: parse_timestamp(&updated_at)?,
            });
        }
        Ok(records)
    }

    fn list_relations(&self, include_archived: bool) -> Result<Vec<BrainRelationRecord>> {
        let connection = self.inner.state.connection.lock();
        let mut statement = connection.prepare(
            "select relation_id, relation_kind, from_entity_id, to_entity_id, metadata_json, confidence, archived, created_at, updated_at from brain_relations order by updated_at desc limit 512",
        )?;
        let rows = statement.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, f64>(5)?,
                row.get::<_, i64>(6)?,
                row.get::<_, String>(7)?,
                row.get::<_, String>(8)?,
            ))
        })?;
        let mut records = Vec::new();
        for row in rows {
            let (
                id,
                kind,
                from_id,
                to_id,
                metadata_json,
                confidence,
                archived,
                created_at,
                updated_at,
            ) = row?;
            if archived != 0 && !include_archived {
                continue;
            }
            records.push(BrainRelationRecord {
                id,
                kind,
                from_id,
                to_id,
                metadata_json,
                confidence,
                created_at: parse_timestamp(&created_at)?,
                updated_at: parse_timestamp(&updated_at)?,
            });
        }
        Ok(records)
    }

    fn list_tasks(&self, include_archived: bool) -> Result<Vec<BrainTaskRecord>> {
        let connection = self.inner.state.connection.lock();
        let mut statement = connection.prepare(
            "select task_id, status, priority, due_at, encrypted_blob_json, salience, confidence, archived, created_at, updated_at from brain_tasks order by updated_at desc limit 512",
        )?;
        let rows = statement.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, f64>(5)?,
                row.get::<_, f64>(6)?,
                row.get::<_, i64>(7)?,
                row.get::<_, String>(8)?,
                row.get::<_, String>(9)?,
            ))
        })?;
        let mut records = Vec::new();
        for row in rows {
            let (
                id,
                status,
                priority,
                due_at,
                blob_json,
                salience,
                confidence,
                archived,
                created_at,
                updated_at,
            ) = row?;
            if archived != 0 && !include_archived {
                continue;
            }
            let payload = self.decrypt_payload::<BrainTaskPayload>(&blob_json)?;
            records.push(BrainTaskRecord {
                id,
                title: payload.title,
                summary: payload.summary,
                status,
                priority,
                due_at: due_at.as_deref().map(parse_timestamp).transpose()?,
                scope_json: payload.scope_json,
                links_json: payload.links_json,
                salience,
                confidence,
                archived: archived != 0,
                created_at: parse_timestamp(&created_at)?,
                updated_at: parse_timestamp(&updated_at)?,
            });
        }
        Ok(records)
    }

    fn find_similar_task(&self, summary: &str, scope_json: &str) -> Option<BrainTaskRecord> {
        let target = fingerprint(summary);
        self.list_tasks(false)
            .ok()?
            .into_iter()
            .filter(|task| scope_matches(&task.scope_json, scope_json))
            .filter_map(|task| {
                let score = similarity(&target, &fingerprint(&task.summary));
                (score >= 0.72).then_some((task, score))
            })
            .max_by(|(_, left), (_, right)| left.partial_cmp(right).unwrap_or(Ordering::Equal))
            .map(|(task, _)| task)
    }

    fn find_related_tool_task(&self, target: &str, summary: &str) -> Option<BrainTaskRecord> {
        let target_lower = target.to_ascii_lowercase();
        self.list_tasks(false).ok()?.into_iter().find(|task| {
            matches!(task.status.as_str(), "open" | "in_progress" | "blocked")
                && (task.summary.to_ascii_lowercase().contains(&target_lower)
                    || task.title.to_ascii_lowercase().contains(&target_lower)
                    || similarity(&fingerprint(summary), &fingerprint(&task.summary)) >= 0.72)
        })
    }

    fn query_projection_hits(
        &self,
        namespace: &str,
        query: &str,
        limit: usize,
    ) -> Result<Vec<ProjectionHit>> {
        self.inner
            .state
            .query_memory(
                &MemoryQuery {
                    namespace: namespace.to_owned(),
                    query: query.to_owned(),
                    limit: limit.max(1),
                    filter_json: "{}".to_owned(),
                },
                &self.inner.encryption,
            )
            .map(|records| {
                records
                    .into_iter()
                    .map(|record| ProjectionHit { key: record.key })
                    .collect()
            })
    }

    fn upsert_projection(
        &self,
        namespace: &str,
        key: &str,
        content: &str,
        metadata_json: &str,
    ) -> Result<()> {
        self.inner
            .state
            .upsert_memory(
                namespace,
                key,
                content,
                metadata_json,
                &self.inner.encryption,
            )
            .map(|_| ())
    }

    fn delete_projection(&self, namespace: &str, key: &str) -> Result<()> {
        let connection = self.inner.state.connection.lock();
        connection.execute(
            "delete from vector_memory where namespace = ?1 and key = ?2",
            params![namespace, key],
        )?;
        Ok(())
    }

    fn encrypt_payload<T: Serialize>(&self, payload: &T) -> Result<String> {
        let encoded = serde_json::to_string(payload).context("failed to encode brain payload")?;
        let encrypted = self
            .inner
            .encryption
            .encrypt_text(&encoded)
            .context("failed to encrypt brain payload")?;
        serde_json::to_string(&encrypted).context("failed to encode encrypted brain payload")
    }

    fn decrypt_payload<T: for<'de> Deserialize<'de>>(&self, blob_json: &str) -> Result<T> {
        let blob = serde_json::from_str::<EncryptedBlob>(blob_json)
            .context("failed to decode encrypted brain payload")?;
        let plaintext = self
            .inner
            .encryption
            .decrypt_text(&blob)
            .context("failed to decrypt brain payload")?;
        serde_json::from_str(&plaintext).context("failed to decode brain payload")
    }
}

fn validate_entity_kind(kind: &str) -> Result<()> {
    if matches!(kind, "person" | "environment" | "artifact" | "task") {
        Ok(())
    } else {
        bail!("unsupported brain entity kind {}", kind)
    }
}

fn validate_environment_subtype(subtype: &str) -> Result<()> {
    if matches!(
        subtype,
        "runtime" | "workspace" | "deployment" | "project" | "team" | "place"
    ) {
        Ok(())
    } else {
        bail!("unsupported brain environment subtype {}", subtype)
    }
}

fn validate_task_status(status: &str) -> Result<()> {
    if matches!(
        status,
        "open" | "in_progress" | "blocked" | "done" | "cancelled"
    ) {
        Ok(())
    } else {
        bail!("unsupported brain task status {}", status)
    }
}

fn validate_relation_kind(kind: &str) -> Result<()> {
    if matches!(
        kind,
        "knows"
            | "owns"
            | "belongs_to"
            | "runs_in"
            | "about"
            | "assigned_to"
            | "depends_on"
            | "blocked_by"
            | "mentioned_with"
    ) {
        Ok(())
    } else {
        bail!("unsupported brain relation kind {}", kind)
    }
}

fn normalize_forget_target_kind(kind: &str) -> String {
    match kind {
        "person" | "environment" | "artifact" => "entity".to_owned(),
        "" => "entity".to_owned(),
        other => other.to_owned(),
    }
}

fn default_source_ref(source_ref: &str, seed: &str, prefix: &str) -> String {
    if source_ref.is_empty() {
        format!("{}:{}", prefix, prompt_hash(seed))
    } else {
        source_ref.to_owned()
    }
}

fn preferred_title(primary: &str, fallback: &str) -> String {
    truncate_with_ellipsis(&non_empty_or_default(primary, fallback), 80)
}

fn non_empty_or_default(value: &str, fallback: &str) -> String {
    if value.trim().is_empty() {
        fallback.trim().to_owned()
    } else {
        value.trim().to_owned()
    }
}

fn entity_projection(record: &BrainEntityRecord) -> String {
    format!(
        "{} {} {} {} {}",
        record.kind, record.subtype, record.title, record.content, record.scope_json
    )
}

fn fact_projection(record: &BrainFactRecord) -> String {
    format!("{} {}", record.content, record.scope_json)
}

fn task_projection(record: &BrainTaskRecord) -> String {
    format!(
        "{} {} {} {} {:?}",
        record.title, record.summary, record.status, record.priority, record.due_at
    )
}

fn build_recall_summary(
    entities: &[BrainEntityRecord],
    facts: &[BrainFactRecord],
    tasks: &[BrainTaskRecord],
) -> String {
    let mut parts = Vec::new();
    if !entities.is_empty() {
        parts.push(format!("{} entities", entities.len()));
    }
    if !facts.is_empty() {
        parts.push(format!("{} facts", facts.len()));
    }
    if !tasks.is_empty() {
        parts.push(format!("{} tasks", tasks.len()));
    }
    if parts.is_empty() {
        "no relevant brain context found".to_owned()
    } else {
        format!("matched {}", parts.join(", "))
    }
}

fn scope_matches(record_scope_json: &str, query_scope_json: &str) -> bool {
    if query_scope_json.trim().is_empty() || query_scope_json.trim() == "{}" {
        return true;
    }
    let query = serde_json::from_str::<Value>(query_scope_json).unwrap_or(Value::Null);
    let record = serde_json::from_str::<Value>(record_scope_json).unwrap_or(Value::Null);
    let (Some(query_object), Some(record_object)) = (query.as_object(), record.as_object()) else {
        return false;
    };
    query_object
        .iter()
        .all(|(key, value)| record_object.get(key) == Some(value))
}

fn parse_link_ids(links_json: &str) -> Vec<String> {
    let parsed = serde_json::from_str::<Value>(links_json).unwrap_or(Value::Null);
    match parsed {
        Value::Array(items) => items
            .into_iter()
            .filter_map(|item| match item {
                Value::String(value) => Some(value),
                Value::Object(object) => {
                    object.get("id").and_then(Value::as_str).map(str::to_owned)
                }
                _ => None,
            })
            .collect(),
        Value::Object(object) => object
            .values()
            .flat_map(|value| match value {
                Value::Array(items) => items
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_owned)
                    .collect::<Vec<_>>(),
                Value::String(value) => vec![value.to_owned()],
                _ => Vec::new(),
            })
            .collect(),
        _ => Vec::new(),
    }
}

fn extract_artifacts(content: &str) -> Vec<String> {
    let mut artifacts = content
        .split_whitespace()
        .map(|token| token.trim_matches(|character: char| ",.;:()[]{}<>\"'".contains(character)))
        .filter(|token| {
            token.starts_with("http://")
                || token.starts_with("https://")
                || token.contains('/')
                || [
                    ".rs", ".md", ".json", ".yaml", ".yml", ".toml", ".go", ".ts", ".js", ".proto",
                ]
                .iter()
                .any(|suffix| token.ends_with(suffix))
        })
        .map(str::to_owned)
        .collect::<Vec<_>>();
    artifacts.sort();
    artifacts.dedup();
    artifacts
}

fn extract_environment_mentions(content: &str) -> Vec<(String, String)> {
    let lower = content.to_ascii_lowercase();
    let mut matches = Vec::new();
    for (needle, subtype, title) in [
        (" production ", "deployment", "production"),
        (" prod ", "deployment", "prod"),
        (" staging ", "deployment", "staging"),
        (" stage ", "deployment", "stage"),
        (" development ", "deployment", "development"),
        (" dev ", "deployment", "dev"),
        (" remote ", "place", "remote"),
        (" office ", "place", "office"),
        (" home ", "place", "home"),
    ] {
        if lower.contains(needle) {
            matches.push((subtype.to_owned(), title.to_owned()));
        }
    }
    for prefix in ["project ", "team "] {
        if let Some(index) = lower.find(prefix) {
            let remainder = content[index + prefix.len()..]
                .split_whitespace()
                .take(3)
                .collect::<Vec<_>>()
                .join(" ");
            if !remainder.is_empty() {
                matches.push((
                    if prefix.trim() == "project" {
                        "project".to_owned()
                    } else {
                        "team".to_owned()
                    },
                    remainder,
                ));
            }
        }
    }
    matches.sort();
    matches.dedup();
    matches
}

fn extract_task_summary(content: &str) -> Option<String> {
    for marker in [
        "remind me to ",
        "remember to ",
        "need to ",
        "needs to ",
        "please ",
        "todo: ",
        "todo ",
        "must ",
    ] {
        if let Some(value) = extract_after_case_insensitive(content, marker) {
            return Some(truncate_with_ellipsis(&value, 180));
        }
    }

    let trimmed = content.trim();
    if !trimmed.ends_with('?')
        && ["implement", "create", "fix", "update", "build", "memorize"]
            .iter()
            .any(|verb| trimmed.to_ascii_lowercase().contains(verb))
    {
        return Some(truncate_with_ellipsis(trimmed, 180));
    }
    None
}

fn extract_assistant_commitment(content: &str) -> Option<String> {
    for marker in ["i will ", "i'll ", "next step is to ", "i can "] {
        if let Some(value) = extract_after_case_insensitive(content, marker) {
            return Some(truncate_with_ellipsis(&value, 180));
        }
    }
    None
}

fn extract_after_case_insensitive(content: &str, marker: &str) -> Option<String> {
    let lower = content.to_ascii_lowercase();
    let marker_lower = marker.to_ascii_lowercase();
    let index = lower.find(&marker_lower)?;
    let value = content[index + marker.len()..]
        .trim()
        .trim_matches(|character: char| character == '.' || character == ':')
        .to_owned();
    if value.is_empty() { None } else { Some(value) }
}

fn parse_due_at(content: &str) -> Option<DateTime<Utc>> {
    let lower = content.to_ascii_lowercase();
    if lower.contains("tomorrow") {
        Some(Utc::now() + Duration::days(1))
    } else if lower.contains("today") {
        Some(Utc::now() + Duration::hours(12))
    } else if lower.contains("next week") {
        Some(Utc::now() + Duration::days(7))
    } else {
        None
    }
}

fn task_priority_from_text(content: &str) -> String {
    let lower = content.to_ascii_lowercase();
    if lower.contains("urgent") || lower.contains("asap") {
        "urgent".to_owned()
    } else if lower.contains("high priority") || lower.contains("important") {
        "high".to_owned()
    } else if lower.contains("low priority") {
        "low".to_owned()
    } else {
        "normal".to_owned()
    }
}

fn priority_weight(priority: &str) -> f64 {
    match priority {
        "urgent" => 1.0,
        "high" => 0.8,
        "low" => 0.25,
        _ => 0.5,
    }
}

fn suggestion_reason(
    task: &BrainTaskRecord,
    stale: bool,
    due_bonus: f64,
    blocked_bonus: f64,
) -> String {
    if blocked_bonus > 0.0 {
        "task is blocked and needs attention".to_owned()
    } else if due_bonus >= 0.35 {
        "task is overdue".to_owned()
    } else if due_bonus > 0.0 {
        "task is due soon".to_owned()
    } else if stale {
        "task has gone stale".to_owned()
    } else {
        format!("{} priority task", task.priority)
    }
}

fn truncate_with_ellipsis(value: &str, max_chars: usize) -> String {
    let count = value.chars().count();
    if count <= max_chars {
        return value.to_owned();
    }
    value
        .chars()
        .take(max_chars.saturating_sub(3))
        .collect::<String>()
        + "..."
}

fn parse_timestamp(value: &str) -> Result<DateTime<Utc>> {
    Ok(DateTime::parse_from_rfc3339(value)
        .with_context(|| format!("invalid timestamp {}", value))?
        .with_timezone(&Utc))
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
    brain: BrainManager,
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
    fn new(config: AppConfig, state: StateStore, tools: ToolExecutor, brain: BrainManager) -> Self {
        let max_pending = config.orchestration.max_inflight.max(1) * 8;
        let inner = Arc::new(QueueInner {
            state,
            tools,
            brain,
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
                if outcome.success {
                    let _ = worker.brain.ingest_tool_result(
                        &ToolCall {
                            target: task.target.clone(),
                            arguments_json: task.arguments_json.clone(),
                            allow_network: false,
                            priority: task.priority.clone(),
                        },
                        &outcome,
                    );
                }
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
    use super::{
        BrainForget, BrainManager, BrainRemember, BrainSuggestQuery, EncryptionManager,
        QueueBuckets, StateStore, ToolCall, ToolOutcome, fingerprint, similarity,
    };
    use openpinch_common::{AppConfig, MessageEnvelope, OpenPinchPaths, QueuePriority};
    use std::fs;
    use uuid::Uuid;

    fn test_paths() -> OpenPinchPaths {
        let root = std::env::temp_dir().join(format!("openpinch-brain-test-{}", Uuid::new_v4()));
        let paths = OpenPinchPaths {
            config_dir: root.join("config"),
            data_dir: root.join("data"),
            state_dir: root.join("state"),
            config_file: root.join("config").join("config.toml"),
            runtime_dir: root.join("state").join("runtime"),
            log_dir: root.join("state").join("logs"),
            log_file: root.join("state").join("logs").join("openpinch.log"),
            database_file: root.join("data").join("openpinch.sqlite"),
            skills_dir: root.join("data").join("skills"),
            installs_dir: root.join("data").join("skills").join("installed"),
            runtime_socket: root.join("state").join("runtime").join("engine.sock"),
            runtime_state_file: root
                .join("state")
                .join("runtime")
                .join("runtime-state.json"),
        };
        paths.ensure_all().expect("create test paths");
        fs::write(&paths.config_file, "").expect("write config placeholder");
        paths
    }

    fn test_brain() -> BrainManager {
        let paths = test_paths();
        let config = AppConfig::default();
        let state = StateStore::open(&paths.database_file).expect("open state");
        let encryption = EncryptionManager::load_or_init(&config, &paths).expect("encryption");
        BrainManager::new(&config, &paths, state, encryption).expect("brain manager")
    }

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

    #[test]
    fn brain_remember_recall_and_forget_round_trip() {
        let brain = test_brain();
        let remembered = brain
            .remember(BrainRemember {
                kind: "person".to_owned(),
                subtype: "contact".to_owned(),
                title: "Alice".to_owned(),
                content: "Alice prefers status updates about deployments".to_owned(),
                importance: 0.9,
                scope_json: "{\"connector\":\"telegram\"}".to_owned(),
                links_json: "[]".to_owned(),
                source_ref: "remember:alice".to_owned(),
            })
            .expect("remember entity");
        assert!(remembered.entity.is_some());

        let task = brain
            .remember(BrainRemember {
                kind: "task".to_owned(),
                subtype: String::new(),
                title: "Deployment follow-up".to_owned(),
                content: "Need to update Alice after the deployment".to_owned(),
                importance: 0.95,
                scope_json: "{\"connector\":\"telegram\"}".to_owned(),
                links_json: serde_json::to_string(&vec![remembered.entity.unwrap().id])
                    .expect("encode links"),
                source_ref: "remember:task".to_owned(),
            })
            .expect("remember task");
        assert!(task.task.is_some());

        let recall = brain
            .recall(openpinch_common::BrainRecallQuery {
                query: "deployment Alice".to_owned(),
                scope_json: "{\"connector\":\"telegram\"}".to_owned(),
                limit: 5,
                include_archived: false,
            })
            .expect("recall");
        assert!(!recall.entities.is_empty());
        assert!(!recall.tasks.is_empty());

        let forgotten = brain
            .forget(BrainForget {
                target_kind: "task".to_owned(),
                target_id: task.task.expect("task record").id,
                mode: "archive".to_owned(),
                reason: "completed".to_owned(),
            })
            .expect("forget");
        assert!(forgotten.forgotten);
    }

    #[test]
    fn brain_message_ingest_is_idempotent_for_same_contact_and_task() {
        let brain = test_brain();
        let message = MessageEnvelope {
            connector: "telegram".to_owned(),
            channel_id: "chan-1".to_owned(),
            sender: "peshala".to_owned(),
            body: "Please create the OpenPinch brain task tracker".to_owned(),
            metadata_json: "{}".to_owned(),
        };

        brain.ingest_message(&message).expect("ingest once");
        brain.ingest_message(&message).expect("ingest twice");

        let tasks = brain.list_tasks(false).expect("list tasks");
        let matching = tasks
            .iter()
            .filter(|task| task.summary.contains("OpenPinch brain task tracker"))
            .count();
        assert_eq!(matching, 1);

        let entities = brain.list_entities(false).expect("list entities");
        assert!(entities.iter().any(|entity| entity.subtype == "owner"));
        assert!(entities.iter().any(|entity| entity.title == "peshala"));
    }

    #[test]
    fn brain_suggestions_prioritize_open_tasks_and_tool_results_close_matching_work() {
        let brain = test_brain();
        let message = MessageEnvelope {
            connector: "telegram".to_owned(),
            channel_id: "chan-2".to_owned(),
            sender: "ops".to_owned(),
            body: "Need to run builtin.echo for the incident today".to_owned(),
            metadata_json: "{}".to_owned(),
        };
        brain.ingest_message(&message).expect("ingest task");

        let suggestions = brain
            .suggest(BrainSuggestQuery {
                scope_json:
                    "{\"connector\":\"telegram\",\"channel_id\":\"chan-2\",\"sender\":\"ops\"}"
                        .to_owned(),
                limit: 3,
            })
            .expect("suggest");
        assert!(!suggestions.suggestions.is_empty());

        brain
            .ingest_tool_result(
                &ToolCall {
                    target: "builtin.echo".to_owned(),
                    arguments_json: "{}".to_owned(),
                    allow_network: false,
                    priority: QueuePriority::Interactive,
                },
                &ToolOutcome {
                    success: true,
                    summary: "builtin.echo completed".to_owned(),
                    data_json: "{}".to_owned(),
                    error: String::new(),
                    logs: vec![],
                },
            )
            .expect("ingest tool result");

        let tasks = brain.list_tasks(false).expect("list tasks");
        assert!(
            tasks
                .iter()
                .any(|task| { task.summary.contains("builtin.echo") && task.status == "done" })
        );
    }
}
