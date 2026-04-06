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
	TailLogs(proto.Message, grpc.ServerStream) error
}

func New(cfg *config.Config, bridge *enginebridge.Client, scheduler *scheduler.Scheduler) (*Service, error) {
	schema, err := pb.Load()
	if err != nil {
		return nil, err
	}
	return &Service{
		cfg:       cfg,
		bridge:    bridge,
		scheduler: scheduler,
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
	pb.SetString(reflection, "version", "0.1.0")
	pb.SetString(reflection, "runtime_endpoint", s.cfg.Gateway.EngineEndpoint)
	pb.SetString(reflection, "gateway_endpoint", s.cfg.Gateway.ListenAddress)
	for _, connector := range connectors.NewRegistry(s.cfg, s.bridge).EnabledNames() {
		pb.AddString(reflection, "enabled_connectors", connector)
	}
	pb.AddString(reflection, "available_model_backends", "ollama")
	pb.AddString(reflection, "available_model_backends", "llamacpp")
	pb.AddString(reflection, "available_model_backends", "localai")
	pb.SetInt64(reflection, "uptime_seconds", int64(time.Since(s.startedAt).Seconds()))
	pb.SetString(reflection, "started_at", s.startedAt.Format(time.RFC3339))
	pb.SetString(reflection, "data_dir", s.cfg.Paths.DataDir)
	pb.SetString(reflection, "log_file", s.cfg.Paths.LogFile)
	return message, nil
}

func (s *Service) Execute(ctx context.Context, request proto.Message) (proto.Message, error) {
	reflection := request.ProtoReflect()
	result, err := s.bridge.RunTool(
		ctx,
		pb.GetString(reflection, "target"),
		pb.GetString(reflection, "arguments_json"),
		pb.GetBool(reflection, "allow_network"),
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
	return filepath.Join(workingDir, "..", path)
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
