#!/usr/bin/env bash
# Builds the browser bundle into crates/ur-client/web/pkg. Serve crates/ur-client/web with any static server.
# Needs: rustup target add wasm32-unknown-unknown; cargo install wasm-bindgen-cli (same version as the wasm-bindgen crate in Cargo.lock).
set -euo pipefail
cd "$(dirname "$0")/../.."
cargo build -p ur-client --lib --target wasm32-unknown-unknown --profile web
wasm-bindgen --target web --no-typescript --out-dir crates/ur-client/web/pkg \
  target/wasm32-unknown-unknown/web/ur_client.wasm
# Optional size pass. Needs a RECENT binaryen (>= 117): older wasm-opt (e.g. Ubuntu's v108)
# rewrites modern rustc output into a module browsers reject. Opt in with WASM_OPT=1.
if [ "${WASM_OPT:-0}" = "1" ]; then
  wasm-opt -Oz --all-features -o crates/ur-client/web/pkg/ur_client_bg.wasm crates/ur-client/web/pkg/ur_client_bg.wasm
fi
ls -la crates/ur-client/web/pkg
