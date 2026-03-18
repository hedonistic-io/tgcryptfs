#!/usr/bin/env bash
#
# tgcryptfs universal installer
# Installs pre-built binaries from GitHub releases for Linux and macOS.
#
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/hedonistic-io/tgcryptfs/main/scripts/install.sh | bash
#   curl -fsSL ... | bash -s -- --version 0.3.0
#
# Supports: Linux (Debian/Ubuntu, Fedora/RHEL, Arch), macOS (Intel + Apple Silicon)

set -euo pipefail

# -- Constants ----------------------------------------------------------------

REPO="hedonistic-io/tgcryptfs"
BINARY_NAME="tgcryptfs"
GITHUB_API="https://api.github.com/repos/${REPO}"
GITHUB_RELEASES="https://github.com/${REPO}/releases"
CONFIG_DIR="${HOME}/.config/tgcryptfs"

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
fatal()   { error "$@"; cleanup; exit 1; }
header()  { printf "\n${BOLD}%s${RESET}\n" "$*"; }

# -- Cleanup / rollback -------------------------------------------------------

TMPDIR_INSTALL=""
INSTALLED_BIN=""
INSTALLED_COMPLETIONS=()

cleanup() {
    if [ -n "$TMPDIR_INSTALL" ] && [ -d "$TMPDIR_INSTALL" ]; then
        rm -rf "$TMPDIR_INSTALL"
    fi
}

rollback() {
    warn "Rolling back partial installation..."
    if [ -n "$INSTALLED_BIN" ] && [ -f "$INSTALLED_BIN" ]; then
        rm -f "$INSTALLED_BIN"
        warn "Removed ${INSTALLED_BIN}"
    fi
    for f in "${INSTALLED_COMPLETIONS[@]}"; do
        if [ -f "$f" ]; then
            rm -f "$f"
            warn "Removed ${f}"
        fi
    done
    cleanup
}

trap rollback ERR
trap cleanup EXIT

# -- Argument parsing ---------------------------------------------------------

VERSION=""
while [ $# -gt 0 ]; do
    case "$1" in
        --version)
            VERSION="$2"
            shift 2
            ;;
        --version=*)
            VERSION="${1#*=}"
            shift
            ;;
        --help|-h)
            printf "Usage: install.sh [--version VERSION]\n"
            printf "  --version VERSION   Install a specific release (default: latest)\n"
            exit 0
            ;;
        *)
            fatal "Unknown argument: $1"
            ;;
    esac
done

# -- Platform detection -------------------------------------------------------

detect_os() {
    local os
    os="$(uname -s)"
    case "$os" in
        Linux)  echo "linux" ;;
        Darwin) echo "darwin" ;;
        *)      fatal "Unsupported operating system: ${os}" ;;
    esac
}

detect_arch() {
    local arch
    arch="$(uname -m)"
    case "$arch" in
        x86_64|amd64)   echo "x86_64" ;;
        aarch64|arm64)   echo "aarch64" ;;
        *)               fatal "Unsupported architecture: ${arch}" ;;
    esac
}

detect_linux_distro() {
    if [ -f /etc/os-release ]; then
        # shellcheck disable=SC1091
        . /etc/os-release
        echo "${ID:-unknown}"
    elif command -v lsb_release >/dev/null 2>&1; then
        lsb_release -is | tr '[:upper:]' '[:lower:]'
    else
        echo "unknown"
    fi
}

OS="$(detect_os)"
ARCH="$(detect_arch)"

# Normalize arch name for release artifacts (macOS uses arm64 convention)
if [ "$OS" = "darwin" ] && [ "$ARCH" = "aarch64" ]; then
    ARCH_LABEL="arm64"
else
    ARCH_LABEL="$ARCH"
fi

header "tgcryptfs installer"
info "Detected platform: ${OS}/${ARCH_LABEL}"

# -- Resolve version ----------------------------------------------------------

resolve_version() {
    if [ -n "$VERSION" ]; then
        # Strip leading 'v' if user provided it
        VERSION="${VERSION#v}"
        info "Requested version: ${VERSION}"
        return
    fi

    info "Fetching latest release..."
    local api_response
    if command -v curl >/dev/null 2>&1; then
        api_response="$(curl -fsSL "${GITHUB_API}/releases/latest" 2>/dev/null)" || \
            fatal "Failed to query GitHub API. Check your network connection."
    elif command -v wget >/dev/null 2>&1; then
        api_response="$(wget -qO- "${GITHUB_API}/releases/latest" 2>/dev/null)" || \
            fatal "Failed to query GitHub API. Check your network connection."
    else
        fatal "Neither curl nor wget found. Install one and retry."
    fi

    VERSION="$(printf '%s' "$api_response" | grep '"tag_name"' | head -1 | sed 's/.*"tag_name"[[:space:]]*:[[:space:]]*"v\{0,1\}\([^"]*\)".*/\1/')"
    if [ -z "$VERSION" ]; then
        fatal "Could not determine latest version from GitHub API."
    fi
    info "Latest version: ${VERSION}"
}

resolve_version

# -- Download helper -----------------------------------------------------------

download() {
    local url="$1" dest="$2"
    if command -v curl >/dev/null 2>&1; then
        curl -fSL --progress-bar -o "$dest" "$url"
    elif command -v wget >/dev/null 2>&1; then
        wget --show-progress -qO "$dest" "$url"
    else
        fatal "Neither curl nor wget available."
    fi
}

# -- Install FUSE dependencies ------------------------------------------------

install_fuse_deps() {
    header "Checking FUSE dependencies"

    if [ "$OS" = "darwin" ]; then
        if [ -d "/Library/Filesystems/macfuse.fs" ] || [ -d "/usr/local/lib/libfuse.dylib" ]; then
            success "macFUSE is already installed."
            return
        fi
        if command -v brew >/dev/null 2>&1; then
            info "Installing macFUSE via Homebrew..."
            brew install --cask macfuse || {
                warn "Homebrew install failed. Please install macFUSE manually:"
                warn "  https://osxfuse.github.io/"
                warn "Continuing with binary installation..."
            }
        else
            warn "macFUSE is not installed and Homebrew is not available."
            warn "Please install macFUSE manually: https://osxfuse.github.io/"
            warn "Continuing with binary installation..."
        fi
        return
    fi

    # Linux: check if libfuse is present
    if pkg-config --exists fuse 2>/dev/null || pkg-config --exists fuse3 2>/dev/null; then
        success "FUSE development libraries are already installed."
        return
    fi

    local distro
    distro="$(detect_linux_distro)"
    info "Linux distribution: ${distro}"

    local use_sudo=""
    if [ "$(id -u)" -ne 0 ]; then
        if command -v sudo >/dev/null 2>&1; then
            use_sudo="sudo"
        else
            warn "Not running as root and sudo is not available."
            warn "Please install FUSE development libraries manually."
            return
        fi
    fi

    case "$distro" in
        ubuntu|debian|pop|linuxmint|elementary|zorin)
            info "Installing libfuse3-dev via apt..."
            $use_sudo apt-get update -qq
            $use_sudo apt-get install -y -qq libfuse3-dev fuse3 pkg-config
            ;;
        fedora|rhel|centos|rocky|alma|ol)
            info "Installing fuse3-devel via dnf..."
            $use_sudo dnf install -y fuse3-devel fuse3 pkg-config
            ;;
        arch|manjaro|endeavouros)
            info "Installing fuse3 via pacman..."
            $use_sudo pacman -S --noconfirm --needed fuse3 pkg-config
            ;;
        opensuse*|suse|sles)
            info "Installing fuse3-devel via zypper..."
            $use_sudo zypper install -y fuse3-devel fuse3 pkg-config
            ;;
        alpine)
            info "Installing fuse3-dev via apk..."
            $use_sudo apk add --no-cache fuse3-dev fuse3 pkgconf
            ;;
        *)
            warn "Unknown distribution '${distro}'. Please install FUSE 3 development"
            warn "libraries manually (e.g., libfuse3-dev, fuse3-devel)."
            ;;
    esac
    success "FUSE dependencies installed."
}

install_fuse_deps

# -- Download binary -----------------------------------------------------------

header "Downloading tgcryptfs v${VERSION}"

TMPDIR_INSTALL="$(mktemp -d)"
ARCHIVE_NAME="${BINARY_NAME}-v${VERSION}-${OS}-${ARCH_LABEL}.tar.gz"
ARCHIVE_URL="${GITHUB_RELEASES}/download/v${VERSION}/${ARCHIVE_NAME}"
ARCHIVE_PATH="${TMPDIR_INSTALL}/${ARCHIVE_NAME}"

info "Downloading ${ARCHIVE_URL}"
download "$ARCHIVE_URL" "$ARCHIVE_PATH"

# Verify checksum if available
CHECKSUM_URL="${GITHUB_RELEASES}/download/v${VERSION}/checksums-sha256.txt"
CHECKSUM_PATH="${TMPDIR_INSTALL}/checksums-sha256.txt"
if download "$CHECKSUM_URL" "$CHECKSUM_PATH" 2>/dev/null; then
    info "Verifying SHA-256 checksum..."
    expected="$(grep "${ARCHIVE_NAME}" "$CHECKSUM_PATH" | awk '{print $1}')"
    if [ -n "$expected" ]; then
        if command -v sha256sum >/dev/null 2>&1; then
            actual="$(sha256sum "$ARCHIVE_PATH" | awk '{print $1}')"
        elif command -v shasum >/dev/null 2>&1; then
            actual="$(shasum -a 256 "$ARCHIVE_PATH" | awk '{print $1}')"
        else
            warn "No sha256sum or shasum available; skipping checksum verification."
            actual="$expected"
        fi
        if [ "$actual" != "$expected" ]; then
            fatal "Checksum mismatch! Expected: ${expected}, Got: ${actual}"
        fi
        success "Checksum verified."
    else
        warn "Archive not found in checksum file; skipping verification."
    fi
else
    warn "Checksum file not available; skipping verification."
fi

# -- Extract -------------------------------------------------------------------

info "Extracting archive..."
tar -xzf "$ARCHIVE_PATH" -C "$TMPDIR_INSTALL"

# Locate the binary (may be at top level or inside a directory)
EXTRACTED_BIN=""
if [ -f "${TMPDIR_INSTALL}/${BINARY_NAME}" ]; then
    EXTRACTED_BIN="${TMPDIR_INSTALL}/${BINARY_NAME}"
else
    EXTRACTED_BIN="$(find "$TMPDIR_INSTALL" -name "$BINARY_NAME" -type f | head -1)"
fi

if [ -z "$EXTRACTED_BIN" ] || [ ! -f "$EXTRACTED_BIN" ]; then
    fatal "Binary '${BINARY_NAME}' not found in archive."
fi

chmod +x "$EXTRACTED_BIN"

# -- Install binary ------------------------------------------------------------

header "Installing binary"

INSTALL_DIR=""
if [ -w /usr/local/bin ]; then
    INSTALL_DIR="/usr/local/bin"
elif command -v sudo >/dev/null 2>&1 && [ "$(id -u)" -ne 0 ]; then
    INSTALL_DIR="/usr/local/bin"
    info "Installing to /usr/local/bin (requires sudo)..."
    sudo install -m 755 "$EXTRACTED_BIN" "${INSTALL_DIR}/${BINARY_NAME}"
    INSTALLED_BIN="${INSTALL_DIR}/${BINARY_NAME}"
else
    INSTALL_DIR="${HOME}/.local/bin"
    mkdir -p "$INSTALL_DIR"
    warn "Cannot write to /usr/local/bin. Installing to ${INSTALL_DIR} instead."
    case ":${PATH}:" in
        *":${INSTALL_DIR}:"*) ;;
        *)
            warn "Add ${INSTALL_DIR} to your PATH:"
            warn "  export PATH=\"${INSTALL_DIR}:\$PATH\""
            ;;
    esac
fi

if [ -z "$INSTALLED_BIN" ]; then
    install -m 755 "$EXTRACTED_BIN" "${INSTALL_DIR}/${BINARY_NAME}"
    INSTALLED_BIN="${INSTALL_DIR}/${BINARY_NAME}"
fi

success "Binary installed to ${INSTALLED_BIN}"

# -- Install shell completions -------------------------------------------------

header "Installing shell completions"

install_completion() {
    local shell="$1" content="$2" dest="$3" dir
    dir="$(dirname "$dest")"
    if [ -d "$dir" ] || mkdir -p "$dir" 2>/dev/null; then
        printf '%s' "$content" > "$dest" 2>/dev/null && {
            INSTALLED_COMPLETIONS+=("$dest")
            success "Installed ${shell} completions to ${dest}"
            return
        }
    fi
    # Try with sudo
    if command -v sudo >/dev/null 2>&1; then
        sudo mkdir -p "$dir" 2>/dev/null
        printf '%s' "$content" | sudo tee "$dest" >/dev/null 2>/dev/null && {
            INSTALLED_COMPLETIONS+=("$dest")
            success "Installed ${shell} completions to ${dest}"
            return
        }
    fi
    warn "Could not install ${shell} completions to ${dest}"
}

# Generate completions if the binary supports it
if "${INSTALLED_BIN}" completions bash >/dev/null 2>&1; then
    BASH_COMP="$("${INSTALLED_BIN}" completions bash 2>/dev/null)"
    ZSH_COMP="$("${INSTALLED_BIN}" completions zsh 2>/dev/null)"
    FISH_COMP="$("${INSTALLED_BIN}" completions fish 2>/dev/null)"

    # Bash completions
    if [ -n "$BASH_COMP" ]; then
        if [ "$OS" = "darwin" ]; then
            install_completion "bash" "$BASH_COMP" "/usr/local/etc/bash_completion.d/${BINARY_NAME}"
        else
            install_completion "bash" "$BASH_COMP" "/etc/bash_completion.d/${BINARY_NAME}"
        fi
        # Also install to user directory as fallback
        install_completion "bash (user)" "$BASH_COMP" "${HOME}/.local/share/bash-completion/completions/${BINARY_NAME}"
    fi

    # Zsh completions
    if [ -n "$ZSH_COMP" ]; then
        if [ "$OS" = "darwin" ]; then
            install_completion "zsh" "$ZSH_COMP" "/usr/local/share/zsh/site-functions/_${BINARY_NAME}"
        else
            install_completion "zsh" "$ZSH_COMP" "/usr/local/share/zsh/site-functions/_${BINARY_NAME}"
        fi
        install_completion "zsh (user)" "$ZSH_COMP" "${HOME}/.local/share/zsh/site-functions/_${BINARY_NAME}"
    fi

    # Fish completions
    if [ -n "$FISH_COMP" ]; then
        install_completion "fish" "$FISH_COMP" "${HOME}/.config/fish/completions/${BINARY_NAME}.fish"
    fi
else
    warn "Binary does not support 'completions' command; skipping shell completions."
fi

# -- Create config directory ---------------------------------------------------

header "Setting up configuration"

mkdir -p "$CONFIG_DIR"
chmod 700 "$CONFIG_DIR"
success "Config directory: ${CONFIG_DIR}"

if [ ! -f "${CONFIG_DIR}/.env" ]; then
    info "No Telegram credentials found. Run 'tgcryptfs setup-telegram' or"
    info "see scripts/setup-telegram.sh to configure your API credentials."
fi

# -- Verify installation -------------------------------------------------------

header "Verifying installation"

if command -v "$BINARY_NAME" >/dev/null 2>&1; then
    installed_version="$("$BINARY_NAME" --version 2>/dev/null || echo "unknown")"
    success "tgcryptfs is installed and available on PATH."
    info "Version: ${installed_version}"
elif [ -x "$INSTALLED_BIN" ]; then
    installed_version="$("$INSTALLED_BIN" --version 2>/dev/null || echo "unknown")"
    success "tgcryptfs installed to ${INSTALLED_BIN}"
    info "Version: ${installed_version}"
    warn "Binary is not on PATH. You may need to restart your shell or update PATH."
else
    fatal "Installation verification failed."
fi

# -- Telemetry (anonymous install counter) ------------------------------------

# Sends a single anonymous ping to track install counts.
# No personal data, no IP logging -- just OS, arch, and version.
# Set TGCRYPTFS_NO_TELEMETRY=1 to disable.
if [ "${TGCRYPTFS_NO_TELEMETRY:-}" != "1" ]; then
    curl -fsSL -o /dev/null -w "" \
        "https://tgcryptfs.hedonistic.io/install-ping?os=${OS}&arch=${ARCH}&v=${VERSION}" \
        2>/dev/null &
fi

# -- Done ----------------------------------------------------------------------

header "Installation complete"
printf "\n"
info "Quick start:"
info "  tgcryptfs setup-telegram     # Configure Telegram API credentials"
info "  tgcryptfs auth login          # Authenticate with Telegram"
info "  tgcryptfs volume create myfs  # Create an encrypted volume"
info "  tgcryptfs mount myfs ~/mnt    # Mount the volume"
printf "\n"
info "Documentation: https://github.com/${REPO}#readme"
printf "\n"
