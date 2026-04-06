# OpenPinch

OpenPinch is a local-first autonomous agent runtime built for people who want full control over execution, models, skills, and messaging connectivity on their own hardware.

The stack is split deliberately:

- Rust powers the CLI, engine, tool execution, local state, and sandbox orchestration.
- Go powers the public gateway, scheduler, and high-concurrency messaging connectors.
- gRPC provides the contract between the Rust runtime and the Go gateway.
- The sandbox layer defaults to strong local isolation, with a native Firecracker backend on Linux and hypervisor-oriented backends on macOS and Windows.
- Security is deny-by-default: capability matrices, encrypted memory, attestation hooks, audit events, and optional mTLS are all built into the runtime surface.

## Highlights

- Single monorepo with Rust workspace + Go gateway
- Local-only model backends: Ollama, `llama.cpp` server, and OpenAI-compatible local endpoints
- Orchestration layer with exact, prefix, and semantic caches plus speculative draft routing
- Async task queue with interactive, connector, autonomy, and background priorities
- Encrypted vector-memory fallback backed by SQLite with a LanceDB-oriented config surface
- Signed skills registry with Ed25519 verification
- Gateway-first messaging architecture with a 20+ connector catalog and Telegram implemented end to end
- Zero-trust gateway options: mTLS, per-channel allowlists, attestation reporting, audit export
- Formal protocol spec support for multi-agent handoff flows
- Structured logging, local SQLite state, graceful shutdown, and deterministic setup scripts
- Branch-aware CI with `dev -> main` promotion automation and separate Rust/Go/Flutter workflows

## Repository Layout

```text
.
├── cli/                  # openpinch CLI binary
├── core/crates/          # Rust libraries
│   ├── common/           # config, types, proto bindings, verification helpers
│   ├── engine/           # runtime, model providers, state store, internal RPC
│   ├── sandbox/          # sandbox backends and capability checks
│   └── tools/            # built-ins, skill execution, install/verify flows
├── gateway/              # Go public gRPC API, Telegram connector, scheduler
├── deploy/operator/      # Kubernetes operator scaffold and CRD manifests
├── proto/                # versioned protobuf definitions
├── skills/               # trust roots, registry, example skills
├── docs/                 # architecture, setup, sandbox, skills
├── scripts/              # bootstrap and helper scripts
└── ui/                   # reserved for future Flutter UI
```

## Quick Start

```bash
git clone https://github.com/PeshalaDilshan/openpinch.git
cd openpinch
make setup
make cli
./target/release/openpinch config init
./target/release/openpinch start --foreground
```

The generated CLI also exposes:

```bash
openpinch connector list
openpinch memory query "recent failures"
openpinch policy show builtin.command
openpinch audit export --sink json
openpinch attest --include-hardware
openpinch agent protocol handoff.v1 --initiator planner --message planner:executor:run-task
```

## Local Runtime Model Support

OpenPinch never assumes a hosted model. The default provider order is:

1. `llama.cpp` draft/fast local endpoint when enabled
2. Ollama target model
3. OpenAI-compatible local endpoints such as LM Studio or LocalAI

Configure providers in the generated `config.toml`. No remote endpoint is enabled by default.

## Sandbox Model

- Linux: Firecracker + `jailer` + seccomp + containerd-in-guest expectation + ephemeral workspace
- macOS: virtualization backend contract plus Seatbelt-oriented policy path
- Windows: Hyper-V backend contract plus Job Object restriction path

Capability enforcement is loaded from `skills/policies/default.yaml`. The Linux Firecracker path is fully wired in code and validated through the sandbox doctor. Provisioning guest assets is described in [docs/sandbox.md](docs/sandbox.md).

## Developer Workflow

```bash
make setup
make build
make test
```

Branch model:

- `main` is the stable branch.
- `dev` is the active integration branch.
- `test` is the experimental patch branch.

Promotion model:

1. Land work in `dev`.
2. CI runs on `dev`.
3. Automation opens or updates a `dev -> main` PR.
4. CI runs again on the PR into `main`.
5. Merges into `main` produce release artifacts.

Important: direct pushes to `main` must be blocked with GitHub branch protection. Workflow YAML can validate and promote, but it cannot replace repository protection rules.

Useful commands:

```bash
openpinch status
openpinch execute builtin.echo --args '{"message":"hello"}'
openpinch skill list
openpinch skill verify skills/examples/echo.skill
openpinch connector list --json
openpinch memory put incident-1 "sandbox degraded" --metadata '{"source":"cli"}'
openpinch logs --tail 100
```

## Documentation

- [Setup](docs/setup.md)
- [Architecture](docs/architecture.md)
- [Branching And CI](docs/branching-and-ci.md)
- [Ownership](docs/ownership.md)
- [Maintainers](MAINTAINERS.md)
- [Issue And Triage Metadata](.github/labels.json)
- [Sandbox](docs/sandbox.md)
- [Skills](docs/skills.md)
- [Formal Protocols](docs/formal/handoff_v1.tla)
- [Migration From OpenClaw](docs/migration-from-openclaw.md)

## Project Status

This repository is intentionally production-shaped, but still early in product maturity. The orchestration, connector catalog, policy surface, encryption hooks, and audit/attestation APIs are in place; the deepest platform-specific pieces such as real guest images, eBPF collectors, and operator reconciliation are the areas that will continue to evolve.
