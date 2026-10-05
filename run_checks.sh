#!/usr/bin/env bash
set -euo pipefail

echo "=== [1/3] Ruff Lint & Formatting ==="
ruff check .
ruff format --check .

echo "=== [2/3] Mypy Strict Type Check ==="
mypy medplum_mcp tests

echo "=== [3/3] Pytest Test Suite ==="
pytest -v

echo "=== ALL CHECKS PASSED (Green & Clean) ==="
