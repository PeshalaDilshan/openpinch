use anyhow::{Context, Result};
use directories::ProjectDirs;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct OpenPinchPaths {
    pub config_dir: PathBuf,
    pub data_dir: PathBuf,
    pub state_dir: PathBuf,
    pub config_file: PathBuf,
    pub runtime_dir: PathBuf,
    pub log_dir: PathBuf,
    pub log_file: PathBuf,
    pub database_file: PathBuf,
    pub skills_dir: PathBuf,
    pub installs_dir: PathBuf,
    pub runtime_socket: PathBuf,
    pub runtime_state_file: PathBuf,
}

impl OpenPinchPaths {
    pub fn discover() -> Result<Self> {
        let project = ProjectDirs::from("dev", "openpinch", "OpenPinch")
            .context("failed to locate OS-specific project directories")?;

        let config_dir = project.config_dir().to_path_buf();
        let data_dir = project.data_dir().to_path_buf();
        let state_dir = project
            .state_dir()
            .unwrap_or_else(|| project.cache_dir())
            .to_path_buf();
        let runtime_dir = state_dir.join("runtime");
        let log_dir = state_dir.join("logs");
        let skills_dir = data_dir.join("skills");
        let installs_dir = skills_dir.join("installed");
        let runtime_socket = if cfg!(windows) {
            PathBuf::from(r"\\.\pipe\openpinch-engine")
        } else {
            runtime_dir.join("engine.sock")
        };

        Ok(Self {
            config_file: config_dir.join("config.toml"),
            database_file: data_dir.join("openpinch.sqlite"),
            log_file: log_dir.join("openpinch.log"),
            runtime_state_file: runtime_dir.join("runtime-state.json"),
            config_dir,
            data_dir,
            state_dir,
            runtime_dir,
            log_dir,
            skills_dir,
            installs_dir,
            runtime_socket,
        })
    }

    pub fn ensure_all(&self) -> Result<()> {
        for dir in [
            &self.config_dir,
            &self.data_dir,
            &self.state_dir,
            &self.runtime_dir,
            &self.log_dir,
            &self.skills_dir,
            &self.installs_dir,
        ] {
            fs::create_dir_all(dir)
                .with_context(|| format!("failed to create {}", dir.display()))?;
        }

        Ok(())
    }
}
