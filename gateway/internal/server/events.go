package server

import (
	"encoding/json"
	"fmt"
	"sync"
	"time"
)

type DesktopEvent struct {
	ID          string `json:"id"`
	EventType   string `json:"event_type"`
	PayloadJSON string `json:"payload_json"`
	CreatedAt   string `json:"created_at"`
}

type EventHub struct {
	mu      sync.Mutex
	nextID  int
	history []DesktopEvent
	subs    map[int]chan DesktopEvent
}

func NewEventHub() *EventHub {
	return &EventHub{
		history: make([]DesktopEvent, 0, 64),
		subs:    map[int]chan DesktopEvent{},
	}
}

func (h *EventHub) Publish(eventType string, payload any) {
	raw, _ := json.Marshal(payload)
	event := DesktopEvent{
		ID:          fmt.Sprintf("ui-%d", time.Now().UTC().UnixNano()),
		EventType:   eventType,
		PayloadJSON: string(raw),
		CreatedAt:   time.Now().UTC().Format(time.RFC3339),
	}

	h.mu.Lock()
	h.history = append(h.history, event)
	if len(h.history) > 128 {
		h.history = h.history[len(h.history)-128:]
	}
	for _, ch := range h.subs {
		select {
		case ch <- event:
		default:
		}
	}
	h.mu.Unlock()
}

func (h *EventHub) Subscribe() (int, <-chan DesktopEvent) {
	h.mu.Lock()
	defer h.mu.Unlock()
	id := h.nextID
	h.nextID++
	ch := make(chan DesktopEvent, 32)
	h.subs[id] = ch
	return id, ch
}

func (h *EventHub) Unsubscribe(id int) {
	h.mu.Lock()
	defer h.mu.Unlock()
	if ch, ok := h.subs[id]; ok {
		delete(h.subs, id)
		close(ch)
	}
}

func (h *EventHub) History() []DesktopEvent {
	h.mu.Lock()
	defer h.mu.Unlock()
	snapshot := make([]DesktopEvent, len(h.history))
	copy(snapshot, h.history)
	return snapshot
}
