#!/bin/bash
cargo build --release --manifest-path core/Cargo.toml
cd gateway && go build -o ../bin/gateway ./cmd/gateway
echo "✅ Build complete"