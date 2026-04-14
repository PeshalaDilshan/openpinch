package connectors

import (
	"context"
	"fmt"
	"log"

	"github.com/PeshalaDilshan/openpinch/gateway/internal/config"
	"github.com/PeshalaDilshan/openpinch/gateway/internal/enginebridge"
)

type Descriptor struct {
	Name        string
	Enabled     bool
	Implemented bool
	Mode        string
	Health      string
	Allowlist   []string
	Details     map[string]string
}

type Connector interface {
	Name() string
	Enabled() bool
	Start(context.Context) error
	SendMessage(context.Context, string, string) error
	Descriptor() Descriptor
}

type Registry struct {
	connectors []Connector
	cancel     context.CancelFunc
}

func NewRegistry(cfg *config.Config, bridge *enginebridge.Client) *Registry {
	return &Registry{
		connectors: []Connector{
			NewTelegram(cfg, bridge),
			NewWebChat(cfg),
			newStub(cfg, "discord", "bot"),
			newStub(cfg, "slack", "socket-mode"),
			newStub(cfg, "whatsapp", "cloud-api"),
			newStub(cfg, "signal", "signal-cli"),
			newStub(cfg, "matrix", "appservice"),
			newStub(cfg, "xmpp", "client"),
			newStub(cfg, "irc", "client"),
			newStub(cfg, "mattermost", "websocket"),
			newStub(cfg, "microsoft-teams", "graph"),
			newStub(cfg, "rocketchat", "rest"),
			newStub(cfg, "zulip", "rest"),
			newStub(cfg, "google-chat", "rest"),
			newStub(cfg, "webex", "rest"),
			newStub(cfg, "line", "messaging-api"),
			newStub(cfg, "viber", "bot"),
			newStub(cfg, "smtp", "outbound"),
			newStub(cfg, "imap", "inbound"),
			newStub(cfg, "twilio-sms", "sms"),
			newStub(cfg, "twilio-mms", "mms"),
			newStub(cfg, "webhook-inbound", "http"),
			newStub(cfg, "webhook-outbound", "http"),
		},
	}
}

func (r *Registry) Start(ctx context.Context) error {
	ctx, cancel := context.WithCancel(ctx)
	r.cancel = cancel

	for _, connector := range r.connectors {
		if !connector.Enabled() {
			continue
		}
		go func(connector Connector) {
			if err := connector.Start(ctx); err != nil {
				log.Printf("%s connector stopped: %v", connector.Name(), err)
			}
		}(connector)
	}

	return nil
}

func (r *Registry) Stop() {
	if r.cancel != nil {
		r.cancel()
	}
}

func (r *Registry) EnabledNames() []string {
	names := make([]string, 0, len(r.connectors))
	for _, connector := range r.connectors {
		if connector.Enabled() {
			names = append(names, connector.Name())
		}
	}
	return names
}

func (r *Registry) DescribeAll() []Descriptor {
	descriptors := make([]Descriptor, 0, len(r.connectors))
	for _, connector := range r.connectors {
		descriptors = append(descriptors, connector.Descriptor())
	}
	return descriptors
}

func (r *Registry) Status(name string) (Descriptor, bool) {
	for _, connector := range r.connectors {
		if connector.Name() == name {
			return connector.Descriptor(), true
		}
	}
	return Descriptor{}, false
}

func (r *Registry) SendMessage(ctx context.Context, name string, channelID string, text string) error {
	for _, connector := range r.connectors {
		if connector.Name() == name {
			if !connector.Enabled() {
				return fmt.Errorf("connector %s is disabled", name)
			}
			return connector.SendMessage(ctx, channelID, text)
		}
	}
	return fmt.Errorf("connector %s not found", name)
}

type stub struct {
	cfg  *config.Config
	name string
	mode string
}

func newStub(cfg *config.Config, name string, mode string) Connector {
	return &stub{cfg: cfg, name: name, mode: mode}
}

func (s *stub) Name() string {
	return s.name
}

func (s *stub) Enabled() bool {
	connector, ok := s.cfg.Connectors[s.name]
	return ok && connector.Enabled
}

func (s *stub) Descriptor() Descriptor {
	connector := s.cfg.Connectors[s.name]
	return Descriptor{
		Name:        s.name,
		Enabled:     connector.Enabled,
		Implemented: false,
		Mode:        firstNonEmpty(connector.Mode, s.mode, "disabled"),
		Health:      healthFor(connector.Enabled, false),
		Allowlist:   connector.Allowlist,
		Details: map[string]string{
			"transport": "api-first",
			"status":    "scaffolded",
		},
	}
}

func (s *stub) Start(context.Context) error {
	return nil
}

func (s *stub) SendMessage(context.Context, string, string) error {
	return fmt.Errorf("connector %s is not implemented yet", s.name)
}

func healthFor(enabled bool, implemented bool) string {
	if !enabled {
		return "disabled"
	}
	if implemented {
		return "ready"
	}
	return "scaffolded"
}

func firstNonEmpty(values ...string) string {
	for _, value := range values {
		if value != "" {
			return value
		}
	}
	return ""
}
