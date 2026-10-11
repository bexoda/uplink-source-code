# Ghostline

A hacker-career simulation written in Rust, targeting Windows, macOS, Linux and the browser (wasm).

You take contracts, bounce your connection through relays, break layered server security, steal or
erase data, and wipe your logs before a trace walks back to you. Three strikes and the run is over.

**Status: early foundation.** The simulation, encrypted saves and sync server work and are tested.
There is no UI yet.

> **Private project. Do not publish or distribute.**
> Written by someone who had read the Introversion *Uplink* developer source, so it is **not** a true
> clean room. No code, data or assets are carried over, but the licensing question with Introversion is
> unresolved. Never use the Uplink name, art, audio, fonts or `.dat` files here.

## Layout

| crate | what it is |
|---|---|
| `crates/gl-sim` | Headless, deterministic game simulation. Same seed and commands give identical state on every platform. |
| `crates/gl-save` | Encrypted save container, desktop file store, optional OS keychain, sync decision logic. |
| `crates/gl-server` | Axum + Postgres sync API. Stores client-encrypted blobs it cannot read. |

Design details, save format, sync model and known gaps: [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md).

## Requirements

- Rust 1.80+ (developed on 1.97)
- For the web target: `rustup target add wasm32-unknown-unknown`
- For server tests: a PostgreSQL 13+ instance

## Build and test

```sh
cargo test --workspace                  # sim + save tests; server tests are SKIPPED without a database
cargo clippy --workspace --all-targets
cargo check -p gl-sim -p gl-save --target wasm32-unknown-unknown
cargo check -p gl-save --features os-keyring   # macOS Keychain / Windows Credential Manager / Linux keyutils
```

Run the server tests against a real database (they are silently skipped otherwise, and still report "ok"):

```sh
createdb gl_test
TEST_DATABASE_URL=postgres://user:pass@localhost/gl_test cargo test -p gl-server
```

## Run the sync server

```sh
export DATABASE_URL=postgres://user:pass@localhost/ghostline
export GL_REGISTRATION_KEY=$(openssl rand -hex 16)    # without this, anyone can create accounts
export GL_ALLOWED_ORIGIN=https://your-web-client.example   # CORS; omit for same-origin only
export BIND=127.0.0.1:8080
cargo run -p gl-server --release
```

Migrations run on startup. Put it behind a TLS-terminating reverse proxy; it does no TLS or rate limiting itself.

### API

All routes except `/healthz` and account creation need `Authorization: Bearer <token>`.

| method | path | notes |
|---|---|---|
| `POST` | `/v1/accounts` | Needs `x-registration-key`. Returns `{ "token": "..." }` once; only its hash is stored. |
| `GET` | `/v1/saves` | List slots: name, revision, size, updated time. |
| `GET` | `/v1/saves/{slot}` | Returns the blob; `x-revision` header has its revision. |
| `PUT` | `/v1/saves/{slot}` | Needs `x-if-revision` (0 to create). `409` with the current `x-revision` on conflict. Max 2 MiB, 8 slots. |

## Saves

`GLSV | format | schema | key mode | salt | nonce | XChaCha20-Poly1305( lz4( postcard(World) ) )`

- **Desktop:** key in the OS keychain, atomic file writes, one backup.
- **Web:** key derived from a passphrase (Argon2id). Lose it and the save is unrecoverable.
- Encryption is client-side, so the server cannot read or validate saves.
- A change to the `World` shape currently makes old saves unreadable; migrations are not built yet.

## Roadmap

- [ ] `egui` client (native + wasm): world map, connect screen, task manager, trace bar
- [ ] HTTP sync client and conflict-resolution UI
- [ ] Save schema migrations
- [ ] More tools, missions, economy, and new hacking mechanics
- [ ] Web build pipeline (`wasm-opt`, size budget)
- [ ] Settle licensing with Introversion before any public release
