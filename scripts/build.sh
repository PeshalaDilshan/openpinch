#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
GO_CACHE="${ROOT_DIR}/.cache/go-build"
GO_MOD_CACHE="${ROOT_DIR}/.cache/go-mod"

. "$HOME/.cargo/env" 2>/dev/null || true

cargo build --workspace --release
(
  cd "${ROOT_DIR}/gateway"
  GOCACHE="${GO_CACHE}" GOMODCACHE="${GO_MOD_CACHE}" go build -o "${ROOT_DIR}/bin/openpinch-gateway" ./cmd/gateway
)
