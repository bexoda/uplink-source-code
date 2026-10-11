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
export GL_SERVER_KEY=$(openssl rand -hex 32)          # seals stored runs. BACK THIS UP: losing it loses every run
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
| `PUT` | `/v1/saves/{slot}` | Needs `x-if-revision` (0 to create). `409` with the current `x-revision` on conflict. Max 2 MiB, 8 slots. **Unverified backup only.** |
| `POST` | `/v1/seeds?n=1` | Issue seeds (decimal strings) to start verified runs. Max 10 unused. |
| `PUT` | `/v1/runs/{seed}/segment` | Upload a command-log segment. `200` with the server's computed credits/digest; `409` + `x-verified-tick` if it doesn't follow; `422` if illegal or implausible; `426` if the rules version differs. |
| `GET` | `/v1/runs/{seed}/state` | The server-verified world, for resuming on another device. |
| `GET` | `/v1/runs` | List your verified runs. |

## Cheating

The local save is encrypted, but that is **not** anti-cheat: the key is on the player's machine and
Cheat Engine edits memory, not files. Instead, clients upload the commands they played and the server
replays them on its own copy of the world, so edited numbers never reach it. Seeds are server-issued,
game time is capped by real elapsed time, and forged or illegal logs are rejected. Details and limits
(bots are not stopped) in [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md).

## Saves

`GLSV | format | schema | key mode | salt | nonce | XChaCha20-Poly1305( lz4( postcard(World) ) )`

- **Desktop:** key in the OS keychain, atomic file writes, one backup. Protects the file from casual edits only.
- **Web:** key derived from a passphrase (Argon2id). Lose it and the save is unrecoverable.
- Backups are encrypted client-side, so the server cannot read or validate them. Verified runs are the opposite: the server holds and seals the state it computed itself.
- A change to the `World` shape currently makes old saves unreadable; migrations are not built yet.

## Roadmap

- [x] Run recording, server-side replay verification, server-issued seeds
- [ ] OAuth login (Google, GitHub, Discord, email magic link) and local-to-online account linking
- [ ] `egui` client (native + wasm): world map, connect screen, task manager, trace bar
- [ ] HTTP sync client and conflict-resolution UI
- [ ] Save schema migrations
- [ ] More tools, missions, economy, and new hacking mechanics
- [ ] Web build pipeline (`wasm-opt`, size budget)
- [ ] Settle licensing with Introversion before any public release
