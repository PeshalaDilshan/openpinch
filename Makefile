.PHONY: help setup dev build test clean release cli

help:
	@echo "OpenPinch — Autonomous AI Agent"
	@echo "Commands:"
	@echo "  make setup     Install Rust, Go, Flutter tooling"
	@echo "  make dev       Run everything in development"
	@echo "  make build     Build core + gateway + CLI"
	@echo "  make cli       Build only the CLI binary"
	@echo "  make test      Run all tests"

setup:
	rustup update
	go mod download -C gateway
	@echo "✅ Setup complete"

dev:
	@echo "→ Run CLI: cargo run -p openpinch-cli -- start"
	@echo "→ Or use: openpinch start (after build)"

build:
	cargo build --release
	cd gateway && go build -o ../bin/gateway ./cmd/gateway
	@echo "✅ Built: target/release/openpinch (CLI) + gateway binary"

cli:
	cargo build -p openpinch-cli --release
	@echo "✅ CLI binary ready → ./target/release/openpinch"

test:
	cargo test

clean:
	cargo clean
	rm -rf gateway/bin/
	rm -rf target/

release: clean build
	@echo "Release artifacts ready"