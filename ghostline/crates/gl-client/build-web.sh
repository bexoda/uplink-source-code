#!/usr/bin/env bash
# Builds the browser bundle into crates/gl-client/web/pkg. Serve crates/gl-client/web with any static server.
# Needs: rustup target add wasm32-unknown-unknown; cargo install wasm-bindgen-cli (same version as the wasm-bindgen crate in Cargo.lock).
set -euo pipefail
cd "$(dirname "$0")/../.."
cargo build -p gl-client --lib --target wasm32-unknown-unknown --profile web
wasm-bindgen --target web --no-typescript --out-dir crates/gl-client/web/pkg \
  target/wasm32-unknown-unknown/web/gl_client.wasm
# Optional size pass. Needs a RECENT binaryen (>= 117): older wasm-opt (e.g. Ubuntu's v108)
# rewrites modern rustc output into a module browsers reject. Opt in with WASM_OPT=1.
if [ "${WASM_OPT:-0}" = "1" ]; then
  wasm-opt -Oz --all-features -o crates/gl-client/web/pkg/gl_client_bg.wasm crates/gl-client/web/pkg/gl_client_bg.wasm
fi
ls -la crates/gl-client/web/pkg
