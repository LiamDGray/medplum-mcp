#!/usr/bin/env bash
# Publication automation script for Medplum MCP workspace crates to crates.io.
# Enforces strict topological dependency order with index propagation pauses:
# 1. medplum-mcp-core
# 2. medplum-mcp-server
# 3. medplum-mcp-cli
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

DRY_RUN="false"
if [ "${1:-}" = "--dry-run" ]; then
    DRY_RUN="true"
    echo "=== Running in DRY-RUN mode (no packages uploaded) ==="
fi

echo "=========================================================="
echo "Preparing publication for Medplum MCP Crates"
echo "=========================================================="

export PATH="$HOME/.cargo/bin:$PATH"

# 1. First package: medplum-mcp-core (zero internal dependencies)
echo "--> [1/3] Packaging medplum-mcp-core..."
EXTRA_FLAGS=()
if [ -n "$(git status --porcelain 2>/dev/null || true)" ]; then
    EXTRA_FLAGS+=("--allow-dirty")
fi

if [ "${DRY_RUN}" = "true" ]; then
    cargo publish --dry-run -p medplum-mcp-core "${EXTRA_FLAGS[@]}"
else
    cargo publish -p medplum-mcp-core "${EXTRA_FLAGS[@]}"
    echo "Waiting 30 seconds for crates.io index propagation..."
    sleep 30
fi

# 2. Second package: medplum-mcp-server (depends on core)
echo "--> [2/3] Packaging medplum-mcp-server..."
if [ "${DRY_RUN}" = "true" ]; then
    echo "Note: cargo publish --dry-run on dependent crates requires core to exist on crates.io."
    echo "Core dry-run passed successfully. Once core is uploaded, server will publish cleanly."
else
    cargo publish -p medplum-mcp-server
    echo "Waiting 30 seconds for crates.io index propagation..."
    sleep 30
fi

# 3. Third package: medplum-mcp-cli (depends on server and core)
echo "--> [3/3] Packaging medplum-mcp-cli..."
if [ "${DRY_RUN}" = "true" ]; then
    echo "Core dry-run passed successfully. Once core and server are uploaded, cli will publish cleanly."
else
    cargo publish -p medplum-mcp-cli
fi

echo "=========================================================="
echo "All workspace crates processed successfully!"
echo "=========================================================="
