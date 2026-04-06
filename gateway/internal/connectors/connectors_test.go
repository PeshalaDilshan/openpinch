package connectors

import (
	"testing"

	"github.com/PeshalaDilshan/openpinch/gateway/internal/config"
)

func TestRegistryExposesConnectorCatalog(t *testing.T) {
	cfg := &config.Config{
		Gateway: config.GatewayConfig{},
		Connectors: map[string]config.ConnectorConfig{
			"telegram": {Enabled: true, Mode: "polling"},
			"matrix":   {Enabled: true, Mode: "bridge"},
		},
	}

	registry := NewRegistry(cfg, nil)
	descriptors := registry.DescribeAll()
	if len(descriptors) < 20 {
		t.Fatalf("expected 20+ connector descriptors, got %d", len(descriptors))
	}

	status, ok := registry.Status("telegram")
	if !ok {
		t.Fatalf("expected telegram descriptor")
	}
	if !status.Implemented {
		t.Fatalf("expected telegram to be implemented")
	}
}
