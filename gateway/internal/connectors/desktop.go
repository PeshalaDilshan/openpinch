package connectors

import (
	"context"

	"github.com/PeshalaDilshan/openpinch/gateway/internal/config"
)

type DesktopConnector struct {
	cfg *config.Config
}

func NewDesktop(cfg *config.Config) Connector {
	return &DesktopConnector{cfg: cfg}
}

func (d *DesktopConnector) Name() string {
	return "desktop"
}

func (d *DesktopConnector) Enabled() bool {
	connector, ok := d.cfg.Connectors["desktop"]
	return ok && connector.Enabled
}

func (d *DesktopConnector) Descriptor() Descriptor {
	connector := d.cfg.Connectors["desktop"]
	return Descriptor{
		Name:        "desktop",
		Enabled:     d.Enabled(),
		Implemented: true,
		Mode:        firstNonEmpty(connector.Mode, "native"),
		Health:      healthFor(d.Enabled(), true),
		Allowlist:   connector.Allowlist,
		Details: map[string]string{
			"transport": "local-process",
			"status":    "ready",
		},
	}
}

func (d *DesktopConnector) Start(context.Context) error {
	return nil
}

func (d *DesktopConnector) SendMessage(context.Context, string, string) error {
	return nil
}
