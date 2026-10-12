#!/usr/bin/env bash
# Build game_client for wasm and generate JS glue into web/pkg. Usage: build-web.sh [--dev]
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."

profile="wasm-release"
target_dir="wasm-release"
if [[ "${1:-}" == "--dev" ]]; then
  profile=dev
  target_dir=debug
fi

want=$(awk '/^name = "wasm-bindgen"$/ {getline; gsub(/[^0-9.]/, "", $0); print; exit}' Cargo.lock)
if ! command -v wasm-bindgen >/dev/null; then
  echo "error: wasm-bindgen not found. Run: cargo install wasm-bindgen-cli --version ${want} --locked" >&2
  exit 1
fi
have=$(wasm-bindgen --version | awk '{print $2}')
if [[ "$have" != "$want" ]]; then
  echo "error: wasm-bindgen ${have} != Cargo.lock ${want}. Run: cargo install wasm-bindgen-cli --version ${want} --locked --force" >&2
  exit 1
fi

cargo build -p game_client --target wasm32-unknown-unknown --profile "$profile"

wasm=target/wasm32-unknown-unknown/${target_dir}/game_client.wasm
wasm-bindgen --target web --no-typescript --out-dir web/pkg --out-name game_client "$wasm"

if command -v wasm-opt >/dev/null && [[ "$profile" != dev ]]; then
  wasm-opt -Os -o web/pkg/game_client_bg.wasm web/pkg/game_client_bg.wasm
else
  echo "wasm-opt skipped (not installed or dev build)"
fi

if [[ -d assets ]]; then
  rm -rf web/assets
  cp -R assets web/assets
fi
echo "ok: web/pkg ready; serve with scripts/serve-web.sh"
