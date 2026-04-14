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
    #[serde(default)]
    pub orchestration: OrchestrationConfig,
    #[serde(default)]
    pub runtime: RuntimeConfig,
    #[serde(default)]
    pub security: SecurityConfig,
    #[serde(default)]
    pub agents: AgentsConfig,
    #[serde(default)]
    pub brain: BrainConfig,
    #[serde(default)]
    pub sessions: SessionsConfig,
    #[serde(default)]
    pub routing: RoutingConfig,
    #[serde(default)]
    pub presence: PresenceConfig,
    #[serde(default)]
    pub usage: UsageConfig,
    #[serde(default)]
    pub media: MediaConfig,
    #[serde(default)]
    pub browser: BrowserConfig,
    #[serde(default)]
    pub vector_memory: VectorMemoryConfig,
    #[serde(default = "default_model_profiles")]
    pub model_profiles: BTreeMap<String, ModelProfileConfig>,
    #[serde(default)]
    pub model_failover: ModelFailoverConfig,
    #[serde(default)]
    pub rbac: RbacConfig,
    #[serde(default)]
    pub siem: SiemConfig,
    #[serde(default)]
    pub operator: OperatorConfig,
    #[serde(default = "default_connectors")]
    pub connectors: BTreeMap<String, ConnectorConfig>,
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
            "gateway.telegram_bot_token" => self.gateway.telegram_bot_token = value.to_owned(),
            "gateway.web.enabled" => {
                self.gateway.web.enabled = value
                    .parse()
                    .context("expected bool for gateway.web.enabled")?
            }
            "gateway.web.listen_address" => self.gateway.web.listen_address = value.to_owned(),
            "gateway.auth.enabled" => {
                self.gateway.auth.enabled = value
                    .parse()
                    .context("expected bool for gateway.auth.enabled")?
            }
            "gateway.remote.enabled" => {
                self.gateway.remote.enabled = value
                    .parse()
                    .context("expected bool for gateway.remote.enabled")?
            }
            "gateway.tls.enabled" => {
                self.gateway.tls.enabled = value
                    .parse()
                    .context("expected bool for gateway.tls.enabled")?
            }
            "logging.level" => self.logging.level = value.to_owned(),
            "logging.json" => {
                self.logging.json = value.parse().context("expected bool for logging.json")?
            }
            "sandbox.firecracker_path" => self.sandbox.firecracker_path = value.to_owned(),
            "sandbox.jailer_path" => self.sandbox.jailer_path = value.to_owned(),
            "sandbox.kernel_image" => self.sandbox.kernel_image = value.to_owned(),
            "sandbox.rootfs_image" => self.sandbox.rootfs_image = value.to_owned(),
            "sandbox.capabilities.matrix_path" => {
                self.sandbox.capabilities.matrix_path = value.to_owned()
            }
            "sandbox.capabilities.default_deny" => {
                self.sandbox.capabilities.default_deny = value
                    .parse()
                    .context("expected bool for sandbox.capabilities.default_deny")?
            }
            "skills.registry_index" => self.skills.registry_index = value.to_owned(),
            "skills.registry_signature" => self.skills.registry_signature = value.to_owned(),
            "orchestration.default_priority" => {
                self.orchestration.default_priority = value.to_owned()
            }
            "orchestration.max_inflight" => {
                self.orchestration.max_inflight = value
                    .parse()
                    .context("expected integer for orchestration.max_inflight")?
            }
            "security.audit.enabled" => {
                self.security.audit.enabled = value
                    .parse()
                    .context("expected bool for security.audit.enabled")?
            }
            "security.encryption.enabled" => {
                self.security.encryption.enabled = value
                    .parse()
                    .context("expected bool for security.encryption.enabled")?
            }
            "brain.enabled" => {
                self.brain.enabled = value.parse().context("expected bool for brain.enabled")?
            }
            "brain.auto_ingest_messages" => {
                self.brain.auto_ingest_messages = value
                    .parse()
                    .context("expected bool for brain.auto_ingest_messages")?
            }
            "brain.auto_ingest_tool_results" => {
                self.brain.auto_ingest_tool_results = value
                    .parse()
                    .context("expected bool for brain.auto_ingest_tool_results")?
            }
            "brain.auto_ingest_assistant_commitments" => {
                self.brain.auto_ingest_assistant_commitments = value
                    .parse()
                    .context("expected bool for brain.auto_ingest_assistant_commitments")?
            }
            "brain.inline_suggestions_in_replies" => {
                self.brain.inline_suggestions_in_replies = value
                    .parse()
                    .context("expected bool for brain.inline_suggestions_in_replies")?
            }
            "brain.max_inline_suggestions" => {
                self.brain.max_inline_suggestions = value
                    .parse()
                    .context("expected integer for brain.max_inline_suggestions")?
            }
            "brain.context_budget_chars" => {
                self.brain.context_budget_chars = value
                    .parse()
                    .context("expected integer for brain.context_budget_chars")?
            }
            "brain.archive_decay_days" => {
                self.brain.archive_decay_days = value
                    .parse()
                    .context("expected integer for brain.archive_decay_days")?
            }
            "brain.stale_task_hours" => {
                self.brain.stale_task_hours = value
                    .parse()
                    .context("expected integer for brain.stale_task_hours")?
            }
            "sessions.prune_after_hours" => {
                self.sessions.prune_after_hours = value
                    .parse()
                    .context("expected integer for sessions.prune_after_hours")?
            }
            "routing.auto_pair_dm" => {
                self.routing.auto_pair_dm = value
                    .parse()
                    .context("expected bool for routing.auto_pair_dm")?
            }
            "presence.enabled" => {
                self.presence.enabled = value
                    .parse()
                    .context("expected bool for presence.enabled")?
            }
            "usage.enabled" => {
                self.usage.enabled = value.parse().context("expected bool for usage.enabled")?
            }
            "media.max_upload_bytes" => {
                self.media.max_upload_bytes = value
                    .parse()
                    .context("expected integer for media.max_upload_bytes")?
            }
            "browser.enabled" => {
                self.browser.enabled = value.parse().context("expected bool for browser.enabled")?
            }
            "model_failover.default_profile" => {
                self.model_failover.default_profile = value.to_owned()
            }
            "vector_memory.default_namespace" => {
                self.vector_memory.default_namespace = value.to_owned()
            }
            "operator.enabled" => {
                self.operator.enabled = value
                    .parse()
                    .context("expected bool for operator.enabled")?
            }
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
                    "draft_model" => provider.draft_model = value.to_owned(),
                    "quantization" => provider.quantization = value.to_owned(),
                    "cache_policy" => provider.cache_policy = value.to_owned(),
                    "enabled" => {
                        provider.enabled = value.parse().context("expected bool for enabled")?
                    }
                    "speculative_enabled" => {
                        provider.speculative_enabled = value
                            .parse()
                            .context("expected bool for speculative_enabled")?
                    }
                    _ => bail!("unsupported model field {}", parts[2]),
                }
            }
            _ if key.starts_with("connectors.") => {
                let parts = key.split('.').collect::<Vec<_>>();
                if parts.len() != 3 {
                    bail!("connector keys must use connectors.<name>.<field>");
                }
                let connector = self.connectors.entry(parts[1].to_owned()).or_default();
                match parts[2] {
                    "enabled" => {
                        connector.enabled = value.parse().context("expected bool for enabled")?
                    }
                    "mode" => connector.mode = value.to_owned(),
                    "endpoint" => connector.endpoint = value.to_owned(),
                    _ => bail!("unsupported connector field {}", parts[2]),
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
            orchestration: OrchestrationConfig::default(),
            runtime: RuntimeConfig::default(),
            security: SecurityConfig::default(),
            agents: AgentsConfig::default(),
            brain: BrainConfig::default(),
            sessions: SessionsConfig::default(),
            routing: RoutingConfig::default(),
            presence: PresenceConfig::default(),
            usage: UsageConfig::default(),
            media: MediaConfig::default(),
            browser: BrowserConfig::default(),
            vector_memory: VectorMemoryConfig::default(),
            model_profiles: default_model_profiles(),
            model_failover: ModelFailoverConfig::default(),
            rbac: RbacConfig::default(),
            siem: SiemConfig::default(),
            operator: OperatorConfig::default(),
            connectors: default_connectors(),
            models: default_models(),
        }
    }
}

fn default_connectors() -> BTreeMap<String, ConnectorConfig> {
    let mut connectors = BTreeMap::new();
    for name in [
        "telegram",
        "discord",
        "slack",
        "whatsapp",
        "signal",
        "matrix",
        "xmpp",
        "irc",
        "mattermost",
        "microsoft-teams",
        "rocketchat",
        "zulip",
        "google-chat",
        "webex",
        "line",
        "viber",
        "smtp",
        "imap",
        "twilio-sms",
        "twilio-mms",
        "webchat",
        "webhook-inbound",
        "webhook-outbound",
    ] {
        connectors.insert(
            name.to_owned(),
            ConnectorConfig {
                enabled: name == "telegram" || name == "webchat",
                mode: if name == "telegram" {
                    "polling".to_owned()
                } else if name == "webchat" {
                    "web".to_owned()
                } else {
                    "disabled".to_owned()
                },
                endpoint: String::new(),
                allowlist: Vec::new(),
                api_first: true,
                deferred: name != "telegram" && name != "webchat",
                auth_mode: if name == "webchat" {
                    "local-session".to_owned()
                } else {
                    "token".to_owned()
                },
                pair_dm: true,
                chunk_limit: 1800,
                mention_only: name != "telegram" && name != "webchat",
            },
        );
    }
    connectors
}

fn default_models() -> BTreeMap<String, ModelProviderConfig> {
    let mut models = BTreeMap::new();
    models.insert(
        "ollama".to_owned(),
        ModelProviderConfig {
            kind: "ollama".to_owned(),
            endpoint: "http://127.0.0.1:11434".to_owned(),
            model: "qwen2.5:7b".to_owned(),
            draft_model: "qwen2.5:3b".to_owned(),
            quantization: "Q4_K_M".to_owned(),
            context_window: 8192,
            max_concurrency: 2,
            gpu_layers: 0,
            speculative_enabled: true,
            cache_policy: "exact+prefix+semantic".to_owned(),
            enabled: true,
        },
    );
    models.insert(
        "llamacpp".to_owned(),
        ModelProviderConfig {
            kind: "llamacpp".to_owned(),
            endpoint: "http://127.0.0.1:8080".to_owned(),
            model: "local".to_owned(),
            draft_model: "local-draft".to_owned(),
            quantization: "Q5_K_M".to_owned(),
            context_window: 16384,
            max_concurrency: 4,
            gpu_layers: 32,
            speculative_enabled: true,
            cache_policy: "exact+prefix+semantic".to_owned(),
            enabled: false,
        },
    );
    models.insert(
        "localai".to_owned(),
        ModelProviderConfig {
            kind: "openai-compatible".to_owned(),
            endpoint: "http://127.0.0.1:1234/v1".to_owned(),
            model: "local-model".to_owned(),
            draft_model: String::new(),
            quantization: "auto".to_owned(),
            context_window: 8192,
            max_concurrency: 2,
            gpu_layers: 0,
            speculative_enabled: false,
            cache_policy: "exact".to_owned(),
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
    #[serde(default)]
    pub tls: GatewayTlsConfig,
    #[serde(default)]
    pub web: GatewayWebConfig,
    #[serde(default)]
    pub auth: GatewayAuthConfig,
    #[serde(default)]
    pub remote: GatewayRemoteConfig,
    #[serde(default)]
    pub allowlists: BTreeMap<String, Vec<String>>,
}

impl Default for GatewayConfig {
    fn default() -> Self {
        let mut allowlists = BTreeMap::new();
        allowlists.insert("telegram".to_owned(), Vec::new());
        allowlists.insert("webhook-outbound".to_owned(), Vec::new());

        Self {
            listen_address: default_gateway_listen(),
            binary: default_gateway_binary(),
            engine_endpoint: String::new(),
            telegram_bot_token: String::new(),
            telegram_poll_interval_seconds: 5,
            tls: GatewayTlsConfig::default(),
            web: GatewayWebConfig::default(),
            auth: GatewayAuthConfig::default(),
            remote: GatewayRemoteConfig::default(),
            allowlists,
        }
    }
}

fn default_gateway_listen() -> String {
    "127.0.0.1:50051".to_owned()
}

fn default_gateway_binary() -> String {
    "./bin/openpinch-gateway".to_owned()
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GatewayTlsConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub cert_file: String,
    #[serde(default)]
    pub key_file: String,
    #[serde(default)]
    pub client_ca_file: String,
    #[serde(default)]
    pub rotate_on_start: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayWebConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_gateway_web_listen")]
    pub listen_address: String,
    #[serde(default = "default_gateway_web_ui_dir")]
    pub ui_dir: String,
    #[serde(default)]
    pub cors_origin: String,
}

impl Default for GatewayWebConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            listen_address: default_gateway_web_listen(),
            ui_dir: default_gateway_web_ui_dir(),
            cors_origin: String::new(),
        }
    }
}

fn default_gateway_web_listen() -> String {
    "127.0.0.1:8088".to_owned()
}

fn default_gateway_web_ui_dir() -> String {
    "ui/build/web".to_owned()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayAuthConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub token: String,
    #[serde(default)]
    pub password: String,
}

impl Default for GatewayAuthConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            token: String::new(),
            password: String::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayRemoteConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_remote_mode")]
    pub mode: String,
    #[serde(default)]
    pub public_base_url: String,
}

impl Default for GatewayRemoteConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            mode: default_remote_mode(),
            public_base_url: String::new(),
        }
    }
}

fn default_remote_mode() -> String {
    "disabled".to_owned()
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
    #[serde(default = "default_containerd_path")]
    pub containerd_path: String,
    #[serde(default)]
    pub kernel_image: String,
    #[serde(default)]
    pub rootfs_image: String,
    #[serde(default)]
    pub seccomp_profile: String,
    #[serde(default)]
    pub host_network_cidr: String,
    #[serde(default)]
    pub capabilities: SandboxCapabilitiesConfig,
    #[serde(default)]
    pub linux: LinuxSandboxPlatformConfig,
    #[serde(default)]
    pub macos: MacOsSandboxPlatformConfig,
    #[serde(default)]
    pub windows: WindowsSandboxPlatformConfig,
}

impl Default for SandboxConfig {
    fn default() -> Self {
        Self {
            firecracker_path: default_firecracker_path(),
            jailer_path: default_jailer_path(),
            containerd_path: default_containerd_path(),
            kernel_image: String::new(),
            rootfs_image: String::new(),
            seccomp_profile: String::new(),
            host_network_cidr: "172.16.0.0/30".to_owned(),
            capabilities: SandboxCapabilitiesConfig::default(),
            linux: LinuxSandboxPlatformConfig::default(),
            macos: MacOsSandboxPlatformConfig::default(),
            windows: WindowsSandboxPlatformConfig::default(),
        }
    }
}

fn default_firecracker_path() -> String {
    "firecracker".to_owned()
}

fn default_jailer_path() -> String {
    "jailer".to_owned()
}

fn default_containerd_path() -> String {
    "containerd".to_owned()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxCapabilitiesConfig {
    #[serde(default = "default_capability_matrix")]
    pub matrix_path: String,
    #[serde(default = "default_true")]
    pub default_deny: bool,
}

impl Default for SandboxCapabilitiesConfig {
    fn default() -> Self {
        Self {
            matrix_path: default_capability_matrix(),
            default_deny: true,
        }
    }
}

fn default_capability_matrix() -> String {
    "skills/policies/default.yaml".to_owned()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinuxSandboxPlatformConfig {
    #[serde(default = "default_true")]
    pub firecracker_enabled: bool,
    #[serde(default = "default_true")]
    pub containerd_in_guest: bool,
    #[serde(default)]
    pub tap_interface: String,
}

impl Default for LinuxSandboxPlatformConfig {
    fn default() -> Self {
        Self {
            firecracker_enabled: true,
            containerd_in_guest: true,
            tap_interface: String::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MacOsSandboxPlatformConfig {
    #[serde(default = "default_true")]
    pub virtualization_enabled: bool,
    #[serde(default)]
    pub seatbelt_profile: String,
}

impl Default for MacOsSandboxPlatformConfig {
    fn default() -> Self {
        Self {
            virtualization_enabled: true,
            seatbelt_profile: String::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowsSandboxPlatformConfig {
    #[serde(default = "default_true")]
    pub hyperv_enabled: bool,
    #[serde(default = "default_true")]
    pub job_object_restrictions: bool,
}

impl Default for WindowsSandboxPlatformConfig {
    fn default() -> Self {
        Self {
            hyperv_enabled: true,
            job_object_restrictions: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillsConfig {
    #[serde(default = "default_registry_index")]
    pub registry_index: String,
    #[serde(default = "default_registry_signature")]
    pub registry_signature: String,
    #[serde(default = "default_trust_root")]
    pub trust_root: String,
    #[serde(default = "default_skill_registry_backend")]
    pub registry_backend: String,
    #[serde(default)]
    pub decentralized_mirrors: Vec<String>,
}

impl Default for SkillsConfig {
    fn default() -> Self {
        Self {
            registry_index: default_registry_index(),
            registry_signature: default_registry_signature(),
            trust_root: default_trust_root(),
            registry_backend: default_skill_registry_backend(),
            decentralized_mirrors: vec!["ipfs://openpinch-skill-registry".to_owned()],
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

fn default_skill_registry_backend() -> String {
    "git+ipfs".to_owned()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrchestrationConfig {
    #[serde(default = "default_provider_order")]
    pub provider_order: Vec<String>,
    #[serde(default = "default_exact_cache_capacity")]
    pub exact_cache_capacity: usize,
    #[serde(default = "default_prefix_cache_capacity")]
    pub prefix_cache_capacity: usize,
    #[serde(default = "default_semantic_cache_capacity")]
    pub semantic_cache_capacity: usize,
    #[serde(default = "default_true")]
    pub semantic_cache_enabled: bool,
    #[serde(default = "default_true")]
    pub prefix_cache_enabled: bool,
    #[serde(default = "default_true")]
    pub exact_cache_enabled: bool,
    #[serde(default = "default_true")]
    pub speculative_enabled: bool,
    #[serde(default = "default_max_inflight")]
    pub max_inflight: usize,
    #[serde(default = "default_priority")]
    pub default_priority: String,
}

impl Default for OrchestrationConfig {
    fn default() -> Self {
        Self {
            provider_order: default_provider_order(),
            exact_cache_capacity: default_exact_cache_capacity(),
            prefix_cache_capacity: default_prefix_cache_capacity(),
            semantic_cache_capacity: default_semantic_cache_capacity(),
            semantic_cache_enabled: true,
            prefix_cache_enabled: true,
            exact_cache_enabled: true,
            speculative_enabled: true,
            max_inflight: default_max_inflight(),
            default_priority: default_priority(),
        }
    }
}

fn default_provider_order() -> Vec<String> {
    vec![
        "llamacpp".to_owned(),
        "ollama".to_owned(),
        "localai".to_owned(),
    ]
}

fn default_exact_cache_capacity() -> usize {
    2048
}

fn default_prefix_cache_capacity() -> usize {
    1024
}

fn default_semantic_cache_capacity() -> usize {
    4096
}

fn default_max_inflight() -> usize {
    8
}

fn default_priority() -> String {
    "interactive".to_owned()
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RuntimeConfig {
    #[serde(default)]
    pub queues: QueueRuntimeConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueRuntimeConfig {
    #[serde(default = "default_interactive_weight")]
    pub interactive_weight: u32,
    #[serde(default = "default_connector_weight")]
    pub connector_weight: u32,
    #[serde(default = "default_autonomy_weight")]
    pub autonomy_weight: u32,
    #[serde(default = "default_background_weight")]
    pub background_weight: u32,
    #[serde(default = "default_cpu_budget")]
    pub cpu_budget_percent: u32,
    #[serde(default = "default_memory_budget")]
    pub memory_budget_mb: u64,
    #[serde(default = "default_token_budget")]
    pub token_budget_per_minute: u64,
}

impl Default for QueueRuntimeConfig {
    fn default() -> Self {
        Self {
            interactive_weight: default_interactive_weight(),
            connector_weight: default_connector_weight(),
            autonomy_weight: default_autonomy_weight(),
            background_weight: default_background_weight(),
            cpu_budget_percent: default_cpu_budget(),
            memory_budget_mb: default_memory_budget(),
            token_budget_per_minute: default_token_budget(),
        }
    }
}

fn default_interactive_weight() -> u32 {
    100
}

fn default_connector_weight() -> u32 {
    80
}

fn default_autonomy_weight() -> u32 {
    40
}

fn default_background_weight() -> u32 {
    20
}

fn default_cpu_budget() -> u32 {
    60
}

fn default_memory_budget() -> u64 {
    4096
}

fn default_token_budget() -> u64 {
    200_000
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SecurityConfig {
    #[serde(default)]
    pub encryption: EncryptionConfig,
    #[serde(default)]
    pub attestation: AttestationConfig,
    #[serde(default)]
    pub audit: AuditConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptionConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub key_file: String,
    #[serde(default = "default_true")]
    pub encrypt_memory: bool,
    #[serde(default = "default_true")]
    pub encrypt_agent_channels: bool,
}

impl Default for EncryptionConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            key_file: String::new(),
            encrypt_memory: true,
            encrypt_agent_channels: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttestationConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub require_hardware: bool,
    #[serde(default)]
    pub tpm_device: String,
}

impl Default for AttestationConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            require_hardware: false,
            tpm_device: "/dev/tpmrm0".to_owned(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_audit_mode")]
    pub mode: String,
    #[serde(default)]
    pub ebpf_enabled: bool,
    #[serde(default = "default_audit_threshold")]
    pub anomaly_threshold: f64,
}

impl Default for AuditConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            mode: default_audit_mode(),
            ebpf_enabled: false,
            anomaly_threshold: default_audit_threshold(),
        }
    }
}

fn default_audit_mode() -> String {
    "local-buffered".to_owned()
}

fn default_audit_threshold() -> f64 {
    0.85
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentsConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_max_agents")]
    pub max_agents: usize,
    #[serde(default = "default_protocol_dir")]
    pub protocol_dir: String,
}

impl Default for AgentsConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_agents: default_max_agents(),
            protocol_dir: default_protocol_dir(),
        }
    }
}

fn default_max_agents() -> usize {
    8
}

fn default_protocol_dir() -> String {
    "docs/formal".to_owned()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrainConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_true")]
    pub auto_ingest_messages: bool,
    #[serde(default = "default_true")]
    pub auto_ingest_tool_results: bool,
    #[serde(default = "default_true")]
    pub auto_ingest_assistant_commitments: bool,
    #[serde(default = "default_true")]
    pub inline_suggestions_in_replies: bool,
    #[serde(default = "default_brain_inline_suggestions")]
    pub max_inline_suggestions: usize,
    #[serde(default = "default_brain_context_budget")]
    pub context_budget_chars: usize,
    #[serde(default = "default_brain_archive_decay_days")]
    pub archive_decay_days: i64,
    #[serde(default = "default_brain_stale_task_hours")]
    pub stale_task_hours: i64,
}

impl Default for BrainConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            auto_ingest_messages: true,
            auto_ingest_tool_results: true,
            auto_ingest_assistant_commitments: true,
            inline_suggestions_in_replies: true,
            max_inline_suggestions: default_brain_inline_suggestions(),
            context_budget_chars: default_brain_context_budget(),
            archive_decay_days: default_brain_archive_decay_days(),
            stale_task_hours: default_brain_stale_task_hours(),
        }
    }
}

fn default_brain_inline_suggestions() -> usize {
    2
}

fn default_brain_context_budget() -> usize {
    2_000
}

fn default_brain_archive_decay_days() -> i64 {
    30
}

fn default_brain_stale_task_hours() -> i64 {
    24
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionsConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_session_reply_mode")]
    pub default_reply_mode: String,
    #[serde(default = "default_session_queue_mode")]
    pub default_queue_mode: String,
    #[serde(default = "default_sessions_prune_hours")]
    pub prune_after_hours: u32,
}

impl Default for SessionsConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            default_reply_mode: default_session_reply_mode(),
            default_queue_mode: default_session_queue_mode(),
            prune_after_hours: default_sessions_prune_hours(),
        }
    }
}

fn default_session_reply_mode() -> String {
    "reply-back".to_owned()
}

fn default_session_queue_mode() -> String {
    "connector".to_owned()
}

fn default_sessions_prune_hours() -> u32 {
    24 * 14
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingConfig {
    #[serde(default = "default_true")]
    pub auto_pair_dm: bool,
    #[serde(default = "default_true")]
    pub mention_only_in_groups: bool,
    #[serde(default = "default_routing_mentions")]
    pub mention_names: Vec<String>,
}

impl Default for RoutingConfig {
    fn default() -> Self {
        Self {
            auto_pair_dm: true,
            mention_only_in_groups: true,
            mention_names: default_routing_mentions(),
        }
    }
}

fn default_routing_mentions() -> Vec<String> {
    vec!["openpinch".to_owned()]
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PresenceConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_true")]
    pub typing_events: bool,
}

impl Default for PresenceConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            typing_events: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
}

impl Default for UsageConfig {
    fn default() -> Self {
        Self { enabled: true }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_media_max_upload_bytes")]
    pub max_upload_bytes: u64,
    #[serde(default)]
    pub temp_dir: String,
}

impl Default for MediaConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_upload_bytes: default_media_max_upload_bytes(),
            temp_dir: String::new(),
        }
    }
}

fn default_media_max_upload_bytes() -> u64 {
    20 * 1024 * 1024
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub profile_dir: String,
}

impl Default for BrowserConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            profile_dir: String::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorMemoryConfig {
    #[serde(default = "default_vector_backend")]
    pub requested_backend: String,
    #[serde(default = "default_vector_namespace")]
    pub default_namespace: String,
    #[serde(default)]
    pub lancedb_uri: String,
    #[serde(default = "default_memory_dimension")]
    pub embedding_dimensions: usize,
}

impl Default for VectorMemoryConfig {
    fn default() -> Self {
        Self {
            requested_backend: default_vector_backend(),
            default_namespace: default_vector_namespace(),
            lancedb_uri: String::new(),
            embedding_dimensions: default_memory_dimension(),
        }
    }
}

fn default_vector_backend() -> String {
    "lancedb".to_owned()
}

fn default_vector_namespace() -> String {
    "default".to_owned()
}

fn default_memory_dimension() -> usize {
    128
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RbacConfig {
    #[serde(default = "default_default_role")]
    pub default_role: String,
    #[serde(default = "default_role_bindings")]
    pub role_bindings: BTreeMap<String, Vec<String>>,
}

impl Default for RbacConfig {
    fn default() -> Self {
        Self {
            default_role: default_default_role(),
            role_bindings: default_role_bindings(),
        }
    }
}

fn default_default_role() -> String {
    "admin".to_owned()
}

fn default_role_bindings() -> BTreeMap<String, Vec<String>> {
    let mut bindings = BTreeMap::new();
    bindings.insert("local-user".to_owned(), vec!["admin".to_owned()]);
    bindings
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SiemConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub ocsf_file: String,
    #[serde(default)]
    pub syslog_endpoint: String,
    #[serde(default)]
    pub https_batch_endpoint: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperatorConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_operator_namespace")]
    pub namespace: String,
}

impl Default for OperatorConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            namespace: default_operator_namespace(),
        }
    }
}

fn default_operator_namespace() -> String {
    "openpinch-system".to_owned()
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ConnectorConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub mode: String,
    #[serde(default)]
    pub endpoint: String,
    #[serde(default)]
    pub allowlist: Vec<String>,
    #[serde(default = "default_true")]
    pub api_first: bool,
    #[serde(default)]
    pub deferred: bool,
    #[serde(default)]
    pub auth_mode: String,
    #[serde(default = "default_true")]
    pub pair_dm: bool,
    #[serde(default = "default_chunk_limit")]
    pub chunk_limit: usize,
    #[serde(default)]
    pub mention_only: bool,
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
    pub draft_model: String,
    #[serde(default)]
    pub quantization: String,
    #[serde(default)]
    pub context_window: usize,
    #[serde(default)]
    pub max_concurrency: usize,
    #[serde(default)]
    pub gpu_layers: usize,
    #[serde(default)]
    pub speculative_enabled: bool,
    #[serde(default)]
    pub cache_policy: String,
    #[serde(default)]
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelProfileConfig {
    #[serde(default)]
    pub provider_order: Vec<String>,
    #[serde(default = "default_model_profile_mode")]
    pub mode: String,
    #[serde(default = "default_model_profile_timeout_seconds")]
    pub timeout_seconds: u32,
    #[serde(default = "default_model_profile_retry_budget")]
    pub retry_budget: u32,
    #[serde(default)]
    pub hosted: bool,
    #[serde(default)]
    pub auth_mode: String,
}

impl Default for ModelProfileConfig {
    fn default() -> Self {
        Self {
            provider_order: default_provider_order(),
            mode: default_model_profile_mode(),
            timeout_seconds: default_model_profile_timeout_seconds(),
            retry_budget: default_model_profile_retry_budget(),
            hosted: false,
            auth_mode: String::new(),
        }
    }
}

fn default_model_profiles() -> BTreeMap<String, ModelProfileConfig> {
    let mut profiles = BTreeMap::new();
    profiles.insert(
        "default".to_owned(),
        ModelProfileConfig {
            provider_order: default_provider_order(),
            mode: "local-first".to_owned(),
            timeout_seconds: default_model_profile_timeout_seconds(),
            retry_budget: 2,
            hosted: false,
            auth_mode: String::new(),
        },
    );
    profiles.insert(
        "hosted-fallback".to_owned(),
        ModelProfileConfig {
            provider_order: default_provider_order(),
            mode: "hybrid-failover".to_owned(),
            timeout_seconds: 30,
            retry_budget: 3,
            hosted: true,
            auth_mode: "api-key".to_owned(),
        },
    );
    profiles
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelFailoverConfig {
    #[serde(default = "default_model_profile_name")]
    pub default_profile: String,
    #[serde(default = "default_model_fallback_profile")]
    pub fallback_profile: String,
}

impl Default for ModelFailoverConfig {
    fn default() -> Self {
        Self {
            default_profile: default_model_profile_name(),
            fallback_profile: default_model_fallback_profile(),
        }
    }
}

fn default_model_profile_name() -> String {
    "default".to_owned()
}

fn default_model_fallback_profile() -> String {
    "hosted-fallback".to_owned()
}

fn default_model_profile_mode() -> String {
    "local-first".to_owned()
}

fn default_model_profile_timeout_seconds() -> u32 {
    20
}

fn default_model_profile_retry_budget() -> u32 {
    2
}

fn default_chunk_limit() -> usize {
    1800
}

fn default_true() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::AppConfig;

    #[test]
    fn defaults_include_v2_sections() {
        let config = AppConfig::default();
        assert!(config.orchestration.speculative_enabled);
        assert!(config.security.encryption.enabled);
        assert!(config.brain.enabled);
        assert_eq!(config.brain.max_inline_suggestions, 2);
        assert_eq!(config.vector_memory.requested_backend, "lancedb");
        assert!(config.connectors.contains_key("telegram"));
        assert!(config.connectors.contains_key("webchat"));
        assert!(config.connectors.contains_key("twilio-mms"));
        assert!(config.gateway.web.enabled);
        assert_eq!(config.model_failover.default_profile, "default");
    }

    #[test]
    fn set_updates_v2_fields() {
        let mut config = AppConfig::default();
        config
            .set("models.ollama.quantization", "Q8_0")
            .expect("set model field");
        config
            .set("security.audit.enabled", "false")
            .expect("set audit field");
        config
            .set("connectors.matrix.mode", "bridge")
            .expect("set connector field");
        config
            .set("brain.context_budget_chars", "4096")
            .expect("set brain field");
        config
            .set("gateway.web.enabled", "false")
            .expect("set gateway web field");
        config
            .set("sessions.prune_after_hours", "24")
            .expect("set session field");

        assert_eq!(config.models["ollama"].quantization, "Q8_0");
        assert!(!config.security.audit.enabled);
        assert_eq!(config.brain.context_budget_chars, 4096);
        assert_eq!(config.connectors["matrix"].mode, "bridge");
        assert!(!config.gateway.web.enabled);
        assert_eq!(config.sessions.prune_after_hours, 24);
    }
}
