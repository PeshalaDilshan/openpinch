# Architecture

OpenPinch is split into three long-running components:

- The Rust runtime, started through `openpinch start`, owns configuration, SQLite state, model routing, sandbox orchestration, skill verification, and the private engine RPC.
- The Go gateway owns the public localhost gRPC API, connectors, and scheduling.
- The Flutter desktop client owns the native operator experience on Linux, macOS, and Windows, and supervises the local runtime through `openpinch desktop host`.
- Formal protocol specs live alongside the runtime and are enforced by generated validator rules before multi-agent flows are accepted.

## Runtime Topology

1. `openpinch start` loads configuration and initializes local directories.
2. The Rust engine starts a private `EngineRuntimeService` endpoint on a Unix domain socket on Unix platforms.
3. `openpinch desktop host` or `openpinch start` supervises the Go gateway as a child process and injects the private engine endpoint through environment variables.
4. The Go gateway exposes `GatewayService` on localhost and forwards execution, memory, audit, attestation, and multi-agent protocol requests to the engine.
5. The Flutter desktop client launches the desktop host, reconnects to an already-running host when present, and uses CLI/gRPC surfaces for control, status, and local messaging.

## Orchestration Layer

- The engine owns a local orchestration layer with exact, prefix, and semantic caches.
- Semantic cache entries are stored in encrypted local vector-memory records. The current implementation uses a SQLite fallback backend behind a LanceDB-oriented config surface.
- Requests are routed according to provider order and can use speculative draft execution when a draft-capable provider is configured.
- Background autonomy work is separated from interactive work by a queue manager with explicit priorities.

## Rust Crates

- `common`: shared configuration, protocol bindings, path helpers, and skill verification primitives
- `engine`: orchestration, local state store, encrypted memory, attestation hooks, audit trail, queue manager, runtime service, message handling
- `sandbox`: sandbox backends, capability checks, Firecracker orchestration on Linux
- `tools`: built-in tool dispatch, skill install/list/verify, deny-by-default capability enforcement, sandboxed execution entrypoints

## Gateway

- `internal/config`: gateway configuration loading
- `internal/enginebridge`: gRPC client for the private engine service
- `internal/connectors`: Telegram, local `desktop`, plus a typed 20+ connector catalog
- `internal/scheduler`: cron-style local scheduling for proactive autonomy flows
- `internal/server`: public local gRPC server implementation, desktop event streams, and connector inventory
- `deploy/operator`: Kubernetes deployment/operator scaffold for enterprise rollouts

## Desktop Client

- `ui/` is a Flutter desktop application, not a web surface.
- The desktop app is the primary operator experience and no longer depends on gateway-hosted browser routes.
- The local desktop chat path is represented as the `desktop` connector inside the engine and gateway.
- Native packaging is intended to bundle the desktop app with `openpinch` and `openpinch-gateway` sidecars in the same installable artifact.

## Data Plane

- Configuration lives in the OS-standard config directory.
- SQLite state lives in the OS-standard data directory.
- Logs live in the OS-standard state/log directory.
- Installed skills live under the local data directory and are verified before activation.
- Capability matrices live under `skills/policies/`.
- Formal protocol specs live under `docs/formal/`.
