#!/usr/bin/env bash
set -euo pipefail

echo "=== Running Coverage-Guided Fuzz Suite (libFuzzer + AddressSanitizer) ==="
export PATH="$HOME/.cargo/bin:$PATH"

TARGETS=("fuzz_simd_distillation" "fuzz_binary_audit_header" "fuzz_jsonrpc_dispatch")

for target in "${TARGETS[@]}"; do
    echo "--- Fuzzing target: ${target} (3 seconds) ---"
    cargo +nightly fuzz run "${target}" -- -max_total_time=3
done

echo "=== ALL FUZZ TARGETS COMPLETED CLEANLY (Zero Panics, Zero ASAN Violations) ==="
