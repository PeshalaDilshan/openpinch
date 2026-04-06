package pb

import (
	"fmt"
	"sync"

	"github.com/jhump/protoreflect/desc/protoparse"
	"google.golang.org/protobuf/proto"
	"google.golang.org/protobuf/reflect/protodesc"
	"google.golang.org/protobuf/reflect/protoreflect"
	"google.golang.org/protobuf/types/dynamicpb"
)

const source = `syntax = "proto3";

package openpinch.v1;

service GatewayService {
  rpc GetStatus(Empty) returns (StatusResponse);
  rpc Execute(ExecuteRequest) returns (ExecuteResponse);
  rpc ListSkills(Empty) returns (SkillListResponse);
  rpc InstallSkill(InstallSkillRequest) returns (InstallSkillResponse);
  rpc TailLogs(LogRequest) returns (stream LogChunk);
  rpc SubmitMessage(SubmitMessageRequest) returns (SubmitMessageResponse);
  rpc ScheduleJob(ScheduleJobRequest) returns (ScheduleJobResponse);
}

service EngineRuntimeService {
  rpc DeliverMessage(EngineMessageRequest) returns (SubmitMessageResponse);
  rpc RunTool(EngineToolRequest) returns (ExecuteResponse);
  rpc RunSkill(EngineSkillRequest) returns (ExecuteResponse);
  rpc Health(Empty) returns (HealthResponse);
}

message Empty {}
message StatusResponse {
  string status = 1;
  string version = 2;
  string runtime_endpoint = 3;
  string gateway_endpoint = 4;
  repeated string enabled_connectors = 5;
  repeated string available_model_backends = 6;
  int64 uptime_seconds = 7;
  string started_at = 8;
  string data_dir = 9;
  string log_file = 10;
}
message ExecuteRequest {
  string target = 1;
  string arguments_json = 2;
  bool allow_network = 3;
}
message ExecuteResponse {
  bool success = 1;
  string summary = 2;
  string data_json = 3;
  string error = 4;
  repeated string logs = 5;
}
message SkillInfo {
  string id = 1;
  string version = 2;
  string name = 3;
  string description = 4;
  bool installed = 5;
  bool verified = 6;
  string entrypoint = 7;
  string language = 8;
}
message SkillListResponse {
  repeated SkillInfo skills = 1;
  string registry_version = 2;
}
message InstallSkillRequest {
  string source = 1;
  bool force = 2;
}
message InstallSkillResponse {
  bool installed = 1;
  SkillInfo skill = 2;
  string message = 3;
}
message LogRequest {
  uint32 tail = 1;
  bool follow = 2;
}
message LogChunk {
  string line = 1;
  string timestamp = 2;
}
message SubmitMessageRequest {
  string connector = 1;
  string channel_id = 2;
  string sender = 3;
  string body = 4;
  string metadata_json = 5;
}
message SubmitMessageResponse {
  bool accepted = 1;
  string message_id = 2;
  string reply = 3;
}
message ScheduleJobRequest {
  string job_id = 1;
  string cron = 2;
  string tool = 3;
  string arguments_json = 4;
}
message ScheduleJobResponse {
  bool accepted = 1;
  string message = 2;
}
message EngineMessageRequest {
  SubmitMessageRequest message = 1;
}
message EngineToolRequest {
  string tool = 1;
  string arguments_json = 2;
  bool allow_network = 3;
}
message EngineSkillRequest {
  string skill_id = 1;
  string arguments_json = 2;
}
message HealthResponse {
  string status = 1;
  repeated string missing_prerequisites = 2;
  string sandbox_backend = 3;
}
`

type Schema struct {
	file     protoreflect.FileDescriptor
	services map[string]protoreflect.ServiceDescriptor
	messages map[string]protoreflect.MessageDescriptor
}

var (
	once   sync.Once
	schema *Schema
	err    error
)

func Load() (*Schema, error) {
	once.Do(func() {
		parser := protoparse.Parser{
			Accessor: protoparse.FileContentsFromMap(map[string]string{
				"openpinch.proto": source,
			}),
		}
		files, parseErr := parser.ParseFiles("openpinch.proto")
		if parseErr != nil {
			err = fmt.Errorf("parse proto: %w", parseErr)
			return
		}
		file, convertErr := protodesc.NewFile(files[0].AsFileDescriptorProto(), nil)
		if convertErr != nil {
			err = fmt.Errorf("convert descriptor: %w", convertErr)
			return
		}
		schema = &Schema{
			file:     file,
			services: map[string]protoreflect.ServiceDescriptor{},
			messages: map[string]protoreflect.MessageDescriptor{},
		}
		services := file.Services()
		for i := 0; i < services.Len(); i++ {
			service := services.Get(i)
			schema.services[string(service.FullName())] = service
		}
		messages := file.Messages()
		for i := 0; i < messages.Len(); i++ {
			message := messages.Get(i)
			schema.messages[string(message.FullName())] = message
		}
	})

	return schema, err
}

func MustLoad() *Schema {
	value, loadErr := Load()
	if loadErr != nil {
		panic(loadErr)
	}
	return value
}

func (s *Schema) Service(fullName string) protoreflect.ServiceDescriptor {
	return s.services[fullName]
}

func (s *Schema) Message(fullName string) protoreflect.MessageDescriptor {
	return s.messages[fullName]
}

func (s *Schema) NewMessage(fullName string) *dynamicpb.Message {
	return dynamicpb.NewMessage(s.Message(fullName))
}

func FieldByName(message protoreflect.Message, name string) protoreflect.FieldDescriptor {
	return message.Descriptor().Fields().ByName(protoreflect.Name(name))
}

func SetString(message protoreflect.Message, name string, value string) {
	field := FieldByName(message, name)
	message.Set(field, protoreflect.ValueOfString(value))
}

func SetBool(message protoreflect.Message, name string, value bool) {
	field := FieldByName(message, name)
	message.Set(field, protoreflect.ValueOfBool(value))
}

func SetInt64(message protoreflect.Message, name string, value int64) {
	field := FieldByName(message, name)
	message.Set(field, protoreflect.ValueOfInt64(value))
}

func SetUint32(message protoreflect.Message, name string, value uint32) {
	field := FieldByName(message, name)
	message.Set(field, protoreflect.ValueOfUint32(value))
}

func AddString(message protoreflect.Message, name string, value string) {
	field := FieldByName(message, name)
	list := message.Mutable(field).List()
	list.Append(protoreflect.ValueOfString(value))
}

func SetMessage(message protoreflect.Message, name string, child proto.Message) {
	field := FieldByName(message, name)
	message.Set(field, protoreflect.ValueOfMessage(child.ProtoReflect()))
}

func GetString(message protoreflect.Message, name string) string {
	field := FieldByName(message, name)
	return message.Get(field).String()
}

func GetBool(message protoreflect.Message, name string) bool {
	field := FieldByName(message, name)
	return message.Get(field).Bool()
}

func GetInt64(message protoreflect.Message, name string) int64 {
	field := FieldByName(message, name)
	return message.Get(field).Int()
}

func GetUint32(message protoreflect.Message, name string) uint32 {
	field := FieldByName(message, name)
	return uint32(message.Get(field).Uint())
}

func GetMessage(message protoreflect.Message, name string) protoreflect.Message {
	field := FieldByName(message, name)
	return message.Get(field).Message()
}
