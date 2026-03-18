# TGCryptFS: Cryptographic design of a post-quantum encrypted filesystem (XChaCha20-Poly1305, ML-KEM-768, BLAKE3, HKDF)

**Source:** https://github.com/hedonistic-io/tgcryptfs (Rust, MIT)

I'd like to get feedback on the cryptographic design of TGCryptFS, an encrypted filesystem that uses Telegram as a cloud storage backend. I'll focus on the crypto decisions and their rationale rather than the product itself.

## Symmetric encryption: XChaCha20-Poly1305

We chose XChaCha20-Poly1305 over AES-256-GCM for a few reasons:

- **192-bit nonce.** With a 24-byte nonce, random nonce generation is safe for 2^96 messages under a single key with negligible collision probability. AES-GCM's 96-bit nonce makes random generation risky past 2^32 messages. In a filesystem that might process millions of blocks over its lifetime, this margin matters.
- **No hardware dependency.** ChaCha20 performs consistently across platforms without requiring AES-NI. The filesystem targets a range of hardware including ARM and older x86.
- **Misuse resistance.** The extended nonce space makes nonce-reuse accidents essentially impossible in practice.

## AAD binding

Every encrypted object type has a structured AAD format:

- Inodes: `"inode:{ino}"`
- Policies: `"policy:{pid}"`
- Users: `"user:{uid}"`
- Dead man's switch configs: `"deadman:{vid}"`
- Shares: `"share:{user_id}"`
- Invites: `"invite:{invite_id}"`

This binds each ciphertext to its identity. An attacker who can manipulate the storage layer cannot swap an encrypted inode blob into a policy slot (or between different inodes) without authentication failure. This is particularly relevant since blocks are stored remotely in Telegram where the storage layer is untrusted.

## Key hierarchy and derivation

Entry point is Argon2id (m=65536, t=3, p=4) from a user passphrase, producing a 256-bit master key.

From the master key, HKDF-SHA256 derives purpose-specific subkeys with domain separation:

- Data encryption key
- Metadata encryption key
- Schema obfuscation key
- Sharing key material

The hierarchy implements `ZeroizeOnDrop` at the Rust type level, so subkeys are zeroed from memory when they go out of scope.

**Forward secrecy** is handled through epoch rotation. When an epoch advances, new subkeys are derived and the old epoch material is zeroed. This limits the window of exposure if a key is compromised.

## Schema obfuscation via BLAKE3

The SQLite metadata database doesn't use human-readable identifiers. Every table name and column name is computed as `BLAKE3(K_schema || domain || name)`. Without `K_schema`, the database schema itself is opaque. This is a defense-in-depth measure: even if the encrypted content is unbreakable, the schema shouldn't leak structural information about what's stored.

## Post-quantum sharing: ML-KEM-768

Multi-user access uses ML-KEM-768 (FIPS 203) for key encapsulation. When user A shares a volume with user B:

1. B's ML-KEM public key is used to encapsulate a shared secret
2. The shared secret wraps the volume's access key
3. Revocation replaces the encapsulated key without re-encrypting volume content

We chose ML-KEM-768 as the middle security tier -- ML-KEM-512 felt too conservative given NIST's own hedging about security margins, and ML-KEM-1024 adds size overhead that's unnecessary for this threat model.

## Sentence-encoded references

Block references are encoded as 22-word sentences. Four wordlists of 4096 entries each (12 bits per word), rotating by position. 22 words * 12 bits = 264 bits, encoding 256 bits of reference data with 8 bits of padding. This is conceptually similar to BIP-39 but purpose-built -- the rotating wordlists prevent dictionary attacks against individual word positions.

## Content-defined chunking

Files are split using content-defined chunking (CDC) rather than fixed blocks. This means that an insertion in the middle of a file only affects the chunks around the edit boundary, not every subsequent chunk. Each chunk gets its own nonce and AAD-bound encryption.

## Known limitations / open questions

- Argon2id parameters are fixed at compile time. Making them tunable per-volume is on the roadmap.
- The forward secrecy epoch rotation is time-based. An operation-count trigger might be more appropriate for some workloads.
- We haven't done a formal audit. The code is MIT-licensed and open for review.

471 tests, v0.1.0. Feedback on any of these design choices is welcome.

https://tgcryptfs.hedonistic.io

(c) 2026 Hedonistic, LLC
