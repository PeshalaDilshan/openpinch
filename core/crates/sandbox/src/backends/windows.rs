use crate::{SandboxBackend, SandboxCommand, SandboxHealth, SandboxOutput, SandboxPaths};
use anyhow::{Context, Result, bail};
use async_trait::async_trait;
use openpinch_common::SandboxConfig;
use tokio::process::Command;

#[derive(Clone)]
pub struct WindowsHyperVBackend {
    config: SandboxConfig,
    paths: SandboxPaths,
}

impl WindowsHyperVBackend {
    pub fn new(config: SandboxConfig, paths: SandboxPaths) -> Self {
        Self { config, paths }
    }
}

#[async_trait]
impl SandboxBackend for WindowsHyperVBackend {
    fn name(&self) -> &str {
        "windows-hyperv"
    }

    fn health(&self) -> SandboxHealth {
        let mut missing = Vec::new();
        if !cfg!(target_os = "windows") {
            missing.push("native Hyper-V backend requires Windows".to_owned());
        }
        if which::which("openpinch-hyperv-run").is_err() {
            missing.push("openpinch-hyperv-run helper not found on PATH".to_owned());
        }

        let _ = &self.config;
        let _ = &self.paths;

        SandboxHealth {
            backend: self.name().to_owned(),
            missing_prerequisites: missing,
        }
    }

    async fn execute(&self, command: SandboxCommand) -> Result<SandboxOutput> {
        let health = self.health();
        if !health.missing_prerequisites.is_empty() {
            bail!(
                "sandbox prerequisites missing: {}",
                health.missing_prerequisites.join(", ")
            );
        }

        let payload =
            serde_json::to_string(&command).context("failed to serialize Hyper-V payload")?;
        let output = Command::new("openpinch-hyperv-run")
            .arg(payload)
            .output()
            .await
            .context("failed to execute Hyper-V helper")?;

        Ok(SandboxOutput {
            success: output.status.success(),
            exit_code: output.status.code().unwrap_or(-1),
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            logs: vec!["sandbox backend: windows-hyperv".to_owned()],
        })
    }
}
