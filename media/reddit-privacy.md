# TGCryptFS: Encrypted filesystem that uses your own Telegram account as cloud storage -- no third-party services

**Website:** https://tgcryptfs.hedonistic.io
**Source:** https://github.com/hedonistic-io/tgcryptfs (MIT, open source)

## What is it

TGCryptFS lets you mount an encrypted filesystem on your computer that stores its data as encrypted blocks in your Telegram account. To you, it looks like a normal folder. Behind the scenes, every file is split into chunks, encrypted, and pushed to a Telegram chat as opaque blobs.

## Why Telegram as a backend

You might think "why Telegram?" Fair question. Here's the reasoning:

1. **You control the account.** There's no third-party cloud provider. Your encrypted blocks sit in your own Telegram storage. If you delete them, they're gone.
2. **No storage fees.** Telegram offers generous cloud storage. You don't need to pay AWS or Google for the privilege of storing data you've already encrypted.
3. **The blocks are opaque.** Telegram sees encrypted blobs. They can't read them. They don't know if it's a photo, a document, or random noise. The local metadata database also uses obfuscated table names (derived from hashes), so even the schema reveals nothing.
4. **You already have an account.** Most people already have Telegram. There's nothing new to sign up for.

## Privacy features

**Post-quantum encryption.** Files are encrypted with XChaCha20-Poly1305 and the key sharing system uses ML-KEM-768, which is the NIST-standardized post-quantum key encapsulation mechanism. This means that even if large-scale quantum computers arrive, previously captured ciphertext can't be retroactively decrypted.

**Opaque metadata.** The local SQLite database doesn't use human-readable table names. Every table name, column name, and identifier is derived through BLAKE3 hashing. Someone who gets your database file sees gibberish schema and encrypted content.

**Dead man's switch.** You set a check-in interval (say, every 7 days). If you don't check in, the system initiates a destruction sequence on your volume. This is for situations where you need assurance that data won't persist if you're unable to manage it.

**22-word recovery phrases.** Your key material can be backed up as a 22-word sentence -- similar to cryptocurrency seed phrases. You can write it on paper and store it in a safe. No USB keys, no password managers, no digital trail.

**Sharing without exposing keys.** If you want to give someone access to your volume, the key exchange happens through ML-KEM. They get their own encapsulated key. You can revoke access without re-encrypting everything.

## What it isn't

This is v0.1.0. It works, it's tested (471 tests), and the code is open source under MIT license for anyone to audit. But it's new software. I wouldn't recommend it as your only copy of anything important yet. It's also a command-line tool for now -- no graphical interface.

It's written in Rust. You can install it with `cargo install tgcryptfs-cli` if you have the Rust toolchain, or grab a binary from the releases page.

(c) 2026 Hedonistic, LLC
