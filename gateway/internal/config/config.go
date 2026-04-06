package config

import (
	"fmt"
	"os"
	"path/filepath"

	toml "github.com/pelletier/go-toml/v2"
)

type Config struct {
	Gateway      GatewayConfig              `toml:"gateway"`
	Logging      LoggingConfig              `toml:"logging"`
	Skills       SkillsConfig               `toml:"skills"`
	Connectors   map[string]ConnectorConfig `toml:"connectors"`
	Security     SecurityConfig             `toml:"security"`
	SIEM         SiemConfig                 `toml:"siem"`
	Operator     OperatorConfig             `toml:"operator"`
	VectorMemory VectorMemoryConfig         `toml:"vector_memory"`
	Paths        PathsConfig                `toml:"-"`
}

type GatewayConfig struct {
	ListenAddress               string              `toml:"listen_address"`
	Binary                      string              `toml:"binary"`
	EngineEndpoint              string              `toml:"engine_endpoint"`
	TelegramBotToken            string              `toml:"telegram_bot_token"`
	TelegramPollIntervalSeconds uint64              `toml:"telegram_poll_interval_seconds"`
	TLS                         GatewayTLSConfig    `toml:"tls"`
	Allowlists                  map[string][]string `toml:"allowlists"`
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
	Enabled   bool     `toml:"enabled"`
	Mode      string   `toml:"mode"`
	Endpoint  string   `toml:"endpoint"`
	Allowlist []string `toml:"allowlist"`
	APIFirst  bool     `toml:"api_first"`
}

type SecurityConfig struct {
	Attestation AttestationConfig `toml:"attestation"`
	Audit       AuditConfig       `toml:"audit"`
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
	if cfg.Connectors == nil {
		cfg.Connectors = map[string]ConnectorConfig{}
	}
	if cfg.Gateway.Allowlists == nil {
		cfg.Gateway.Allowlists = map[string][]string{}
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
