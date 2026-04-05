# ================================================
# OpenPinch Makefile
# One-stop build & dev commands for Rust + Go + Flutter
# ================================================

.PHONY: help setup dev build test clean release

# Default target
help:
	@echo "OpenPinch - Autonomous AI Agent"
	@echo "Available commands:"
	@echo "  make setup     - Install dependencies and tooling"
	@echo "  make dev       - Run development mode (core + gateway + Flutter hot reload)"
	@echo "  make build     - Build all components"
	@echo "  make test      - Run tests for all languages"
	@echo "  make clean     - Clean build artifacts"
	@echo "  make release   - Prepare release binaries"

# Setup tooling (run once)
setup:
	@echo "Setting up OpenPinch..."
	# Rust
	rustup update
	cargo --version || (echo "Please install Rust from https://rustup.rs" && exit 1)
	# Go
	go version || (echo "Please install Go from https://go.dev" && exit 1)
	# Flutter
	flutter --version || (echo "Please install Flutter from https://flutter.dev" && exit 1)
	flutter pub get --directory=ui
	@echo "Setup complete! Run 'make dev' to start development."

# Development mode (parallel where possible)
dev:
	@echo "Starting OpenPinch in development mode..."
	@echo "→ Rust core will need manual cargo run in another terminal for now"
	@echo "→ Go gateway: cd gateway && go run ./cmd/gateway"
	@echo "→ Flutter UI: cd ui && flutter run -d chrome"  # or your preferred device
	# For better parallel dev, consider using tmux or overmind in the future

# Build everything
build:
	@echo "Building OpenPinch..."
	# Rust core
	cargo build --release --manifest-path core/Cargo.toml
	# Go gateway
	cd gateway && go build -o ../bin/gateway ./cmd/gateway
	# Flutter UI (desktop + web for starters)
	cd ui && flutter build web
	cd ui && flutter build macos  # or windows, linux depending on host
	@echo "Build complete! Binaries in ./bin/ and ui/build/"

# Run tests
test:
	@echo "Running tests..."
	cargo test --manifest-path core/Cargo.toml
	cd gateway && go test ./...
	cd ui && flutter test
	@echo "All tests completed."

# Clean artifacts
clean:
	cargo clean --manifest-path core/Cargo.toml
	rm -rf gateway/bin/
	rm -rf ui/build/
	rm -rf target/
	@echo "Cleaned build artifacts."

# Release preparation (expand later with packaging)
release: clean build
	@echo "Preparing release..."
	# Add packaging steps here (e.g., zip binaries, create installer)
	@echo "Release artifacts ready."
