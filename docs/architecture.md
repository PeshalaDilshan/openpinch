# OpenPinch Architecture

- **Rust Core** (`core/`): Orchestration, memory, decision engine, sandbox
- **Go Gateway** (`gateway/`): Messaging connectors, scheduler, gRPC server
- **Kernel Sandbox**: Firecracker microVMs + seccomp (mandatory)
- Communication: gRPC via `proto/`