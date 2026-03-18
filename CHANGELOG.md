# Changelog

All notable changes to TGCryptFS are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] - 2026-02-18

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
