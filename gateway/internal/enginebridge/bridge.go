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

func (c *Client) RunTool(ctx context.Context, target string, argumentsJSON string, allowNetwork bool) (*ExecuteResult, error) {
	request := c.schema.NewMessage("openpinch.v1.EngineToolRequest")
	pb.SetString(request.ProtoReflect(), "tool", target)
	pb.SetString(request.ProtoReflect(), "arguments_json", argumentsJSON)
	pb.SetBool(request.ProtoReflect(), "allow_network", allowNetwork)

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

	field := pb.FieldByName(response.ProtoReflect(), "missing_prerequisites")
	list := response.ProtoReflect().Get(field).List()
	missing := make([]string, 0, list.Len())
	for i := 0; i < list.Len(); i++ {
		missing = append(missing, list.Get(i).String())
	}

	return &HealthResult{
		Status:               pb.GetString(response.ProtoReflect(), "status"),
		MissingPrerequisites: missing,
		SandboxBackend:       pb.GetString(response.ProtoReflect(), "sandbox_backend"),
	}, nil
}

func (c *Client) invoke(ctx context.Context, method string, request proto.Message, response proto.Message) error {
	callCtx, cancel := context.WithTimeout(ctx, 10*time.Second)
	defer cancel()
	return c.conn.Invoke(callCtx, method, request, response)
}

func decodeExecuteResult(message proto.Message) *ExecuteResult {
	reflection := message.ProtoReflect()
	result := &ExecuteResult{
		Success:  pb.GetBool(reflection, "success"),
		Summary:  pb.GetString(reflection, "summary"),
		DataJSON: pb.GetString(reflection, "data_json"),
		Error:    pb.GetString(reflection, "error"),
	}
	field := pb.FieldByName(reflection, "logs")
	list := reflection.Get(field).List()
	for i := 0; i < list.Len(); i++ {
		result.Logs = append(result.Logs, list.Get(i).String())
	}
	return result
}
