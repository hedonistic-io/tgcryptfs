# TGCryptFS -- Product Hunt Submission

## Tagline

Encrypted filesystem with Telegram as cloud storage

## Description

TGCryptFS turns your Telegram account into encrypted cloud storage. Install it, mount a volume, and use it like any folder on your computer. Behind the scenes, your files are encrypted with post-quantum cryptography and stored as opaque blocks in your Telegram account. No third-party services, no storage fees, no accounts to create.

The encryption uses XChaCha20-Poly1305 for data and ML-KEM-768 (the NIST post-quantum standard) for key sharing between users. This means your data is protected against both current and future threats, including large-scale quantum computers. Your local metadata database is also obfuscated -- table names are cryptographic hashes, so even the structure of your data is hidden.

TGCryptFS is open source (MIT license), written in Rust, and has 471 tests across its 9-crate workspace. It's v0.1.0 -- functional and tested, with a CLI interface. A graphical interface is planned for a future release.

## Key Features

- **FUSE mount** -- Appears as a normal directory on your system. No special tools needed to work with your files.
- **Post-quantum encryption** -- XChaCha20-Poly1305 + ML-KEM-768. Protected against both classical and quantum attacks.
- **Telegram as storage** -- Your encrypted data lives in your own Telegram account. Free, no sign-ups, you control it.
- **Dead man's switch** -- Configure automatic data destruction if you don't check in within a set interval.
- **22-word key backup** -- Back up your encryption keys as a sentence you can write on paper. Like a crypto seed phrase.
- **Multi-user sharing** -- Share encrypted volumes with others via post-quantum key exchange. Revoke access without re-encrypting.
- **Opaque metadata** -- Even your local database schema is obfuscated with BLAKE3-derived names.
- **REST API** -- 21 endpoints for programmatic access and integration.
- **Open source** -- MIT licensed, written in Rust. Full source available for audit.

## Links

- Website: https://tgcryptfs.hedonistic.io
- GitHub: https://github.com/hedonistic-io/tgcryptfs
- Install: `cargo install tgcryptfs-cli`

## Makers

Hedonistic, LLC

(c) 2026 Hedonistic, LLC
