mod providers;

use anyhow::{Context, Result, bail};
use chrono::{DateTime, Utc};
use openpinch_common::openpinch::engine_runtime_service_server::{
    EngineRuntimeService, EngineRuntimeServiceServer,
};
use openpinch_common::openpinch::{
    Empty, EngineMessageRequest, EngineSkillRequest, EngineToolRequest, ExecuteResponse,
    HealthResponse, StatusResponse, SubmitMessageResponse,
};
use openpinch_common::{
    AppConfig, MessageEnvelope, OpenPinchPaths, RuntimeStatus, ScheduleRequest, ToolCall,
    ToolOutcome,
};
use openpinch_sandbox::SandboxManager;
use openpinch_tools::{SkillManager, ToolExecutor};
use parking_lot::Mutex;
use providers::ProviderRegistry;
use rusqlite::{Connection, params};
use std::path::Path;
use std::sync::Arc;
#[cfg(not(unix))]
use tokio::net::TcpListener;
#[cfg(unix)]
use tokio::net::UnixListener;
use tokio::task::JoinHandle;
#[cfg(not(unix))]
use tokio_stream::wrappers::TcpListenerStream;
#[cfg(unix)]
use tokio_stream::wrappers::UnixListenerStream;
use tokio_util::sync::CancellationToken;
use tonic::{Request, Response, Status, transport::Server};
use tracing::warn;

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
    providers: ProviderRegistry,
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
        );
        let providers = ProviderRegistry::from_config(&config.models);
        let state = StateStore::open(&paths.database_file)?;

        Ok(Self {
            inner: Arc::new(EngineInner {
                config,
                paths,
                state,
                tools,
                _skills: skills,
                providers,
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

            return Ok(EngineHandle {
                endpoint,
                join,
                shutdown,
            });
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

        match self.inner.providers.generate(&prompt).await {
            Ok(reply) => Ok(reply),
            Err(error) => Ok(format!(
                "OpenPinch received the message but no local model backend produced a reply: {error}"
            )),
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
            enabled_connectors: vec![
                "telegram".to_owned(),
                "discord".to_owned(),
                "slack".to_owned(),
                "signal".to_owned(),
                "whatsapp".to_owned(),
            ],
            available_model_backends: self.inner.providers.enabled_names(),
            uptime_seconds: (Utc::now() - self.inner.started_at).num_seconds(),
            started_at: self.inner.started_at,
            data_dir: self.inner.paths.data_dir.display().to_string(),
            log_file: self.inner.paths.log_file.display().to_string(),
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

fn internal_status(error: anyhow::Error) -> Status {
    Status::internal(error.to_string())
}

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
