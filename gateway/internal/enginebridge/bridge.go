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
	Accepted  bool
	MessageID string
	Reply     string
}

type MemoryRecord struct {
	Key          string
	Namespace    string
	Content      string
	MetadataJSON string
	Score        float64
	CreatedAt    string
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
		Accepted:  pb.GetBool(response.ProtoReflect(), "accepted"),
		MessageID: pb.GetString(response.ProtoReflect(), "message_id"),
		Reply:     pb.GetString(response.ProtoReflect(), "reply"),
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
