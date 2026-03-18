# TGCryptFS: 9-crate Rust workspace for a post-quantum encrypted FUSE filesystem (471 tests, MIT)

**Repo:** https://github.com/hedonistic-io/tgcryptfs

I've been working on TGCryptFS, an encrypted filesystem that uses Telegram as cloud storage. I wanted to share some notes on the Rust implementation since this sub tends to appreciate the architectural details.

## Workspace layout

Nine crates with clear boundaries:

- `tgcryptfs-core` -- crypto primitives, block chunking, metadata, key derivation, sentence encoding, volume management. All pure computation, zero I/O.
- `tgcryptfs-store` -- opaque SQLite schema via rusqlite. Table/column names are BLAKE3 hashes.
- `tgcryptfs-telegram` -- transport trait + grammers-based implementation + unconditional `MockTransport` export for cross-crate testing.
- `tgcryptfs-cache` -- encrypted LRU with disk persistence and eviction.
- `tgcryptfs-fuse` -- FUSE ops via `fuser` 0.14, handle table, mount management.
- `tgcryptfs-sharing` -- ML-KEM-768 key exchange, invites, access control.
- `tgcryptfs-deadman` -- check-in timers, destruction executor.
- `tgcryptfs-api` -- axum 0.7, 21 REST endpoints, tower middleware.
- `tgcryptfs-cli` -- clap 4 with derive, shell completions.

The goal was that each crate compiles and tests independently. `tgcryptfs-core` in particular has no I/O dependencies -- everything is `&[u8]` in, `Vec<u8>` out, which makes property testing straightforward.

## FUSE async dispatch

The v1 implementation had a nasty bug: calling `block_on()` from FUSE callback threads caused CPU spin under load. The fix in v2 is `tokio::task::spawn_blocking()` for the blocking `fuser::mount2()` call, with proper channel-based dispatch between the FUSE thread pool and the tokio runtime. This avoids the nested runtime panic and the CPU issue.

## Key material handling

`KeyHierarchy` derives `ZeroizeOnDrop`, so the compiler enforces that you can't accidentally move keys out of the hierarchy without an explicit `.clone()`. This was annoying to get right but catches a real class of bugs -- any function that needs a subkey has to consciously copy it, and the original is zeroed when the hierarchy drops.

The key derivation uses HKDF-SHA256 with domain-separated contexts for each key purpose. Argon2id for passphrase stretching at the entry point.

## Property testing

The core crate uses proptest extensively for the crypto round-trip paths. Sentence encoding (22 words <-> 256 bits) is a good example: generate arbitrary 32-byte arrays, encode to words, decode back, assert equality. The content-defined chunking (CDC) also gets property tests for determinism and boundary stability.

## What's there and what isn't

It's v0.1.0. The crypto, storage, FUSE mount, sharing, dead man's switch, API, and CLI all work and are tested (471 tests across the workspace). The Telegram transport works but hasn't been stress-tested with large volumes. No GUI yet.

```
cargo install tgcryptfs-cli
```

MSRV is 1.75. MIT licensed.

https://tgcryptfs.hedonistic.io

(c) 2026 Hedonistic, LLC
