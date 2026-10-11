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
> (`.gitignore` blocks `*.dat`, `*.tif`, `*.ttf`, `*.wav`, `*.mp3`, `*.uni` here as a safety net).

## Layout

| crate | what it is |
|---|---|
| `crates/gl-sim` | Headless, deterministic game simulation. Same seed and commands give identical state on every platform. |
| `crates/gl-save` | Encrypted save container, desktop file store, optional OS keychain, sync decision logic. |
| `crates/gl-assets` | Reads Uplink's `.dat` archives at runtime (REDSHIRT wrapper, ZIP, TIFF sprites, fonts, WAV). Never bundles or copies them. |
| `crates/gl-audio` | Pure-Rust sound: a MikMod UniMod (UN04/UN05) loader and replayer ported from libmikmod, a WAV reader, and the final mixer. No I/O, no device. **LGPL-2.0-or-later** (see below). |
| `crates/gl-client` | egui client, one codebase for desktop and browser (wasm). Game logic is in `game.rs`, free of UI code, so it is unit-tested. |
| `crates/gl-server` | Axum + Postgres sync API. Stores client-encrypted blobs it cannot read. |

Design details, save format, sync model and known gaps: [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md).

## Requirements

- Rust 1.80+ (developed on 1.97)
- For the web target: `rustup target add wasm32-unknown-unknown`
- For server tests: a PostgreSQL 13+ instance

## Play it

```sh
# Desktop (Windows, macOS, Linux). Linux needs libGL, libxkbcommon-x11 and ALSA (libasound2; libasound2-dev to build).
cargo run -p gl-client --release

# Browser. Needs `rustup target add wasm32-unknown-unknown` and `cargo install wasm-bindgen-cli`
# at the SAME version as the wasm-bindgen crate in Cargo.lock.
crates/gl-client/build-web.sh
cd crates/gl-client/web && python3 -m http.server 8000   # then open http://localhost:8000
```

How to play (with Uplink data present, you get the original interface):

1. Click the small world map (top right) to open the big one. Click dots to build a connection: relays first, in order,
   then the machine you are going after. White dots become orange when they are in your route.
2. Connect to **Uplink Internal Services** first: **Software Sales** sells and upgrades your programs, and the **Bulletin Board**
   posts contracts. You start with 1000 credits and three programs.
3. Connect to a target. Its security systems are listed, each with the original Access Denied / Access Granted image.
   Click one, then run the matching program from the **software button** (bottom left): Crackers (Password Breaker,
   Decrypter) wait for you to click the password box or cypher; Bypassers (Firewall, Proxy) run at once.
4. Your first move against a protected machine starts a **trace**. It crawls back along your connection (red on the map and
   in the Connection Analyser). The **Trace Tracker** window pulses faster as it closes in.
5. With every system down, **Proceed** to the server menu. Arm the **File Copier** (File Utilities) and click a file. Arm the
   **Log Deleter** (Security) and click your log on a machine: a trace cannot cross a machine whose log you erased.
6. Three times traced ends the career.

Ghostline has no interface of its own: without your Uplink data it shows only a request for the files. A new game starts connected to Uplink's own server, as the original does.

- **Desktop saves** autosave to your data directory (`~/.local/share/ghostline`, `~/Library/Application Support/ghostline`, `%APPDATA%\ghostline`),
  sealed and tamper-evident. The key is in the OS keychain when built with `--features os-keyring`, otherwise a key file
  next to the save. That protects against casual edits only; it is not anti-cheat.
- **Browser saves** use the same sealed format as desktop, kept in the browser's localStorage and written every second (a save is under 1 KB). The previous save is kept as a fallback. The key sits next to the save, so like the desktop key file this only catches casual edits; the server's replay check is the anti-cheat. Clearing site data deletes the save.
- `GHOSTLINE_SEED=42` starts a reproducible world (desktop only), handy for bug reports.
- Release size: the wasm bundle is about 6 MB (2 MB gzipped). `WASM_OPT=1` runs `wasm-opt`, but only use binaryen 117 or newer;
  older versions (Ubuntu ships 108) produce a module browsers reject.

## Original Uplink assets

When the player's own Uplink data is found, the client rebuilds the game's interface from the original source: the 1024x768
screen, the "Uplink Blue" theme (read from `patch.dat`), the `dungeon.ttf` font (the only font the game uses in play),
and the sprites. Layouts and positions come straight from `hud_interface.cpp`, `worldmap_interface.cpp`, `sw_interface.cpp`,
the remote screens (`highsecurity`, `password`, `cypher`, `menu`, `fileserver`, `log`, `swsales`, `bbs`), the task manager
(`vanbakel`) and the local panels (hardware, memory, status, finance, analyser, mission).

It looks for `graphics.dat` and `fonts.dat` in, in order: `$UPLINK_DIR`, the usual Steam/GOG/Applications locations, and a
few Linux paths. For the developer CD in this repo:

```sh
UPLINK_DIR=/path/to/uplink-source-code/uplink/bin cargo run -p gl-client --release
```

- Everything is decoded in memory at runtime. Patch archives (`patch*.dat`) override the base graphics, as in the game.
- **Browser:** the page asks for your own `graphics.dat`, `fonts.dat`, `patch*.dat`, `sounds.dat` and `music.dat` with a file
  picker. They are read inside the page, kept in that browser's IndexedDB so a reload doesn't ask again, and never uploaded.
  Open the page with `?forget-assets` to drop them. Until you pick them, the page shows only the file request. We never host their files,
  because that would be distributing them.
- **Not built yet:** the main menu and loading screens, gateway artwork,
  the awards, the LAN and IRC screens, e-mail, the phone dialler, world-map zoom, and the game's own text tables (`data.dat`).
  Faster-than-real-time speeds are shown but disabled, because online runs are checked against real time.
- `cargo test -p gl-audio` plays all six songs to the end and checks each length against libmikmod; it also decodes every
  effect. `cargo test -p gl-assets` decodes every sprite in the real archives, and `cargo test -p gl-client --test uplink_ui` plays a whole
  hack through the interface with synthesised mouse clicks (both use the dev CD in this repo, or `UPLINK_DIR`).

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

## Sound and music

With the original interface, the client plays the original sounds and music. It decodes them from your own `sounds.dat` and `music.dat`.

- **Music:** the six modules in `music.dat` are MikMod UniMod files (`UN05`). `gl-audio` replays them with a Rust port of
  libmikmod's player and software mixer, using the settings the original SDL_mixer backend used: volume 85/128, `md_volume`
  96 and full stereo separation. Checked against libmikmod 3.3.11 on all six songs: same lengths, same loudness, and
  99.996% or more of the samples are identical. The rest differ by tiny amounts at mixer-buffer boundaries.
  Songs follow the original "main" playlist: bluevalley, serenity, mystique, a94final, symphonic, myst2. When one ends, the next starts.
  The original SDL build repeated one song forever; the Windows build moved through the list. We follow the Windows build.
- **Effects** play at a quarter volume (`MIX_MAX_VOLUME / 4`) on up to 15 channels, at the moments the original played them:
  - mouseclick on any button press
  - softwaremenu when the software menu or a category opens
  - runsoftware when a program starts
  - addlink when a bounce is added on the world map
  - close for the world map and local screens
  - login when getting past a system's security
  - done when a password breaks
  - success when a mission completes
  - failure when a purchase is refused
  - newmail when a mission or a fine message arrives
  - tracebleep while a trace runs
  
  The trace bleep speeds up as the trace advances. The original timed it from its trace-time estimate, so our gap is an approximation.
- `world.dat` is a 14 MB MP3 that nothing in the game code references; it is not used.
- No device, or `GHOSTLINE_AUDIO=off`, means silence. Nothing else changes.
- **Browser:** same sound through Web Audio, once you have picked your files (the pick is the click browsers require
  before they allow audio).
- **Licence:** `crates/gl-audio/src/player.rs` and `uni.rs` port libmikmod (LGPL-2.0-or-later), so `gl-audio` carries that
  licence. That is fine for private builds. Before any public binary, either ship `gl-audio` under the LGPL's terms (keep it
  a separate, replaceable component and offer its source) or replace the port with a clean-room player.

## Roadmap

- [x] Run recording, server-side replay verification, server-issued seeds
- [x] OAuth login (Google, GitHub, Discord), sessions, account linking, hacker handles, account deletion
- [ ] Email magic link (needs SMTP), Steam (OpenID 2.0), Microsoft
- [x] `egui` client (native + wasm): world map, connect, tasks, trace bar, contracts, tools, desktop autosave
- [x] The original Uplink interface rebuilt from the source (HUD, world map, software menu, task manager, security, password, cypher, file, log, shop and contract screens, local panels), desktop
- [x] Original sounds and music
- [x] Browser: the player's own `.dat` files through a file picker, cached in the browser, never uploaded
- [ ] Main menu, phone dialler, map zoom, e-mail
- [ ] Client login screen and sync (seeds, segments, resume from server state, conflict handling)
- [x] Browser saves (localStorage, sealed, with a backup)
- [ ] Cross-platform determinism check: run the sim in wasm and compare digests with native
- [ ] Contests with per-player variants and a leaderboard (needs a deeper sim first)
- [ ] HTTP sync client and conflict-resolution UI
- [ ] Save schema migrations
- [ ] More tools, missions, economy, and new hacking mechanics
- [ ] Web build pipeline (`wasm-opt`, size budget)
- [ ] Settle licensing with Introversion before any public release
