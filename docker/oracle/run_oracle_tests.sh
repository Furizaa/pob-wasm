#!/bin/bash
# Build and run oracle tests in Docker.
set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"

echo "Building oracle test Docker image..."
docker build -f "$SCRIPT_DIR/Dockerfile" -t pob-wasm-oracle "$REPO_ROOT"

echo "Running oracle tests..."
docker run --rm pob-wasm-oracle
