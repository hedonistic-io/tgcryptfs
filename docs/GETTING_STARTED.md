# Getting Started with TGCryptFS

This guide walks you through installing TGCryptFS, setting up Telegram API credentials, creating your first encrypted volume, and mounting it as a normal directory.

---

## Prerequisites

### All platforms

- A Telegram account
- Telegram API credentials (API ID and API Hash) -- see [Telegram Setup](TELEGRAM_SETUP.md)

### Linux

- Rust 1.75+ (for building from source)
- FUSE 3: `libfuse3-dev` (Debian/Ubuntu) or `fuse3` (Fedora/Arch)

```bash
# Debian / Ubuntu
sudo apt install libfuse3-dev pkg-config

# Fedora
sudo dnf install fuse3-devel

# Arch
sudo pacman -S fuse3
```

### macOS

- Rust 1.75+ (for building from source)
- [macFUSE](https://osxfuse.github.io/) 4.x or later

```bash
brew install macfuse
```

After installing macFUSE, you may need to allow the kernel extension in System Settings > Privacy & Security, then reboot.

### Windows (experimental)

- Rust 1.75+
- [WinFsp](https://winfsp.dev/) 2.0 or later

FUSE mounting on Windows is experimental. The REST API server works without FUSE.

---

## Installation

### Option 1: Install script (Linux and macOS)

```bash
curl -fsSL https://raw.githubusercontent.com/hedonistic-io/tgcryptfs/main/scripts/install.sh | bash
```

This downloads the latest release binary for your platform and installs it to `/usr/local/bin/tgcryptfs`.

### Option 2: Cargo install

```bash
cargo install tgcryptfs-cli
```

### Option 3: Build from source

```bash
git clone https://github.com/hedonistic-io/tgcryptfs.git
cd tgcryptfs
cargo build --release
sudo cp target/release/tgcryptfs /usr/local/bin/
```

Verify the installation:

```bash
tgcryptfs --version
```

---

## Telegram API Setup

TGCryptFS uses the Telegram API to store encrypted blocks in your account. You need an API ID and API Hash.

**Quick method** -- run the setup script:

```bash
./scripts/setup-telegram.sh
```

**Manual method** -- see [docs/TELEGRAM_SETUP.md](TELEGRAM_SETUP.md) for a detailed walkthrough.

Once you have your credentials, authenticate:

```bash
tgcryptfs auth login
```

You will be prompted for your Telegram phone number and a verification code sent to your Telegram app. If you have two-factor authentication enabled, you will also be prompted for your password.

Check authentication status:

```bash
tgcryptfs auth status
```

---

## Creating Your First Volume

Create a new encrypted volume:

```bash
tgcryptfs volume create --name mydata
```

You will be prompted to set a passphrase. This passphrase protects your volume's master key via Argon2id key derivation. Choose a strong passphrase and store it securely -- if you lose it, you will need your sentence reference backup to recover access.

The command outputs a 22-word sentence reference. **Write this down and store it securely.** It is the only way to recover your volume if you lose your passphrase or local configuration.

List your volumes:

```bash
tgcryptfs volume list
```

View details for a specific volume:

```bash
tgcryptfs volume info mydata
```

---

## Mounting a Volume

Mount your volume to a local directory:

```bash
mkdir -p ~/secure
tgcryptfs volume mount mydata ~/secure
```

The mount point now behaves like a normal directory:

```bash
# Copy files in
cp ~/Documents/report.pdf ~/secure/

# List contents
ls -la ~/secure/

# Read files normally
cat ~/secure/report.pdf

# Create directories
mkdir ~/secure/projects
```

All data written to `~/secure/` is automatically encrypted, chunked, and uploaded to Telegram. All data read from `~/secure/` is automatically downloaded, reassembled, and decrypted.

Unmount when you are done:

```bash
tgcryptfs volume unmount ~/secure
```

---

## Using the REST API

Start the API server:

```bash
tgcryptfs serve --bind 127.0.0.1:8080
```

The server requires bearer token authentication. The token is generated at server startup and printed to the console. Use it in the `Authorization` header:

```bash
# Check server status
curl -H "Authorization: Bearer <token>" http://127.0.0.1:8080/api/v1/status

# List volumes
curl -H "Authorization: Bearer <token>" http://127.0.0.1:8080/api/v1/volumes

# Create a volume
curl -X POST -H "Authorization: Bearer <token>" \
     -H "Content-Type: application/json" \
     -d '{"name": "api-volume"}' \
     http://127.0.0.1:8080/api/v1/volumes
```

See [API Reference](API_REFERENCE.md) for all 21 endpoints.

---

## Shell Completions

Generate shell completions for tab-completion support:

```bash
# Bash
tgcryptfs completions bash > ~/.local/share/bash-completion/completions/tgcryptfs

# Zsh
tgcryptfs completions zsh > ~/.zfunc/_tgcryptfs

# Fish
tgcryptfs completions fish > ~/.config/fish/completions/tgcryptfs.fish
```

Restart your shell or source the completion file to activate.

---

## Running as a System Service

### Linux (systemd)

```bash
sudo cp system/tgcryptfs.service /etc/systemd/system/
sudo systemctl daemon-reload
sudo systemctl enable --now tgcryptfs
```

### macOS (launchd)

```bash
cp system/io.hedonistic.tgcryptfs.plist ~/Library/LaunchAgents/
launchctl load ~/Library/LaunchAgents/io.hedonistic.tgcryptfs.plist
```

The service files run the REST API server by default. Edit them to configure bind address, auto-mount volumes, or adjust logging.

---

## Key Management

### Export a sentence reference backup

```bash
tgcryptfs key export mydata
```

This prints a 22-word sentence that encodes your volume's key material. Store it offline in a secure location.

### Import from a sentence reference

```bash
tgcryptfs key import "word1 word2 word3 ... word22"
```

### Rotate encryption keys

```bash
tgcryptfs key rotate mydata
```

Key rotation creates a new key epoch. Existing data is re-encrypted with the new key. Previous epochs are retained for reading historical data, but new writes use the current epoch.

---

## Sharing Volumes

Share a volume with another user:

```bash
tgcryptfs share create --volume mydata --user @alice --access read-write
```

Create an invite link:

```bash
tgcryptfs share invite --volume mydata --access read-only --expires-in 48
```

The recipient accepts the invite:

```bash
tgcryptfs share accept <invite-code>
```

Sharing uses ML-KEM-768 post-quantum key exchange. The volume key is encapsulated for the recipient's public key; at no point does the unencrypted key leave your machine during the exchange.

---

## Dead Man's Switch

The dead man's switch automatically destroys your volumes if you fail to check in within a configured interval.

```bash
# View status
tgcryptfs deadman status

# Arm the switch
tgcryptfs deadman arm

# Disarm the switch
tgcryptfs deadman disarm

# Configure triggers
tgcryptfs deadman configure deadman-config.json
```

See [Security Model](SECURITY.md) for details on destruction behavior.

---

## Troubleshooting

### "Transport error: not authenticated"

You need to log in first:

```bash
tgcryptfs auth login
```

### "FUSE mount failed: permission denied"

On Linux, ensure your user is in the `fuse` group:

```bash
sudo usermod -aG fuse $USER
```

Then log out and back in.

On macOS, ensure macFUSE is installed and its kernel extension is allowed in System Settings > Privacy & Security.

### "Failed to connect to Telegram"

Check your internet connection and ensure your API credentials are valid. Telegram may rate-limit new sessions; wait a few minutes and try again.

### Mount point is busy

If a previous mount was not cleanly unmounted:

```bash
# Linux
fusermount -u ~/secure

# macOS
umount ~/secure
```

### Slow uploads or downloads

TGCryptFS performance depends on your network connection to Telegram's servers. Large files are chunked and uploaded in parallel, but Telegram may throttle rapid uploads. Consider using a wired connection for bulk transfers.

### "No such volume"

Volume names are case-sensitive. Use `tgcryptfs volume list` to see exact names. You can also reference volumes by their UUID.
