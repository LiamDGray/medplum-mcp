#!/usr/bin/env bash
set -euo pipefail
export PATH="$HOME/.cargo/bin:$PATH"
echo "=== 1. Checking formatting (rustfmt) ==="
cargo fmt --all -- --check
echo "=== 2. Running static analysis (clippy) ==="
cargo clippy --workspace --all-targets -- -D warnings
echo "=== 3. Running unit & integration tests ==="
cargo test --workspace
echo "All Rust checks passed successfully!"
