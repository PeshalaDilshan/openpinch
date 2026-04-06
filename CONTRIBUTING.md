# Contributing

OpenPinch accepts contributions across Rust, Go, sandboxing, docs, and connector integrations.

## Prerequisites

- Rust toolchain pinned by [`rust-toolchain.toml`](rust-toolchain.toml)
- Go 1.26.1 or newer
- `make`
- On Linux, Firecracker and `jailer` if you want to exercise the full sandbox path

## Workflow

```bash
git clone https://github.com/PeshalaDilshan/openpinch.git
cd openpinch
make setup
make test
```

## Standards

- Keep the project local-first. Do not add cloud-only assumptions to the default flow.
- Preserve the public protobuf contracts in `proto/openpinch.proto`.
- Prefer explicit, typed configuration and structured logging over ad hoc flags.
- Do not weaken sandbox guarantees without documenting the tradeoff.

## Pull Requests

- Run `make test` before opening a PR.
- Include docs updates when behavior changes.
- Explain sandbox, config, or protocol changes clearly in the PR description.

## Areas That Need Help

- Additional messaging connectors
- Guest image automation for Firecracker
- Cross-platform virtualization helpers for macOS and Windows
- Local model provider coverage and evals
- Skills ecosystem and signed registry tooling

