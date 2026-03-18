# Contributing to TGCryptFS

Thank you for your interest in contributing to TGCryptFS! This document provides guidelines and information for contributors.

## Getting Started

### Prerequisites

- Rust 1.75+ (install via [rustup](https://rustup.rs))
- FUSE development libraries:
  - **macOS**: `brew install macfuse`
  - **Debian/Ubuntu**: `sudo apt install libfuse-dev`
  - **Fedora**: `sudo dnf install fuse-devel`

### Setup

```bash
git clone https://github.com/hedonistic-io/tgcryptfs.git
cd tgcryptfs
cargo build
cargo test
```

### Project Structure

```
crates/
  tgcryptfs-core/       Pure crypto, blocks, metadata (no I/O)
  tgcryptfs-store/      SQLite storage with opaque schema
  tgcryptfs-telegram/   Telegram MTProto client
  tgcryptfs-cache/      Encrypted LRU block cache
  tgcryptfs-fuse/       FUSE filesystem implementation
  tgcryptfs-sharing/    ML-KEM key exchange and invites
  tgcryptfs-deadman/    Deadman switch triggers and destruction
  tgcryptfs-api/        Service orchestration layer
  tgcryptfs-cli/        Command-line interface
```

## How to Contribute

### Reporting Bugs

1. Check existing issues to avoid duplicates
2. Use the bug report template
3. Include:
   - Steps to reproduce
   - Expected vs actual behavior
   - OS, Rust version, and tgcryptfs version
   - Relevant logs (with `--verbose` flag)

### Suggesting Features

1. Open a feature request issue
2. Describe the use case, not just the solution
3. Consider security implications

### Submitting Code

1. **Fork** the repository
2. **Branch** from `main`: `git checkout -b feature/your-feature`
3. **Write tests** for new functionality
4. **Ensure all checks pass**:
   ```bash
   make ci    # runs fmt, clippy, and test
   ```
5. **Commit** with conventional commits:
   ```
   feat(core): add epoch key rotation
   fix(store): handle concurrent schema migration
   test(sharing): add ML-KEM multi-recipient test
   ```
6. **Push** and open a Pull Request

### Pull Request Guidelines

- Keep PRs focused on a single concern
- Include tests for new features and bug fixes
- Update documentation if behavior changes
- All CI checks must pass
- Maintainers may request changes before merging

## Code Standards

### Style

- Run `cargo fmt` before committing
- Run `cargo clippy` and resolve all warnings
- Follow existing patterns in the codebase

### Security Requirements

This is a cryptographic application. All contributions must:

- **Never log or print key material** (encryption keys, passwords, salts)
- **Use constant-time comparisons** for secret data
- **Zeroize sensitive data** on drop (use the `zeroize` crate)
- **Bind AAD** to context for all AEAD operations
- **Validate input at boundaries** (CLI args, API inputs, external data)
- **No `unsafe` code** without justification and review
- **No new dependencies** that access the network without discussion

### Testing

- Unit tests go in the same file as the code (`#[cfg(test)] mod tests`)
- Integration tests go in `crates/<crate>/tests/`
- Crypto tests must verify both success and failure cases
- Use `assert_eq!` over `assert!` where possible for better error messages

```bash
# Run all tests
cargo test

# Run a specific crate's tests
cargo test -p tgcryptfs-core

# Run a specific test
cargo test -p tgcryptfs-store inode_lifecycle
```

## Architecture Decisions

Major architectural changes should be discussed in an issue before implementation. Areas that require particular care:

- **Cryptographic algorithms**: Any change to encryption, hashing, or key derivation
- **Wire formats**: Changes to SRB1 block format or opaque schema
- **Key hierarchy**: Modifications to key derivation paths
- **FUSE operations**: New filesystem operations or changes to existing ones
- **Dependencies**: Adding new crates, especially those with native code

## Development Workflow

### Branching

- `main` - stable, release-ready code
- `feature/*` - new features
- `fix/*` - bug fixes
- `refactor/*` - code improvements

### Release Process

1. Update version in workspace `Cargo.toml`
2. Update `CHANGELOG.md`
3. Create a git tag: `git tag v0.x.0`
4. GitHub Actions builds release artifacts

## Community

- Be respectful and constructive
- Follow our [Code of Conduct](CODE_OF_CONDUCT.md)
- Ask questions in issues or discussions

## License

By contributing to TGCryptFS, you agree that your contributions will be licensed under the [MIT License](LICENSE).
