package server

import (
	"context"
	"encoding/json"
	"fmt"
	"net/http"
	"os"
	"path/filepath"
	"strings"

	"github.com/PeshalaDilshan/openpinch/gateway/internal/enginebridge"
)

func (s *Service) HTTPHandler() http.Handler {
	mux := http.NewServeMux()
	mux.HandleFunc("/healthz", s.handleHealthz)
	mux.HandleFunc("/events", s.handleEvents)
	mux.HandleFunc("/api/status", s.handleAPIStatus)
	mux.HandleFunc("/api/connectors", s.handleAPIConnectors)
	mux.HandleFunc("/api/sessions", s.handleAPISessions)
	mux.HandleFunc("/api/sessions/", s.handleAPISession)
	mux.HandleFunc("/api/pairings", s.handleAPIPairings)
	mux.HandleFunc("/api/pairings/", s.handleAPIPairing)
	mux.HandleFunc("/api/messages/send", s.handleAPISendMessage)
	mux.HandleFunc("/api/webchat/message", s.handleAPIWebChatMessage)
	mux.HandleFunc("/api/doctor", s.handleAPIDoctor)
	mux.HandleFunc("/api/model-profiles", s.handleAPIModelProfiles)
	mux.HandleFunc("/", s.handleUI)
	return s.withAuth(mux)
}

func (s *Service) withAuth(next http.Handler) http.Handler {
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		s.writeCORSHeaders(w)
		if r.Method == http.MethodOptions {
			w.WriteHeader(http.StatusNoContent)
			return
		}
		if !s.authorized(r) {
			writeJSON(w, http.StatusUnauthorized, map[string]any{
				"error": "gateway authentication required",
			})
			return
		}
		next.ServeHTTP(w, r)
	})
}

func (s *Service) authorized(r *http.Request) bool {
	if !s.cfg.Gateway.Auth.Enabled {
		return true
	}
	if token := s.cfg.Gateway.Auth.Token; token != "" {
		header := strings.TrimPrefix(r.Header.Get("Authorization"), "Bearer ")
		if header == token || r.URL.Query().Get("token") == token {
			return true
		}
	}
	if password := s.cfg.Gateway.Auth.Password; password != "" {
		if r.Header.Get("X-OpenPinch-Password") == password || r.URL.Query().Get("password") == password {
			return true
		}
	}
	return false
}

func (s *Service) writeCORSHeaders(w http.ResponseWriter) {
	origin := s.cfg.Gateway.Web.CORSOrigin
	if origin == "" {
		origin = "*"
	}
	w.Header().Set("Access-Control-Allow-Origin", origin)
	w.Header().Set("Access-Control-Allow-Headers", "Authorization, Content-Type, X-OpenPinch-Password")
	w.Header().Set("Access-Control-Allow-Methods", "GET, POST, OPTIONS")
}

func (s *Service) handleHealthz(w http.ResponseWriter, _ *http.Request) {
	writeJSON(w, http.StatusOK, map[string]any{"status": "ok"})
}

func (s *Service) handleEvents(w http.ResponseWriter, r *http.Request) {
	flusher, ok := w.(http.Flusher)
	if !ok {
		http.Error(w, "streaming unsupported", http.StatusInternalServerError)
		return
	}
	w.Header().Set("Content-Type", "text/event-stream")
	w.Header().Set("Cache-Control", "no-cache")
	w.Header().Set("Connection", "keep-alive")

	for _, event := range s.events.History() {
		fmt.Fprintf(w, "event: %s\ndata: %s\n\n", event.EventType, event.PayloadJSON)
	}
	flusher.Flush()

	id, ch := s.events.Subscribe()
	defer s.events.Unsubscribe(id)

	for {
		select {
		case <-r.Context().Done():
			return
		case event := <-ch:
			fmt.Fprintf(w, "event: %s\ndata: %s\n\n", event.EventType, event.PayloadJSON)
			flusher.Flush()
		}
	}
}

func (s *Service) handleAPIStatus(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodGet {
		http.Error(w, "method not allowed", http.StatusMethodNotAllowed)
		return
	}
	health, err := s.bridge.Health(r.Context())
	if err != nil {
		writeJSON(w, http.StatusServiceUnavailable, map[string]any{"error": err.Error()})
		return
	}
	writeJSON(w, http.StatusOK, map[string]any{
		"status":                health.Status,
		"sandbox_backend":       health.SandboxBackend,
		"vector_memory_backend": health.VectorMemoryBackend,
		"encryption_state":      health.EncryptionState,
		"audit_mode":            health.AuditMode,
		"started_at":            s.startedAt.Format(timeLayout),
	})
}

func (s *Service) handleAPIConnectors(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodGet {
		http.Error(w, "method not allowed", http.StatusMethodNotAllowed)
		return
	}
	writeJSON(w, http.StatusOK, map[string]any{
		"connectors": s.registry.DescribeAll(),
	})
}

func (s *Service) handleAPISessions(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodGet {
		http.Error(w, "method not allowed", http.StatusMethodNotAllowed)
		return
	}
	limit := parseUintQuery(r, "limit", 20)
	sessions, summary, err := s.bridge.ListSessions(
		r.Context(),
		r.URL.Query().Get("connector"),
		r.URL.Query().Get("status"),
		r.URL.Query().Get("include_archived") == "true",
		limit,
	)
	if err != nil {
		writeJSON(w, http.StatusInternalServerError, map[string]any{"error": err.Error()})
		return
	}
	writeJSON(w, http.StatusOK, map[string]any{
		"summary":  summary,
		"sessions": sessions,
	})
}

func (s *Service) handleAPISession(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodGet {
		http.Error(w, "method not allowed", http.StatusMethodNotAllowed)
		return
	}
	sessionID := strings.TrimPrefix(r.URL.Path, "/api/sessions/")
	detail, err := s.bridge.GetSession(r.Context(), sessionID, parseUintQuery(r, "limit", 100))
	if err != nil {
		writeJSON(w, http.StatusInternalServerError, map[string]any{"error": err.Error()})
		return
	}
	writeJSON(w, http.StatusOK, detail)
}

func (s *Service) handleAPIPairings(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodGet {
		http.Error(w, "method not allowed", http.StatusMethodNotAllowed)
		return
	}
	pairings, err := s.bridge.ListPairings(
		r.Context(),
		r.URL.Query().Get("status"),
		r.URL.Query().Get("connector"),
		parseUintQuery(r, "limit", 50),
	)
	if err != nil {
		writeJSON(w, http.StatusInternalServerError, map[string]any{"error": err.Error()})
		return
	}
	writeJSON(w, http.StatusOK, map[string]any{"pairings": pairings})
}

func (s *Service) handleAPIPairing(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		http.Error(w, "method not allowed", http.StatusMethodNotAllowed)
		return
	}
	pairingID := strings.TrimPrefix(r.URL.Path, "/api/pairings/")
	var payload struct {
		Action string `json:"action"`
		Note   string `json:"note"`
	}
	if err := json.NewDecoder(r.Body).Decode(&payload); err != nil {
		writeJSON(w, http.StatusBadRequest, map[string]any{"error": err.Error()})
		return
	}
	pairing, err := s.bridge.UpdatePairing(r.Context(), pairingID, payload.Action, payload.Note)
	if err != nil {
		writeJSON(w, http.StatusInternalServerError, map[string]any{"error": err.Error()})
		return
	}
	s.events.Publish("pairing.updated", pairing)
	writeJSON(w, http.StatusOK, pairing)
}

func (s *Service) handleAPISendMessage(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		http.Error(w, "method not allowed", http.StatusMethodNotAllowed)
		return
	}
	var payload struct {
		Connector    string `json:"connector"`
		ChannelID    string `json:"channel_id"`
		Sender       string `json:"sender"`
		Body         string `json:"body"`
		MetadataJSON string `json:"metadata_json"`
		SessionID    string `json:"session_id"`
	}
	if err := json.NewDecoder(r.Body).Decode(&payload); err != nil {
		writeJSON(w, http.StatusBadRequest, map[string]any{"error": err.Error()})
		return
	}
	if err := s.registry.SendMessage(r.Context(), payload.Connector, payload.ChannelID, payload.Body); err != nil {
		writeJSON(w, http.StatusInternalServerError, map[string]any{"error": err.Error()})
		return
	}
	result, err := s.bridge.RecordOutboundMessage(
		r.Context(),
		payload.Connector,
		payload.ChannelID,
		payload.Sender,
		payload.Body,
		payload.MetadataJSON,
		payload.SessionID,
	)
	if err != nil {
		writeJSON(w, http.StatusInternalServerError, map[string]any{"error": err.Error()})
		return
	}
	s.events.Publish("message.outbound", result)
	writeJSON(w, http.StatusOK, result)
}

func (s *Service) handleAPIWebChatMessage(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		http.Error(w, "method not allowed", http.StatusMethodNotAllowed)
		return
	}
	var payload struct {
		Sender       string `json:"sender"`
		ChannelID    string `json:"channel_id"`
		Body         string `json:"body"`
		MetadataJSON string `json:"metadata_json"`
	}
	if err := json.NewDecoder(r.Body).Decode(&payload); err != nil {
		writeJSON(w, http.StatusBadRequest, map[string]any{"error": err.Error()})
		return
	}
	if payload.Sender == "" {
		payload.Sender = "web-user"
	}
	if payload.ChannelID == "" {
		payload.ChannelID = payload.Sender
	}
	result, err := s.bridge.DeliverMessage(r.Context(), enginebridge.Message{
		Connector:    "webchat",
		ChannelID:    payload.ChannelID,
		Sender:       payload.Sender,
		Body:         payload.Body,
		MetadataJSON: payload.MetadataJSON,
	})
	if err != nil {
		writeJSON(w, http.StatusInternalServerError, map[string]any{"error": err.Error()})
		return
	}
	s.events.Publish("message.inbound", map[string]any{
		"connector":      "webchat",
		"channel_id":     payload.ChannelID,
		"sender":         payload.Sender,
		"body":           payload.Body,
		"session_id":     result.SessionID,
		"delivery_state": result.DeliveryState,
	})
	if result.Reply != "" {
		s.events.Publish("message.reply", map[string]any{
			"connector":  "webchat",
			"channel_id": payload.ChannelID,
			"body":       result.Reply,
			"session_id": result.SessionID,
		})
	}
	writeJSON(w, http.StatusOK, result)
}

func (s *Service) handleAPIDoctor(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodGet {
		http.Error(w, "method not allowed", http.StatusMethodNotAllowed)
		return
	}
	report, err := s.bridge.GetDoctorReport(r.Context(), true, true, true)
	if err != nil {
		writeJSON(w, http.StatusInternalServerError, map[string]any{"error": err.Error()})
		return
	}
	writeJSON(w, http.StatusOK, report)
}

func (s *Service) handleAPIModelProfiles(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodGet {
		http.Error(w, "method not allowed", http.StatusMethodNotAllowed)
		return
	}
	profiles, err := s.bridge.ListModelProfiles(r.Context())
	if err != nil {
		writeJSON(w, http.StatusInternalServerError, map[string]any{"error": err.Error()})
		return
	}
	writeJSON(w, http.StatusOK, map[string]any{"profiles": profiles})
}

func (s *Service) handleUI(w http.ResponseWriter, r *http.Request) {
	if s.cfg.Gateway.Web.UIDir != "" {
		baseDirAbs, err := filepath.Abs(s.cfg.Gateway.Web.UIDir)
		if err == nil {
			requestPath := filepath.Clean(strings.TrimPrefix(r.URL.Path, "/"))
			candidate := filepath.Join(baseDirAbs, requestPath)
			candidateAbs, err := filepath.Abs(candidate)
			if err == nil {
				rel, relErr := filepath.Rel(baseDirAbs, candidateAbs)
				if relErr == nil && !filepath.IsAbs(rel) && rel != ".." && !strings.HasPrefix(rel, ".."+string(filepath.Separator)) {
					if info, statErr := os.Stat(candidateAbs); statErr == nil && !info.IsDir() {
						http.ServeFile(w, r, candidateAbs)
						return
					}
				}
			}
			index := filepath.Join(baseDirAbs, "index.html")
			if _, err := os.Stat(index); err == nil {
				http.ServeFile(w, r, index)
				return
			}
		}
	}
	w.Header().Set("Content-Type", "text/html; charset=utf-8")
	_, _ = w.Write([]byte(defaultHTML))
}

func writeJSON(w http.ResponseWriter, statusCode int, value any) {
	w.Header().Set("Content-Type", "application/json")
	w.WriteHeader(statusCode)
	_ = json.NewEncoder(w).Encode(value)
}

func parseUintQuery(r *http.Request, key string, fallback uint32) uint32 {
	if raw := r.URL.Query().Get(key); raw != "" {
		var parsed uint32
		if _, err := fmt.Sscanf(raw, "%d", &parsed); err == nil && parsed > 0 {
			return parsed
		}
	}
	return fallback
}

const timeLayout = "2006-01-02T15:04:05Z07:00"

const defaultHTML = `<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>OpenPinch Control</title>
  <style>
    :root { color-scheme: light; --bg: #f5f1e8; --panel: #fffdf9; --ink: #182024; --accent: #005f73; --muted: #667085; }
    body { margin: 0; font-family: Georgia, "Times New Roman", serif; background: linear-gradient(180deg, #f5f1e8 0%, #efe7d8 100%); color: var(--ink); }
    main { max-width: 880px; margin: 0 auto; padding: 48px 20px; }
    h1 { font-size: 3rem; margin: 0 0 12px; }
    p { font-size: 1.05rem; line-height: 1.6; }
    .card { background: var(--panel); border: 1px solid rgba(24,32,36,0.08); border-radius: 20px; padding: 24px; box-shadow: 0 12px 30px rgba(24,32,36,0.08); margin-top: 24px; }
    code { background: rgba(0,95,115,0.08); padding: 2px 6px; border-radius: 8px; }
  </style>
</head>
<body>
  <main>
    <h1>OpenPinch Control Surface</h1>
    <p>The gateway web API is live. Build the Flutter app in <code>ui/</code> and the gateway will serve the compiled assets from <code>ui/build/web</code>.</p>
    <div class="card">
      <p>Available endpoints: <code>/api/status</code>, <code>/api/sessions</code>, <code>/api/pairings</code>, <code>/api/doctor</code>, <code>/api/model-profiles</code>, <code>/api/webchat/message</code>, and <code>/events</code>.</p>
    </div>
  </main>
</body>
</html>`

func (s *Service) publishSnapshot(ctx context.Context) {
	if ctx == nil {
		ctx = context.Background()
	}
	if sessions, summary, err := s.bridge.ListSessions(ctx, "", "", false, 20); err == nil {
		s.events.Publish("sessions.snapshot", map[string]any{
			"summary":  summary,
			"sessions": sessions,
		})
	}
}
