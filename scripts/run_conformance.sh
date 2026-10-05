#!/usr/bin/env bash
# Official MCP Conformance Suite Runner for medplum-mcp-rs
# Runs @modelcontextprotocol/conformance against the local server over Streamable HTTP/SSE.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

PORT="${MCP_CONFORMANCE_PORT:-8930}"
SCENARIO="${1:-server-initialize}"

echo "======================================================================"
echo "Starting Medplum MCP Conformance Test Harness on port ${PORT}"
echo "Scenario: ${SCENARIO}"
echo "======================================================================"

# Ensure binary is built
if [ ! -f "${WORKSPACE_ROOT}/target/debug/medplum-mcp-rs" ]; then
    echo "Building medplum-mcp-rs..."
    cargo build -p medplum-mcp-cli
fi

# Launch server in background
"${WORKSPACE_ROOT}/target/debug/medplum-mcp-rs" serve --transport sse --port "${PORT}" --demo &
SERVER_PID=$!

cleanup() {
    echo "Stopping Medplum MCP server (PID ${SERVER_PID})..."
    kill "${SERVER_PID}" 2>/dev/null || true
    wait "${SERVER_PID}" 2>/dev/null || true
}
trap cleanup EXIT INT TERM

# Wait for server to be responsive
echo "Waiting for server to listen on port ${PORT}..."
MAX_WAIT=20
WAIT_COUNT=0
until curl -s --max-time 1 -X POST "http://127.0.0.1:${PORT}/mcp" >/dev/null 2>&1 || [ "${WAIT_COUNT}" -eq "${MAX_WAIT}" ]; do
    sleep 0.2
    WAIT_COUNT=$((WAIT_COUNT + 1))
done

if [ "${WAIT_COUNT}" -eq "${MAX_WAIT}" ]; then
    echo "Error: Server failed to start on port ${PORT} within 4 seconds."
    exit 1
fi

echo "Server is ready. Executing conformance suite..."
if [ "${SCENARIO}" = "all" ]; then
    npx -y @modelcontextprotocol/conformance server --url "http://127.0.0.1:${PORT}/mcp" || true
else
    npx -y @modelcontextprotocol/conformance server --url "http://127.0.0.1:${PORT}/mcp" --scenario "${SCENARIO}"
fi
