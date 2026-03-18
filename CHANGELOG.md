# Changelog

All notable changes to TGCryptFS are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.2] - 2026-03-18

### Security
- Constant-time password verification (fixes timing side-channel)
- Snapshot AAD unified to inode-scoped format (fixes cross-record substitution)
- Write buffer plaintext zeroized on file handle close
- Length-prefixed BLAKE3 domain encoding (fixes collision ambiguity)
- HKDF uses volume salt in extract step (cross-volume domain separation)
- FUSE mutations wrapped in SQLite transactions (crash consistency)
- ML-KEM intermediate key bytes zeroized after generation
- Volume config file restricted to 0600 permissions
- Epoch tracking wired into FUSE block records

### Changed
- Argon2id defaults bumped to 256 MB memory / 4 iterations
- Production English wordlists (4 x 4096 words) replace placeholders
- 473 tests passing

## [0.1.0] - 2026-03-18

### Added
- Post-quantum encryption engine (XChaCha20-Poly1305 + ML-KEM-768)
- Telegram cloud storage backend
- FUSE filesystem mounting
- REST API server (21 endpoints)
- Dead man's switch with configurable triggers
- Multi-user key sharing via ML-KEM key exchange
- 22-word sentence reference backup system
- Opaque SQLite metadata storage
- Content-defined chunking with deduplication
- CLI with 9 subcommands
- Shell completions (bash, zsh, fish)
- Bearer token API authentication
- Multi-platform CI/CD (Linux x86_64/aarch64, macOS x86_64/aarch64)

[0.1.2]: https://github.com/hedonistic-io/tgcryptfs/compare/v0.1.0...v0.1.2
[0.1.0]: https://github.com/hedonistic-io/tgcryptfs/releases/tag/v0.1.0
