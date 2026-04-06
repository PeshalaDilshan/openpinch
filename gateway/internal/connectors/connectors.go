package connectors

import (
	"context"
	"log"

	"github.com/PeshalaDilshan/openpinch/gateway/internal/config"
	"github.com/PeshalaDilshan/openpinch/gateway/internal/enginebridge"
)

type Connector interface {
	Name() string
	Enabled() bool
	Start(context.Context) error
}

type Registry struct {
	connectors []Connector
	cancel     context.CancelFunc
}

func NewRegistry(cfg *config.Config, bridge *enginebridge.Client) *Registry {
	return &Registry{
		connectors: []Connector{
			NewTelegram(cfg, bridge),
			newStub("discord"),
			newStub("slack"),
			newStub("signal"),
			newStub("whatsapp"),
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

type stub struct {
	name string
}

func newStub(name string) Connector {
	return &stub{name: name}
}

func (s *stub) Name() string {
	return s.name
}

func (s *stub) Enabled() bool {
	return false
}

func (s *stub) Start(context.Context) error {
	return nil
}
