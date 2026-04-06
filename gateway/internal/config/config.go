package config

import (
	"fmt"
	"os"
	"path/filepath"

	toml "github.com/pelletier/go-toml/v2"
)

type Config struct {
	Gateway GatewayConfig `toml:"gateway"`
	Logging LoggingConfig `toml:"logging"`
	Skills  SkillsConfig  `toml:"skills"`
	Paths   PathsConfig   `toml:"-"`
}

type GatewayConfig struct {
	ListenAddress               string `toml:"listen_address"`
	Binary                      string `toml:"binary"`
	EngineEndpoint              string `toml:"engine_endpoint"`
	TelegramBotToken            string `toml:"telegram_bot_token"`
	TelegramPollIntervalSeconds uint64 `toml:"telegram_poll_interval_seconds"`
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
