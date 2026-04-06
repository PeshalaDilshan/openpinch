#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
GO_CACHE="${ROOT_DIR}/.cache/go-build"
GO_MOD_CACHE="${ROOT_DIR}/.cache/go-mod"

mkdir -p "${GO_CACHE}" "${GO_MOD_CACHE}" "${ROOT_DIR}/bin"

echo "==> Preparing OpenPinch development environment"

if ! command -v rustup >/dev/null 2>&1 && [ ! -x "$HOME/.cargo/bin/rustup" ]; then
  echo "rustup not found; installing the pinned Rust toolchain manager"
  curl https://sh.rustup.rs -sSf | sh -s -- -y --profile minimal --default-toolchain 1.94.1
fi

if ! command -v go >/dev/null 2>&1; then
  echo "go is required but was not found." >&2
  exit 1
fi

. "$HOME/.cargo/env" 2>/dev/null || true

if ! command -v cargo >/dev/null 2>&1; then
  echo "cargo is required but was not found after rustup setup." >&2
  exit 1
fi

rustup toolchain install 1.94.1 --profile minimal >/dev/null
rustup component add rustfmt clippy --toolchain 1.94.1 >/dev/null

cargo fetch --locked || true

(
  cd "${ROOT_DIR}/gateway"
  GOCACHE="${GO_CACHE}" GOMODCACHE="${GO_MOD_CACHE}" go mod download
)

if [[ "$(uname -s)" == "Linux" ]]; then
  mkdir -p "${ROOT_DIR}/.openpinch/sandbox"
  echo "Linux sandbox assets directory prepared at ${ROOT_DIR}/.openpinch/sandbox"
fi

echo "OpenPinch setup complete"
