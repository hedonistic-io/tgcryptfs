# TGCryptFS v2 User Guide

## Overview

TGCryptFS is a post-quantum encrypted filesystem that stores your files in Telegram's cloud. Files are encrypted on your device before upload, chunked into blocks, and stored as messages in your Telegram Saved Messages. You access your files by mounting an encrypted volume as a regular directory via FUSE.

**Key properties:**
- Telegram cannot read your files (encrypted before upload)
- Your database reveals nothing without your password (opaque schema)
- Quantum-resistant key exchange for sharing (ML-KEM-768)
- Automatic destruction if you stop checking in (deadman switch)

---

## Getting Started

### 1. Install Prerequisites

**macOS:**
```bash
brew install macfuse
```

**Linux (Debian/Ubuntu):**
```bash
sudo apt-get install libfuse-dev
```

**Rust toolchain** (if building from source):
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

### 2. Install TGCryptFS

**From pre-built binaries:**
```bash
# Download from GitHub Releases
tar xzf tgcryptfs-v0.1.0-x86_64-unknown-linux-gnu.tar.gz
sudo mv tgcryptfs-v0.1.0-x86_64-unknown-linux-gnu/tgcryptfs /usr/local/bin/
```

**From source:**
```bash
git clone https://github.com/hedonistic-io/tgcryptfs.git
cd tgcryptfs
cargo build --release
sudo cp target/release/tgcryptfs /usr/local/bin/
```

### 3. Get Telegram API Credentials

1. Visit https://my.telegram.org and log in with your phone number
2. Go to **API development tools**
3. Create a new application (any name/description)
4. Note your **API ID** (a number) and **API Hash** (a hex string)

### 4. Run the Setup Wizard

```bash
tgcryptfs configure
```

The wizard will:
- Ask for your Telegram API credentials
- Create the config directory (`~/.config/tgcryptfs/`)
- Create the volumes directory (`~/.local/share/tgcryptfs/volumes/`)
- Write credentials to `~/.config/tgcryptfs/.env` with `0600` permissions
- Optionally add shell integration to your `.bashrc`/`.zshrc`/`config.fish`

### 5. Authenticate with Telegram

```bash
tgcryptfs auth login --api-id YOUR_API_ID --api-hash YOUR_API_HASH
```

If you ran `tgcryptfs configure` and added shell integration, the env vars are loaded automatically:
```bash
tgcryptfs auth login
```

The login flow is interactive:
1. Enter your phone number (with country code, e.g. `+12025551234`)
2. Enter the verification code sent to your Telegram app
3. If you have 2FA enabled, enter your 2FA password

You get 3 attempts for both the verification code and 2FA password.

### 6. Create Your First Volume

```bash
tgcryptfs volume create --name "my-vault"
```

You'll be prompted to:
1. Enter a password (minimum 8 characters)
2. Confirm the password

On success, you'll see:
```
Volume created successfully.
  Name:       my-vault
  Volume ID:  a1b2c3d4-...

Sentence Reference (store securely for key recovery):
  correct horse battery staple ... (22 words)

IMPORTANT: Store your password securely. It cannot be recovered.
```

**Write down the sentence reference.** It's a 22-word mnemonic that can recover your root key without your password. Store it somewhere safe (printed paper, password manager, etc.).

### 7. Mount the Volume

```bash
mkdir ~/vault
tgcryptfs volume mount my-vault ~/vault
```

Enter your password when prompted. The filesystem will mount and block until you press Ctrl+C:
```
Volume 'my-vault' authenticated successfully.
Mounting at '/Users/you/vault'...
Press Ctrl+C to unmount.
```

Now use `~/vault` like any directory — files are transparently encrypted and synced to Telegram.

### 8. Unmount

Press **Ctrl+C** in the terminal running mount, or from another terminal:
```bash
tgcryptfs volume unmount ~/vault
```

---

## Volume Management

### List Volumes

```bash
tgcryptfs volume list
```

Output:
```
VOLUME ID                              NAME                  STATUS
──────────────────────────────────────────────────────────────────────
a1b2c3d4-e5f6-...                      my-vault              unmounted
b2c3d4e5-f6a7-...                      work-files            unmounted

2 volume(s) total.
```

### Volume Info

```bash
tgcryptfs volume info my-vault
```

Output:
```
Volume Information
==================
  Volume ID:    a1b2c3d4-e5f6-...
  Name:         my-vault
  Block size:   1048576
  Compression:  Lz4
  KDF memory:   65536 KiB
  KDF iter:     3
```

### Delete a Volume

```bash
tgcryptfs volume delete my-vault
```

You'll be asked to confirm. Use `--force` to skip confirmation:
```bash
tgcryptfs volume delete my-vault --force
```

**This is irreversible.** The volume directory and all local data are removed.

---

## Key Management

### Key Rotation

Rotate to a new encryption epoch for forward secrecy:

```bash
tgcryptfs key rotate my-vault
```

This re-encrypts block metadata with a new epoch key. After rotation, the old epoch key can be destroyed — even with your password, data encrypted under old epochs cannot be recovered once the old epoch key is gone.

### Export Sentence Reference

If you lost your sentence reference, export it again:

```bash
tgcryptfs key export my-vault
```

### Import from Sentence Reference

Recover a root key from a 22-word sentence:

```bash
tgcryptfs key import "word1 word2 word3 ... word22"
```

---

## Sharing Volumes

TGCryptFS supports multi-user access with ML-KEM-768 post-quantum key exchange.

### Share with a User

```bash
tgcryptfs share create --volume my-vault --user alice --access read-write
```

Access levels:
| Level | Aliases | Permissions |
|-------|---------|-------------|
| `read-only` | `readonly`, `ro` | View files only |
| `read-write` | `readwrite`, `rw` | View and modify files |
| `admin` | — | Full access including sharing |

### List Shares

```bash
tgcryptfs share list --volume my-vault
```

### Revoke Access

```bash
tgcryptfs share revoke --volume my-vault --user alice
```

### Create an Invite Link

Generate a time-limited, use-limited invite:

```bash
tgcryptfs share invite --volume my-vault --access read-only --max-uses 5 --expires-in 24
```

Options:
- `--max-uses N` — Maximum number of times the invite can be used (omit for unlimited)
- `--expires-in HOURS` — Hours until expiry (omit for no expiry)

### Accept an Invite

```bash
tgcryptfs share accept INVITE_CODE
```

---

## Deadman Switch

The deadman switch automatically destroys your volume data if certain conditions are met. This protects against physical seizure — if you can't check in, the data is destroyed.

### Configure Triggers

Create a JSON configuration file:

```json
{
  "enabled": true,
  "check_interval_secs": 3600,
  "grace_period_secs": 86400,
  "triggers": [
    {
      "name": "heartbeat",
      "trigger_type": "Heartbeat",
      "active": true,
      "config": { "timeout_secs": 259200 }
    }
  ],
  "destruction_phases": [
    { "WipeDirectory": { "path": "~/.local/share/tgcryptfs" } }
  ]
}
```

Apply it:
```bash
tgcryptfs deadman configure ~/deadman-config.json
```

The config is saved to `~/.config/tgcryptfs/deadman.json`.

### Trigger Types

| Type | Description |
|------|-------------|
| **Heartbeat** | Requires periodic confirmation; triggers if timeout exceeded |
| **NetworkCheck** | Monitors connectivity to specific hosts |
| **OsEvent** | Watches for login attempts, USB insertion, etc. |
| **RpcCheck** | Polls external verification endpoints |
| **CustomCommand** | Runs arbitrary check scripts |

### Arm the Deadman Switch

```bash
tgcryptfs deadman arm
```

This starts a foreground daemon that evaluates triggers on the configured interval. Press Ctrl+C to disarm and stop.

### Check Status

```bash
tgcryptfs deadman status
```

### Disarm

```bash
tgcryptfs deadman disarm
```

### Destruction Sequence

When triggered, the deadman switch executes multi-phase destruction:
1. **Wipe encryption keys** from memory
2. **Shred metadata database** (SQLite)
3. **Delete Telegram messages** (the encrypted blocks)
4. **Overwrite cache** files

---

## REST API Server

Start the API server for programmatic access:

```bash
tgcryptfs serve --bind 127.0.0.1:8080
```

The server exposes 17 endpoints under `/api/v1/`. See the [API Reference](API_REFERENCE.md) for full documentation.

Quick test:
```bash
curl http://localhost:8080/api/v1/status
curl http://localhost:8080/api/v1/version
```

---

## System Status

Check overall system health:

```bash
tgcryptfs status
```

Output:
```
TGCryptFS v2 Status
====================

Version:    0.1.0
Volumes:    2 configured, 0 mounted
Telegram:   session present
Cache:      not initialized
Deadman:    disarmed
Data dir:   /home/you/.local/share/tgcryptfs/volumes

Use `tgcryptfs volume list` to see volumes.
```

---

## Configuration

### File Locations

| Path | Purpose |
|------|---------|
| `~/.config/tgcryptfs/.env` | API credentials and settings |
| `~/.config/tgcryptfs/deadman.json` | Deadman switch configuration |
| `~/.local/share/tgcryptfs/volumes/` | Volume data directory |
| `~/.local/share/tgcryptfs/volumes/{id}/config.json` | Volume metadata |
| `~/.local/share/tgcryptfs/volumes/{id}/metadata.db` | Encrypted SQLite database |
| `~/.local/share/tgcryptfs/volumes/{id}/cache/` | Block cache directory |
| `./tgcryptfs.session` | Telegram session token |

### Environment Variables

| Variable | Purpose |
|----------|---------|
| `TG_API_ID` | Telegram API ID |
| `TG_API_HASH` | Telegram API hash |
| `TGCRYPTFS_VOLUMES_DIR` | Override volumes directory |
| `TGCRYPTFS_SESSION_PATH` | Override session file location |
| `TGCRYPTFS_DEADMAN_ENABLED` | Enable deadman switch |
| `TGCRYPTFS_DEADMAN_TIMEOUT_HOURS` | Heartbeat timeout |

### Verbose Logging

Add `-v` to any command for debug-level logging:

```bash
tgcryptfs -v volume mount my-vault ~/vault
```

---

## Security Best Practices

1. **Use a strong password.** Argon2id is resilient to brute-force, but weak passwords are still weak.
2. **Store your sentence reference offline.** Write it on paper and keep it in a safe. Never store it digitally alongside your volume.
3. **Rotate keys periodically.** Use `tgcryptfs key rotate` to limit the impact of a compromised epoch key.
4. **Use read-only shares when possible.** Don't grant `admin` access unless the user needs to manage sharing.
5. **Keep your Telegram session secure.** The `tgcryptfs.session` file grants access to your Telegram account. Protect it like a private key.
6. **Enable the deadman switch** if you're in a high-risk environment. Configure a heartbeat trigger with a reasonable timeout.
7. **Never share your API credentials.** The `.env` file is created with `0600` permissions for a reason.

---

## Troubleshooting

### "Telegram API ID required"

You haven't set your credentials. Either:
- Run `tgcryptfs configure` to set them up
- Pass `--api-id` and `--api-hash` to `tgcryptfs auth login`
- Set `TG_API_ID` and `TG_API_HASH` environment variables

### "Not authenticated"

Run `tgcryptfs auth login` to create a Telegram session.

### "Volume not found"

Check the volume name with `tgcryptfs volume list`. Volume lookup matches by ID or display name.

### Mount fails with "FUSE mount failed"

- **macOS:** Ensure macFUSE is installed (`brew install macfuse`) and you've allowed the kernel extension in System Settings > Privacy & Security.
- **Linux:** Ensure `libfuse-dev` is installed and your user is in the `fuse` group: `sudo usermod -aG fuse $USER`

### "Passwords do not match"

When creating a volume, the confirmation password must match exactly. Re-enter both passwords.

### Key import says "expected 22 words, got N"

The sentence reference must be exactly 22 words, separated by spaces. Check for missing or extra words.

### "umount exited with" error

The volume may be in use. Close all applications using files in the mount directory and try again. On macOS, you can also use `diskutil unmount ~/vault`.
