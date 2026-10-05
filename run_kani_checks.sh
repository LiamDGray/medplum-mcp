#!/usr/bin/env bash
set -euo pipefail

echo "=== Running Kani Bounded Model Checking Formal Verification ==="
export PATH="$HOME/.cargo/bin:$PATH"

cargo kani -p medplum-mcp-core --tests

echo "=== KANI FORMAL VERIFICATION SUCCESSFUL (Bit-precise mathematical proofs complete) ==="
