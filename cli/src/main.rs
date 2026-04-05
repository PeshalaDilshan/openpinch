// cli/src/main.rs
use clap::{Parser, Subcommand};
use openpinch_common::ToolRequest;
use openpinch_engine::Engine;
use openpinch_sandbox::FirecrackerSandbox;
use openpinch_tools::ToolExecutor;
use std::sync::Arc;
use tracing::{info, warn};

#[derive(Parser)]
#[command(name = "openpinch")]
#[command(about = "OpenPinch — Faster, more secure autonomous AI agent", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the OpenPinch agent (daemon mode)
    Start,

    /// Show current agent status
    Status,

    /// Execute a tool/skill directly
    Execute {
        tool: String,
        #[arg(short, long, default_value = "{}")]
        params: String,
    },

    /// Skill management
    Skill {
        #[command(subcommand)]
        action: SkillCommands,
    },

    /// Show version
    Version,
}

#[derive(Subcommand)]
enum SkillCommands {
    List,
    // Install { name: String }, etc. can be added later
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("openpinch=info")
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Start => {
            info!("🚀 Starting OpenPinch agent...");
            let sandbox = Arc::new(FirecrackerSandbox::new().await?);
            let tools = ToolExecutor::new(sandbox.clone());
            let mut engine = Engine::new(tools);
            engine.run().await?;
        }

        Commands::Status => {
            println!("✅ OpenPinch is running");
            println!("   Version: {}", env!("CARGO_PKG_VERSION"));
            // TODO: connect to running daemon later via gRPC
        }

        Commands::Execute { tool, params } => {
            let sandbox = Arc::new(FirecrackerSandbox::new().await?);
            let tools = ToolExecutor::new(sandbox);
            let request = ToolRequest {
                tool_name: tool,
                parameters: serde_json::from_str(&params).unwrap_or_default(),
                skill_id: None,
            };
            let response = tools.execute(request).await;
            if response.success {
                println!("✅ Success: {}", response.result);
            } else {
                eprintln!("❌ Error: {}", response.error.unwrap_or_default());
            }
        }

        Commands::Skill { action } => match action {
            SkillCommands::List => {
                println!("📋 Available skills:");
                println!("   • example-web-search");
                println!("   • example-email-send");
                // TODO: load from skills/registry.json
            }
        },

        Commands::Version => {
            println!("OpenPinch CLI v{}", env!("CARGO_PKG_VERSION"));
        }
    }

    Ok(())
}