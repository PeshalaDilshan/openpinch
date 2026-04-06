# Setup

## Prerequisites

- Rust pinned to `1.94.1` via `rust-toolchain.toml`
- Go `1.26.1`
- `make`
- On Linux, optional Firecracker + `jailer` for the full sandbox backend

## Bootstrap

```bash
make setup
make cli
```

The setup script prepares writable Go caches, downloads Go dependencies, and installs the pinned Rust toolchain if `rustup` is present.

## Branch Setup

After cloning, switch to the integration branch unless you are intentionally working on a stable hotfix:

```bash
git checkout dev
git pull --ff-only origin dev
```

Use `test` only for unstable experiments. Stable work should still flow through `dev` before `main`.

## GitHub Repository Setup

To match the repository automation:

1. Protect `main`.
2. Require pull requests for `main`.
3. Require status checks from the Rust, Go, and Flutter workflows.
4. Enable auto-merge if you want the `dev -> main` promotion PR to merge without manual clicking.

The full branch and CI policy is documented in [branching-and-ci.md](branching-and-ci.md).

## First Run

```bash
./target/release/openpinch config init
./target/release/openpinch start --foreground
```

## Optional Local Models

- Ollama: `http://127.0.0.1:11434`
- `llama.cpp` server: `http://127.0.0.1:8080`
- OpenAI-compatible local endpoints: configure explicitly in `config.toml`

## CI And Release Notes

- Pushes to `dev` run Rust, Go, and Flutter CI.
- Successful `dev` validation triggers the promotion automation in `.github/workflows/release.yml`.
- Pull requests into `main` run the CI workflows again as a stability gate.
- Pushes to `main` build release artifacts for the CLI and gateway.
