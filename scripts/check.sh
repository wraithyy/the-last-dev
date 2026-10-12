#!/usr/bin/env bash
# Full local gate; stops at first failure.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."

steps=(
  "cargo fmt --all --check"
  "cargo clippy --workspace --all-targets -- -D warnings"
  "cargo test --workspace"
  "cargo build -p game_client"
  "cargo clippy -p game_client --target wasm32-unknown-unknown -- -D warnings"
)
for s in "${steps[@]}"; do
  echo "==> $s"
  $s
done
echo "all checks passed"
