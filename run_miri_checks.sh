#!/usr/bin/env bash
set -euo pipefail

echo "=== Running Miri Undefined Behavior (UB) & Provenance Analysis ==="
export PATH="$HOME/.cargo/bin:$PATH"

cargo +nightly miri test -p medplum-mcp-core --test test_miri_zerocopy_and_transmutation

echo "=== MIRI VERIFICATION CLEAN (Zero Undefined Behavior, Zero Alignment/Provenance Violations) ==="
