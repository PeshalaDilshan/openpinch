package enginebridge

import (
	"context"
	"fmt"
	"net"
	"strings"
	"time"

	"github.com/PeshalaDilshan/openpinch/gateway/pkg/pb"
	"google.golang.org/grpc"
	"google.golang.org/grpc/credentials/insecure"
	"google.golang.org/protobuf/proto"
	"google.golang.org/protobuf/reflect/protoreflect"
)

type Client struct {
	conn   *grpc.ClientConn
	schema *pb.Schema
}

type ExecuteResult struct {
	Success  bool
	Summary  string
	DataJSON string
	Error    string
	Logs     []string
}

type HealthResult struct {
	Status               string
	MissingPrerequisites []string
	SandboxBackend       string
	VectorMemoryBackend  string
	EncryptionState      string
	AuditMode            string
}

type Message struct {
	Connector    string
	ChannelID    string
	Sender       string
	Body         string
	MetadataJSON string
}

type MessageResult struct {
	Accepted      bool
	MessageID     string
	Reply         string
	SessionID     string
	PairingID     string
	DeliveryState string
}

type MemoryRecord struct {
	Key          string
	Namespace    string
	Content      string
	MetadataJSON string
	Score        float64
	CreatedAt    string
}

type SessionRecord struct {
	ID                 string
	Connector          string
	ChannelID          string
	Participant        string
	SessionType        string
	Title              string
	Status             string
	ReplyMode          string
	QueueMode          string
	ModelProfile       string
	MentionOnly        bool
	PendingPairing     bool
	LastMessagePreview string
	MessageCount       uint32
	CreatedAt          string
	UpdatedAt          string
}

type SessionMessage struct {
	ID           string
	SessionID    string
	Connector    string
	Role         string
	Sender       string
	Body         string
	MetadataJSON string
	CreatedAt    string
}

type SessionDetail struct {
	Session  *SessionRecord
	Messages []SessionMessage
}

type PairingRecord struct {
	ID        string
	Connector string
	ChannelID string
	Sender    string
	SessionID string
	Status    string
	Reason    string
	CreatedAt string
	UpdatedAt string
}

type OutboundMessageResult struct {
	Accepted  bool
	MessageID string
	SessionID string
	Status    string
	Detail    string
}

type DoctorFinding struct {
	ID        string
	Component string
	Severity  string
	Status    string
	Summary   string
	Detail    string
}

type DoctorReport struct {
	Status   string
	Findings []DoctorFinding
}

type ModelProfile struct {
	Name           string
	ProviderOrder  []string
	Mode           string
	TimeoutSeconds uint32
	RetryBudget    uint32
	Hosted         bool
	AuthMode       string
	DefaultProfile bool
}

type BrainEntity struct {
	ID         string
	Kind       string
	Subtype    string
	Title      string
	Content    string
	ScopeJSON  string
	LinksJSON  string
	Salience   float64
	Confidence float64
	Archived   bool
	CreatedAt  string
	UpdatedAt  string
}

type BrainFact struct {
	ID         string
	EntityID   string
	Content    string
	ScopeJSON  string
	Salience   float64
	Confidence float64
	Archived   bool
	CreatedAt  string
	UpdatedAt  string
}

type BrainRelation struct {
	ID           string
	Kind         string
	FromID       string
	ToID         string
	MetadataJSON string
	Confidence   float64
	CreatedAt    string
	UpdatedAt    string
}

type BrainTask struct {
	ID         string
	Title      string
	Summary    string
	Status     string
	Priority   string
	DueAt      string
	ScopeJSON  string
	LinksJSON  string
	Salience   float64
	Confidence float64
	Archived   bool
	CreatedAt  string
	UpdatedAt  string
}

type BrainSuggestion struct {
	ID          string
	TaskID      string
	Summary     string
	Reason      string
	Score       float64
	ContextJSON string
}

type BrainRememberResult struct {
	Stored bool
	Entity *BrainEntity
	Task   *BrainTask
	Digest string
}

type BrainRecallResult struct {
	Summary   string
	Entities  []BrainEntity
	Facts     []BrainFact
	Relations []BrainRelation
	Tasks     []BrainTask
}

type BrainSuggestResult struct {
	Summary     string
	Suggestions []BrainSuggestion
}

type BrainTaskListResult struct {
	Summary string
	Tasks   []BrainTask
}

type BrainForgetResult struct {
	Forgotten bool
	Mode      string
	TargetID  string
}

type AttestationResult struct {
	Status         string
	Subject        string
	Platform       string
	HardwareBacked bool
	Nonce          string
	PublicKey      string
	Measurements   map[string]string
}

type AuditEvent struct {
	ID           string
	Category     string
	Severity     string
	Summary      string
	AnomalyScore float64
	PayloadJSON  string
	CreatedAt    string
}

type QueueReceipt struct {
	Accepted bool
	TaskID   string
	Queue    string
	Message  string
}

type PolicyReport struct {
	Subject             string
	AllowedCapabilities []string
	DeniedCapabilities  []string
	Source              string
}

type AgentMessage struct {
	Sender        string
	Recipient     string
	Body          string
	MetadataJSON  string
	EncryptedBody string
}

type AgentProtocolResult struct {
	Accepted       bool
	ProtocolID     string
	TranscriptJSON string
	Findings       []string
}

func New(endpoint string) (*Client, error) {
	if endpoint == "" {
		return nil, fmt.Errorf("engine endpoint is empty")
	}

	schema, err := pb.Load()
	if err != nil {
		return nil, err
	}

	options := []grpc.DialOption{
		grpc.WithTransportCredentials(insecure.NewCredentials()),
	}

	target := endpoint
	if strings.HasPrefix(endpoint, "unix://") {
		socketPath := strings.TrimPrefix(endpoint, "unix://")
		target = "unix"
		options = append(options, grpc.WithContextDialer(func(ctx context.Context, _ string) (net.Conn, error) {
			var dialer net.Dialer
			return dialer.DialContext(ctx, "unix", socketPath)
		}))
	} else if strings.HasPrefix(endpoint, "tcp://") {
		target = strings.TrimPrefix(endpoint, "tcp://")
	}

	conn, err := grpc.NewClient(target, options...)
	if err != nil {
		return nil, fmt.Errorf("dial engine endpoint: %w", err)
	}

	return &Client{conn: conn, schema: schema}, nil
}

func (c *Client) Close() error {
	return c.conn.Close()
}

func (c *Client) RunTool(ctx context.Context, target string, argumentsJSON string, allowNetwork bool, priority string) (*ExecuteResult, error) {
	request := c.schema.NewMessage("openpinch.v1.EngineToolRequest")
	pb.SetString(request.ProtoReflect(), "tool", target)
	pb.SetString(request.ProtoReflect(), "arguments_json", argumentsJSON)
	pb.SetBool(request.ProtoReflect(), "allow_network", allowNetwork)
	pb.SetString(request.ProtoReflect(), "priority", priority)

	response := c.schema.NewMessage("openpinch.v1.ExecuteResponse")
	if err := c.invoke(ctx, "/openpinch.v1.EngineRuntimeService/RunTool", request, response); err != nil {
		return nil, err
	}
	return decodeExecuteResult(response), nil
}

func (c *Client) RunSkill(ctx context.Context, skillID string, argumentsJSON string) (*ExecuteResult, error) {
	request := c.schema.NewMessage("openpinch.v1.EngineSkillRequest")
	pb.SetString(request.ProtoReflect(), "skill_id", skillID)
	pb.SetString(request.ProtoReflect(), "arguments_json", argumentsJSON)

	response := c.schema.NewMessage("openpinch.v1.ExecuteResponse")
	if err := c.invoke(ctx, "/openpinch.v1.EngineRuntimeService/RunSkill", request, response); err != nil {
		return nil, err
	}
	return decodeExecuteResult(response), nil
}

func (c *Client) DeliverMessage(ctx context.Context, message Message) (*MessageResult, error) {
	inner := c.schema.NewMessage("openpinch.v1.SubmitMessageRequest")
	pb.SetString(inner.ProtoReflect(), "connector", message.Connector)
	pb.SetString(inner.ProtoReflect(), "channel_id", message.ChannelID)
	pb.SetString(inner.ProtoReflect(), "sender", message.Sender)
	pb.SetString(inner.ProtoReflect(), "body", message.Body)
	pb.SetString(inner.ProtoReflect(), "metadata_json", message.MetadataJSON)

	request := c.schema.NewMessage("openpinch.v1.EngineMessageRequest")
	pb.SetMessage(request.ProtoReflect(), "message", inner)

	response := c.schema.NewMessage("openpinch.v1.SubmitMessageResponse")
	if err := c.invoke(ctx, "/openpinch.v1.EngineRuntimeService/DeliverMessage", request, response); err != nil {
		return nil, err
	}

	return &MessageResult{
		Accepted:      pb.GetBool(response.ProtoReflect(), "accepted"),
		MessageID:     pb.GetString(response.ProtoReflect(), "message_id"),
		Reply:         pb.GetString(response.ProtoReflect(), "reply"),
		SessionID:     pb.GetString(response.ProtoReflect(), "session_id"),
		PairingID:     pb.GetString(response.ProtoReflect(), "pairing_id"),
		DeliveryState: pb.GetString(response.ProtoReflect(), "delivery_state"),
	}, nil
}

func (c *Client) Health(ctx context.Context) (*HealthResult, error) {
	request := c.schema.NewMessage("openpinch.v1.Empty")
	response := c.schema.NewMessage("openpinch.v1.HealthResponse")
	if err := c.invoke(ctx, "/openpinch.v1.EngineRuntimeService/Health", request, response); err != nil {
		return nil, err
	}

	return &HealthResult{
		Status:               pb.GetString(response.ProtoReflect(), "status"),
		MissingPrerequisites: repeatedStrings(response.ProtoReflect(), "missing_prerequisites"),
		SandboxBackend:       pb.GetString(response.ProtoReflect(), "sandbox_backend"),
		VectorMemoryBackend:  pb.GetString(response.ProtoReflect(), "vector_memory_backend"),
		EncryptionState:      pb.GetString(response.ProtoReflect(), "encryption_state"),
		AuditMode:            pb.GetString(response.ProtoReflect(), "audit_mode"),
	}, nil
}

func (c *Client) QueryMemory(ctx context.Context, namespace string, query string, limit uint32, filterJSON string) ([]MemoryRecord, string, error) {
	request := c.schema.NewMessage("openpinch.v1.MemoryQueryRequest")
	pb.SetString(request.ProtoReflect(), "namespace", namespace)
	pb.SetString(request.ProtoReflect(), "query", query)
	pb.SetUint32(request.ProtoReflect(), "limit", limit)
	pb.SetString(request.ProtoReflect(), "filter_json", filterJSON)

	response := c.schema.NewMessage("openpinch.v1.MemoryQueryResponse")
	if err := c.invoke(ctx, "/openpinch.v1.EngineRuntimeService/QueryMemory", request, response); err != nil {
		return nil, "", err
	}

	field := pb.FieldByName(response.ProtoReflect(), "records")
	list := response.ProtoReflect().Get(field).List()
	records := make([]MemoryRecord, 0, list.Len())
	for i := 0; i < list.Len(); i++ {
		record := list.Get(i).Message()
		records = append(records, MemoryRecord{
			Key:          pb.GetString(record, "key"),
			Namespace:    pb.GetString(record, "namespace"),
			Content:      pb.GetString(record, "content"),
			MetadataJSON: pb.GetString(record, "metadata_json"),
			Score:        float64(record.Get(pb.FieldByName(record, "score")).Float()),
			CreatedAt:    pb.GetString(record, "created_at"),
		})
	}
	return records, pb.GetString(response.ProtoReflect(), "backend"), nil
}

func (c *Client) UpsertMemory(ctx context.Context, namespace string, key string, content string, metadataJSON string) (bool, string, string, error) {
	request := c.schema.NewMessage("openpinch.v1.MemoryUpsertRequest")
	pb.SetString(request.ProtoReflect(), "namespace", namespace)
	pb.SetString(request.ProtoReflect(), "key", key)
	pb.SetString(request.ProtoReflect(), "content", content)
	pb.SetString(request.ProtoReflect(), "metadata_json", metadataJSON)

	response := c.schema.NewMessage("openpinch.v1.MemoryUpsertResponse")
	if err := c.invoke(ctx, "/openpinch.v1.EngineRuntimeService/UpsertMemory", request, response); err != nil {
		return false, "", "", err
	}
	return pb.GetBool(response.ProtoReflect(), "stored"), pb.GetString(response.ProtoReflect(), "backend"), pb.GetString(response.ProtoReflect(), "digest"), nil
}

func (c *Client) ListSessions(ctx context.Context, connector string, status string, includeArchived bool, limit uint32) ([]SessionRecord, string, error) {
	request := c.schema.NewMessage("openpinch.v1.SessionListRequest")
	pb.SetString(request.ProtoReflect(), "connector", connector)
	pb.SetString(request.ProtoReflect(), "status", status)
	pb.SetBool(request.ProtoReflect(), "include_archived", includeArchived)
	pb.SetUint32(request.ProtoReflect(), "limit", limit)

	response := c.schema.NewMessage("openpinch.v1.SessionListResponse")
	if err := c.invoke(ctx, "/openpinch.v1.EngineRuntimeService/ListSessions", request, response); err != nil {
		return nil, "", err
	}
	return decodeSessions(response.ProtoReflect(), "sessions"), pb.GetString(response.ProtoReflect(), "summary"), nil
}

func (c *Client) GetSession(ctx context.Context, sessionID string, limit uint32) (*SessionDetail, error) {
	request := c.schema.NewMessage("openpinch.v1.SessionRequest")
	pb.SetString(request.ProtoReflect(), "session_id", sessionID)
	pb.SetUint32(request.ProtoReflect(), "limit", limit)

	response := c.schema.NewMessage("openpinch.v1.SessionResponse")
	if err := c.invoke(ctx, "/openpinch.v1.EngineRuntimeService/GetSession", request, response); err != nil {
		return nil, err
	}
	detail := &SessionDetail{}
	if field := pb.FieldByName(response.ProtoReflect(), "session"); response.ProtoReflect().Has(field) {
		detail.Session = decodeSessionRecord(response.ProtoReflect().Get(field).Message())
	}
	detail.Messages = decodeSessionMessages(response.ProtoReflect(), "messages")
	return detail, nil
}

func (c *Client) PruneSessions(ctx context.Context, olderThanHours uint32, archiveOnly bool) (uint32, error) {
	request := c.schema.NewMessage("openpinch.v1.SessionPruneRequest")
	pb.SetUint32(request.ProtoReflect(), "older_than_hours", olderThanHours)
	pb.SetBool(request.ProtoReflect(), "archive_only", archiveOnly)

	response := c.schema.NewMessage("openpinch.v1.SessionPruneResponse")
	if err := c.invoke(ctx, "/openpinch.v1.EngineRuntimeService/PruneSessions", request, response); err != nil {
		return 0, err
	}
	return pb.GetUint32(response.ProtoReflect(), "pruned"), nil
}

func (c *Client) ListPairings(ctx context.Context, status string, connector string, limit uint32) ([]PairingRecord, error) {
	request := c.schema.NewMessage("openpinch.v1.PairingListRequest")
	pb.SetString(request.ProtoReflect(), "status", status)
	pb.SetString(request.ProtoReflect(), "connector", connector)
	pb.SetUint32(request.ProtoReflect(), "limit", limit)

	response := c.schema.NewMessage("openpinch.v1.PairingListResponse")
	if err := c.invoke(ctx, "/openpinch.v1.EngineRuntimeService/ListPairings", request, response); err != nil {
		return nil, err
	}
	return decodePairings(response.ProtoReflect(), "pairings"), nil
}

func (c *Client) UpdatePairing(ctx context.Context, pairingID string, action string, note string) (*PairingRecord, error) {
	request := c.schema.NewMessage("openpinch.v1.PairingUpdateRequest")
	pb.SetString(request.ProtoReflect(), "pairing_id", pairingID)
	pb.SetString(request.ProtoReflect(), "action", action)
	pb.SetString(request.ProtoReflect(), "note", note)

	response := c.schema.NewMessage("openpinch.v1.PairingUpdateResponse")
	if err := c.invoke(ctx, "/openpinch.v1.EngineRuntimeService/UpdatePairing", request, response); err != nil {
		return nil, err
	}
	field := pb.FieldByName(response.ProtoReflect(), "pairing")
	if !response.ProtoReflect().Has(field) {
		return nil, fmt.Errorf("pairing update response missing pairing")
	}
	return decodePairingRecord(response.ProtoReflect().Get(field).Message()), nil
}

func (c *Client) RecordOutboundMessage(ctx context.Context, connector string, channelID string, sender string, body string, metadataJSON string, sessionID string) (*OutboundMessageResult, error) {
	request := c.schema.NewMessage("openpinch.v1.ChannelMessageRequest")
	pb.SetString(request.ProtoReflect(), "connector", connector)
	pb.SetString(request.ProtoReflect(), "channel_id", channelID)
	pb.SetString(request.ProtoReflect(), "sender", sender)
	pb.SetString(request.ProtoReflect(), "body", body)
	pb.SetString(request.ProtoReflect(), "metadata_json", metadataJSON)
	pb.SetString(request.ProtoReflect(), "session_id", sessionID)

	response := c.schema.NewMessage("openpinch.v1.ChannelMessageResponse")
	if err := c.invoke(ctx, "/openpinch.v1.EngineRuntimeService/RecordOutboundMessage", request, response); err != nil {
		return nil, err
	}
	return &OutboundMessageResult{
		Accepted:  pb.GetBool(response.ProtoReflect(), "accepted"),
		MessageID: pb.GetString(response.ProtoReflect(), "message_id"),
		SessionID: pb.GetString(response.ProtoReflect(), "session_id"),
		Status:    pb.GetString(response.ProtoReflect(), "status"),
		Detail:    pb.GetString(response.ProtoReflect(), "detail"),
	}, nil
}

func (c *Client) GetDoctorReport(ctx context.Context, includeConnectors bool, includeModels bool, includeWeb bool) (*DoctorReport, error) {
	request := c.schema.NewMessage("openpinch.v1.DoctorReportRequest")
	pb.SetBool(request.ProtoReflect(), "include_connectors", includeConnectors)
	pb.SetBool(request.ProtoReflect(), "include_models", includeModels)
	pb.SetBool(request.ProtoReflect(), "include_web", includeWeb)

	response := c.schema.NewMessage("openpinch.v1.DoctorReportResponse")
	if err := c.invoke(ctx, "/openpinch.v1.EngineRuntimeService/GetDoctorReport", request, response); err != nil {
		return nil, err
	}
	return &DoctorReport{
		Status:   pb.GetString(response.ProtoReflect(), "status"),
		Findings: decodeDoctorFindings(response.ProtoReflect(), "findings"),
	}, nil
}

func (c *Client) ListModelProfiles(ctx context.Context) ([]ModelProfile, error) {
	request := c.schema.NewMessage("openpinch.v1.Empty")
	response := c.schema.NewMessage("openpinch.v1.ModelProfileListResponse")
	if err := c.invoke(ctx, "/openpinch.v1.EngineRuntimeService/ListModelProfiles", request, response); err != nil {
		return nil, err
	}
	return decodeModelProfiles(response.ProtoReflect(), "profiles"), nil
}

func (c *Client) RememberBrain(ctx context.Context, kind string, subtype string, title string, content string, importance float64, scopeJSON string, linksJSON string, sourceRef string) (*BrainRememberResult, error) {
	request := c.schema.NewMessage("openpinch.v1.BrainRememberRequest")
	reflection := request.ProtoReflect()
	pb.SetString(reflection, "kind", kind)
	pb.SetString(reflection, "subtype", subtype)
	pb.SetString(reflection, "title", title)
	pb.SetString(reflection, "content", content)
	field := pb.FieldByName(reflection, "importance")
	reflection.Set(field, protoreflect.ValueOfFloat64(importance))
	pb.SetString(reflection, "scope_json", scopeJSON)
	pb.SetString(reflection, "links_json", linksJSON)
	pb.SetString(reflection, "source_ref", sourceRef)

	response := c.schema.NewMessage("openpinch.v1.BrainRememberResponse")
	if err := c.invoke(ctx, "/openpinch.v1.EngineRuntimeService/RememberBrain", request, response); err != nil {
		return nil, err
	}

	result := &BrainRememberResult{
		Stored: pb.GetBool(response.ProtoReflect(), "stored"),
		Digest: pb.GetString(response.ProtoReflect(), "digest"),
	}
	if entityField := pb.FieldByName(response.ProtoReflect(), "entity"); response.ProtoReflect().Has(entityField) {
		entity := response.ProtoReflect().Get(entityField).Message()
		result.Entity = decodeBrainEntity(entity)
	}
	if taskField := pb.FieldByName(response.ProtoReflect(), "task"); response.ProtoReflect().Has(taskField) {
		task := response.ProtoReflect().Get(taskField).Message()
		result.Task = decodeBrainTask(task)
	}
	return result, nil
}

func (c *Client) RecallBrain(ctx context.Context, query string, scopeJSON string, limit uint32, includeArchived bool) (*BrainRecallResult, error) {
	request := c.schema.NewMessage("openpinch.v1.BrainRecallRequest")
	pb.SetString(request.ProtoReflect(), "query", query)
	pb.SetString(request.ProtoReflect(), "scope_json", scopeJSON)
	pb.SetUint32(request.ProtoReflect(), "limit", limit)
	pb.SetBool(request.ProtoReflect(), "include_archived", includeArchived)

	response := c.schema.NewMessage("openpinch.v1.BrainRecallResponse")
	if err := c.invoke(ctx, "/openpinch.v1.EngineRuntimeService/RecallBrain", request, response); err != nil {
		return nil, err
	}
	result := &BrainRecallResult{
		Summary: pb.GetString(response.ProtoReflect(), "summary"),
	}
	result.Entities = decodeBrainEntities(response.ProtoReflect(), "entities")
	result.Facts = decodeBrainFacts(response.ProtoReflect(), "facts")
	result.Relations = decodeBrainRelations(response.ProtoReflect(), "relations")
	result.Tasks = decodeBrainTasks(response.ProtoReflect(), "tasks")
	return result, nil
}

func (c *Client) SuggestBrain(ctx context.Context, scopeJSON string, limit uint32) (*BrainSuggestResult, error) {
	request := c.schema.NewMessage("openpinch.v1.BrainSuggestRequest")
	pb.SetString(request.ProtoReflect(), "scope_json", scopeJSON)
	pb.SetUint32(request.ProtoReflect(), "limit", limit)

	response := c.schema.NewMessage("openpinch.v1.BrainSuggestResponse")
	if err := c.invoke(ctx, "/openpinch.v1.EngineRuntimeService/SuggestBrain", request, response); err != nil {
		return nil, err
	}
	return &BrainSuggestResult{
		Summary:     pb.GetString(response.ProtoReflect(), "summary"),
		Suggestions: decodeBrainSuggestions(response.ProtoReflect(), "suggestions"),
	}, nil
}

func (c *Client) ListBrainTasks(ctx context.Context, scopeJSON string, statuses []string, priorities []string, dueBefore string, limit uint32) (*BrainTaskListResult, error) {
	request := c.schema.NewMessage("openpinch.v1.BrainTaskListRequest")
	reflection := request.ProtoReflect()
	pb.SetString(reflection, "scope_json", scopeJSON)
	for _, statusValue := range statuses {
		pb.AddString(reflection, "statuses", statusValue)
	}
	for _, priority := range priorities {
		pb.AddString(reflection, "priorities", priority)
	}
	pb.SetString(reflection, "due_before", dueBefore)
	pb.SetUint32(reflection, "limit", limit)

	response := c.schema.NewMessage("openpinch.v1.BrainTaskListResponse")
	if err := c.invoke(ctx, "/openpinch.v1.EngineRuntimeService/ListBrainTasks", request, response); err != nil {
		return nil, err
	}
	return &BrainTaskListResult{
		Summary: pb.GetString(response.ProtoReflect(), "summary"),
		Tasks:   decodeBrainTasks(response.ProtoReflect(), "tasks"),
	}, nil
}

func (c *Client) UpdateBrainTask(ctx context.Context, taskID string, status string, priority string, dueAt string, summary string, linksJSON string, sourceRef string) (*BrainTask, error) {
	request := c.schema.NewMessage("openpinch.v1.BrainTaskUpdateRequest")
	reflection := request.ProtoReflect()
	pb.SetString(reflection, "task_id", taskID)
	pb.SetString(reflection, "status", status)
	pb.SetString(reflection, "priority", priority)
	pb.SetString(reflection, "due_at", dueAt)
	pb.SetString(reflection, "summary", summary)
	pb.SetString(reflection, "links_json", linksJSON)
	pb.SetString(reflection, "source_ref", sourceRef)

	response := c.schema.NewMessage("openpinch.v1.BrainTaskUpdateResponse")
	if err := c.invoke(ctx, "/openpinch.v1.EngineRuntimeService/UpdateBrainTask", request, response); err != nil {
		return nil, err
	}
	taskField := pb.FieldByName(response.ProtoReflect(), "task")
	if !response.ProtoReflect().Has(taskField) {
		return nil, fmt.Errorf("brain task update response missing task")
	}
	return decodeBrainTask(response.ProtoReflect().Get(taskField).Message()), nil
}

func (c *Client) ForgetBrain(ctx context.Context, targetKind string, targetID string, mode string, reason string) (*BrainForgetResult, error) {
	request := c.schema.NewMessage("openpinch.v1.BrainForgetRequest")
	reflection := request.ProtoReflect()
	pb.SetString(reflection, "target_kind", targetKind)
	pb.SetString(reflection, "target_id", targetID)
	pb.SetString(reflection, "mode", mode)
	pb.SetString(reflection, "reason", reason)

	response := c.schema.NewMessage("openpinch.v1.BrainForgetResponse")
	if err := c.invoke(ctx, "/openpinch.v1.EngineRuntimeService/ForgetBrain", request, response); err != nil {
		return nil, err
	}
	return &BrainForgetResult{
		Forgotten: pb.GetBool(response.ProtoReflect(), "forgotten"),
		Mode:      pb.GetString(response.ProtoReflect(), "mode"),
		TargetID:  pb.GetString(response.ProtoReflect(), "target_id"),
	}, nil
}

func (c *Client) AttestSession(ctx context.Context, subject string, nonce string, includeHardware bool) (*AttestationResult, error) {
	request := c.schema.NewMessage("openpinch.v1.AttestationRequest")
	pb.SetString(request.ProtoReflect(), "subject", subject)
	pb.SetString(request.ProtoReflect(), "nonce", nonce)
	pb.SetBool(request.ProtoReflect(), "include_hardware", includeHardware)

	response := c.schema.NewMessage("openpinch.v1.AttestationResponse")
	if err := c.invoke(ctx, "/openpinch.v1.EngineRuntimeService/AttestSession", request, response); err != nil {
		return nil, err
	}

	return &AttestationResult{
		Status:         pb.GetString(response.ProtoReflect(), "status"),
		Subject:        pb.GetString(response.ProtoReflect(), "subject"),
		Platform:       pb.GetString(response.ProtoReflect(), "platform"),
		HardwareBacked: pb.GetBool(response.ProtoReflect(), "hardware_backed"),
		Nonce:          pb.GetString(response.ProtoReflect(), "nonce"),
		PublicKey:      pb.GetString(response.ProtoReflect(), "public_key"),
		Measurements:   stringMap(response.ProtoReflect(), "measurements"),
	}, nil
}

func (c *Client) ExportAudit(ctx context.Context, sink string, limit uint32) ([]AuditEvent, string, error) {
	request := c.schema.NewMessage("openpinch.v1.AuditExportRequest")
	pb.SetString(request.ProtoReflect(), "sink", sink)
	pb.SetUint32(request.ProtoReflect(), "limit", limit)

	response := c.schema.NewMessage("openpinch.v1.AuditExportResponse")
	if err := c.invoke(ctx, "/openpinch.v1.EngineRuntimeService/ExportAudit", request, response); err != nil {
		return nil, "", err
	}

	field := pb.FieldByName(response.ProtoReflect(), "events")
	list := response.ProtoReflect().Get(field).List()
	events := make([]AuditEvent, 0, list.Len())
	for i := 0; i < list.Len(); i++ {
		event := list.Get(i).Message()
		events = append(events, AuditEvent{
			ID:           pb.GetString(event, "id"),
			Category:     pb.GetString(event, "category"),
			Severity:     pb.GetString(event, "severity"),
			Summary:      pb.GetString(event, "summary"),
			AnomalyScore: float64(event.Get(pb.FieldByName(event, "anomaly_score")).Float()),
			PayloadJSON:  pb.GetString(event, "payload_json"),
			CreatedAt:    pb.GetString(event, "created_at"),
		})
	}
	return events, pb.GetString(response.ProtoReflect(), "format"), nil
}

func (c *Client) QueueTask(ctx context.Context, taskID string, taskType string, target string, argumentsJSON string, priority string) (*QueueReceipt, error) {
	request := c.schema.NewMessage("openpinch.v1.QueueTaskRequest")
	pb.SetString(request.ProtoReflect(), "task_id", taskID)
	pb.SetString(request.ProtoReflect(), "task_type", taskType)
	pb.SetString(request.ProtoReflect(), "target", target)
	pb.SetString(request.ProtoReflect(), "arguments_json", argumentsJSON)
	pb.SetString(request.ProtoReflect(), "priority", priority)

	response := c.schema.NewMessage("openpinch.v1.QueueTaskResponse")
	if err := c.invoke(ctx, "/openpinch.v1.EngineRuntimeService/QueueTask", request, response); err != nil {
		return nil, err
	}
	return &QueueReceipt{
		Accepted: pb.GetBool(response.ProtoReflect(), "accepted"),
		TaskID:   pb.GetString(response.ProtoReflect(), "task_id"),
		Queue:    pb.GetString(response.ProtoReflect(), "queue"),
		Message:  pb.GetString(response.ProtoReflect(), "message"),
	}, nil
}

func (c *Client) GetPolicyReport(ctx context.Context, subject string, capability string) (*PolicyReport, error) {
	request := c.schema.NewMessage("openpinch.v1.PolicyReportRequest")
	pb.SetString(request.ProtoReflect(), "subject", subject)
	pb.SetString(request.ProtoReflect(), "capability", capability)

	response := c.schema.NewMessage("openpinch.v1.PolicyReportResponse")
	if err := c.invoke(ctx, "/openpinch.v1.EngineRuntimeService/GetPolicyReport", request, response); err != nil {
		return nil, err
	}
	return &PolicyReport{
		Subject:             pb.GetString(response.ProtoReflect(), "subject"),
		AllowedCapabilities: repeatedStrings(response.ProtoReflect(), "allowed_capabilities"),
		DeniedCapabilities:  repeatedStrings(response.ProtoReflect(), "denied_capabilities"),
		Source:              pb.GetString(response.ProtoReflect(), "source"),
	}, nil
}

func (c *Client) RunAgentProtocol(ctx context.Context, protocolID string, initiator string, messages []AgentMessage, policyScope string) (*AgentProtocolResult, error) {
	request := c.schema.NewMessage("openpinch.v1.AgentProtocolRequest")
	pb.SetString(request.ProtoReflect(), "protocol_id", protocolID)
	pb.SetString(request.ProtoReflect(), "initiator", initiator)
	pb.SetString(request.ProtoReflect(), "policy_scope", policyScope)
	field := pb.FieldByName(request.ProtoReflect(), "messages")
	list := request.ProtoReflect().Mutable(field).List()
	for _, message := range messages {
		entry := c.schema.NewMessage("openpinch.v1.AgentMessage")
		pb.SetString(entry.ProtoReflect(), "sender", message.Sender)
		pb.SetString(entry.ProtoReflect(), "recipient", message.Recipient)
		pb.SetString(entry.ProtoReflect(), "body", message.Body)
		pb.SetString(entry.ProtoReflect(), "metadata_json", message.MetadataJSON)
		pb.SetString(entry.ProtoReflect(), "encrypted_body", message.EncryptedBody)
		list.Append(protoreflect.ValueOfMessage(entry.ProtoReflect()))
	}

	response := c.schema.NewMessage("openpinch.v1.AgentProtocolResponse")
	if err := c.invoke(ctx, "/openpinch.v1.EngineRuntimeService/RunAgentProtocol", request, response); err != nil {
		return nil, err
	}
	return &AgentProtocolResult{
		Accepted:       pb.GetBool(response.ProtoReflect(), "accepted"),
		ProtocolID:     pb.GetString(response.ProtoReflect(), "protocol_id"),
		TranscriptJSON: pb.GetString(response.ProtoReflect(), "transcript_json"),
		Findings:       repeatedStrings(response.ProtoReflect(), "findings"),
	}, nil
}

func (c *Client) invoke(ctx context.Context, method string, request proto.Message, response proto.Message) error {
	callCtx, cancel := context.WithTimeout(ctx, 10*time.Second)
	defer cancel()
	return c.conn.Invoke(callCtx, method, request, response)
}

func decodeExecuteResult(message proto.Message) *ExecuteResult {
	reflection := message.ProtoReflect()
	return &ExecuteResult{
		Success:  pb.GetBool(reflection, "success"),
		Summary:  pb.GetString(reflection, "summary"),
		DataJSON: pb.GetString(reflection, "data_json"),
		Error:    pb.GetString(reflection, "error"),
		Logs:     repeatedStrings(reflection, "logs"),
	}
}

func repeatedStrings(message protoreflect.Message, name string) []string {
	field := pb.FieldByName(message, name)
	list := message.Get(field).List()
	values := make([]string, 0, list.Len())
	for i := 0; i < list.Len(); i++ {
		values = append(values, list.Get(i).String())
	}
	return values
}

func stringMap(message protoreflect.Message, name string) map[string]string {
	field := pb.FieldByName(message, name)
	target := message.Get(field).Map()
	values := map[string]string{}
	target.Range(func(key protoreflect.MapKey, value protoreflect.Value) bool {
		values[key.String()] = value.String()
		return true
	})
	return values
}

func decodeSessionRecord(message protoreflect.Message) *SessionRecord {
	return &SessionRecord{
		ID:                 pb.GetString(message, "id"),
		Connector:          pb.GetString(message, "connector"),
		ChannelID:          pb.GetString(message, "channel_id"),
		Participant:        pb.GetString(message, "participant"),
		SessionType:        pb.GetString(message, "session_type"),
		Title:              pb.GetString(message, "title"),
		Status:             pb.GetString(message, "status"),
		ReplyMode:          pb.GetString(message, "reply_mode"),
		QueueMode:          pb.GetString(message, "queue_mode"),
		ModelProfile:       pb.GetString(message, "model_profile"),
		MentionOnly:        pb.GetBool(message, "mention_only"),
		PendingPairing:     pb.GetBool(message, "pending_pairing"),
		LastMessagePreview: pb.GetString(message, "last_message_preview"),
		MessageCount:       pb.GetUint32(message, "message_count"),
		CreatedAt:          pb.GetString(message, "created_at"),
		UpdatedAt:          pb.GetString(message, "updated_at"),
	}
}

func decodeSessionMessage(message protoreflect.Message) SessionMessage {
	return SessionMessage{
		ID:           pb.GetString(message, "id"),
		SessionID:    pb.GetString(message, "session_id"),
		Connector:    pb.GetString(message, "connector"),
		Role:         pb.GetString(message, "role"),
		Sender:       pb.GetString(message, "sender"),
		Body:         pb.GetString(message, "body"),
		MetadataJSON: pb.GetString(message, "metadata_json"),
		CreatedAt:    pb.GetString(message, "created_at"),
	}
}

func decodePairingRecord(message protoreflect.Message) *PairingRecord {
	return &PairingRecord{
		ID:        pb.GetString(message, "id"),
		Connector: pb.GetString(message, "connector"),
		ChannelID: pb.GetString(message, "channel_id"),
		Sender:    pb.GetString(message, "sender"),
		SessionID: pb.GetString(message, "session_id"),
		Status:    pb.GetString(message, "status"),
		Reason:    pb.GetString(message, "reason"),
		CreatedAt: pb.GetString(message, "created_at"),
		UpdatedAt: pb.GetString(message, "updated_at"),
	}
}

func decodeDoctorFinding(message protoreflect.Message) DoctorFinding {
	return DoctorFinding{
		ID:        pb.GetString(message, "id"),
		Component: pb.GetString(message, "component"),
		Severity:  pb.GetString(message, "severity"),
		Status:    pb.GetString(message, "status"),
		Summary:   pb.GetString(message, "summary"),
		Detail:    pb.GetString(message, "detail"),
	}
}

func decodeModelProfile(message protoreflect.Message) ModelProfile {
	return ModelProfile{
		Name:           pb.GetString(message, "name"),
		ProviderOrder:  repeatedStrings(message, "provider_order"),
		Mode:           pb.GetString(message, "mode"),
		TimeoutSeconds: pb.GetUint32(message, "timeout_seconds"),
		RetryBudget:    pb.GetUint32(message, "retry_budget"),
		Hosted:         pb.GetBool(message, "hosted"),
		AuthMode:       pb.GetString(message, "auth_mode"),
		DefaultProfile: pb.GetBool(message, "default_profile"),
	}
}

func decodeBrainEntity(message protoreflect.Message) *BrainEntity {
	return &BrainEntity{
		ID:         pb.GetString(message, "id"),
		Kind:       pb.GetString(message, "kind"),
		Subtype:    pb.GetString(message, "subtype"),
		Title:      pb.GetString(message, "title"),
		Content:    pb.GetString(message, "content"),
		ScopeJSON:  pb.GetString(message, "scope_json"),
		LinksJSON:  pb.GetString(message, "links_json"),
		Salience:   float64(message.Get(pb.FieldByName(message, "salience")).Float()),
		Confidence: float64(message.Get(pb.FieldByName(message, "confidence")).Float()),
		Archived:   pb.GetBool(message, "archived"),
		CreatedAt:  pb.GetString(message, "created_at"),
		UpdatedAt:  pb.GetString(message, "updated_at"),
	}
}

func decodeBrainFact(message protoreflect.Message) BrainFact {
	return BrainFact{
		ID:         pb.GetString(message, "id"),
		EntityID:   pb.GetString(message, "entity_id"),
		Content:    pb.GetString(message, "content"),
		ScopeJSON:  pb.GetString(message, "scope_json"),
		Salience:   float64(message.Get(pb.FieldByName(message, "salience")).Float()),
		Confidence: float64(message.Get(pb.FieldByName(message, "confidence")).Float()),
		Archived:   pb.GetBool(message, "archived"),
		CreatedAt:  pb.GetString(message, "created_at"),
		UpdatedAt:  pb.GetString(message, "updated_at"),
	}
}

func decodeBrainRelation(message protoreflect.Message) BrainRelation {
	return BrainRelation{
		ID:           pb.GetString(message, "id"),
		Kind:         pb.GetString(message, "kind"),
		FromID:       pb.GetString(message, "from_id"),
		ToID:         pb.GetString(message, "to_id"),
		MetadataJSON: pb.GetString(message, "metadata_json"),
		Confidence:   float64(message.Get(pb.FieldByName(message, "confidence")).Float()),
		CreatedAt:    pb.GetString(message, "created_at"),
		UpdatedAt:    pb.GetString(message, "updated_at"),
	}
}

func decodeBrainTask(message protoreflect.Message) *BrainTask {
	return &BrainTask{
		ID:         pb.GetString(message, "id"),
		Title:      pb.GetString(message, "title"),
		Summary:    pb.GetString(message, "summary"),
		Status:     pb.GetString(message, "status"),
		Priority:   pb.GetString(message, "priority"),
		DueAt:      pb.GetString(message, "due_at"),
		ScopeJSON:  pb.GetString(message, "scope_json"),
		LinksJSON:  pb.GetString(message, "links_json"),
		Salience:   float64(message.Get(pb.FieldByName(message, "salience")).Float()),
		Confidence: float64(message.Get(pb.FieldByName(message, "confidence")).Float()),
		Archived:   pb.GetBool(message, "archived"),
		CreatedAt:  pb.GetString(message, "created_at"),
		UpdatedAt:  pb.GetString(message, "updated_at"),
	}
}

func decodeBrainSuggestion(message protoreflect.Message) BrainSuggestion {
	return BrainSuggestion{
		ID:          pb.GetString(message, "id"),
		TaskID:      pb.GetString(message, "task_id"),
		Summary:     pb.GetString(message, "summary"),
		Reason:      pb.GetString(message, "reason"),
		Score:       float64(message.Get(pb.FieldByName(message, "score")).Float()),
		ContextJSON: pb.GetString(message, "context_json"),
	}
}

func decodeBrainEntities(message protoreflect.Message, name string) []BrainEntity {
	field := pb.FieldByName(message, name)
	list := message.Get(field).List()
	values := make([]BrainEntity, 0, list.Len())
	for i := 0; i < list.Len(); i++ {
		values = append(values, *decodeBrainEntity(list.Get(i).Message()))
	}
	return values
}

func decodeBrainFacts(message protoreflect.Message, name string) []BrainFact {
	field := pb.FieldByName(message, name)
	list := message.Get(field).List()
	values := make([]BrainFact, 0, list.Len())
	for i := 0; i < list.Len(); i++ {
		values = append(values, decodeBrainFact(list.Get(i).Message()))
	}
	return values
}

func decodeBrainRelations(message protoreflect.Message, name string) []BrainRelation {
	field := pb.FieldByName(message, name)
	list := message.Get(field).List()
	values := make([]BrainRelation, 0, list.Len())
	for i := 0; i < list.Len(); i++ {
		values = append(values, decodeBrainRelation(list.Get(i).Message()))
	}
	return values
}

func decodeBrainTasks(message protoreflect.Message, name string) []BrainTask {
	field := pb.FieldByName(message, name)
	list := message.Get(field).List()
	values := make([]BrainTask, 0, list.Len())
	for i := 0; i < list.Len(); i++ {
		values = append(values, *decodeBrainTask(list.Get(i).Message()))
	}
	return values
}

func decodeBrainSuggestions(message protoreflect.Message, name string) []BrainSuggestion {
	field := pb.FieldByName(message, name)
	list := message.Get(field).List()
	values := make([]BrainSuggestion, 0, list.Len())
	for i := 0; i < list.Len(); i++ {
		values = append(values, decodeBrainSuggestion(list.Get(i).Message()))
	}
	return values
}

func decodeSessions(message protoreflect.Message, name string) []SessionRecord {
	field := pb.FieldByName(message, name)
	list := message.Get(field).List()
	values := make([]SessionRecord, 0, list.Len())
	for i := 0; i < list.Len(); i++ {
		values = append(values, *decodeSessionRecord(list.Get(i).Message()))
	}
	return values
}

func decodeSessionMessages(message protoreflect.Message, name string) []SessionMessage {
	field := pb.FieldByName(message, name)
	list := message.Get(field).List()
	values := make([]SessionMessage, 0, list.Len())
	for i := 0; i < list.Len(); i++ {
		values = append(values, decodeSessionMessage(list.Get(i).Message()))
	}
	return values
}

func decodePairings(message protoreflect.Message, name string) []PairingRecord {
	field := pb.FieldByName(message, name)
	list := message.Get(field).List()
	values := make([]PairingRecord, 0, list.Len())
	for i := 0; i < list.Len(); i++ {
		values = append(values, *decodePairingRecord(list.Get(i).Message()))
	}
	return values
}

func decodeDoctorFindings(message protoreflect.Message, name string) []DoctorFinding {
	field := pb.FieldByName(message, name)
	list := message.Get(field).List()
	values := make([]DoctorFinding, 0, list.Len())
	for i := 0; i < list.Len(); i++ {
		values = append(values, decodeDoctorFinding(list.Get(i).Message()))
	}
	return values
}

func decodeModelProfiles(message protoreflect.Message, name string) []ModelProfile {
	field := pb.FieldByName(message, name)
	list := message.Get(field).List()
	values := make([]ModelProfile, 0, list.Len())
	for i := 0; i < list.Len(); i++ {
		values = append(values, decodeModelProfile(list.Get(i).Message()))
	}
	return values
}
