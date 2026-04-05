// core/crates/engine/src/lib.rs
use openpinch_tools::ToolExecutor;
use tracing::info;

pub struct Engine {
    tools: ToolExecutor,
}

impl Engine {
    pub fn new(tools: ToolExecutor) -> Self {
        Self { tools }
    }

    pub async fn run(&mut self) -> anyhow::Result<()> {
        info!("🧠 OpenPinch Decision Engine started");
        // TODO: Add planner, memory, autonomous loop here
        Ok(())
    }
}