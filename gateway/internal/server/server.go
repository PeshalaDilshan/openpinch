package server

import (
	"bufio"
	"context"
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"time"

	"github.com/PeshalaDilshan/openpinch/gateway/internal/config"
	"github.com/PeshalaDilshan/openpinch/gateway/internal/connectors"
	"github.com/PeshalaDilshan/openpinch/gateway/internal/enginebridge"
	"github.com/PeshalaDilshan/openpinch/gateway/internal/scheduler"
	"github.com/PeshalaDilshan/openpinch/gateway/pkg/pb"
	"google.golang.org/grpc"
	"google.golang.org/grpc/codes"
	"google.golang.org/grpc/status"
	"google.golang.org/protobuf/proto"
	"google.golang.org/protobuf/reflect/protoreflect"
)

type Service struct {
	cfg       *config.Config
	bridge    *enginebridge.Client
	scheduler *scheduler.Scheduler
	registry  *connectors.Registry
	schema    *pb.Schema
	startedAt time.Time
}

type GatewayServiceServer interface {
	GetStatus(context.Context, proto.Message) (proto.Message, error)
	Execute(context.Context, proto.Message) (proto.Message, error)
	ListSkills(context.Context, proto.Message) (proto.Message, error)
	InstallSkill(context.Context, proto.Message) (proto.Message, error)
	SubmitMessage(context.Context, proto.Message) (proto.Message, error)
	ScheduleJob(context.Context, proto.Message) (proto.Message, error)
	ListConnectors(context.Context, proto.Message) (proto.Message, error)
	GetConnectorStatus(context.Context, proto.Message) (proto.Message, error)
	AttestSession(context.Context, proto.Message) (proto.Message, error)
	ExportAudit(context.Context, proto.Message) (proto.Message, error)
	QueryMemory(context.Context, proto.Message) (proto.Message, error)
	UpsertMemory(context.Context, proto.Message) (proto.Message, error)
	RunAgentProtocol(context.Context, proto.Message) (proto.Message, error)
	QueueTask(context.Context, proto.Message) (proto.Message, error)
	GetPolicyReport(context.Context, proto.Message) (proto.Message, error)
	TailLogs(proto.Message, grpc.ServerStream) error
}

func New(cfg *config.Config, bridge *enginebridge.Client, scheduler *scheduler.Scheduler, registry *connectors.Registry) (*Service, error) {
	schema, err := pb.Load()
	if err != nil {
		return nil, err
	}
	return &Service{
		cfg:       cfg,
		bridge:    bridge,
		scheduler: scheduler,
		registry:  registry,
		schema:    schema,
		startedAt: time.Now().UTC(),
	}, nil
}

func (s *Service) Register(server *grpc.Server) error {
	server.RegisterService(&grpc.ServiceDesc{
		ServiceName: "openpinch.v1.GatewayService",
		HandlerType: (*GatewayServiceServer)(nil),
		Methods: []grpc.MethodDesc{
			{MethodName: "GetStatus", Handler: unaryHandler("openpinch.v1.Empty", s.GetStatus)},
			{MethodName: "Execute", Handler: unaryHandler("openpinch.v1.ExecuteRequest", s.Execute)},
			{MethodName: "ListSkills", Handler: unaryHandler("openpinch.v1.Empty", s.ListSkills)},
			{MethodName: "InstallSkill", Handler: unaryHandler("openpinch.v1.InstallSkillRequest", s.InstallSkill)},
			{MethodName: "SubmitMessage", Handler: unaryHandler("openpinch.v1.SubmitMessageRequest", s.SubmitMessage)},
			{MethodName: "ScheduleJob", Handler: unaryHandler("openpinch.v1.ScheduleJobRequest", s.ScheduleJob)},
			{MethodName: "ListConnectors", Handler: unaryHandler("openpinch.v1.Empty", s.ListConnectors)},
			{MethodName: "GetConnectorStatus", Handler: unaryHandler("openpinch.v1.ConnectorStatusRequest", s.GetConnectorStatus)},
			{MethodName: "AttestSession", Handler: unaryHandler("openpinch.v1.AttestationRequest", s.AttestSession)},
			{MethodName: "ExportAudit", Handler: unaryHandler("openpinch.v1.AuditExportRequest", s.ExportAudit)},
			{MethodName: "QueryMemory", Handler: unaryHandler("openpinch.v1.MemoryQueryRequest", s.QueryMemory)},
			{MethodName: "UpsertMemory", Handler: unaryHandler("openpinch.v1.MemoryUpsertRequest", s.UpsertMemory)},
			{MethodName: "RunAgentProtocol", Handler: unaryHandler("openpinch.v1.AgentProtocolRequest", s.RunAgentProtocol)},
			{MethodName: "QueueTask", Handler: unaryHandler("openpinch.v1.QueueTaskRequest", s.QueueTask)},
			{MethodName: "GetPolicyReport", Handler: unaryHandler("openpinch.v1.PolicyReportRequest", s.GetPolicyReport)},
		},
		Streams: []grpc.StreamDesc{
			{StreamName: "TailLogs", Handler: streamHandler("openpinch.v1.LogRequest", s.TailLogs), ServerStreams: true},
		},
	}, s)
	return nil
}

func (s *Service) GetStatus(ctx context.Context, _ proto.Message) (proto.Message, error) {
	health, err := s.bridge.Health(ctx)
	if err != nil {
		return nil, status.Errorf(codes.Unavailable, "engine health: %v", err)
	}

	message := s.schema.NewMessage("openpinch.v1.StatusResponse")
	reflection := message.ProtoReflect()
	pb.SetString(reflection, "status", health.Status)
	pb.SetString(reflection, "version", "0.2.0")
	pb.SetString(reflection, "runtime_endpoint", s.cfg.Gateway.EngineEndpoint)
	pb.SetString(reflection, "gateway_endpoint", s.cfg.Gateway.ListenAddress)
	for _, connector := range s.registry.EnabledNames() {
		pb.AddString(reflection, "enabled_connectors", connector)
	}
	pb.AddString(reflection, "available_model_backends", "ollama")
	pb.AddString(reflection, "available_model_backends", "llamacpp")
	pb.AddString(reflection, "available_model_backends", "localai")
	pb.SetInt64(reflection, "uptime_seconds", int64(time.Since(s.startedAt).Seconds()))
	pb.SetString(reflection, "started_at", s.startedAt.Format(time.RFC3339))
	pb.SetString(reflection, "data_dir", s.cfg.Paths.DataDir)
	pb.SetString(reflection, "log_file", s.cfg.Paths.LogFile)
	pb.SetString(reflection, "vector_memory_backend", health.VectorMemoryBackend)
	pb.SetString(reflection, "encryption_state", health.EncryptionState)
	pb.SetString(reflection, "audit_mode", health.AuditMode)
	pb.SetString(reflection, "attestation_state", attestationState(s.cfg))
	return message, nil
}

func (s *Service) Execute(ctx context.Context, request proto.Message) (proto.Message, error) {
	reflection := request.ProtoReflect()
	result, err := s.bridge.RunTool(
		ctx,
		pb.GetString(reflection, "target"),
		pb.GetString(reflection, "arguments_json"),
		pb.GetBool(reflection, "allow_network"),
		pb.GetString(reflection, "priority"),
	)
	if err != nil {
		return nil, status.Errorf(codes.Internal, "execute tool: %v", err)
	}
	return executeResultMessage(s.schema, result), nil
}

func (s *Service) ListSkills(_ context.Context, _ proto.Message) (proto.Message, error) {
	indexPath := resolvePath(s.cfg.Skills.RegistryIndex)
	raw, err := os.ReadFile(indexPath)
	if err != nil {
		return nil, status.Errorf(codes.Internal, "read registry: %v", err)
	}

	var index registryIndex
	if err := json.Unmarshal(raw, &index); err != nil {
		return nil, status.Errorf(codes.Internal, "parse registry: %v", err)
	}

	response := s.schema.NewMessage("openpinch.v1.SkillListResponse")
	reflection := response.ProtoReflect()
	pb.SetString(reflection, "registry_version", index.Version)
	field := pb.FieldByName(reflection, "skills")
	list := reflection.Mutable(field).List()

	for _, skill := range index.Skills {
		entry := s.schema.NewMessage("openpinch.v1.SkillInfo")
		entryRef := entry.ProtoReflect()
		pb.SetString(entryRef, "id", skill.ID)
		pb.SetString(entryRef, "version", skill.Version)
		pb.SetString(entryRef, "name", skill.Name)
		pb.SetString(entryRef, "description", skill.Description)
		pb.SetString(entryRef, "entrypoint", skill.Entrypoint)
		pb.SetString(entryRef, "language", skill.Language)
		pb.SetBool(entryRef, "verified", true)
		pb.SetBool(entryRef, "installed", installedSkillExists(s.cfg.Paths.InstallsDir, skill.ID, skill.Version))
		list.Append(protoreflect.ValueOfMessage(entryRef))
	}

	return response, nil
}

func (s *Service) InstallSkill(_ context.Context, request proto.Message) (proto.Message, error) {
	reflection := request.ProtoReflect()
	response := s.schema.NewMessage("openpinch.v1.InstallSkillResponse")
	responseRef := response.ProtoReflect()
	pb.SetBool(responseRef, "installed", false)
	pb.SetString(
		responseRef,
		"message",
		fmt.Sprintf("gateway install is disabled for %s; use `openpinch skill install` locally", pb.GetString(reflection, "source")),
	)
	return response, nil
}

func (s *Service) SubmitMessage(ctx context.Context, request proto.Message) (proto.Message, error) {
	reflection := request.ProtoReflect()
	result, err := s.bridge.DeliverMessage(ctx, enginebridge.Message{
		Connector:    pb.GetString(reflection, "connector"),
		ChannelID:    pb.GetString(reflection, "channel_id"),
		Sender:       pb.GetString(reflection, "sender"),
		Body:         pb.GetString(reflection, "body"),
		MetadataJSON: pb.GetString(reflection, "metadata_json"),
	})
	if err != nil {
		return nil, status.Errorf(codes.Internal, "deliver message: %v", err)
	}

	response := s.schema.NewMessage("openpinch.v1.SubmitMessageResponse")
	responseRef := response.ProtoReflect()
	pb.SetBool(responseRef, "accepted", result.Accepted)
	pb.SetString(responseRef, "message_id", result.MessageID)
	pb.SetString(responseRef, "reply", result.Reply)
	return response, nil
}

func (s *Service) ScheduleJob(_ context.Context, request proto.Message) (proto.Message, error) {
	reflection := request.ProtoReflect()
	if err := s.scheduler.Schedule(
		pb.GetString(reflection, "job_id"),
		pb.GetString(reflection, "cron"),
		pb.GetString(reflection, "tool"),
		pb.GetString(reflection, "arguments_json"),
	); err != nil {
		return nil, status.Errorf(codes.InvalidArgument, "schedule job: %v", err)
	}

	response := s.schema.NewMessage("openpinch.v1.ScheduleJobResponse")
	responseRef := response.ProtoReflect()
	pb.SetBool(responseRef, "accepted", true)
	pb.SetString(responseRef, "message", "job scheduled")
	return response, nil
}

func (s *Service) ListConnectors(_ context.Context, _ proto.Message) (proto.Message, error) {
	response := s.schema.NewMessage("openpinch.v1.ConnectorListResponse")
	reflection := response.ProtoReflect()
	field := pb.FieldByName(reflection, "connectors")
	list := reflection.Mutable(field).List()
	for _, descriptor := range s.registry.DescribeAll() {
		entry := connectorInfoMessage(s.schema, descriptor)
		list.Append(protoreflect.ValueOfMessage(entry.ProtoReflect()))
	}
	return response, nil
}

func (s *Service) GetConnectorStatus(_ context.Context, request proto.Message) (proto.Message, error) {
	name := pb.GetString(request.ProtoReflect(), "name")
	descriptor, ok := s.registry.Status(name)
	if !ok {
		return nil, status.Errorf(codes.NotFound, "connector %s not found", name)
	}
	response := s.schema.NewMessage("openpinch.v1.ConnectorStatusResponse")
	pb.SetMessage(response.ProtoReflect(), "connector", connectorInfoMessage(s.schema, descriptor))
	return response, nil
}

func (s *Service) AttestSession(ctx context.Context, request proto.Message) (proto.Message, error) {
	reflection := request.ProtoReflect()
	result, err := s.bridge.AttestSession(
		ctx,
		pb.GetString(reflection, "subject"),
		pb.GetString(reflection, "nonce"),
		pb.GetBool(reflection, "include_hardware"),
	)
	if err != nil {
		return nil, status.Errorf(codes.Internal, "attest session: %v", err)
	}
	response := s.schema.NewMessage("openpinch.v1.AttestationResponse")
	responseRef := response.ProtoReflect()
	pb.SetString(responseRef, "status", result.Status)
	pb.SetString(responseRef, "subject", result.Subject)
	pb.SetString(responseRef, "platform", result.Platform)
	pb.SetBool(responseRef, "hardware_backed", result.HardwareBacked)
	pb.SetString(responseRef, "nonce", result.Nonce)
	pb.SetString(responseRef, "public_key", result.PublicKey)
	pb.SetStringMap(responseRef, "measurements", result.Measurements)
	return response, nil
}

func (s *Service) ExportAudit(ctx context.Context, request proto.Message) (proto.Message, error) {
	reflection := request.ProtoReflect()
	events, format, err := s.bridge.ExportAudit(
		ctx,
		pb.GetString(reflection, "sink"),
		pb.GetUint32(reflection, "limit"),
	)
	if err != nil {
		return nil, status.Errorf(codes.Internal, "export audit: %v", err)
	}
	response := s.schema.NewMessage("openpinch.v1.AuditExportResponse")
	responseRef := response.ProtoReflect()
	pb.SetBool(responseRef, "exported", true)
	pb.SetString(responseRef, "format", format)
	field := pb.FieldByName(responseRef, "events")
	list := responseRef.Mutable(field).List()
	for _, event := range events {
		list.Append(protoreflect.ValueOfMessage(protoAuditEventMessage(s.schema, event).ProtoReflect()))
	}
	return response, nil
}

func (s *Service) QueryMemory(ctx context.Context, request proto.Message) (proto.Message, error) {
	reflection := request.ProtoReflect()
	records, backend, err := s.bridge.QueryMemory(
		ctx,
		pb.GetString(reflection, "namespace"),
		pb.GetString(reflection, "query"),
		pb.GetUint32(reflection, "limit"),
		pb.GetString(reflection, "filter_json"),
	)
	if err != nil {
		return nil, status.Errorf(codes.Internal, "query memory: %v", err)
	}
	response := s.schema.NewMessage("openpinch.v1.MemoryQueryResponse")
	responseRef := response.ProtoReflect()
	pb.SetString(responseRef, "backend", backend)
	field := pb.FieldByName(responseRef, "records")
	list := responseRef.Mutable(field).List()
	for _, record := range records {
		list.Append(protoreflect.ValueOfMessage(protoMemoryRecordMessage(s.schema, record).ProtoReflect()))
	}
	return response, nil
}

func (s *Service) UpsertMemory(ctx context.Context, request proto.Message) (proto.Message, error) {
	reflection := request.ProtoReflect()
	stored, backend, digest, err := s.bridge.UpsertMemory(
		ctx,
		pb.GetString(reflection, "namespace"),
		pb.GetString(reflection, "key"),
		pb.GetString(reflection, "content"),
		pb.GetString(reflection, "metadata_json"),
	)
	if err != nil {
		return nil, status.Errorf(codes.Internal, "upsert memory: %v", err)
	}
	response := s.schema.NewMessage("openpinch.v1.MemoryUpsertResponse")
	responseRef := response.ProtoReflect()
	pb.SetBool(responseRef, "stored", stored)
	pb.SetString(responseRef, "backend", backend)
	pb.SetString(responseRef, "digest", digest)
	return response, nil
}

func (s *Service) RunAgentProtocol(ctx context.Context, request proto.Message) (proto.Message, error) {
	reflection := request.ProtoReflect()
	field := pb.FieldByName(reflection, "messages")
	list := reflection.Get(field).List()
	messages := make([]enginebridge.AgentMessage, 0, list.Len())
	for i := 0; i < list.Len(); i++ {
		message := list.Get(i).Message()
		messages = append(messages, enginebridge.AgentMessage{
			Sender:        pb.GetString(message, "sender"),
			Recipient:     pb.GetString(message, "recipient"),
			Body:          pb.GetString(message, "body"),
			MetadataJSON:  pb.GetString(message, "metadata_json"),
			EncryptedBody: pb.GetString(message, "encrypted_body"),
		})
	}
	result, err := s.bridge.RunAgentProtocol(
		ctx,
		pb.GetString(reflection, "protocol_id"),
		pb.GetString(reflection, "initiator"),
		messages,
		pb.GetString(reflection, "policy_scope"),
	)
	if err != nil {
		return nil, status.Errorf(codes.Internal, "run agent protocol: %v", err)
	}
	response := s.schema.NewMessage("openpinch.v1.AgentProtocolResponse")
	responseRef := response.ProtoReflect()
	pb.SetBool(responseRef, "accepted", result.Accepted)
	pb.SetString(responseRef, "protocol_id", result.ProtocolID)
	pb.SetString(responseRef, "transcript_json", result.TranscriptJSON)
	for _, finding := range result.Findings {
		pb.AddString(responseRef, "findings", finding)
	}
	return response, nil
}

func (s *Service) QueueTask(ctx context.Context, request proto.Message) (proto.Message, error) {
	reflection := request.ProtoReflect()
	receipt, err := s.bridge.QueueTask(
		ctx,
		pb.GetString(reflection, "task_id"),
		pb.GetString(reflection, "task_type"),
		pb.GetString(reflection, "target"),
		pb.GetString(reflection, "arguments_json"),
		pb.GetString(reflection, "priority"),
	)
	if err != nil {
		return nil, status.Errorf(codes.Internal, "queue task: %v", err)
	}
	response := s.schema.NewMessage("openpinch.v1.QueueTaskResponse")
	responseRef := response.ProtoReflect()
	pb.SetBool(responseRef, "accepted", receipt.Accepted)
	pb.SetString(responseRef, "task_id", receipt.TaskID)
	pb.SetString(responseRef, "queue", receipt.Queue)
	pb.SetString(responseRef, "message", receipt.Message)
	return response, nil
}

func (s *Service) GetPolicyReport(ctx context.Context, request proto.Message) (proto.Message, error) {
	reflection := request.ProtoReflect()
	report, err := s.bridge.GetPolicyReport(
		ctx,
		pb.GetString(reflection, "subject"),
		pb.GetString(reflection, "capability"),
	)
	if err != nil {
		return nil, status.Errorf(codes.Internal, "get policy report: %v", err)
	}
	response := s.schema.NewMessage("openpinch.v1.PolicyReportResponse")
	responseRef := response.ProtoReflect()
	pb.SetString(responseRef, "subject", report.Subject)
	for _, capability := range report.AllowedCapabilities {
		pb.AddString(responseRef, "allowed_capabilities", capability)
	}
	for _, capability := range report.DeniedCapabilities {
		pb.AddString(responseRef, "denied_capabilities", capability)
	}
	pb.SetString(responseRef, "source", report.Source)
	return response, nil
}

func (s *Service) TailLogs(request proto.Message, stream grpc.ServerStream) error {
	reflection := request.ProtoReflect()
	tail := int(pb.GetUint32(reflection, "tail"))
	follow := pb.GetBool(reflection, "follow")

	lines, err := tailFile(s.cfg.Paths.LogFile, tail)
	if err != nil {
		return status.Errorf(codes.Internal, "tail logs: %v", err)
	}
	for _, line := range lines {
		if err := stream.SendMsg(logChunkMessage(s.schema, line)); err != nil {
			return err
		}
	}

	if !follow {
		return nil
	}

	seen := len(lines)
	for {
		time.Sleep(500 * time.Millisecond)
		latest, err := tailFile(s.cfg.Paths.LogFile, tail+seen)
		if err != nil {
			return status.Errorf(codes.Internal, "follow logs: %v", err)
		}
		if len(latest) <= seen {
			continue
		}
		for _, line := range latest[seen:] {
			if err := stream.SendMsg(logChunkMessage(s.schema, line)); err != nil {
				return err
			}
		}
		seen = len(latest)
	}
}

func unaryHandler(inputFullName string, handler func(context.Context, proto.Message) (proto.Message, error)) grpc.MethodHandler {
	return func(srv interface{}, ctx context.Context, dec func(interface{}) error, interceptor grpc.UnaryServerInterceptor) (interface{}, error) {
		request := pb.MustLoad().NewMessage(inputFullName)
		if err := dec(request); err != nil {
			return nil, err
		}

		if interceptor == nil {
			return handler(ctx, request)
		}

		info := &grpc.UnaryServerInfo{
			Server:     srv,
			FullMethod: inputFullName,
		}
		return interceptor(ctx, request, info, func(ctx context.Context, req interface{}) (interface{}, error) {
			return handler(ctx, req.(proto.Message))
		})
	}
}

func streamHandler(inputFullName string, handler func(proto.Message, grpc.ServerStream) error) grpc.StreamHandler {
	return func(srv interface{}, stream grpc.ServerStream) error {
		request := pb.MustLoad().NewMessage(inputFullName)
		if err := stream.RecvMsg(request); err != nil {
			return err
		}
		return handler(request, stream)
	}
}

func executeResultMessage(schema *pb.Schema, result *enginebridge.ExecuteResult) proto.Message {
	response := schema.NewMessage("openpinch.v1.ExecuteResponse")
	reflection := response.ProtoReflect()
	pb.SetBool(reflection, "success", result.Success)
	pb.SetString(reflection, "summary", result.Summary)
	pb.SetString(reflection, "data_json", result.DataJSON)
	pb.SetString(reflection, "error", result.Error)
	for _, line := range result.Logs {
		pb.AddString(reflection, "logs", line)
	}
	return response
}

func connectorInfoMessage(schema *pb.Schema, descriptor connectors.Descriptor) proto.Message {
	message := schema.NewMessage("openpinch.v1.ConnectorInfo")
	reflection := message.ProtoReflect()
	pb.SetString(reflection, "name", descriptor.Name)
	pb.SetBool(reflection, "enabled", descriptor.Enabled)
	pb.SetBool(reflection, "implemented", descriptor.Implemented)
	pb.SetString(reflection, "mode", descriptor.Mode)
	pb.SetString(reflection, "health", descriptor.Health)
	for _, entry := range descriptor.Allowlist {
		pb.AddString(reflection, "allowlist", entry)
	}
	pb.SetStringMap(reflection, "details", descriptor.Details)
	return message
}

func protoAuditEventMessage(schema *pb.Schema, event enginebridge.AuditEvent) proto.Message {
	message := schema.NewMessage("openpinch.v1.AuditEvent")
	reflection := message.ProtoReflect()
	pb.SetString(reflection, "id", event.ID)
	pb.SetString(reflection, "category", event.Category)
	pb.SetString(reflection, "severity", event.Severity)
	pb.SetString(reflection, "summary", event.Summary)
	field := pb.FieldByName(reflection, "anomaly_score")
	reflection.Set(field, protoreflect.ValueOfFloat64(event.AnomalyScore))
	pb.SetString(reflection, "payload_json", event.PayloadJSON)
	pb.SetString(reflection, "created_at", event.CreatedAt)
	return message
}

func protoMemoryRecordMessage(schema *pb.Schema, record enginebridge.MemoryRecord) proto.Message {
	message := schema.NewMessage("openpinch.v1.MemoryRecord")
	reflection := message.ProtoReflect()
	pb.SetString(reflection, "key", record.Key)
	pb.SetString(reflection, "namespace", record.Namespace)
	pb.SetString(reflection, "content", record.Content)
	pb.SetString(reflection, "metadata_json", record.MetadataJSON)
	field := pb.FieldByName(reflection, "score")
	reflection.Set(field, protoreflect.ValueOfFloat64(record.Score))
	pb.SetString(reflection, "created_at", record.CreatedAt)
	return message
}

func logChunkMessage(schema *pb.Schema, line string) proto.Message {
	message := schema.NewMessage("openpinch.v1.LogChunk")
	reflection := message.ProtoReflect()
	pb.SetString(reflection, "line", line)
	pb.SetString(reflection, "timestamp", time.Now().UTC().Format(time.RFC3339))
	return message
}

func tailFile(path string, tail int) ([]string, error) {
	file, err := os.Open(path)
	if err != nil {
		return nil, err
	}
	defer file.Close()

	lines := []string{}
	scanner := bufio.NewScanner(file)
	for scanner.Scan() {
		lines = append(lines, scanner.Text())
		if tail > 0 && len(lines) > tail {
			lines = lines[1:]
		}
	}
	return lines, scanner.Err()
}

func installedSkillExists(root string, skillID string, version string) bool {
	path := filepath.Join(root, fmt.Sprintf("%s-%s", skillID, version))
	_, err := os.Stat(path)
	return err == nil
}

func resolvePath(path string) string {
	if filepath.IsAbs(path) {
		return path
	}
	workingDir, err := os.Getwd()
	if err != nil {
		return path
	}
	candidates := []string{
		filepath.Join(workingDir, path),
		filepath.Join(workingDir, "..", path),
		path,
	}
	for _, candidate := range candidates {
		if _, statErr := os.Stat(candidate); statErr == nil {
			return candidate
		}
	}
	return filepath.Join(workingDir, path)
}

func attestationState(cfg *config.Config) string {
	if cfg.Security.Attestation.Enabled {
		if cfg.Security.Attestation.RequireHardware {
			return "hardware-required"
		}
		return "enabled"
	}
	return "disabled"
}

type registryIndex struct {
	Version string          `json:"version"`
	Skills  []registrySkill `json:"skills"`
}

type registrySkill struct {
	ID          string `json:"id"`
	Version     string `json:"version"`
	Name        string `json:"name"`
	Description string `json:"description"`
	Language    string `json:"language"`
	Entrypoint  string `json:"entrypoint"`
}
