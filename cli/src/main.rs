use anyhow::{Context, Result, anyhow};
use clap::{Args, Parser, Subcommand};
use openpinch_common::openpinch::gateway_service_client::GatewayServiceClient;
use openpinch_common::openpinch::{
    AgentMessage, AgentProtocolRequest, AttestationRequest, AuditExportRequest,
    ConnectorStatusRequest, Empty, ExecuteRequest, MemoryQueryRequest, MemoryUpsertRequest,
    PolicyReportRequest,
};
use openpinch_common::{AppConfig, OpenPinchPaths, QueuePriority};
use openpinch_engine::{
    EngineRuntime, fetch_gateway_status, load_runtime_status, wait_for_gateway,
};
use openpinch_tools::SkillManager;
use serde_json::json;
use std::path::PathBuf;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio_util::sync::CancellationToken;
use tracing::info;

#[derive(Debug, Parser)]
#[command(name = "openpinch")]
#[command(about = "Local-first autonomous agent runtime")]
struct Cli {
    #[arg(long, default_value_t = false)]
    json: bool,
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    Start(StartCommand),
    Execute(ExecuteCommand),
    Status,
    Skill {
        #[command(subcommand)]
        command: SkillCommand,
    },
    Config {
        #[command(subcommand)]
        command: ConfigCommand,
    },
    Logs(LogCommand),
    Connector {
        #[command(subcommand)]
        command: ConnectorCommand,
    },
    Memory {
        #[command(subcommand)]
        command: MemoryCommand,
    },
    Policy {
        #[command(subcommand)]
        command: PolicyCommand,
    },
    Audit {
        #[command(subcommand)]
        command: AuditCommand,
    },
    Attest(AttestCommand),
    Agent {
        #[command(subcommand)]
        command: AgentCommand,
    },
    Rbac {
        #[command(subcommand)]
        command: RbacCommand,
    },
    Operator {
        #[command(subcommand)]
        command: OperatorCommand,
    },
}

#[derive(Debug, Args)]
struct StartCommand {
    #[arg(long, default_value_t = false)]
    foreground: bool,
}

#[derive(Debug, Args)]
struct ExecuteCommand {
    target: String,
    #[arg(long, default_value = "{}")]
    args: String,
    #[arg(long, default_value_t = false)]
    allow_network: bool,
    #[arg(long, default_value = "interactive")]
    priority: String,
}

#[derive(Debug, Subcommand)]
enum SkillCommand {
    List,
    Install {
        source: PathBuf,
        #[arg(long, default_value_t = false)]
        force: bool,
    },
    Verify {
        source: PathBuf,
    },
}

#[derive(Debug, Subcommand)]
enum ConfigCommand {
    Init,
    Show,
    Set { key: String, value: String },
}

#[derive(Debug, Args)]
struct LogCommand {
    #[arg(long, default_value_t = 50)]
    tail: usize,
    #[arg(long, default_value_t = false)]
    follow: bool,
}

#[derive(Debug, Subcommand)]
enum ConnectorCommand {
    List,
    Status { name: String },
}

#[derive(Debug, Subcommand)]
enum MemoryCommand {
    Query {
        query: String,
        #[arg(long, default_value = "default")]
        namespace: String,
        #[arg(long, default_value_t = 5)]
        limit: u32,
        #[arg(long, default_value = "{}")]
        filter: String,
    },
    Put {
        key: String,
        content: String,
        #[arg(long, default_value = "default")]
        namespace: String,
        #[arg(long, default_value = "{}")]
        metadata: String,
    },
}

#[derive(Debug, Subcommand)]
enum PolicyCommand {
    Show {
        subject: String,
        #[arg(long)]
        capability: Option<String>,
    },
}

#[derive(Debug, Subcommand)]
enum AuditCommand {
    Export {
        #[arg(long, default_value = "json")]
        sink: String,
        #[arg(long, default_value_t = 50)]
        limit: u32,
    },
}

#[derive(Debug, Args)]
struct AttestCommand {
    #[arg(long, default_value = "openpinch-session")]
    subject: String,
    #[arg(long, default_value = "local-cli")]
    nonce: String,
    #[arg(long, default_value_t = false)]
    include_hardware: bool,
}

#[derive(Debug, Subcommand)]
enum AgentCommand {
    Protocol {
        protocol_id: String,
        #[arg(long)]
        initiator: String,
        #[arg(long, default_value = "default")]
        policy_scope: String,
        #[arg(long = "message")]
        messages: Vec<String>,
    },
}

#[derive(Debug, Subcommand)]
enum RbacCommand {
    List,
}

#[derive(Debug, Subcommand)]
enum OperatorCommand {
    Status,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let paths = OpenPinchPaths::discover()?;
    paths.ensure_all()?;
    let config = AppConfig::load_or_init(&paths)?;
    init_logging(&config, &paths)?;

    match cli.command {
        Commands::Start(command) => start_runtime(config, paths, command, cli.json).await?,
        Commands::Execute(command) => execute_command(config, paths, command, cli.json).await?,
        Commands::Status => show_status(&config, &paths, cli.json).await?,
        Commands::Skill { command } => {
            handle_skill_command(config, paths, command, cli.json).await?
        }
        Commands::Config { command } => handle_config_command(config, paths, command, cli.json)?,
        Commands::Logs(command) => show_logs(&paths, command, cli.json).await?,
        Commands::Connector { command } => {
            handle_connector_command(&config, command, cli.json).await?
        }
        Commands::Memory { command } => handle_memory_command(&config, command, cli.json).await?,
        Commands::Policy { command } => handle_policy_command(&config, command, cli.json).await?,
        Commands::Audit { command } => handle_audit_command(&config, command, cli.json).await?,
        Commands::Attest(command) => handle_attest_command(&config, command, cli.json).await?,
        Commands::Agent { command } => handle_agent_command(&config, command, cli.json).await?,
        Commands::Rbac { command } => handle_rbac_command(&config, command, cli.json)?,
        Commands::Operator { command } => handle_operator_command(&config, command, cli.json)?,
    }

    Ok(())
}

async fn start_runtime(
    config: AppConfig,
    paths: OpenPinchPaths,
    command: StartCommand,
    json_output: bool,
) -> Result<()> {
    let runtime = EngineRuntime::bootstrap(config.clone(), paths.clone()).await?;
    let shutdown = CancellationToken::new();
    let handle = runtime.start_private_rpc(shutdown.clone()).await?;
    runtime.write_runtime_state(&config.gateway.listen_address, &handle.endpoint)?;

    let mut gateway = spawn_gateway(&config, &paths, &handle.endpoint).await?;
    wait_for_gateway(
        &config.gateway.listen_address,
        std::time::Duration::from_secs(5),
    )
    .await?;
    info!("OpenPinch runtime started");
    info!("engine endpoint: {}", handle.endpoint);
    info!("gateway endpoint: {}", config.gateway.listen_address);

    if !command.foreground {
        emit(
            json_output,
            json!({
                "status": "started",
                "gateway": config.gateway.listen_address,
                "runtime": handle.endpoint,
                "log_file": paths.log_file,
            }),
            format!(
                "OpenPinch started. Gateway: {}. Logs: {}",
                config.gateway.listen_address,
                paths.log_file.display()
            ),
        );
    }

    tokio::signal::ctrl_c()
        .await
        .context("failed to listen for Ctrl+C")?;
    info!("shutting down");

    if let Some(id) = gateway.id() {
        info!("terminating gateway process {}", id);
    }
    let _ = gateway.start_kill();
    let _ = gateway.wait().await;
    handle.shutdown().await?;

    Ok(())
}

async fn execute_command(
    config: AppConfig,
    paths: OpenPinchPaths,
    command: ExecuteCommand,
    json_output: bool,
) -> Result<()> {
    let gateway_endpoint = format!("http://{}", config.gateway.listen_address);
    let request = ExecuteRequest {
        target: command.target.clone(),
        arguments_json: command.args.clone(),
        allow_network: command.allow_network,
        priority: command.priority.clone(),
    };

    if let Ok(mut client) = GatewayServiceClient::connect(gateway_endpoint.clone()).await {
        let response = client
            .execute(request)
            .await
            .context("gateway execute failed")?;
        let inner = response.into_inner();
        emit_execute_response(
            &inner.summary,
            &inner.data_json,
            &inner.error,
            inner.success,
            json_output,
        );
        return Ok(());
    }

    let runtime = EngineRuntime::bootstrap(config, paths).await?;
    let result = runtime
        .execute_tool(openpinch_common::ToolCall {
            target: command.target,
            arguments_json: command.args,
            allow_network: command.allow_network,
            priority: command
                .priority
                .parse()
                .unwrap_or(QueuePriority::Interactive),
        })
        .await;
    emit_execute_response(
        &result.summary,
        &result.data_json,
        &result.error,
        result.success,
        json_output,
    );
    Ok(())
}

async fn show_status(config: &AppConfig, paths: &OpenPinchPaths, json_output: bool) -> Result<()> {
    match fetch_gateway_status(&config.gateway.listen_address).await {
        Ok(status) => {
            emit(
                json_output,
                json!({
                    "status": status.status,
                    "version": status.version,
                    "runtime": status.runtime_endpoint,
                    "gateway": status.gateway_endpoint,
                    "connectors": status.enabled_connectors,
                    "models": status.available_model_backends,
                    "vector_memory_backend": status.vector_memory_backend,
                    "encryption_state": status.encryption_state,
                    "audit_mode": status.audit_mode,
                    "attestation_state": status.attestation_state,
                    "logs": status.log_file,
                }),
                format!(
                    "status: {}\nversion: {}\nruntime: {}\ngateway: {}\nconnectors: {}\nmodels: {}\nvector memory: {}\nencryption: {}\naudit: {}\nattestation: {}\nlogs: {}",
                    status.status,
                    status.version,
                    status.runtime_endpoint,
                    status.gateway_endpoint,
                    status.enabled_connectors.join(", "),
                    status.available_model_backends.join(", "),
                    status.vector_memory_backend,
                    status.encryption_state,
                    status.audit_mode,
                    status.attestation_state,
                    status.log_file,
                ),
            );
        }
        Err(_) => {
            let cached = load_runtime_status(paths)
                .context("gateway is unavailable and no cached runtime state was found")?;
            emit(
                json_output,
                json!({
                    "status": "gateway unavailable",
                    "runtime": cached.runtime_endpoint,
                    "gateway": cached.gateway_endpoint,
                    "log_file": cached.log_file,
                    "vector_memory_backend": cached.vector_memory_backend,
                    "encryption_state": cached.encryption_state,
                    "audit_mode": cached.audit_mode,
                }),
                format!(
                    "status: gateway unavailable\nlast known runtime: {}\nlast known gateway: {}\nlog file: {}",
                    cached.runtime_endpoint, cached.gateway_endpoint, cached.log_file
                ),
            );
        }
    }

    Ok(())
}

async fn handle_skill_command(
    config: AppConfig,
    paths: OpenPinchPaths,
    command: SkillCommand,
    json_output: bool,
) -> Result<()> {
    let manager = SkillManager::new(config.skills, paths);
    match command {
        SkillCommand::List => {
            let (skills, registry_version) = manager.list()?;
            let skill_values = skills
                .iter()
                .map(|skill| {
                    json!({
                        "id": skill.id,
                        "version": skill.version,
                        "name": skill.name,
                        "description": skill.description,
                        "installed": skill.installed,
                        "verified": skill.verified,
                        "entrypoint": skill.entrypoint,
                        "language": skill.language,
                    })
                })
                .collect::<Vec<_>>();
            emit(
                json_output,
                json!({ "registry": registry_version, "skills": skill_values }),
                {
                    let mut lines = vec![format!("registry: {}", registry_version)];
                    for skill in skills {
                        lines.push(format!(
                            "{} {} [{}] installed={} verified={}",
                            skill.id,
                            skill.version,
                            skill.language,
                            skill.installed,
                            skill.verified
                        ));
                    }
                    lines.join("\n")
                },
            );
        }
        SkillCommand::Install { source, force } => {
            let skill = manager.install(&source, force)?;
            emit(
                json_output,
                json!({
                    "installed": true,
                    "skill": {
                        "id": skill.id,
                        "version": skill.version,
                        "name": skill.name,
                        "description": skill.description,
                        "installed": skill.installed,
                        "verified": skill.verified,
                        "entrypoint": skill.entrypoint,
                        "language": skill.language,
                    }
                }),
                format!("installed {}@{}", skill.id, skill.version),
            );
        }
        SkillCommand::Verify { source } => {
            let manifest = manager.verify(&source)?;
            emit(
                json_output,
                serde_json::to_value(&manifest)?,
                format!(
                    "verified {}@{} ({})",
                    manifest.id, manifest.version, manifest.entrypoint
                ),
            );
        }
    }
    Ok(())
}

fn handle_config_command(
    config: AppConfig,
    paths: OpenPinchPaths,
    command: ConfigCommand,
    json_output: bool,
) -> Result<()> {
    match command {
        ConfigCommand::Init => {
            config.write(&paths)?;
            emit(
                json_output,
                json!({ "initialized": paths.config_file }),
                format!("initialized {}", paths.config_file.display()),
            );
        }
        ConfigCommand::Show => {
            let raw = std::fs::read_to_string(&paths.config_file)
                .with_context(|| format!("failed to read {}", paths.config_file.display()))?;
            if json_output {
                let parsed =
                    toml::from_str::<toml::Value>(&raw).context("failed to parse config")?;
                emit(true, serde_json::to_value(parsed)?, String::new());
            } else {
                println!("{raw}");
            }
        }
        ConfigCommand::Set { key, value } => {
            let mut config = config;
            config.set(&key, &value)?;
            config.write(&paths)?;
            emit(
                json_output,
                json!({ "updated": key, "value": value }),
                format!("updated {}", key),
            );
        }
    }
    Ok(())
}

async fn show_logs(paths: &OpenPinchPaths, command: LogCommand, json_output: bool) -> Result<()> {
    let log_file = tokio::fs::File::open(&paths.log_file)
        .await
        .with_context(|| format!("failed to open {}", paths.log_file.display()))?;
    let reader = BufReader::new(log_file);
    let mut lines = reader.lines();
    let mut buffer = Vec::new();

    while let Some(line) = lines.next_line().await? {
        buffer.push(line);
        if buffer.len() > command.tail {
            buffer.remove(0);
        }
    }

    if json_output {
        emit(true, json!({ "lines": buffer }), String::new());
    } else {
        for line in &buffer {
            println!("{line}");
        }
    }

    if command.follow {
        loop {
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            let stream = tokio::fs::File::open(&paths.log_file).await?;
            let mut lines = BufReader::new(stream).lines();
            let mut latest = Vec::new();
            while let Some(line) = lines.next_line().await? {
                latest.push(line);
            }
            if latest.len() > buffer.len() {
                for line in latest.iter().skip(buffer.len()) {
                    println!("{line}");
                }
                buffer = latest;
            }
        }
    }

    Ok(())
}

async fn handle_connector_command(
    config: &AppConfig,
    command: ConnectorCommand,
    json_output: bool,
) -> Result<()> {
    let mut client = connect_gateway(config).await?;
    match command {
        ConnectorCommand::List => {
            let response = client.list_connectors(Empty {}).await?.into_inner();
            let connector_values = response
                .connectors
                .iter()
                .map(|connector| {
                    json!({
                        "name": connector.name,
                        "enabled": connector.enabled,
                        "implemented": connector.implemented,
                        "mode": connector.mode,
                        "health": connector.health,
                        "allowlist": connector.allowlist,
                        "details": connector.details,
                    })
                })
                .collect::<Vec<_>>();
            emit(json_output, json!({ "connectors": connector_values }), {
                let mut lines = Vec::new();
                for connector in response.connectors {
                    lines.push(format!(
                        "{} enabled={} implemented={} mode={} health={}",
                        connector.name,
                        connector.enabled,
                        connector.implemented,
                        connector.mode,
                        connector.health
                    ));
                }
                lines.join("\n")
            });
        }
        ConnectorCommand::Status { name } => {
            let response = client
                .get_connector_status(ConnectorStatusRequest { name })
                .await?
                .into_inner();
            let connector = response.connector.as_ref().cloned();
            emit(
                json_output,
                json!({
                    "connector": connector.as_ref().map(|connector| json!({
                        "name": connector.name,
                        "enabled": connector.enabled,
                        "implemented": connector.implemented,
                        "mode": connector.mode,
                        "health": connector.health,
                        "allowlist": connector.allowlist,
                        "details": connector.details,
                    }))
                }),
                format!(
                    "{} enabled={} implemented={} mode={} health={}",
                    connector
                        .as_ref()
                        .map(|c| c.name.clone())
                        .unwrap_or_default(),
                    connector.as_ref().map(|c| c.enabled).unwrap_or(false),
                    connector.as_ref().map(|c| c.implemented).unwrap_or(false),
                    connector
                        .as_ref()
                        .map(|c| c.mode.clone())
                        .unwrap_or_default(),
                    connector
                        .as_ref()
                        .map(|c| c.health.clone())
                        .unwrap_or_default(),
                ),
            );
        }
    }
    Ok(())
}

async fn handle_memory_command(
    config: &AppConfig,
    command: MemoryCommand,
    json_output: bool,
) -> Result<()> {
    let mut client = connect_gateway(config).await?;
    match command {
        MemoryCommand::Query {
            query,
            namespace,
            limit,
            filter,
        } => {
            let response = client
                .query_memory(MemoryQueryRequest {
                    namespace,
                    query,
                    limit,
                    filter_json: filter,
                })
                .await?
                .into_inner();
            let value = json!({
                "backend": response.backend,
                "records": response.records.iter().map(|record| json!({
                    "key": record.key,
                    "namespace": record.namespace,
                    "content": record.content,
                    "metadata_json": record.metadata_json,
                    "score": record.score,
                    "created_at": record.created_at,
                })).collect::<Vec<_>>(),
            });
            emit(
                json_output,
                value.clone(),
                serde_json::to_string_pretty(&value)?,
            );
        }
        MemoryCommand::Put {
            key,
            content,
            namespace,
            metadata,
        } => {
            let response = client
                .upsert_memory(MemoryUpsertRequest {
                    namespace,
                    key,
                    content,
                    metadata_json: metadata,
                })
                .await?
                .into_inner();
            let value = json!({
                "stored": response.stored,
                "backend": response.backend,
                "digest": response.digest,
            });
            emit(
                json_output,
                value.clone(),
                serde_json::to_string_pretty(&value)?,
            );
        }
    }
    Ok(())
}

async fn handle_policy_command(
    config: &AppConfig,
    command: PolicyCommand,
    json_output: bool,
) -> Result<()> {
    let mut client = connect_gateway(config).await?;
    match command {
        PolicyCommand::Show {
            subject,
            capability,
        } => {
            let response = client
                .get_policy_report(PolicyReportRequest {
                    subject,
                    capability: capability.unwrap_or_default(),
                })
                .await?
                .into_inner();
            let value = json!({
                "subject": response.subject,
                "allowed_capabilities": response.allowed_capabilities,
                "denied_capabilities": response.denied_capabilities,
                "source": response.source,
            });
            emit(
                json_output,
                value.clone(),
                serde_json::to_string_pretty(&value)?,
            );
        }
    }
    Ok(())
}

async fn handle_audit_command(
    config: &AppConfig,
    command: AuditCommand,
    json_output: bool,
) -> Result<()> {
    let mut client = connect_gateway(config).await?;
    match command {
        AuditCommand::Export { sink, limit } => {
            let response = client
                .export_audit(AuditExportRequest { sink, limit })
                .await?
                .into_inner();
            let value = json!({
                "exported": response.exported,
                "format": response.format,
                "events": response.events.iter().map(|event| json!({
                    "id": event.id,
                    "category": event.category,
                    "severity": event.severity,
                    "summary": event.summary,
                    "anomaly_score": event.anomaly_score,
                    "payload_json": event.payload_json,
                    "created_at": event.created_at,
                })).collect::<Vec<_>>(),
            });
            emit(
                json_output,
                value.clone(),
                serde_json::to_string_pretty(&value)?,
            );
        }
    }
    Ok(())
}

async fn handle_attest_command(
    config: &AppConfig,
    command: AttestCommand,
    json_output: bool,
) -> Result<()> {
    let mut client = connect_gateway(config).await?;
    let response = client
        .attest_session(AttestationRequest {
            subject: command.subject,
            nonce: command.nonce,
            include_hardware: command.include_hardware,
        })
        .await?
        .into_inner();
    let value = json!({
        "status": response.status,
        "subject": response.subject,
        "platform": response.platform,
        "hardware_backed": response.hardware_backed,
        "nonce": response.nonce,
        "public_key": response.public_key,
        "measurements": response.measurements,
    });
    emit(
        json_output,
        value.clone(),
        serde_json::to_string_pretty(&value)?,
    );
    Ok(())
}

async fn handle_agent_command(
    config: &AppConfig,
    command: AgentCommand,
    json_output: bool,
) -> Result<()> {
    let mut client = connect_gateway(config).await?;
    match command {
        AgentCommand::Protocol {
            protocol_id,
            initiator,
            policy_scope,
            messages,
        } => {
            let parsed_messages = messages
                .into_iter()
                .map(parse_agent_message)
                .collect::<Result<Vec<_>>>()?;
            let response = client
                .run_agent_protocol(AgentProtocolRequest {
                    protocol_id,
                    initiator,
                    messages: parsed_messages,
                    policy_scope,
                })
                .await?
                .into_inner();
            let value = json!({
                "accepted": response.accepted,
                "protocol_id": response.protocol_id,
                "transcript_json": response.transcript_json,
                "findings": response.findings,
            });
            emit(
                json_output,
                value.clone(),
                serde_json::to_string_pretty(&value)?,
            );
        }
    }
    Ok(())
}

fn handle_rbac_command(config: &AppConfig, command: RbacCommand, json_output: bool) -> Result<()> {
    match command {
        RbacCommand::List => {
            let bindings = config
                .rbac
                .role_bindings
                .iter()
                .map(|(subject, roles)| json!({ "subject": subject, "roles": roles }))
                .collect::<Vec<_>>();
            emit(
                json_output,
                json!({ "default_role": config.rbac.default_role, "bindings": bindings }),
                serde_json::to_string_pretty(&bindings)?,
            );
        }
    }
    Ok(())
}

fn handle_operator_command(
    config: &AppConfig,
    command: OperatorCommand,
    json_output: bool,
) -> Result<()> {
    match command {
        OperatorCommand::Status => emit(
            json_output,
            json!({
                "enabled": config.operator.enabled,
                "namespace": config.operator.namespace,
                "manifest_dir": "deploy/operator/config",
            }),
            format!(
                "operator enabled={} namespace={} manifest_dir=deploy/operator/config",
                config.operator.enabled, config.operator.namespace
            ),
        ),
    }
    Ok(())
}

async fn connect_gateway(
    config: &AppConfig,
) -> Result<GatewayServiceClient<tonic::transport::Channel>> {
    GatewayServiceClient::connect(format!("http://{}", config.gateway.listen_address))
        .await
        .with_context(|| {
            format!(
                "failed to connect to gateway at {}",
                config.gateway.listen_address
            )
        })
}

fn parse_agent_message(raw: String) -> Result<AgentMessage> {
    let parts = raw.splitn(3, ':').collect::<Vec<_>>();
    if parts.len() != 3 {
        return Err(anyhow!("agent messages must use sender:recipient:body"));
    }
    Ok(AgentMessage {
        sender: parts[0].to_owned(),
        recipient: parts[1].to_owned(),
        body: parts[2].to_owned(),
        metadata_json: "{}".to_owned(),
        encrypted_body: String::new(),
    })
}

async fn spawn_gateway(
    config: &AppConfig,
    paths: &OpenPinchPaths,
    engine_endpoint: &str,
) -> Result<tokio::process::Child> {
    let mut command = if PathBuf::from(&config.gateway.binary).exists() {
        Command::new(&config.gateway.binary)
    } else {
        let mut fallback = Command::new("go");
        fallback.arg("run").arg("./cmd/gateway");
        fallback.current_dir("gateway");
        fallback
    };

    let config_path = paths.config_file.display().to_string();
    command
        .env("OPENPINCH_CONFIG_PATH", config_path)
        .env("OPENPINCH_ENGINE_ENDPOINT", engine_endpoint)
        .env(
            "OPENPINCH_GATEWAY_LISTEN_ADDRESS",
            &config.gateway.listen_address,
        )
        .env("GOCACHE", std::env::current_dir()?.join(".cache/go-build"))
        .env("GOMODCACHE", std::env::current_dir()?.join(".cache/go-mod"));

    command
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .spawn()
        .context("failed to start gateway process")
}

fn init_logging(config: &AppConfig, paths: &OpenPinchPaths) -> Result<()> {
    paths.ensure_all()?;
    let file_appender = tracing_appender::rolling::never(&paths.log_dir, "openpinch.log");
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);
    let _ = Box::leak(Box::new(guard));
    let fmt = tracing_subscriber::fmt()
        .with_env_filter(config.logging.level.clone())
        .with_writer(non_blocking);

    if config.logging.json {
        fmt.json()
            .try_init()
            .map_err(|error| anyhow!("failed to initialize JSON logging: {error}"))
    } else {
        fmt.try_init()
            .map_err(|error| anyhow!("failed to initialize logging: {error}"))
    }
}

fn emit(json_output: bool, value: serde_json::Value, fallback: String) {
    if json_output {
        println!(
            "{}",
            serde_json::to_string_pretty(&value).unwrap_or_else(|_| "{}".to_owned())
        );
    } else if !fallback.is_empty() {
        println!("{fallback}");
    }
}

fn emit_execute_response(
    summary: &str,
    data_json: &str,
    error: &str,
    success: bool,
    json_output: bool,
) {
    if json_output {
        emit(
            true,
            json!({
                "success": success,
                "summary": summary,
                "data_json": data_json,
                "error": error,
            }),
            String::new(),
        );
    } else if success {
        println!("success: {summary}");
        if !data_json.is_empty() {
            println!("{data_json}");
        }
    } else {
        println!("error: {summary}");
        if !error.is_empty() {
            println!("{error}");
        }
    }
}
