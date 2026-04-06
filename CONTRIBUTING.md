# Contributing

OpenPinch accepts contributions across Rust, Go, sandboxing, docs, and connector integrations.

## Branch Strategy

OpenPinch uses three long-lived branches:

- `main`: stable branch
- `dev`: active development and integration branch
- `test`: experimental branch for patches and feature trials

The planned promotion path does not change:

1. Build and review work against `dev`.
2. Let CI validate `dev`.
3. Let automation open or update the `dev -> main` PR.
4. Let `main` re-run CI before merge.

Direct pushes to `main` should be blocked in GitHub branch protection settings. That part is operational policy, not something a workflow file can enforce by itself.

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

Recommended local flow:

```bash
git checkout dev
git pull --ff-only origin dev
git checkout -b feature/my-change
make test
```

## Standards

- Keep the project local-first. Do not add cloud-only assumptions to the default flow.
- Preserve the public protobuf contracts in `proto/openpinch.proto`.
- Prefer explicit, typed configuration and structured logging over ad hoc flags.
- Do not weaken sandbox guarantees without documenting the tradeoff.

## Ownership

- Path ownership is defined in `.github/CODEOWNERS`.
- Maintainer responsibilities and escalation rules are defined in `MAINTAINERS.md`.
- Repository ownership policy is documented in `docs/ownership.md`.
- Changes that touch CI, sandbox policy, protobuf contracts, release automation, or multiple subsystems should be treated as owner-sensitive reviews.

## Issues And Triage

- Use the issue forms in `.github/ISSUE_TEMPLATE/` when opening new work.
- Area labels are aligned with ownership:
  - `area/core-rust`
  - `area/gateway-go`
  - `area/security`
  - `area/docs`
  - `area/ui`
- The label catalog is stored in `.github/labels.json`.
- Repository labels can be synced with `.github/workflows/sync-labels.yml`.

## Pull Requests

- Target `dev` for normal feature work.
- Reserve `main` for the automated promotion PR unless maintainers explicitly need an emergency stabilization PR.
- Run `make test` before opening a PR.
- Include docs updates when behavior changes.
- Explain sandbox, config, protocol, or CI changes clearly in the PR description.
- Request review from the relevant code owners for every affected subsystem.

## CI Layout

The repository keeps CI split by concern:

- `ci-rust.yml`: formatting, clippy, and Rust tests
- `ci-go.yml`: gofmt, Go tests, and vet
- `ci-flutter.yml`: Flutter analyze/test/build once the UI exists
- `release.yml`: dev promotion automation and main release artifacts

See [docs/branching-and-ci.md](docs/branching-and-ci.md) for the exact branch and workflow behavior.

## Areas That Need Help

- Additional messaging connectors
- Guest image automation for Firecracker
- Cross-platform virtualization helpers for macOS and Windows
- Local model provider coverage and evals
- Skills ecosystem and signed registry tooling
