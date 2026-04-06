package scheduler

import (
	"context"
	"log"
	"sync"

	"github.com/PeshalaDilshan/openpinch/gateway/internal/enginebridge"
	"github.com/robfig/cron/v3"
)

type Scheduler struct {
	cron   *cron.Cron
	bridge *enginebridge.Client
	mu     sync.Mutex
	ids    map[string]cron.EntryID
}

func New(bridge *enginebridge.Client) *Scheduler {
	return &Scheduler{
		cron:   cron.New(),
		bridge: bridge,
		ids:    map[string]cron.EntryID{},
	}
}

func (s *Scheduler) Start() {
	s.cron.Start()
}

func (s *Scheduler) Stop() {
	ctx := s.cron.Stop()
	<-ctx.Done()
}

func (s *Scheduler) Schedule(jobID string, spec string, tool string, argumentsJSON string) error {
	s.mu.Lock()
	defer s.mu.Unlock()

	if existing, ok := s.ids[jobID]; ok {
		s.cron.Remove(existing)
	}

	entryID, err := s.cron.AddFunc(spec, func() {
		if _, err := s.bridge.RunTool(context.Background(), tool, argumentsJSON, false); err != nil {
			log.Printf("scheduled job %s failed: %v", jobID, err)
		}
	})
	if err != nil {
		return err
	}

	s.ids[jobID] = entryID
	return nil
}
