pub mod backends;

use anyhow::Result;
use async_trait::async_trait;
use backends::{
    linux::LinuxFirecrackerBackend, macos::MacOsVirtualizationBackend,
    windows::WindowsHyperVBackend,
};
use openpinch_common::{OpenPinchPaths, SandboxConfig};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxCommand {
    pub program: String,
    pub args: Vec<String>,
    pub env: Vec<(String, String)>,
    pub allow_network: bool,
    pub workspace_archive_b64: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxOutput {
    pub success: bool,
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
    pub logs: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct SandboxHealth {
    pub backend: String,
    pub missing_prerequisites: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct SandboxPaths {
    pub workspace_dir: PathBuf,
}

#[async_trait]
pub trait SandboxBackend: Send + Sync {
    fn name(&self) -> &str;
    fn health(&self) -> SandboxHealth;
    async fn execute(&self, command: SandboxCommand) -> Result<SandboxOutput>;
}

#[derive(Clone)]
pub struct SandboxManager {
    backend: Arc<dyn SandboxBackend>,
}

impl SandboxManager {
    pub fn from_config(config: &SandboxConfig, paths: &OpenPinchPaths) -> Result<Self> {
        let sandbox_paths = SandboxPaths {
            workspace_dir: paths.runtime_dir.join("sandbox"),
        };

        let backend: Arc<dyn SandboxBackend> = if cfg!(target_os = "linux") {
            Arc::new(LinuxFirecrackerBackend::new(config.clone(), sandbox_paths)?)
        } else if cfg!(target_os = "macos") {
            Arc::new(MacOsVirtualizationBackend::new(
                config.clone(),
                sandbox_paths,
            ))
        } else {
            Arc::new(WindowsHyperVBackend::new(config.clone(), sandbox_paths))
        };

        Ok(Self { backend })
    }

    pub fn health(&self) -> SandboxHealth {
        self.backend.health()
    }

    pub async fn execute(&self, command: SandboxCommand) -> Result<SandboxOutput> {
        self.backend.execute(command).await
    }
}
