package connectors

import (
	"context"

	"github.com/PeshalaDilshan/openpinch/gateway/internal/config"
)

type WebChatConnector struct {
	cfg *config.Config
}

func NewWebChat(cfg *config.Config) Connector {
	return &WebChatConnector{cfg: cfg}
}

func (w *WebChatConnector) Name() string {
	return "webchat"
}

func (w *WebChatConnector) Enabled() bool {
	connector, ok := w.cfg.Connectors["webchat"]
	return ok && connector.Enabled
}

func (w *WebChatConnector) Descriptor() Descriptor {
	connector := w.cfg.Connectors["webchat"]
	return Descriptor{
		Name:        "webchat",
		Enabled:     w.Enabled(),
		Implemented: true,
		Mode:        firstNonEmpty(connector.Mode, "web"),
		Health:      healthFor(w.Enabled(), true),
		Allowlist:   connector.Allowlist,
		Details: map[string]string{
			"transport": "http+sse",
			"status":    "ready",
		},
	}
}

func (w *WebChatConnector) Start(context.Context) error {
	return nil
}

func (w *WebChatConnector) SendMessage(context.Context, string, string) error {
	return nil
}
