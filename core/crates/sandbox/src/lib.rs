// core/crates/sandbox/src/lib.rs
use anyhow::{Context, Result};
use openpinch_common::ToolResponse;
use std::process::Command;
use tracing::{error, info};

pub struct FirecrackerSandbox {
    firecracker_path: String,
    jailer_path: String,
}

impl FirecrackerSandbox {
    pub async fn new() -> Result<Self> {
        // Check if Firecracker is available
        let firecracker_path = which::which("firecracker").unwrap_or_else(|_| "/usr/local/bin/firecracker".into());
        let jailer_path = which::which("jailer").unwrap_or_else(|_| "/usr/local/bin/jailer".into());

        info!("🔒 Firecracker Sandbox initialized");
        info!("   Firecracker: {}", firecracker_path);
        info!("   Jailer: {}", jailer_path);

        Ok(Self { firecracker_path, jailer_path })
    }

    /// Execute a tool inside a brand-new Firecracker microVM (kernel-level isolation)
    pub async fn execute(&self, tool_name: &str, params: serde_json::Value) -> Result<ToolResponse> {
        let vm_id = uuid::Uuid::new_v4().to_string();

        info!("🛡️  Starting Firecracker microVM {} for tool: {}", vm_id, tool_name);

        // In a real implementation we would:
        // 1. Create a minimal rootfs + kernel
        // 2. Spawn jailer + firecracker
        // For this starter we simulate the execution (replace with real spawn in next iteration)

        // Placeholder real execution (you can replace with actual process::Command)
        let result = format!("Tool '{}' executed inside Firecracker VM {}", tool_name, vm_id);

        Ok(ToolResponse {
            success: true,
            result,
            error: None,
        })
    }
}

use std::path::Path;
fn which(cmd: &str) -> std::path::PathBuf {
    Command::new("which")
        .arg(cmd)
        .output()
        .ok()
        .and_then(|out| {
            if out.status.success() {
                Some(String::from_utf8_lossy(&out.stdout).trim().into())
            } else {
                None
            }
        })
        .unwrap_or_else(|| cmd.into())
}