# Show HN: TGCryptFS -- Post-quantum encrypted filesystem using Telegram as storage

https://github.com/hedonistic-io/tgcryptfs

TGCryptFS is an encrypted filesystem that stores data as opaque blocks in Telegram's cloud. You mount it via FUSE, use it like a normal directory, and your files end up as encrypted blobs in a Telegram chat. The Telegram account is yours; there's no third-party service in the middle.

Some of the design decisions that might be interesting:

**Opaque SQLite schema.** The local metadata database doesn't use readable table or column names. Every identifier is derived via BLAKE3(K_schema || domain || name), so the schema itself reveals nothing about what's stored. If someone gets your SQLite file, they see hashed table names and encrypted blobs.

**Post-quantum key exchange for sharing.** Multi-user access uses ML-KEM-768 (the NIST post-quantum KEM standard) for key encapsulation. The symmetric layer is XChaCha20-Poly1305, not AES-GCM -- the 192-bit nonce eliminates nonce management concerns entirely.

**22-word sentence references.** Block references are encoded as 22-word sentences using 4 rotating wordlists at 12 bits per word. Similar idea to BIP-39 seed phrases, but for addressing encrypted blocks rather than deriving wallet keys.

**Dead man's switch.** You configure a check-in interval. If you don't check in, the system triggers a destruction sequence against the volume. The use case is the obvious one.

**AAD binding.** Every encrypted object (inodes, policies, users, shares) has its own AAD format that binds the ciphertext to its identity -- `"inode:{ino}"`, `"share:{user_id}"`, etc. This prevents block substitution attacks where an attacker swaps encrypted blobs between contexts.

**Key hierarchy with HKDF domain separation.** A single master key fans out through HKDF into per-purpose subkeys (data, metadata, schema, sharing), with epoch rotation for forward secrecy.

It's a 9-crate Rust workspace, v0.1.0, MIT licensed, 471 tests. The crypto layer is zero-I/O so it's straightforward to test in isolation.

This is a ground-up rewrite of two earlier attempts (~42k LOC of lessons learned). It works, it's tested, but it's early -- there's no GUI yet and the Telegram transport is functional but not battle-tested at scale.

Website: https://tgcryptfs.hedonistic.io

(c) 2026 Hedonistic, LLC
