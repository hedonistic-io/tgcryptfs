# TGCryptFS

**Post-quantum encrypted filesystem with Telegram cloud storage**

[![CI](https://github.com/hedonistic-io/tgcryptfs/actions/workflows/ci.yml/badge.svg)](https://github.com/hedonistic-io/tgcryptfs/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Rust MSRV: 1.75](https://img.shields.io/badge/MSRV-1.75-orange.svg)](https://www.rust-lang.org)
[![Tests: 471](https://img.shields.io/badge/tests-471%20passing-brightgreen.svg)]()

---

## What is TGCryptFS?

TGCryptFS is an encrypted filesystem that uses Telegram as its cloud storage backend. Files written to a TGCryptFS volume are encrypted locally with post-quantum cryptography (XChaCha20-Poly1305 authenticated encryption combined with ML-KEM-768 key encapsulation), split into content-defined chunks with deduplication, and uploaded as opaque blocks to your Telegram account. To you, it looks like a normal mounted directory. To everyone else -- including Telegram -- it is meaningless ciphertext.

The metadata layer uses an opaque SQLite schema where every table name, column name, and index name is derived through BLAKE3 keyed hashing, making the database itself resistant to schema analysis even if captured. Key material is protected by Argon2id password hashing, hierarchical key derivation, and optional forward secrecy through epoch-based key rotation.

TGCryptFS includes a dead man's switch that can automatically destroy volumes if you fail to check in, multi-user key sharing through ML-KEM post-quantum key exchange, and a REST API for programmatic integration. A 22-word sentence reference system provides human-readable key backup that can reconstruct access to any volume.

## Key Features

- **Post-quantum cryptography** -- XChaCha20-Poly1305 for authenticated encryption, ML-KEM-768 for key encapsulation, BLAKE3 for hashing, Argon2id for password derivation
- **Telegram cloud storage backend** -- unlimited, free, end-to-end encrypted block storage using your own Telegram account
- **FUSE mounting** -- mount encrypted volumes as normal directories; transparent read/write access from any application
- **REST API** -- 21 endpoints covering volumes, sharing, authentication, dead man's switch, and system status
- **Dead man's switch** -- configurable auto-destruction triggers with armed/disarmed states
- **Multi-user key sharing** -- share volumes with other users via ML-KEM key exchange or invite links
- **22-word sentence references** -- human-readable, memorizable key backup for volume recovery
- **Opaque SQLite schema** -- all database identifiers derived via BLAKE3 keyed hashing; resistant to schema analysis
- **Content-defined chunking** -- deduplication across and within volumes using rolling hash boundaries
- **Forward secrecy** -- key epoch rotation so compromise of current keys does not expose past data

## Quick Install

**Install script (Linux and macOS):**

```bash
curl -fsSL https://raw.githubusercontent.com/hedonistic-io/tgcryptfs/main/scripts/install.sh | bash
```

**From crates.io:**

```bash
cargo install tgcryptfs-cli
```

**Build from source:**

```bash
git clone https://github.com/hedonistic-io/tgcryptfs.git
cd tgcryptfs
cargo build --release
# Binary at target/release/tgcryptfs
```

See [Building from Source](#building-from-source) for details and platform-specific notes.

## Getting Started

**1. Set up Telegram API credentials**

TGCryptFS needs a Telegram API ID and hash to access your account. See [docs/TELEGRAM_SETUP.md](docs/TELEGRAM_SETUP.md) for a step-by-step walkthrough, or run the setup script:

```bash
./scripts/setup-telegram.sh
```

**2. Authenticate**

```bash
tgcryptfs auth login
```

**3. Create an encrypted volume**

```bash
tgcryptfs volume create --name mydata
```

**4. Mount it**

```bash
tgcryptfs volume mount mydata ~/secure
```

**5. Use it like a normal directory**

```bash
cp documents/*.pdf ~/secure/
ls ~/secure/
cat ~/secure/report.pdf
```

**6. Unmount when done**

```bash
tgcryptfs volume unmount ~/secure
```

**7. Start the REST API server (optional)**

```bash
tgcryptfs serve --bind 127.0.0.1:8080
```

For the complete walkthrough, see [docs/GETTING_STARTED.md](docs/GETTING_STARTED.md).

## Architecture

TGCryptFS is organized as a 9-crate Rust workspace:

```
                          +----------------+
                          | tgcryptfs-cli  |    CLI binary
                          +-------+--------+
                                  |
                    +-------------+-------------+
                    |                           |
             +------+------+           +-------+-------+
             | tgcryptfs-  |           | tgcryptfs-api |   REST API
             |    fuse     |           +-------+-------+
             +------+------+                   |
                    |                          |
        +-----------+-----------+--------------+----------+
        |           |           |              |          |
  +-----+-----+ +--+-------+ +-+----------+ +-+------+ +-+--------+
  | tgcryptfs- | | tgcryptfs| | tgcryptfs- | |tgcrypt| |tgcryptfs-|
  |   cache    | | -sharing | |  deadman   | |fs-tele| |  store   |
  +-----+------+ +--+-------+ +--+---------+ |gram   | +----+-----+
        |            |            |           +---+----+      |
        +------------+------------+---------------+-----------+
                                  |
                          +-------+--------+
                          | tgcryptfs-core |   Crypto, blocks,
                          +----------------+   metadata, policy
```

| Crate | Purpose |
|-------|---------|
| `tgcryptfs-core` | Cryptographic engine, block management, metadata, policy, sentence encoding, volume lifecycle |
| `tgcryptfs-store` | Opaque SQLite storage with BLAKE3-derived schema, migrations, CRUD operations |
| `tgcryptfs-telegram` | Telegram transport layer, `BlockTransport` trait, mock transport for testing |
| `tgcryptfs-cache` | Encrypted LRU block cache with eviction policies and disk persistence |
| `tgcryptfs-fuse` | FUSE filesystem implementation, file handle table, async dispatch |
| `tgcryptfs-sharing` | Multi-user key sharing, invite system, ML-KEM key exchange |
| `tgcryptfs-deadman` | Dead man's switch triggers, configurable destruction executor |
| `tgcryptfs-api` | Axum-based REST API with bearer token authentication |
| `tgcryptfs-cli` | CLI binary with 9 top-level subcommands |

## Platform Support

| Platform | Install | FUSE | API Server | Notes |
|----------|---------|------|------------|-------|
| Linux x86_64 | Script, cargo, source | Yes | Yes | Requires `libfuse3-dev` or `fuse3` |
| Linux aarch64 | Script, cargo, source | Yes | Yes | Requires `libfuse3-dev` or `fuse3` |
| macOS x86_64 | Script, cargo, source | Yes | Yes | Requires [macFUSE](https://osxfuse.github.io/) |
| macOS Apple Silicon | Script, cargo, source | Yes | Yes | Requires [macFUSE](https://osxfuse.github.io/) |
| Windows | Source only | Experimental | Yes | Requires [WinFsp](https://winfsp.dev/); FUSE support is experimental |

## Documentation

| Document | Description |
|----------|-------------|
| [Getting Started](docs/GETTING_STARTED.md) | Installation, first volume, basic usage |
| [Telegram Setup](docs/TELEGRAM_SETUP.md) | Obtaining and configuring Telegram API credentials |
| [CLI Reference](docs/CLI_REFERENCE.md) | Complete command-line usage for all subcommands |
| [API Reference](docs/API_REFERENCE.md) | REST API endpoints, request/response formats |
| [Architecture](docs/ARCHITECTURE.md) | Crate structure, data flow, design decisions |
| [Security Model](docs/SECURITY.md) | Threat model, cryptographic design, key management |
| [Contributing](CONTRIBUTING.md) | Development setup, testing, pull request guidelines |

## Building from Source

**Prerequisites:**

- Rust 1.75 or later
- FUSE development headers (Linux: `libfuse3-dev`; macOS: macFUSE; Windows: WinFsp)
- SQLite (bundled via `rusqlite`, no system dependency required)

```bash
git clone https://github.com/hedonistic-io/tgcryptfs.git
cd tgcryptfs
cargo build --release
```

The binary is written to `target/release/tgcryptfs`. To install it to your Cargo bin directory:

```bash
cargo install --path crates/tgcryptfs-cli
```

Run the full test suite:

```bash
cargo test --workspace    # 471 tests
```

See `scripts/build.sh` for platform-specific build automation.

## Running as a Service

Systemd and launchd service files are provided in the `system/` directory:

```bash
# Linux (systemd) -- API server as user service
mkdir -p ~/.config/systemd/user
cp system/tgcryptfs-api.service ~/.config/systemd/user/
systemctl --user enable --now tgcryptfs-api

# Linux (systemd) -- dead man's switch timer
cp system/tgcryptfs-deadman.service system/tgcryptfs-deadman.timer ~/.config/systemd/user/
systemctl --user enable --now tgcryptfs-deadman.timer

# macOS (launchd) -- API server as user agent
cp system/io.hedonistic.tgcryptfs-api.plist ~/Library/LaunchAgents/
launchctl load ~/Library/LaunchAgents/io.hedonistic.tgcryptfs-api.plist
```

## Security

TGCryptFS is designed for strong confidentiality:

- **Encryption**: XChaCha20-Poly1305 authenticated encryption with 192-bit nonces. Every block, metadata record, and key blob is individually encrypted with unique associated data.
- **Post-quantum key exchange**: ML-KEM-768 (NIST FIPS 203) for key encapsulation during sharing operations. Resistant to quantum computing attacks on key exchange.
- **Key derivation**: Argon2id for password-to-key derivation. HKDF-SHA256 for hierarchical key derivation with domain separation.
- **Schema obfuscation**: All SQLite table names, column names, and index names are BLAKE3 keyed hashes. The database schema reveals nothing about data structure.
- **Forward secrecy**: Key epoch rotation ensures that compromise of current keys does not retroactively expose data encrypted under previous epochs.
- **Memory safety**: All key material implements `ZeroizeOnDrop` for automatic secure erasure.
- **Dead man's switch**: Configurable triggers for automatic volume destruction.

For the full threat model and cryptographic design, see [docs/SECURITY.md](docs/SECURITY.md).

**Reporting vulnerabilities**: If you discover a security issue, please report it privately to security@hedonistic.io. Do not open a public issue.

## Contributing

Contributions are welcome. See [CONTRIBUTING.md](CONTRIBUTING.md) for development setup, coding standards, and pull request guidelines.

```bash
# Run tests
cargo test --workspace

# Run clippy
cargo clippy --workspace -- -D warnings

# Format check
cargo fmt --check
```

## License

TGCryptFS is licensed under the [MIT License](LICENSE).

Copyright 2026 Hedonistic IO LLC.
