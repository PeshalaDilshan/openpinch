use anyhow::{Context, Result, anyhow};
use clap::{Args, Parser, Subcommand};
use openpinch_common::openpinch::ExecuteRequest;
use openpinch_common::openpinch::gateway_service_client::GatewayServiceClient;
use openpinch_common::{AppConfig, OpenPinchPaths};
use openpinch_engine::{
    EngineRuntime, fetch_gateway_status, load_runtime_status, wait_for_gateway,
};
use openpinch_tools::SkillManager;
use std::path::PathBuf;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio_util::sync::CancellationToken;
use tracing::info;

#[derive(Debug, Parser)]
#[command(name = "openpinch")]
#[command(about = "Local-first autonomous agent runtime")]
struct Cli {
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

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let paths = OpenPinchPaths::discover()?;
    paths.ensure_all()?;
    let config = AppConfig::load_or_init(&paths)?;
    init_logging(&config, &paths)?;

    match cli.command {
        Commands::Start(command) => start_runtime(config, paths, command).await?,
        Commands::Execute(command) => execute_command(config, paths, command).await?,
        Commands::Status => show_status(&config, &paths).await?,
        Commands::Skill { command } => handle_skill_command(config, paths, command).await?,
        Commands::Config { command } => handle_config_command(config, paths, command)?,
        Commands::Logs(command) => show_logs(&paths, command).await?,
    }

    Ok(())
}

async fn start_runtime(
    config: AppConfig,
    paths: OpenPinchPaths,
    command: StartCommand,
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
        println!(
            "OpenPinch started. Gateway: {}. Logs: {}",
            config.gateway.listen_address,
            paths.log_file.display()
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
) -> Result<()> {
    let gateway_endpoint = format!("http://{}", config.gateway.listen_address);
    let request = ExecuteRequest {
        target: command.target.clone(),
        arguments_json: command.args.clone(),
        allow_network: command.allow_network,
    };

    if let Ok(mut client) = GatewayServiceClient::connect(gateway_endpoint.clone()).await {
        let response = client
            .execute(request)
            .await
            .context("gateway execute failed")?;
        let inner = response.into_inner();
        print_execute_response(
            &inner.summary,
            &inner.data_json,
            &inner.error,
            inner.success,
        );
        return Ok(());
    }

    let runtime = EngineRuntime::bootstrap(config, paths).await?;
    let result = runtime
        .execute_tool(openpinch_common::ToolCall {
            target: command.target,
            arguments_json: command.args,
            allow_network: command.allow_network,
        })
        .await;
    print_execute_response(
        &result.summary,
        &result.data_json,
        &result.error,
        result.success,
    );
    Ok(())
}

async fn show_status(config: &AppConfig, paths: &OpenPinchPaths) -> Result<()> {
    match fetch_gateway_status(&config.gateway.listen_address).await {
        Ok(status) => {
            println!("status: {}", status.status);
            println!("version: {}", status.version);
            println!("runtime: {}", status.runtime_endpoint);
            println!("gateway: {}", status.gateway_endpoint);
            println!("connectors: {}", status.enabled_connectors.join(", "));
            println!("models: {}", status.available_model_backends.join(", "));
            println!("logs: {}", status.log_file);
        }
        Err(_) => {
            let cached = load_runtime_status(paths)
                .context("gateway is unavailable and no cached runtime state was found")?;
            println!("status: gateway unavailable");
            println!("last known runtime: {}", cached.runtime_endpoint);
            println!("last known gateway: {}", cached.gateway_endpoint);
            println!("log file: {}", cached.log_file);
        }
    }

    Ok(())
}

async fn handle_skill_command(
    config: AppConfig,
    paths: OpenPinchPaths,
    command: SkillCommand,
) -> Result<()> {
    let manager = SkillManager::new(config.skills, paths);
    match command {
        SkillCommand::List => {
            let (skills, registry_version) = manager.list()?;
            println!("registry: {}", registry_version);
            for skill in skills {
                println!(
                    "{} {} [{}] installed={} verified={}",
                    skill.id, skill.version, skill.language, skill.installed, skill.verified
                );
            }
        }
        SkillCommand::Install { source, force } => {
            let skill = manager.install(&source, force)?;
            println!("installed {}@{}", skill.id, skill.version);
        }
        SkillCommand::Verify { source } => {
            let manifest = manager.verify(&source)?;
            println!(
                "verified {}@{} ({})",
                manifest.id, manifest.version, manifest.entrypoint
            );
        }
    }
    Ok(())
}

fn handle_config_command(
    config: AppConfig,
    paths: OpenPinchPaths,
    command: ConfigCommand,
) -> Result<()> {
    match command {
        ConfigCommand::Init => {
            config.write(&paths)?;
            println!("initialized {}", paths.config_file.display());
        }
        ConfigCommand::Show => {
            let raw = std::fs::read_to_string(&paths.config_file)
                .with_context(|| format!("failed to read {}", paths.config_file.display()))?;
            println!("{raw}");
        }
        ConfigCommand::Set { key, value } => {
            let mut config = config;
            config.set(&key, &value)?;
            config.write(&paths)?;
            println!("updated {}", key);
        }
    }
    Ok(())
}

async fn show_logs(paths: &OpenPinchPaths, command: LogCommand) -> Result<()> {
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

    for line in &buffer {
        println!("{line}");
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
            for line in latest.into_iter().skip(buffer.len()) {
                println!("{line}");
            }
        }
    }

    Ok(())
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

fn print_execute_response(summary: &str, data_json: &str, error: &str, success: bool) {
    if success {
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
