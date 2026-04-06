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

## First Run

```bash
./target/release/openpinch config init
./target/release/openpinch start --foreground
```

## Optional Local Models

- Ollama: `http://127.0.0.1:11434`
- `llama.cpp` server: `http://127.0.0.1:8080`
- OpenAI-compatible local endpoints: configure explicitly in `config.toml`

