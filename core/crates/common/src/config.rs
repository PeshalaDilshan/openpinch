use crate::paths::OpenPinchPaths;
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default)]
    pub gateway: GatewayConfig,
    #[serde(default)]
    pub logging: LoggingConfig,
    #[serde(default)]
    pub sandbox: SandboxConfig,
    #[serde(default)]
    pub skills: SkillsConfig,
    #[serde(default = "default_models")]
    pub models: BTreeMap<String, ModelProviderConfig>,
}

impl AppConfig {
    pub fn load_or_init(paths: &OpenPinchPaths) -> Result<Self> {
        if paths.config_file.exists() {
            let raw = fs::read_to_string(&paths.config_file)
                .with_context(|| format!("failed to read {}", paths.config_file.display()))?;
            let config = toml::from_str::<Self>(&raw).context("failed to parse config.toml")?;
            Ok(config)
        } else {
            let config = Self::default();
            config.write(paths)?;
            Ok(config)
        }
    }

    pub fn write(&self, paths: &OpenPinchPaths) -> Result<()> {
        paths.ensure_all()?;
        let raw = toml::to_string_pretty(self).context("failed to serialize config")?;
        fs::write(&paths.config_file, raw)
            .with_context(|| format!("failed to write {}", paths.config_file.display()))
    }

    pub fn set(&mut self, key: &str, value: &str) -> Result<()> {
        match key {
            "gateway.listen_address" => self.gateway.listen_address = value.to_owned(),
            "gateway.binary" => self.gateway.binary = value.to_owned(),
            "gateway.engine_endpoint" => self.gateway.engine_endpoint = value.to_owned(),
            "logging.level" => self.logging.level = value.to_owned(),
            "sandbox.firecracker_path" => self.sandbox.firecracker_path = value.to_owned(),
            "sandbox.jailer_path" => self.sandbox.jailer_path = value.to_owned(),
            "sandbox.kernel_image" => self.sandbox.kernel_image = value.to_owned(),
            "sandbox.rootfs_image" => self.sandbox.rootfs_image = value.to_owned(),
            "skills.registry_index" => self.skills.registry_index = value.to_owned(),
            "skills.registry_signature" => self.skills.registry_signature = value.to_owned(),
            _ if key.starts_with("models.") => {
                let parts = key.split('.').collect::<Vec<_>>();
                if parts.len() != 3 {
                    bail!("model keys must use models.<provider>.<field>");
                }
                let provider = self.models.entry(parts[1].to_owned()).or_default();
                match parts[2] {
                    "kind" => provider.kind = value.to_owned(),
                    "endpoint" => provider.endpoint = value.to_owned(),
                    "model" => provider.model = value.to_owned(),
                    "enabled" => {
                        provider.enabled = value.parse().context("expected bool for enabled")?
                    }
                    _ => bail!("unsupported model field {}", parts[2]),
                }
            }
            _ => bail!("unsupported config key {}", key),
        }

        Ok(())
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            gateway: GatewayConfig::default(),
            logging: LoggingConfig::default(),
            sandbox: SandboxConfig::default(),
            skills: SkillsConfig::default(),
            models: default_models(),
        }
    }
}

fn default_models() -> BTreeMap<String, ModelProviderConfig> {
    let mut models = BTreeMap::new();
    models.insert(
        "ollama".to_owned(),
        ModelProviderConfig {
            kind: "ollama".to_owned(),
            endpoint: "http://127.0.0.1:11434".to_owned(),
            model: "qwen2.5:7b".to_owned(),
            enabled: true,
        },
    );
    models.insert(
        "llamacpp".to_owned(),
        ModelProviderConfig {
            kind: "llamacpp".to_owned(),
            endpoint: "http://127.0.0.1:8080".to_owned(),
            model: "local".to_owned(),
            enabled: false,
        },
    );
    models.insert(
        "localai".to_owned(),
        ModelProviderConfig {
            kind: "openai-compatible".to_owned(),
            endpoint: "http://127.0.0.1:1234/v1".to_owned(),
            model: "local-model".to_owned(),
            enabled: false,
        },
    );
    models
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayConfig {
    #[serde(default = "default_gateway_listen")]
    pub listen_address: String,
    #[serde(default = "default_gateway_binary")]
    pub binary: String,
    #[serde(default)]
    pub engine_endpoint: String,
    #[serde(default)]
    pub telegram_bot_token: String,
    #[serde(default)]
    pub telegram_poll_interval_seconds: u64,
}

impl Default for GatewayConfig {
    fn default() -> Self {
        Self {
            listen_address: default_gateway_listen(),
            binary: default_gateway_binary(),
            engine_endpoint: String::new(),
            telegram_bot_token: String::new(),
            telegram_poll_interval_seconds: 5,
        }
    }
}

fn default_gateway_listen() -> String {
    "127.0.0.1:50051".to_owned()
}

fn default_gateway_binary() -> String {
    "./bin/openpinch-gateway".to_owned()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    #[serde(default = "default_log_level")]
    pub level: String,
    #[serde(default)]
    pub json: bool,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: default_log_level(),
            json: false,
        }
    }
}

fn default_log_level() -> String {
    "info".to_owned()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxConfig {
    #[serde(default = "default_firecracker_path")]
    pub firecracker_path: String,
    #[serde(default = "default_jailer_path")]
    pub jailer_path: String,
    #[serde(default)]
    pub kernel_image: String,
    #[serde(default)]
    pub rootfs_image: String,
    #[serde(default)]
    pub seccomp_profile: String,
    #[serde(default)]
    pub host_network_cidr: String,
}

impl Default for SandboxConfig {
    fn default() -> Self {
        Self {
            firecracker_path: default_firecracker_path(),
            jailer_path: default_jailer_path(),
            kernel_image: String::new(),
            rootfs_image: String::new(),
            seccomp_profile: String::new(),
            host_network_cidr: "172.16.0.0/30".to_owned(),
        }
    }
}

fn default_firecracker_path() -> String {
    "firecracker".to_owned()
}

fn default_jailer_path() -> String {
    "jailer".to_owned()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillsConfig {
    #[serde(default = "default_registry_index")]
    pub registry_index: String,
    #[serde(default = "default_registry_signature")]
    pub registry_signature: String,
    #[serde(default = "default_trust_root")]
    pub trust_root: String,
}

impl Default for SkillsConfig {
    fn default() -> Self {
        Self {
            registry_index: default_registry_index(),
            registry_signature: default_registry_signature(),
            trust_root: default_trust_root(),
        }
    }
}

fn default_registry_index() -> String {
    "skills/registry/index.json".to_owned()
}

fn default_registry_signature() -> String {
    "skills/registry/index.sig.json".to_owned()
}

fn default_trust_root() -> String {
    "skills/trust/root.json".to_owned()
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ModelProviderConfig {
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub endpoint: String,
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub enabled: bool,
}
