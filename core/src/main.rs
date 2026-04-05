// core/src/main.rs
use anyhow::Result;
use openpinch_engine::Engine;
use openpinch_sandbox::FirecrackerSandbox;
use openpinch_tools::ToolExecutor;
use std::sync::Arc;
use tokio::signal;
use tracing::{info, warn};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("openpinch=info")
        .init();

    info!("🚀 OpenPinch Rust Core v0.1.0 starting...");

    // Initialize components
    let sandbox = Arc::new(FirecrackerSandbox::new().await?);
    let tools = ToolExecutor::new(sandbox.clone());
    let mut engine = Engine::new(tools);

    info!("✅ Core components initialized (Rust engine + Firecracker sandbox)");

    // TODO: Connect to Go gateway via gRPC (will be added in next step)
    // For now we run the engine in background
    tokio::spawn(async move {
        if let Err(e) = engine.run().await {
            warn!("Engine stopped with error: {}", e);
        }
    });

    info!("🎉 OpenPinch Core is fully running and ready!");
    info!("Press Ctrl+C to stop.");

    signal::ctrl_c().await?;
    info!("Shutting down gracefully...");
    Ok(())
}