# uplink_rust architecture

Hacking-career sim. Rust workspace targeting Windows, macOS, Linux and the browser (wasm32).

> **Provenance warning.** This is a derivative of Introversion's *Uplink*, not a clean room. The author read the
> developer source, and the client reads Uplink's original art and fonts from the player's own installation.
> Keep it private and do not distribute builds, the web version or the sync server until Introversion agrees
> in writing. Assets are loaded at runtime and never copied into this tree.

## Crates

| crate | role | targets |
|---|---|---|
| `ur-sim` | Headless deterministic simulation. No I/O, floats, wall-clock or `HashMap` iteration. | native + wasm |
| `ur-save` | Sealed save container, desktop store, sync protocol/decision logic. | native + wasm (`store`, `keychain` native only) |
| `ur-server` | Axum + Postgres sync API. Stores opaque blobs. | server |
| `ur-assets` | Runtime loader for Uplink's `.dat` archives: REDSHIRT wrapper (marker, then every byte +128) around a ZIP, 8-bit and 1-bit TIFF sprites, TTF fonts, WAV. Patch archives override base graphics. Memory-backed mode for a future browser file picker. | native + wasm (`locate` native only) |
| `ur-audio` | UniMod (UN04/UN05) loader and replayer ported from libmikmod's `load_uni.c`, `mplayer.c` and `virtch.c` (same 11-bit fixed-point stepping, interpolation and click ramp, so output matches libmikmod), a WAV reader and the final mixer. Pure: bytes in, f32 stereo out. LGPL-2.0-or-later because of the port. | native + wasm |
| `ur-client` | egui/eframe client over `ur-sim`. `uplink/` is the original interface rebuilt from the source on a 1024x768 virtual screen (`canvas.rs`: scaling, gradients, text, one-frame-late hit testing; `theme.rs`, `sprites.rs`, `hud.rs`, `map.rs`, `swmenu.rs`, `remote.rs`, `local.rs`, `menu.rs` for the login, start-sequence and loading screens), the only interface. `app.rs` is the shell: it finds the data (or asks for it), owns the game, saves, and runs `uplink/` each frame. `game.rs` is the UI-free controller (fixed-step pacing, selection, narration, trace view, sound cues). | native + wasm |

## Simulation rules

- Fixed 100 ms tick. The UI drives `World::tick` from an accumulator; rendering never mutates state.
- All input is a `Command`; all output is an `Event` pushed into a caller-owned `Vec` (no per-tick allocation in the hot path).
- Integer "work units" only. Same seed + same commands = byte-identical state on every platform (tested).
- Entities live in `SlotMap` arenas addressed by `Copy` ids: no `Rc`, no globals, no raw pointers.
- Machines come in three kinds: relays you bounce through, targets with layered defences, and Uplink's own services machine (software shop, contract board). Each log entry records when the connection was made.
- Mechanics: layered server security, relay bounce chains, trace walking back hop by hop, log erasure to
  break a trace, CPU shared across concurrent tasks, three strikes ends the run. All numbers in `tuning.rs`.

## Saves

```
magic "GLSV" | format u16 | schema u32 | key_mode u8 | salt[16] | nonce[24] | XChaCha20-Poly1305(lz4(postcard(World)))
```

The header is the AEAD associated data, so tampering with any byte fails decryption.

**This is not anti-cheat.** The key lives on the player's machine and Cheat Engine edits process memory,
not the file, so local encryption only stops casual edits of the save file. Cheating is stopped by server
verification (next section), never by anything the client does.

- **Desktop:** key lives in the OS keychain (`os-keyring` feature), never on disk. Files are written
  temp -> fsync -> rename, with one `.bak` kept.
- **Web:** no keychain, so the key is Argon2id(passphrase). Lose the passphrase and the save is gone.
- **Schema changes:** postcard is not self-describing, so any change to `World` needs `SCHEMA_VERSION`
  bumped and the old structs frozen for a migration. Today a mismatch is a hard error. Do this
  *before* shipping to anyone, or the first schema change strands their saves.

## Accounts and login

The server is a backend-for-frontend: it runs the OAuth code flow itself (PKCE, single-use `state`) and
clients never see provider tokens. Identity is read from the provider's userinfo endpoint using the access
token received directly from the token endpoint over TLS (OIDC Core 3.1.3.7 permits this instead of
verifying ID-token signatures), which keeps OIDC (Google) and plain OAuth2 (GitHub, Discord) on one path.

- **Keyed on the provider's stable subject id, never email.** Matching email across providers is an
  account-takeover vector, so accounts are never merged automatically. Linking another login is an explicit,
  authenticated action whose intent is stored server-side against the session. An identity that already
  belongs to another account is refused, not stolen.
- **Our own tokens.** Opaque 15 min access token + 60 day refresh token, stored hashed. Refresh rotates every
  use and keeps the previous hash: if an already-rotated token reappears, the whole session is revoked.
  Cost: a client that retries a refresh after a lost response gets logged out. Accepted.
- **Login codes** are single-use, last 60 s, and bound to a secret only the initiating client holds, so a
  code leaked via browser history, a redirect or another local app can't be redeemed.
- **`return_to` is an exact-origin allowlist** (`UPLINK_RUST_WEB_ORIGINS`) plus `http` loopback for desktop. An open
  redirect here would hand login codes to an attacker.
- **Hacker handle** is separate from any provider name, unique case-insensitively and set once.
- **Deleting an account** cascades through identities, sessions, backups, seeds and runs.
- `POST /v1/accounts` (bearer-token-is-the-account) no longer exists.

Not built: email magic link (needs SMTP), Steam (OpenID 2.0, separate flow), Microsoft, rate limiting,
account recovery beyond "log in with another linked provider", handle changes, age gating.
Not tested against the real providers: tests use a fake provider that enforces PKCE and the client secret.
A manual run with real Google/GitHub/Discord credentials is still needed.

## Anti-cheat: the server replays, the client never reports state

Credits only come from events the sim emits, and the sim is deterministic, so:

1. The client records a `Run`: every successful `Command` with its tick.
2. On sync it uploads a `Segment` (`start_tick`, `end_tick`, commands). It never uploads state.
3. The server holds its own verified `World` for the run, replays the segment onto it with the same
   `ur-sim` crate, and stores the result (sealed with `UPLINK_RUST_SERVER_KEY`).
4. Illegal commands, wrong ordering, wrong start tick or a wrong rules version are rejected and nothing is stored.

Hardening in place:
- **Server-issued seeds** (`/v1/seeds`). A run can only start from a seed the server handed out, so players
  cannot reroll worlds. Seeds can be fetched ahead of time for offline starts.
- **Wall-clock budget.** A run may not claim more game time (10 ticks/s) than has elapsed since its seed was
  issued, plus 60 s of slack. Defeats speed-hacking.
- **Chain integrity.** Each segment must start exactly at the server's verified tick; replays and gaps get 409.
  Sync of a run is serialised by a row lock, so concurrent duplicates apply once (tested).
- **Hostile input.** Replay runs on a blocking thread inside `catch_unwind` (the release profile must not use
  `panic = "abort"` for the server). Size, tick and entry limits apply. A fuzz test throws 60k random commands
  with forged ids at the sim and requires zero panics.

What this does **not** stop:
- **Bots.** A script issuing legal commands is indistinguishable from a skilled player.
- **Waiting.** A player can bank real time (fetch a seed, wait, play fast). That equals playing slowly.
- **Offline-only play.** It is unverified until it syncs. Only verified runs should count for leaderboards.

Rules version history: 1 initial, 2 no duplicate contracts, 3 adds the services machine. Save schema 2 adds a timestamp to log entries.

Cost to remember: any change that alters outcomes (balance, mechanics) must bump `SIM_VERSION`. Old logs then
no longer replay, so the client must sync before updating, or the server must keep old sim versions for a while.
Likewise `SCHEMA_VERSION` changes make stored server state unreadable until migrations exist.

## Sync

Two tiers:

- **Verified runs** (`/v1/seeds`, `/v1/runs/...`): described above. The server can read them.
- **Unverified backups** (`/v1/saves/{slot}`): opaque client-encrypted blobs with compare-and-swap revisions.
  Convenient for moving a local game between devices, but the server cannot validate them, so they must never
  feed leaderboards or any shared economy.

The client keeps `{base, dirty}` per backup slot and `ur_save::sync::reconcile` returns
Nothing/Push/Pull/Conflict. Conflicts are never auto-merged; the player chooses local, remote, or keep both.

## Web performance plan

- `--profile web` (opt-level `s`, LTO, strip) then `wasm-opt -Oz`.
- The sim is allocation-light and float-free; budget is a few microseconds per tick.
- Run the sim in the main thread first; move to a Worker only if profiling demands it.
- Argon2 (19 MiB) runs only on load/save-as, not per frame.

## Server config

`DATABASE_URL`, `UPLINK_RUST_SERVER_KEY` (64 hex chars, seals stored runs; **back it up**), `UPLINK_RUST_PUBLIC_URL` (all required);
`UPLINK_RUST_WEB_ORIGINS`, `UPLINK_RUST_ALLOWED_ORIGIN`, `BIND`, and `UPLINK_RUST_OAUTH_{GOOGLE,GITHUB,DISCORD}_CLIENT_{ID,SECRET}`. See the README.
Tests: `TEST_DATABASE_URL=postgres://... cargo test -p ur-server` (**silently skipped when unset**).

## Known gaps

- Audio: one user-facing control only (`UPLINK_RUST_AUDIO=off`); no in-game volume or music toggle yet. Effects with no uplink_rust counterpart (phone, voice analyser, revelation, nuclear war, LAN sirens) are unused until those features exist.

- The client has no login or sync screen and no HTTP client yet; no account recovery. Browser saves live only in that browser until sync exists.
- Not yet checked: that the sim produces identical digests in wasm and native (it compiles for both; nobody has compared output).
- The client clock uses real elapsed time per frame, capped at 0.5 s: on a very slow device the game runs slower than real time (safe for the server's wall-clock budget) rather than jumping ahead.
- OAuth is built but unverified against real providers; no email login, no rate limiting.
- Server state is unreadable after a `SCHEMA_VERSION` change; no migrations yet.
- No rate limiting or TLS termination on the server (put it behind a reverse proxy).
- Account tokens are bearer secrets with no expiry/rotation.
