# Introducing TGCryptFS: A Post-Quantum Encrypted Filesystem on Telegram

Your encrypted data has to live somewhere. Cloud providers will hold it for you, but you're trusting their infrastructure, their access controls, and their business continuity. Self-hosted storage gives you control but requires hardware and maintenance. Both options cost money.

TGCryptFS takes a different approach: it uses your Telegram account as the storage backend for an encrypted filesystem. You mount a volume via FUSE, use it like a normal directory, and your files are transparently encrypted and stored as opaque blocks in a Telegram chat. Telegram sees noise. You see your files.

## Why Telegram

The idea sounds unusual, so let me address it directly.

Telegram provides cloud storage that's free, available on every platform, and tied to an account you already control. When TGCryptFS stores a block in Telegram, it's an encrypted blob -- indistinguishable from random data. Telegram can't read it, can't classify it, and can't do anything useful with it. You're not trusting Telegram with your data; you're using Telegram as a dumb pipe with persistence.

If you delete your blocks, they're gone. If you delete your Telegram account, they're gone. There's no third-party service between you and your storage.

## The Cryptographic Layer

The symmetric encryption is XChaCha20-Poly1305. We chose it over AES-256-GCM for the 192-bit nonce: random nonce generation is safe for a functionally unlimited number of messages under a single key, which eliminates an entire class of nonce-management bugs. For a filesystem that might encrypt millions of blocks over its lifetime, this margin is worth having.

Every encrypted object is bound to its context through authenticated associated data. Inodes are tagged with `"inode:{ino}"`, policies with `"policy:{pid}"`, shares with `"share:{user_id}"`, and so on. This prevents block substitution attacks -- an encrypted inode blob can't be moved to a policy slot without authentication failure, even though both are stored in the same database.

Key derivation starts with Argon2id from a passphrase, then fans out through HKDF-SHA256 with domain separation into purpose-specific subkeys. Forward secrecy is achieved through epoch rotation: when an epoch advances, new subkeys are derived and old material is zeroed from memory.

For multi-user sharing, TGCryptFS uses ML-KEM-768, the NIST-standardized post-quantum key encapsulation mechanism. When you share a volume with another user, their public key encapsulates a shared secret that wraps the volume's access key. This protects the key exchange against both classical and quantum adversaries. Access can be revoked without re-encrypting the volume content.

## Opaque Metadata

The local SQLite database uses an unusual approach: every table name and column name is derived via `BLAKE3(K_schema || domain || name)`. Without the schema key, the database structure itself is opaque. You can't determine what kinds of data are stored, how they relate to each other, or even how many entity types exist. This is a defense-in-depth measure. If the encrypted content is solid, the schema shouldn't be the weak point that leaks structural information.

## Dead Man's Switch

TGCryptFS includes a dead man's switch. You configure a check-in interval -- say, every 72 hours. If you fail to check in, the system initiates a destruction sequence against the volume. The implementation is in a dedicated crate with its own test suite, separate from the rest of the filesystem logic.

The use case is straightforward: you need assurance that your data won't persist indefinitely if you're unable to manage it. This feature is opt-in and configurable.

## 22-Word Sentence References

Your key material can be backed up as a 22-word sentence. Four wordlists of 4096 entries each (12 bits per word), rotating by word position, encode 256 bits of key data. You can write this sentence on a piece of paper and store it physically. No USB drives, no password managers, no digital artifact.

This is conceptually similar to BIP-39 seed phrases from the cryptocurrency world, but purpose-built for this system. The rotating wordlists add a layer of resistance to targeted dictionary attacks against individual word positions.

## Implementation

TGCryptFS is written in Rust, organized as a 9-crate workspace:

- **tgcryptfs-core**: Crypto, blocks, metadata, key derivation, sentence encoding. Pure computation with zero I/O dependencies.
- **tgcryptfs-store**: Opaque SQLite schema and CRUD operations.
- **tgcryptfs-telegram**: Transport layer with a trait-based design and mock implementation.
- **tgcryptfs-cache**: Encrypted LRU cache with eviction and disk persistence.
- **tgcryptfs-fuse**: FUSE filesystem operations via `fuser`.
- **tgcryptfs-sharing**: ML-KEM key exchange and access control.
- **tgcryptfs-deadman**: Dead man's switch logic and destruction executor.
- **tgcryptfs-api**: REST API with 21 endpoints (axum).
- **tgcryptfs-cli**: Command-line interface (clap).

The crypto layer's zero-I/O design makes it straightforward to property-test every round-trip path. The FUSE layer uses `tokio::task::spawn_blocking()` for the blocking mount call, with async dispatch to the tokio runtime -- a lesson from the previous implementation where `block_on()` caused CPU spin under load.

All key material types implement `ZeroizeOnDrop`, ensuring keys are erased from memory when they go out of scope. This is enforced at the type system level.

There are 471 tests across the workspace. This is a ground-up rewrite of two earlier implementations, incorporating the lessons from roughly 42,000 lines of previous code.

## What's Next

TGCryptFS is at v0.1.0. The core functionality works: mounting, reading, writing, sharing, the dead man's switch, and the API are all implemented and tested. What's still ahead:

- A web-based UI for volume management, sharing, and configuration
- Stress testing the Telegram transport with large volumes
- A formal security audit
- Platform-specific packaging beyond `cargo install`

The code is MIT licensed. If the cryptographic design interests you, I'd welcome review and feedback.

**Source:** https://github.com/hedonistic-io/tgcryptfs
**Website:** https://tgcryptfs.hedonistic.io
**Install:** `cargo install tgcryptfs-cli`

(c) 2026 Hedonistic, LLC
