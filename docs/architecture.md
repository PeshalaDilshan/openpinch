# Architecture

OpenPinch is split into two long-running components:

- The Rust runtime, started through `openpinch start`, owns configuration, SQLite state, model routing, sandbox orchestration, skill verification, and the private engine RPC.
- The Go gateway owns the public localhost gRPC API, connectors, and scheduling.

## Runtime Topology

1. `openpinch start` loads configuration and initializes local directories.
2. The Rust engine starts a private `EngineRuntimeService` endpoint on a Unix domain socket on Unix platforms.
3. The CLI supervises the Go gateway as a child process and injects the private engine endpoint through environment variables.
4. The Go gateway exposes `GatewayService` on localhost and forwards execution or messaging requests to the engine.

## Rust Crates

- `common`: shared configuration, protocol bindings, path helpers, and skill verification primitives
- `engine`: model-provider abstraction, local state store, runtime service, message handling
- `sandbox`: sandbox backends, capability checks, Firecracker orchestration on Linux
- `tools`: built-in tool dispatch, skill install/list/verify, sandboxed execution entrypoints

## Gateway

- `internal/config`: gateway configuration loading
- `internal/enginebridge`: gRPC client for the private engine service
- `internal/connectors`: Telegram plus typed stubs for future connectors
- `internal/scheduler`: cron-style local scheduling
- `internal/server`: public gRPC server implementation

## Data Plane

- Configuration lives in the OS-standard config directory.
- SQLite state lives in the OS-standard data directory.
- Logs live in the OS-standard state/log directory.
- Installed skills live under the local data directory and are verified before activation.

