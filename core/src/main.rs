// core/src/main.rs
use tracing::info;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();
    info!("🚀 OpenPinch Rust Core starting...");

    // TODO: Connect to Go gateway via gRPC
    // TODO: Load engine, sandbox, etc.

    info!("OpenPinch Core is running. Press Ctrl+C to stop.");
    tokio::signal::ctrl_c().await?;
    Ok(())
}