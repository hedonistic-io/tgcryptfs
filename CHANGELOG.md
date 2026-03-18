# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- **VolumeSession manager** for open/close lifecycle with DB + key management
- **Service layer** with SharingService, CacheService, DeadmanService wrappers
- All **21 API endpoints** wired to real backend (volumes, auth, sharing, deadman, FUSE mount)
- **HTTP integration tests** (24 tests) using TestApp helper
- **CLI integration tests** (22 tests) covering all subcommands
- **FUSE E2E tests** (13 tests) for full filesystem operations
- **Security test suite** (36 tests) for auth, input validation, and error handling
- **Bearer token authentication** middleware for all API endpoints
- **Shell completions** generation for bash/zsh/fish (`tgcryptfs completions`)
- **Password verification hash** stored in volume config for wrong-password detection
- **454+ tests** across 9 crates

### Changed
- CORS policy restricted to localhost-only origins (was permissive)
- FUSE `AllowOther` is now opt-in via `--allow-other` CLI flag (was default)
- Volume deletion now securely shreds files before removal (3-pass random overwrite)
- Deadman custom commands require explicit `--allow-custom-commands` flag
- `IntegrityError` display no longer leaks hash values

### Security
- API bearer token authentication with BLAKE3-hashed constant-time verification
- Password input echo suppressed via rpassword
- KDF intermediate buffers zeroized after key derivation
- ML-KEM decapsulation key implements `ZeroizeOnDrop`
- Shared secret bytes zeroized after ML-KEM encapsulate/decapsulate
- Session path removed from auth API responses
- Custom command execution gated behind explicit opt-in

### Fixed
- Dangerous `PathBuf::from(".")` fallback in `default_volumes_dir()` replaced with proper error
- Placeholder GitHub URLs updated to actual repository

## [0.1.0] - 2026-02-18

### Added
- **Core cryptographic engine** with XChaCha20-Poly1305 AEAD, Argon2id KDF, HKDF-SHA256 key hierarchy, and BLAKE3 content hashing
- **Post-quantum key exchange** using ML-KEM-768 (FIPS 203) for multi-user sharing
- **Opaque SQLite schema** where all table and column names are derived via BLAKE3, preventing structural analysis of stolen databases
- **Content-defined chunking** (CDC) with Rabin fingerprinting for efficient deduplication
- **SRB1 block format** with versioned headers, compression flags, and AAD-bound encryption
- **FUSE filesystem** with full POSIX operations: lookup, getattr, setattr, read, write, mkdir, create, unlink, rmdir, readdir, rename, symlink, readlink, statfs
- **Telegram block transport** using grammers-client MTProto with concurrent upload/download, retry logic, and rate limiting
- **Encrypted block cache** with LRU eviction, subdirectory sharding, and optional at-rest encryption
- **Multi-user sharing system** with ML-KEM key wrapping, access level hierarchy (ReadOnly/ReadWrite/Admin), time-limited invite tokens, and revocation
- **Deadman switch subsystem** with configurable triggers (heartbeat, network check, RPC, OS events, custom commands) and multi-phase destruction sequence
- **Volume management** with create, open, list, delete, and config persistence
- **22-word sentence references** for mnemonic key backup and recovery
- **Epoch-based forward secrecy** with key rotation and secure zeroization
- **Interactive Telegram auth** with phone number, verification code, and 2FA password flow
- **Key rotation CLI** with epoch management (`tgcryptfs key rotate`)
- **Sharing persistence** with encrypted SQLite storage for shares and invites
- **Deadman daemon** with background trigger monitoring, grace period, and arm/disarm CLI
- **FUSE block I/O** wired to Telegram transport and encrypted cache with FileManifest tracking
- **REST API server** with 17 endpoints for volumes, auth, sharing, and deadman management (`tgcryptfs serve`)
- **Actionable error suggestions** on all 7 error types with user-facing recovery hints
- **API error responses** as structured JSON with error, code, and suggestion fields
- **Property-based tests** (proptest) for crypto roundtrips, CDC, compression, and opaque schema
- **HTTP integration tests** using tower::ServiceExt for API endpoint verification
- **CI security audit** via rustsec/audit-check
- **Code coverage** via cargo-tarpaulin with Codecov upload
- **Release workflow** for automated multi-platform binary builds on tag push
- **Interactive setup wizard** (`tgcryptfs configure`) for guided Telegram API credential setup
- **Dual credential model**: runtime env vars (`TG_API_ID`/`TG_API_HASH`), CLI args, or compile-time baked credentials
- **337 tests** across 9 crates covering crypto, storage, API, sharing, FUSE, and integration
- **CI/CD pipeline** with GitHub Actions (check, test, fmt, clippy, audit, coverage, release builds)
- **Cross-platform builds** for Linux x86_64/aarch64 and macOS x86_64/aarch64

### Security
- AAD binding prevents cross-context block substitution and replay attacks
- All encryption keys are zeroized on drop via `zeroize` crate
- Argon2id with 64MB memory cost, 3 iterations, 4 parallel threads
- .env files written with 0600 permissions
- No key material stored to disk (only volume config with salt and KDF params)

[Unreleased]: https://github.com/hedonistic-io/tgcryptfs/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/hedonistic-io/tgcryptfs/releases/tag/v0.1.0
