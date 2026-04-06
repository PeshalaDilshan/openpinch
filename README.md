# OpenPinch

OpenPinch is a local-first autonomous agent runtime built for people who want full control over execution, models, skills, and messaging connectivity on their own hardware.

The stack is split deliberately:

- Rust powers the CLI, engine, tool execution, local state, and sandbox orchestration.
- Go powers the public gateway, scheduler, and high-concurrency messaging connectors.
- gRPC provides the contract between the Rust runtime and the Go gateway.
- The sandbox layer defaults to strong local isolation, with a native Firecracker backend on Linux and hypervisor-oriented backends on macOS and Windows.

## Highlights

- Single monorepo with Rust workspace + Go gateway
- Local-only model backends: Ollama, `llama.cpp` server, and OpenAI-compatible local endpoints
- Signed skills registry with Ed25519 verification
- Gateway-first messaging architecture with Telegram implemented end to end
- Structured logging, local SQLite state, graceful shutdown, and deterministic setup scripts

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

## Local Runtime Model Support

OpenPinch never assumes a hosted model. The default provider order is:

1. Ollama
2. `llama.cpp` server
3. OpenAI-compatible local endpoints such as LM Studio or LocalAI

Configure providers in the generated `config.toml`. No remote endpoint is enabled by default.

## Sandbox Model

- Linux: Firecracker + `jailer` + seccomp + ephemeral workspace
- macOS: virtualization backend contract for local hypervisor execution
- Windows: Hyper-V backend contract for local hypervisor execution

The Linux Firecracker path is fully wired in code and validated through the sandbox doctor. Provisioning guest assets is described in [docs/sandbox.md](docs/sandbox.md).

## Developer Workflow

```bash
make setup
make build
make test
```

Useful commands:

```bash
openpinch status
openpinch execute builtin.echo --args '{"message":"hello"}'
openpinch skill list
openpinch skill verify skills/examples/echo.skill
openpinch logs --tail 100
```

## Documentation

- [Setup](docs/setup.md)
- [Architecture](docs/architecture.md)
- [Sandbox](docs/sandbox.md)
- [Skills](docs/skills.md)
- [Migration From OpenClaw](docs/migration-from-openclaw.md)

## Project Status

This repository is intentionally production-shaped, but still early in product maturity. The architecture, contracts, and local developer workflow are in place; connector depth, model behavior, and platform-specific sandbox polish will continue to evolve.

