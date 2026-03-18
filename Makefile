.PHONY: build test check clean release install fmt lint ci release-check

# Default target
all: check test build

# Build in debug mode
build:
	cargo build

# Build optimized release binary
release:
	cargo build --release

# Run all tests
test:
	cargo test

# Quick type check without building
check:
	cargo check

# Format code
fmt:
	cargo fmt

# Run clippy linter
lint:
	cargo clippy -- -W clippy::all

# Full CI check (format + lint + test)
ci: fmt lint test

# Clean build artifacts
clean:
	cargo clean

# Install the binary to ~/.cargo/bin
install:
	cargo install --path crates/tgcryptfs-cli

# Count lines of code
loc:
	@echo "Rust source:"
	@find crates -name "*.rs" | xargs wc -l | tail -1
	@echo "Tests:"
	@find crates -name "*.rs" -path "*/tests/*" -o -name "*.rs" | xargs grep -c "#\[test\]" | awk -F: '{s+=$$2}END{print s " test functions"}'

# Run a specific crate's tests
test-%:
	cargo test -p tgcryptfs-$*

# Full release gate check (format + clippy + test + workspace check)
release-check:
	cargo fmt --all -- --check
	cargo clippy --workspace -- -W clippy::all
	cargo test --workspace
	cargo check --workspace
	@echo "=== Release check: all gates passed ==="
