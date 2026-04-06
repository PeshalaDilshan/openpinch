SHELL := /bin/bash
ROOT_DIR := $(CURDIR)
GO_CACHE := $(ROOT_DIR)/.cache/go-build
GO_MOD_CACHE := $(ROOT_DIR)/.cache/go-mod

.PHONY: setup dev build cli test clean release

setup:
	./scripts/setup.sh

dev:
	. "$$HOME/.cargo/env" 2>/dev/null || true; cargo run -p openpinch-cli -- start --foreground

build:
	. "$$HOME/.cargo/env" 2>/dev/null || true; cargo build --workspace --release
	cd gateway && GOCACHE="$(GO_CACHE)" GOMODCACHE="$(GO_MOD_CACHE)" go build -o ../bin/openpinch-gateway ./cmd/gateway

cli:
	. "$$HOME/.cargo/env" 2>/dev/null || true; cargo build -p openpinch-cli --release

test:
	. "$$HOME/.cargo/env" 2>/dev/null || true; cargo test --workspace
	cd gateway && GOCACHE="$(GO_CACHE)" GOMODCACHE="$(GO_MOD_CACHE)" go test ./...

clean:
	. "$$HOME/.cargo/env" 2>/dev/null || true; cargo clean
	rm -rf .cache bin gateway/bin target

release: clean build
	@echo "Artifacts:"
	@echo "  CLI: $(ROOT_DIR)/target/release/openpinch"
	@echo "  Gateway: $(ROOT_DIR)/bin/openpinch-gateway"
