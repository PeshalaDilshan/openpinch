#!/bin/bash
echo "🔧 Setting up OpenPinch..."
rustup update
go mod download
echo "✅ Setup complete"