package config

import (
	"fmt"
	"os"
	"path/filepath"

	toml "github.com/pelletier/go-toml/v2"
)

type Config struct {
	Gateway       GatewayConfig                 `toml:"gateway"`
	Logging       LoggingConfig                 `toml:"logging"`
	Skills        SkillsConfig                  `toml:"skills"`
	Connectors    map[string]ConnectorConfig    `toml:"connectors"`
	Security      SecurityConfig                `toml:"security"`
	Brain         BrainConfig                   `toml:"brain"`
	Sessions      SessionsConfig                `toml:"sessions"`
	Routing       RoutingConfig                 `toml:"routing"`
	Presence      PresenceConfig                `toml:"presence"`
	Usage         UsageConfig                   `toml:"usage"`
	Media         MediaConfig                   `toml:"media"`
	Browser       BrowserConfig                 `toml:"browser"`
	ModelProfiles map[string]ModelProfileConfig `toml:"model_profiles"`
	ModelFailover ModelFailoverConfig           `toml:"model_failover"`
	SIEM          SiemConfig                    `toml:"siem"`
	Operator      OperatorConfig                `toml:"operator"`
	VectorMemory  VectorMemoryConfig            `toml:"vector_memory"`
	Paths         PathsConfig                   `toml:"-"`
}

type GatewayConfig struct {
	ListenAddress               string              `toml:"listen_address"`
	Binary                      string              `toml:"binary"`
	EngineEndpoint              string              `toml:"engine_endpoint"`
	TelegramBotToken            string              `toml:"telegram_bot_token"`
	TelegramPollIntervalSeconds uint64              `toml:"telegram_poll_interval_seconds"`
	TLS                         GatewayTLSConfig    `toml:"tls"`
	Web                         GatewayWebConfig    `toml:"web"`
	Auth                        GatewayAuthConfig   `toml:"auth"`
	Remote                      GatewayRemoteConfig `toml:"remote"`
	Allowlists                  map[string][]string `toml:"allowlists"`
}

type GatewayWebConfig struct {
	Enabled       bool   `toml:"enabled"`
	ListenAddress string `toml:"listen_address"`
	UIDir         string `toml:"ui_dir"`
	CORSOrigin    string `toml:"cors_origin"`
}

type GatewayAuthConfig struct {
	Enabled  bool   `toml:"enabled"`
	Token    string `toml:"token"`
	Password string `toml:"password"`
}

type GatewayRemoteConfig struct {
	Enabled       bool   `toml:"enabled"`
	Mode          string `toml:"mode"`
	PublicBaseURL string `toml:"public_base_url"`
}

type GatewayTLSConfig struct {
	Enabled       bool   `toml:"enabled"`
	CertFile      string `toml:"cert_file"`
	KeyFile       string `toml:"key_file"`
	ClientCAFile  string `toml:"client_ca_file"`
	RotateOnStart bool   `toml:"rotate_on_start"`
}

type LoggingConfig struct {
	Level string `toml:"level"`
	JSON  bool   `toml:"json"`
}

type SkillsConfig struct {
	RegistryIndex     string `toml:"registry_index"`
	RegistrySignature string `toml:"registry_signature"`
	TrustRoot         string `toml:"trust_root"`
}

type ConnectorConfig struct {
	Enabled     bool     `toml:"enabled"`
	Mode        string   `toml:"mode"`
	Endpoint    string   `toml:"endpoint"`
	Allowlist   []string `toml:"allowlist"`
	APIFirst    bool     `toml:"api_first"`
	Deferred    bool     `toml:"deferred"`
	AuthMode    string   `toml:"auth_mode"`
	PairDM      bool     `toml:"pair_dm"`
	ChunkLimit  uint64   `toml:"chunk_limit"`
	MentionOnly bool     `toml:"mention_only"`
}

type SessionsConfig struct {
	Enabled          bool   `toml:"enabled"`
	DefaultReplyMode string `toml:"default_reply_mode"`
	DefaultQueueMode string `toml:"default_queue_mode"`
	PruneAfterHours  uint32 `toml:"prune_after_hours"`
}

type RoutingConfig struct {
	AutoPairDM          bool     `toml:"auto_pair_dm"`
	MentionOnlyInGroups bool     `toml:"mention_only_in_groups"`
	MentionNames        []string `toml:"mention_names"`
}

type PresenceConfig struct {
	Enabled      bool `toml:"enabled"`
	TypingEvents bool `toml:"typing_events"`
}

type UsageConfig struct {
	Enabled bool `toml:"enabled"`
}

type MediaConfig struct {
	Enabled        bool   `toml:"enabled"`
	MaxUploadBytes uint64 `toml:"max_upload_bytes"`
	TempDir        string `toml:"temp_dir"`
}

type BrowserConfig struct {
	Enabled    bool   `toml:"enabled"`
	ProfileDir string `toml:"profile_dir"`
}

type ModelProfileConfig struct {
	ProviderOrder  []string `toml:"provider_order"`
	Mode           string   `toml:"mode"`
	TimeoutSeconds uint32   `toml:"timeout_seconds"`
	RetryBudget    uint32   `toml:"retry_budget"`
	Hosted         bool     `toml:"hosted"`
	AuthMode       string   `toml:"auth_mode"`
}

type ModelFailoverConfig struct {
	DefaultProfile  string `toml:"default_profile"`
	FallbackProfile string `toml:"fallback_profile"`
}

type SecurityConfig struct {
	Attestation AttestationConfig `toml:"attestation"`
	Audit       AuditConfig       `toml:"audit"`
}

type BrainConfig struct {
	Enabled                        bool   `toml:"enabled"`
	AutoIngestMessages             bool   `toml:"auto_ingest_messages"`
	AutoIngestToolResults          bool   `toml:"auto_ingest_tool_results"`
	AutoIngestAssistantCommitments bool   `toml:"auto_ingest_assistant_commitments"`
	InlineSuggestionsInReplies     bool   `toml:"inline_suggestions_in_replies"`
	MaxInlineSuggestions           uint64 `toml:"max_inline_suggestions"`
	ContextBudgetChars             uint64 `toml:"context_budget_chars"`
	ArchiveDecayDays               int64  `toml:"archive_decay_days"`
	StaleTaskHours                 int64  `toml:"stale_task_hours"`
}

type AttestationConfig struct {
	Enabled         bool `toml:"enabled"`
	RequireHardware bool `toml:"require_hardware"`
}

type AuditConfig struct {
	Enabled          bool    `toml:"enabled"`
	Mode             string  `toml:"mode"`
	EBPFEnabled      bool    `toml:"ebpf_enabled"`
	AnomalyThreshold float64 `toml:"anomaly_threshold"`
}

type SiemConfig struct {
	Enabled            bool   `toml:"enabled"`
	OCSFFile           string `toml:"ocsf_file"`
	SyslogEndpoint     string `toml:"syslog_endpoint"`
	HTTPSBatchEndpoint string `toml:"https_batch_endpoint"`
}

type OperatorConfig struct {
	Enabled   bool   `toml:"enabled"`
	Namespace string `toml:"namespace"`
}

type VectorMemoryConfig struct {
	RequestedBackend string `toml:"requested_backend"`
	DefaultNamespace string `toml:"default_namespace"`
}

type PathsConfig struct {
	ConfigFile       string
	DataDir          string
	StateDir         string
	RuntimeStateFile string
	LogFile          string
	InstallsDir      string
}

func Load() (*Config, error) {
	configPath, err := discoverConfigPath()
	if err != nil {
		return nil, err
	}

	raw, err := os.ReadFile(configPath)
	if err != nil {
		return nil, fmt.Errorf("read config: %w", err)
	}

	var cfg Config
	if err := toml.Unmarshal(raw, &cfg); err != nil {
		return nil, fmt.Errorf("parse config: %w", err)
	}

	cfg.Paths = computePaths(configPath)

	if endpoint := os.Getenv("OPENPINCH_ENGINE_ENDPOINT"); endpoint != "" {
		cfg.Gateway.EngineEndpoint = endpoint
	}
	if listen := os.Getenv("OPENPINCH_GATEWAY_LISTEN_ADDRESS"); listen != "" {
		cfg.Gateway.ListenAddress = listen
	}
	if token := os.Getenv("OPENPINCH_TELEGRAM_BOT_TOKEN"); token != "" {
		cfg.Gateway.TelegramBotToken = token
	}
	if cfg.Gateway.TelegramPollIntervalSeconds == 0 {
		cfg.Gateway.TelegramPollIntervalSeconds = 5
	}
	if !cfg.Gateway.Web.Enabled && cfg.Gateway.Web.ListenAddress == "" {
		cfg.Gateway.Web.ListenAddress = "127.0.0.1:8088"
	}
	if cfg.Gateway.Web.ListenAddress == "" {
		cfg.Gateway.Web.ListenAddress = "127.0.0.1:8088"
	}
	if cfg.Gateway.Web.UIDir == "" {
		cfg.Gateway.Web.UIDir = "ui/build/web"
	}
	if cfg.Gateway.Remote.Mode == "" {
		cfg.Gateway.Remote.Mode = "disabled"
	}
	if cfg.Connectors == nil {
		cfg.Connectors = map[string]ConnectorConfig{}
	}
	if cfg.Gateway.Allowlists == nil {
		cfg.Gateway.Allowlists = map[string][]string{}
	}
	if cfg.ModelProfiles == nil {
		cfg.ModelProfiles = map[string]ModelProfileConfig{
			"default": {
				ProviderOrder:  []string{"llamacpp", "ollama", "localai"},
				Mode:           "local-first",
				TimeoutSeconds: 20,
				RetryBudget:    2,
			},
			"hosted-fallback": {
				ProviderOrder:  []string{"llamacpp", "ollama", "localai"},
				Mode:           "hybrid-failover",
				TimeoutSeconds: 30,
				RetryBudget:    3,
				Hosted:         true,
				AuthMode:       "api-key",
			},
		}
	}
	if cfg.ModelFailover.DefaultProfile == "" {
		cfg.ModelFailover.DefaultProfile = "default"
	}
	if cfg.ModelFailover.FallbackProfile == "" {
		cfg.ModelFailover.FallbackProfile = "hosted-fallback"
	}

	return &cfg, nil
}

func discoverConfigPath() (string, error) {
	if configured := os.Getenv("OPENPINCH_CONFIG_PATH"); configured != "" {
		return configured, nil
	}
	base, err := os.UserConfigDir()
	if err != nil {
		return "", fmt.Errorf("config dir: %w", err)
	}
	return filepath.Join(base, "openpinch", "OpenPinch", "config.toml"), nil
}

func computePaths(configPath string) PathsConfig {
	configDir := filepath.Dir(configPath)
	dataDir := configDir
	stateDir := filepath.Join(configDir, "state")

	if home, err := os.UserHomeDir(); err == nil {
		dataDir = filepath.Join(home, ".local", "share", "openpinch", "OpenPinch")
	}
	if base, err := os.UserCacheDir(); err == nil {
		stateDir = filepath.Join(base, "openpinch", "OpenPinch")
	}

	return PathsConfig{
		ConfigFile:       configPath,
		DataDir:          dataDir,
		StateDir:         stateDir,
		RuntimeStateFile: filepath.Join(stateDir, "runtime", "runtime-state.json"),
		LogFile:          filepath.Join(stateDir, "logs", "openpinch.log"),
		InstallsDir:      filepath.Join(dataDir, "skills", "installed"),
	}
}
