#!/usr/bin/env bash
#
# tgcryptfs -- build from source
#
# Prerequisites: Rust >= 1.75, FUSE development libraries, pkg-config
#
# Usage:
#   ./scripts/build.sh                  # Build release binary
#   ./scripts/build.sh --skip-tests     # Build without running tests
#   ./scripts/build.sh --install        # Build and install to /usr/local/bin

set -euo pipefail

# -- Constants ----------------------------------------------------------------

BINARY_NAME="tgcryptfs"
MIN_RUST_VERSION="1.75.0"

# -- Color output -------------------------------------------------------------

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
BOLD='\033[1m'
RESET='\033[0m'

info()    { printf "${BLUE}[info]${RESET}    %s\n" "$*"; }
success() { printf "${GREEN}[ok]${RESET}      %s\n" "$*"; }
warn()    { printf "${YELLOW}[warn]${RESET}    %s\n" "$*"; }
error()   { printf "${RED}[error]${RESET}   %s\n" "$*" >&2; }
fatal()   { error "$@"; exit 1; }
header()  { printf "\n${BOLD}%s${RESET}\n" "$*"; }

# -- Argument parsing ---------------------------------------------------------

SKIP_TESTS=false
DO_INSTALL=false

while [ $# -gt 0 ]; do
    case "$1" in
        --skip-tests)
            SKIP_TESTS=true
            shift
            ;;
        --install)
            DO_INSTALL=true
            shift
            ;;
        --help|-h)
            printf "Usage: build.sh [OPTIONS]\n\n"
            printf "Options:\n"
            printf "  --skip-tests    Skip running the test suite\n"
            printf "  --install       Install the binary after building\n"
            printf "  --help, -h      Show this help message\n"
            exit 0
            ;;
        *)
            fatal "Unknown argument: $1"
            ;;
    esac
done

# -- Find project root -------------------------------------------------------

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

if [ ! -f "${PROJECT_ROOT}/Cargo.toml" ]; then
    fatal "Cannot find Cargo.toml. Run this script from the project root or scripts/ directory."
fi

info "Project root: ${PROJECT_ROOT}"

# -- Version comparison helper ------------------------------------------------

version_ge() {
    # Returns 0 (true) if $1 >= $2 using semantic versioning
    local IFS=.
    local i ver1=($1) ver2=($2)
    for ((i=0; i<${#ver2[@]}; i++)); do
        local v1="${ver1[i]:-0}"
        local v2="${ver2[i]:-0}"
        if ((v1 > v2)); then return 0; fi
        if ((v1 < v2)); then return 1; fi
    done
    return 0
}

# -- Check Rust toolchain -----------------------------------------------------

header "Checking prerequisites"

if ! command -v rustc >/dev/null 2>&1; then
    error "Rust is not installed."
    info "Install Rust via rustup:"
    info "  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
    info "  source ~/.cargo/env"
    fatal "Rust >= ${MIN_RUST_VERSION} is required."
fi

RUST_VERSION="$(rustc --version | sed 's/rustc \([0-9.]*\).*/\1/')"
if version_ge "$RUST_VERSION" "$MIN_RUST_VERSION"; then
    success "Rust ${RUST_VERSION} (>= ${MIN_RUST_VERSION})"
else
    error "Rust ${RUST_VERSION} is too old. Minimum required: ${MIN_RUST_VERSION}"
    info "Update Rust:"
    info "  rustup update stable"
    fatal "Please update your Rust toolchain."
fi

if ! command -v cargo >/dev/null 2>&1; then
    fatal "cargo is not found. Ensure Rust is properly installed."
fi
success "cargo is available."

# -- Check pkg-config ---------------------------------------------------------

if ! command -v pkg-config >/dev/null 2>&1; then
    error "pkg-config is not installed."
    OS="$(uname -s)"
    if [ "$OS" = "Darwin" ]; then
        info "Install via Homebrew: brew install pkg-config"
    elif [ "$OS" = "Linux" ]; then
        info "Install via your package manager:"
        info "  Debian/Ubuntu: sudo apt-get install pkg-config"
        info "  Fedora/RHEL:   sudo dnf install pkgconf-pkg-config"
        info "  Arch:          sudo pacman -S pkgconf"
    fi
    fatal "pkg-config is required for building FUSE bindings."
fi
success "pkg-config is available."

# -- Check FUSE development libraries -----------------------------------------

OS="$(uname -s)"
FUSE_OK=false

if [ "$OS" = "Darwin" ]; then
    # macOS: check for macFUSE
    if [ -d "/Library/Filesystems/macfuse.fs" ] || \
       pkg-config --exists fuse 2>/dev/null || \
       [ -f "/usr/local/lib/libfuse.dylib" ] || \
       [ -f "/opt/homebrew/lib/libfuse.dylib" ]; then
        FUSE_OK=true
    fi
    if [ "$FUSE_OK" = false ]; then
        error "macFUSE development libraries not found."
        info "Install macFUSE:"
        info "  brew install --cask macfuse"
        info "  # or download from https://osxfuse.github.io/"
        fatal "macFUSE is required for building tgcryptfs."
    fi
elif [ "$OS" = "Linux" ]; then
    if pkg-config --exists fuse3 2>/dev/null; then
        FUSE_OK=true
    elif pkg-config --exists fuse 2>/dev/null; then
        FUSE_OK=true
    fi
    if [ "$FUSE_OK" = false ]; then
        error "FUSE development libraries not found."
        info "Install FUSE development libraries:"
        info "  Debian/Ubuntu: sudo apt-get install libfuse3-dev fuse3"
        info "  Fedora/RHEL:   sudo dnf install fuse3-devel fuse3"
        info "  Arch:          sudo pacman -S fuse3"
        info "  Alpine:        sudo apk add fuse3-dev"
        fatal "FUSE development libraries are required."
    fi
fi
success "FUSE development libraries found."

# -- Build ---------------------------------------------------------------------

header "Building tgcryptfs (release mode)"

cd "$PROJECT_ROOT"

info "Running cargo build --release..."
cargo build --release 2>&1

RELEASE_BIN="${PROJECT_ROOT}/target/release/${BINARY_NAME}"
if [ ! -f "$RELEASE_BIN" ]; then
    fatal "Build succeeded but binary not found at expected path: ${RELEASE_BIN}"
fi

BIN_SIZE="$(du -h "$RELEASE_BIN" | cut -f1)"
success "Build complete: ${RELEASE_BIN} (${BIN_SIZE})"

# -- Test ----------------------------------------------------------------------

if [ "$SKIP_TESTS" = false ]; then
    header "Running test suite"
    info "Running cargo test --workspace..."
    cargo test --workspace 2>&1
    success "All tests passed."
else
    warn "Tests skipped (--skip-tests)."
fi

# -- Install -------------------------------------------------------------------

if [ "$DO_INSTALL" = true ]; then
    header "Installing binary"

    # Prefer /usr/local/bin if writable, else ~/.cargo/bin
    INSTALL_DIR=""

    if [ -w /usr/local/bin ]; then
        INSTALL_DIR="/usr/local/bin"
    elif command -v sudo >/dev/null 2>&1; then
        INSTALL_DIR="/usr/local/bin"
        info "Installing to /usr/local/bin (requires sudo)..."
        sudo install -m 755 "$RELEASE_BIN" "${INSTALL_DIR}/${BINARY_NAME}"
        success "Installed to ${INSTALL_DIR}/${BINARY_NAME}"
        INSTALL_DIR=""  # Already handled
    else
        INSTALL_DIR="${HOME}/.cargo/bin"
        mkdir -p "$INSTALL_DIR"
    fi

    if [ -n "$INSTALL_DIR" ]; then
        install -m 755 "$RELEASE_BIN" "${INSTALL_DIR}/${BINARY_NAME}"
        success "Installed to ${INSTALL_DIR}/${BINARY_NAME}"
    fi

    # Create config directory
    CONFIG_DIR="${HOME}/.config/tgcryptfs"
    mkdir -p "$CONFIG_DIR"
    chmod 700 "$CONFIG_DIR"
    success "Config directory: ${CONFIG_DIR}"
else
    info "Binary available at: ${RELEASE_BIN}"
    info "To install, run again with --install or copy manually:"
    info "  sudo install -m 755 ${RELEASE_BIN} /usr/local/bin/${BINARY_NAME}"
fi

# -- Summary -------------------------------------------------------------------

header "Build summary"
info "Binary:   ${RELEASE_BIN}"
info "Size:     ${BIN_SIZE}"
info "Tests:    $([ "$SKIP_TESTS" = true ] && echo "skipped" || echo "passed")"
info "Installed: $([ "$DO_INSTALL" = true ] && echo "yes" || echo "no")"
printf "\n"
