// gateway/cmd/gateway/main.go
package main

import (
    "log"
    "net"

    "google.golang.org/grpc"
    pb "github.com/PeshalaDilshan/openpinch/proto"
)

type server struct {
    pb.UnimplementedAgentServiceServer
}

func (s *server) ExecuteTool(req *pb.ToolRequest, stream pb.AgentService_ExecuteToolServer) error {
    log.Printf("Executing tool: %s", req.ToolName)
    // TODO: Call Rust core via gRPC or internal channel
    return nil
}

func main() {
    lis, err := net.Listen("tcp", ":50051")
    if err != nil {
        log.Fatalf("failed to listen: %v", err)
    }

    s := grpc.NewServer()
    pb.RegisterAgentServiceServer(s, &server{})

    log.Println("🚀 OpenPinch Go Gateway listening on :50051")
    if err := s.Serve(lis); err != nil {
        log.Fatalf("failed to serve: %v", err)
    }
}