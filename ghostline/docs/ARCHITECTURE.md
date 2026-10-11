# Ghostline architecture

Hacking-career sim. Rust workspace targeting Windows, macOS, Linux and the browser (wasm32).

> **Provenance warning.** Written by someone who had read the Introversion *Uplink* developer
> source, so it is not a true clean room. Mechanics are generic (layers, bounce chains, traces);
> no code, data or assets are carried over. Keep private until the licensing path is settled.
> Do not use the Uplink name, art, audio, fonts or `.dat` files.

## Crates

| crate | role | targets |
|---|---|---|
| `gl-sim` | Headless deterministic simulation. No I/O, floats, wall-clock or `HashMap` iteration. | native + wasm |
| `gl-save` | Sealed save container, desktop store, sync protocol/decision logic. | native + wasm (`store`, `keychain` native only) |
| `gl-server` | Axum + Postgres sync API. Stores opaque blobs. | server |
| *(next)* `gl-client` | egui/eframe UI over `gl-sim`. One codebase, native + wasm. | native + wasm |

## Simulation rules

- Fixed 100 ms tick. The UI drives `World::tick` from an accumulator; rendering never mutates state.
- All input is a `Command`; all output is an `Event` pushed into a caller-owned `Vec` (no per-tick allocation in the hot path).
- Integer "work units" only. Same seed + same commands = byte-identical state on every platform (tested).
- Entities live in `SlotMap` arenas addressed by `Copy` ids: no `Rc`, no globals, no raw pointers.
- Mechanics: layered server security, relay bounce chains, trace walking back hop by hop, log erasure to
  break a trace, CPU shared across concurrent tasks, three strikes ends the run. All numbers in `tuning.rs`.

## Saves

```
magic "GLSV" | format u16 | schema u32 | key_mode u8 | salt[16] | nonce[24] | XChaCha20-Poly1305(lz4(postcard(World)))
```

The header is the AEAD associated data, so tampering with any byte fails decryption.

- **Desktop:** key lives in the OS keychain (`os-keyring` feature), never on disk. Files are written
  temp -> fsync -> rename, with one `.bak` kept.
- **Web:** no keychain, so the key is Argon2id(passphrase). Lose the passphrase and the save is gone.
- **Schema changes:** postcard is not self-describing, so any change to `World` needs `SCHEMA_VERSION`
  bumped and the old structs frozen for a migration. Today a mismatch is a hard error. Do this
  *before* shipping to anyone, or the first schema change strands their saves.

## Sync

Encryption is client-side, so Postgres holds `bytea` it cannot read. Per slot the server keeps a
`revision`; `PUT` is compare-and-swap on `x-if-revision` (409 + current revision on mismatch).
The client keeps `{base, dirty}` and `gl_save::sync::reconcile` returns Nothing/Push/Pull/Conflict.
Conflicts are never auto-merged; the player chooses local, remote, or keep both.

Trade-off accepted: the server cannot validate saves, so it cannot do anti-cheat or leaderboards from
saves. If you want those, submit run results as a separate signed/replayable command log
(the sim is deterministic, so the server can replay it).

## Web performance plan

- `--profile web` (opt-level `s`, LTO, strip) then `wasm-opt -Oz`.
- The sim is allocation-light and float-free; budget is a few microseconds per tick.
- Run the sim in the main thread first; move to a Worker only if profiling demands it.
- Argon2 (19 MiB) runs only on load/save-as, not per frame.

## Server config

`DATABASE_URL` (required), `BIND`, `GL_REGISTRATION_KEY` (strongly recommended: otherwise anyone can
create accounts), `GL_ALLOWED_ORIGIN` (the web client's origin for CORS).
Tests: `TEST_DATABASE_URL=postgres://... cargo test -p gl-server` (**silently skipped when unset**).

## Known gaps

- No client/UI yet; no HTTP sync client; no account recovery (token is the account).
- No rate limiting or TLS termination on the server (put it behind a reverse proxy).
- Account tokens are bearer secrets with no expiry/rotation.
