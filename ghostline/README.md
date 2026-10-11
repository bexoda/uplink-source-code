# Ghostline

A hacker-career simulation written in Rust, targeting Windows, macOS, Linux and the browser (wasm).

You take contracts, bounce your connection through relays, break layered server security, steal or
erase data, and wipe your logs before a trace walks back to you. Three strikes and the run is over.

**Status: playable local prototype.** The simulation, desktop and browser client, encrypted local saves, login and
verified-run sync server all work and are tested. The client does not talk to the server yet (no login or sync
screen), so every game is offline and unverified.

![Ghostline mid-hack: tool running, trace closing in](docs/screenshot.png)

> **Private project. Do not publish or distribute anything built from it.**
> This is a derivative of Uplink, not a clean-room project, and should never be described as one: the author
> read the Introversion developer source, and the client reads Uplink's original art and fonts from the
> player's own Uplink installation (or the developer CD). Until Introversion agrees in writing, do not share
> builds, host the web version or the sync server for other people, or put their assets anywhere except the
> original developer-CD tree. The assets are read at runtime and are never copied into `ghostline/`
> (`.gitignore` blocks `*.dat`, `*.tif`, `*.ttf`, `*.wav`, `*.mp3` here as a safety net).

## Layout

| crate | what it is |
|---|---|
| `crates/gl-sim` | Headless, deterministic game simulation. Same seed and commands give identical state on every platform. |
| `crates/gl-save` | Encrypted save container, desktop file store, optional OS keychain, sync decision logic. |
| `crates/gl-assets` | Reads Uplink's `.dat` archives at runtime (REDSHIRT wrapper, ZIP, TIFF sprites, fonts, WAV). Never bundles or copies them. |
| `crates/gl-client` | egui client, one codebase for desktop and browser (wasm). Game logic is in `game.rs`, free of UI code, so it is unit-tested. |
| `crates/gl-server` | Axum + Postgres sync API. Stores client-encrypted blobs it cannot read. |

Design details, save format, sync model and known gaps: [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md).

## Requirements

- Rust 1.80+ (developed on 1.97)
- For the web target: `rustup target add wasm32-unknown-unknown`
- For server tests: a PostgreSQL 13+ instance

## Play it

```sh
# Desktop (Windows, macOS, Linux). Linux needs libGL and libxkbcommon-x11 installed.
cargo run -p gl-client --release

# Browser. Needs `rustup target add wasm32-unknown-unknown` and `cargo install wasm-bindgen-cli`
# at the SAME version as the wasm-bindgen crate in Cargo.lock.
crates/gl-client/build-web.sh
cd crates/gl-client/web && python3 -m http.server 8000   # then open http://localhost:8000
```

How to play: click relays to build a chain (order matters), click a target, accept a contract, connect.
Your first attack starts a trace that walks back from the target hop by hop. Break layers in order, copy
the file, and erase your log at a hop before the tracer reaches it and the trace dies. Three times traced
ends the career.

- **Desktop saves** autosave to your data directory (`~/.local/share/ghostline`, `~/Library/Application Support/ghostline`, `%APPDATA%\ghostline`),
  sealed and tamper-evident. The key is in the OS keychain when built with `--features os-keyring`, otherwise a key file
  next to the save. That protects against casual edits only; it is not anti-cheat.
- **Browser saves are not built yet**: progress is lost on reload. A banner says so.
- `GHOSTLINE_SEED=42` starts a reproducible world (desktop only), handy for bug reports.
- Release size: the wasm bundle is about 6 MB (2 MB gzipped). `WASM_OPT=1` runs `wasm-opt`, but only use binaryen 117 or newer;
  older versions (Ubuntu ships 108) produce a module browsers reject.

## Original Uplink assets

The client uses Uplink's own artwork when it can find it, and falls back to its plain vector look when it can't.
With assets it shows: the world map (servers sit on land, placed by a stable hash of their name), the HUD icons,
the original fonts, and the Access Granted / Access Denied banners.

It looks for `graphics.dat` and `fonts.dat` in, in order: `$UPLINK_DIR`, the usual Steam/GOG/Applications
locations, and a few Linux paths. For the developer CD in this repo:

```sh
UPLINK_DIR=/path/to/uplink-source-code/uplink/bin cargo run -p gl-client --release
```

- Decoded in memory only. Patch archives (`patch*.dat`) override the base graphics, as in the game.
- **Browser:** no assets yet. The plan is a file picker that reads your own `.dat` files locally in the page and
  never uploads them; hosting their assets from our server is a distribution and is off the table for now.
- **Not used yet:** sounds (`sounds.dat`, 48 WAVs), music (`music.dat`, `world.dat`), gateway artwork, the game's
  text tables (`data.dat`). Audio needs a platform audio backend.
- `cargo test -p gl-assets` decodes every sprite in the real archives (set `UPLINK_DIR`, or it uses the dev CD in this repo).

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
export GL_SERVER_KEY=$(openssl rand -hex 32)      # seals stored runs. BACK THIS UP: losing it loses every run
export GL_PUBLIC_URL=https://api.example.com      # where providers send users back; no trailing slash
export GL_WEB_ORIGINS=https://play.example.com    # comma-separated web client origins allowed as login return targets
export GL_ALLOWED_ORIGIN=https://play.example.com # CORS; omit for same-origin only
# Enable each login provider by setting both vars (register the callback
# $GL_PUBLIC_URL/v1/auth/<provider>/callback with the provider):
export GL_OAUTH_GOOGLE_CLIENT_ID=...   GL_OAUTH_GOOGLE_CLIENT_SECRET=...
export GL_OAUTH_GITHUB_CLIENT_ID=...   GL_OAUTH_GITHUB_CLIENT_SECRET=...
export GL_OAUTH_DISCORD_CLIENT_ID=...  GL_OAUTH_DISCORD_CLIENT_SECRET=...
export BIND=127.0.0.1:8080
cargo run -p gl-server --release
```

Migrations run on startup. Put it behind a TLS-terminating reverse proxy; it does no TLS or rate limiting itself.
With no provider configured the server starts but nobody can log in.

### API

Everything except `/healthz` and the `/v1/auth/...` login routes needs `Authorization: Bearer <access_token>`.

**Login and accounts**

| method | path | notes |
|---|---|---|
| `GET` | `/v1/auth/providers` | Names of enabled providers. |
| `GET` | `/v1/auth/{provider}/start?return_to=&client_challenge=` | Open in a browser. Redirects to the provider. `return_to` must be an allowed web origin or `http://127.0.0.1`/`localhost` (desktop). |
| `GET` | `/v1/auth/{provider}/callback` | Provider redirects here. Server redirects on to `return_to?login_code=...` (or `?error=...`). |
| `POST` | `/v1/auth/exchange` | `{login_code, client_verifier}` -> `{access_token, refresh_token, expires_in, account_id, handle, needs_handle}`. |
| `POST` | `/v1/auth/refresh` | `{refresh_token}` -> new pair. Refresh tokens rotate; reusing an old one revokes the session. |
| `POST` | `/v1/auth/logout` | Revokes the current session. |
| `POST` | `/v1/auth/link/{provider}` | Logged in. `{return_to, client_challenge}` -> `{url}` to open. Adds a login to this account. |
| `GET` | `/v1/me` | Account id, handle, linked providers. |
| `PUT` | `/v1/me/handle` | `{handle}`: 3-20 letters/digits/underscore, unique case-insensitively, **set once**. |
| `DELETE` | `/v1/me/identities/{provider}` | Unlink. Refused for your last login. |
| `DELETE` | `/v1/me` | `{confirm}` = your handle (or `DELETE`). Erases the account and all its data. |

**Saves and runs**

| method | path | notes |
|---|---|---|
| `GET` | `/v1/saves` | List backup slots: name, revision, size, updated time. |
| `GET` | `/v1/saves/{slot}` | Returns the blob; `x-revision` header has its revision. |
| `PUT` | `/v1/saves/{slot}` | Needs `x-if-revision` (0 to create). `409` with the current `x-revision` on conflict. Max 2 MiB, 8 slots. **Unverified backup only.** |
| `POST` | `/v1/seeds?n=1` | Issue seeds (decimal strings) to start verified runs. Max 10 unused. |
| `PUT` | `/v1/runs/{seed}/segment` | Upload a command-log segment. `200` with the server's computed credits/digest; `409` + `x-verified-tick` if it doesn't follow; `422` if illegal or implausible; `426` if the rules version differs. |
| `GET` | `/v1/runs/{seed}/state` | The server-verified world, for resuming on another device. |
| `GET` | `/v1/runs` | List your verified runs. |

### Client login flow (web and desktop)

1. Client makes a random secret `client_verifier` and computes `client_challenge = base64url(SHA256(verifier))` (no padding, 43 chars).
2. Client opens `GET /v1/auth/google/start?return_to=<url>&client_challenge=<c>` in the **system browser** (never an embedded webview; Google blocks them).
   Desktop: `return_to` is a loopback listener such as `http://127.0.0.1:41234/cb`. Web: your page.
3. After login the browser lands on `return_to?login_code=...`. The client `POST`s `/v1/auth/exchange` with the code and its `client_verifier`.
   A code intercepted without the verifier is useless, and it is single-use with a 60 s life.
4. Keep the refresh token in the OS keychain (desktop) or an `HttpOnly` cookie set by your own backend (web). Don't put it in `localStorage`.
5. If `needs_handle`, ask for a hacker name and `PUT /v1/me/handle`.

**Local-first:** a player with no account just plays; progress lives in their local profile. When they choose
"sync", they log in, fetch seeds, and upload their command log as segments. Nothing about login changes local play.

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
- [x] OAuth login (Google, GitHub, Discord), sessions, account linking, hacker handles, account deletion
- [ ] Email magic link (needs SMTP), Steam (OpenID 2.0), Microsoft
- [x] `egui` client (native + wasm): world map, connect, tasks, trace bar, contracts, tools, desktop autosave
- [x] Original Uplink art, fonts and world map loaded at runtime from the player's own install (desktop)
- [ ] Sounds and music; gateway/hardware screens; browser file picker for assets
- [ ] Client login screen and sync (seeds, segments, resume from server state, conflict handling)
- [ ] Browser saves
- [ ] Cross-platform determinism check: run the sim in wasm and compare digests with native
- [ ] Contests with per-player variants and a leaderboard (needs a deeper sim first)
- [ ] HTTP sync client and conflict-resolution UI
- [ ] Save schema migrations
- [ ] More tools, missions, economy, and new hacking mechanics
- [ ] Web build pipeline (`wasm-opt`, size budget)
- [ ] Settle licensing with Introversion before any public release
