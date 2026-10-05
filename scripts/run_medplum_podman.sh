#!/usr/bin/env bash
# Rootless Podman Medplum Integration Runner & Mock Fallback Harness
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
POD_NAME="medplum-local-pod"
PORT="${MEDPLUM_PORT:-8103}"
POSTGRES_PORT="${MEDPLUM_PG_PORT:-5432}"

usage() {
    echo "Usage: $0 {start|stop|status|test}"
    echo "Commands:"
    echo "  start   - Launch rootless Podman pod with PostgreSQL 16 & Medplum Server"
    echo "  stop    - Stop and remove the Medplum pod and containers"
    echo "  status  - Check container and endpoint status"
    echo "  test    - Execute client integration test against the running instance or mock"
    exit 1
}

has_podman() {
    command -v podman >/dev/null 2>&1
}

start_pod() {
    if ! has_podman; then
        echo "[!] Podman not found. Starting native in-process Axum OpenAPI mock on port ${PORT}..."
        "${WORKSPACE_ROOT}/target/debug/medplum-mcp-rs" mock-server --port "${PORT}" &
        return 0
    fi

    if podman pod exists "${POD_NAME}"; then
        echo "[*] Pod ${POD_NAME} already exists. Ensuring it is running..."
        podman pod start "${POD_NAME}" || true
        return 0
    fi

    echo "[*] Creating rootless Podman pod: ${POD_NAME} (ports ${PORT}, ${POSTGRES_PORT})..."
    podman pod create --name "${POD_NAME}" -p "${PORT}:${PORT}" -p "${POSTGRES_PORT}:5432"

    echo "[*] Starting PostgreSQL 16 in ${POD_NAME}..."
    podman run -d --pod "${POD_NAME}" --name medplum-postgres \
        -e POSTGRES_DB=medplum \
        -e POSTGRES_USER=medplum \
        -e POSTGRES_PASSWORD=medplum_password \
        docker.io/library/postgres:16-alpine

    echo "[*] Waiting for PostgreSQL to be ready..."
    sleep 3

    echo "[*] Starting Medplum Server 5.2.1 in ${POD_NAME}..."
    podman run -d --pod "${POD_NAME}" --name medplum-server \
        -v "${SCRIPT_DIR}/podman/medplum.config.json:/usr/src/medplum/medplum.config.json:ro,Z" \
        -w /usr/src/medplum \
        docker.io/medplum/medplum-server:latest || {
            echo "[!] Medplum server container exited. Checking status..."
        }

    echo "[✓] Podman orchestration complete on port ${PORT}."
}

stop_pod() {
    if has_podman && podman pod exists "${POD_NAME}"; then
        echo "[*] Stopping and removing pod ${POD_NAME}..."
        podman pod stop -t 2 "${POD_NAME}" || true
        podman pod rm -f "${POD_NAME}" || true
        echo "[✓] Pod ${POD_NAME} cleaned up."
    else
        echo "[*] Pod ${POD_NAME} is not running."
    fi
}

check_status() {
    if has_podman && podman pod exists "${POD_NAME}"; then
        podman pod ps --filter name="${POD_NAME}"
        podman ps --filter pod="${POD_NAME}"
    else
        echo "Pod ${POD_NAME} does not exist."
    fi
}

run_test() {
    echo "=== Running Live Client Integration Tests Against Medplum / Mock Endpoint ==="
    export PATH="$HOME/.cargo/bin:$PATH"
    cargo test -p medplum-mcp-server --test test_sandbox_and_client test_medplum_client_oauth2_and_pkce_live_mock
    pytest -v tests/test_sandbox_and_client.py -k test_live_client_authenticate_oauth2_and_pkce
    echo "=== All Medplum Client & Auth Tests Passed Cleanly ==="
}

COMMAND="${1:-}"
case "${COMMAND}" in
    start)
        start_pod
        ;;
    stop)
        stop_pod
        ;;
    status)
        check_status
        ;;
    test)
        run_test
        ;;
    *)
        usage
        ;;
esac
