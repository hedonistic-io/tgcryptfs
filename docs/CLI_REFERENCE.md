# TGCryptFS CLI Reference

## Synopsis

```
tgcryptfs [OPTIONS] <COMMAND>
```

## Global Options

| Option | Description |
|--------|-------------|
| `-v, --verbose` | Enable debug-level logging |
| `-h, --help` | Print help |
| `--version` | Print version |

---

## Commands

### tgcryptfs configure

Interactive setup wizard. Walks through Telegram API credential configuration, storage paths, optional deadman switch, and shell integration.

```
tgcryptfs configure
```

**Creates:**
- `~/.config/tgcryptfs/.env` (permissions: `0600`)
- `~/.config/tgcryptfs/` directory
- Volumes directory
- Optionally appends to shell RC file (`.bashrc`, `.zshrc`, or `config.fish`)

---

### tgcryptfs auth

Telegram authentication management.

#### tgcryptfs auth login

Authenticate with Telegram via interactive phone/code/2FA flow.

```
tgcryptfs auth login [OPTIONS]
```

| Option | Env Var | Description |
|--------|---------|-------------|
| `--api-id <ID>` | `TG_API_ID` | Telegram API ID (integer) |
| `--api-hash <HASH>` | `TG_API_HASH` | Telegram API hash (hex string) |

**Interactive prompts:**
1. Phone number (with country code)
2. Verification code (sent to Telegram app, 3 attempts)
3. 2FA password (if enabled, 3 attempts)

**Creates:** `tgcryptfs.session` in working directory.

#### tgcryptfs auth logout

Remove the Telegram session file.

```
tgcryptfs auth logout
```

**Deletes:** `tgcryptfs.session` from working directory.

#### tgcryptfs auth status

Check whether a session file exists.

```
tgcryptfs auth status
```

---

### tgcryptfs volume

Encrypted volume management.

#### tgcryptfs volume create

Create a new encrypted volume.

```
tgcryptfs volume create [OPTIONS]
```

| Option | Default | Description |
|--------|---------|-------------|
| `-n, --name <NAME>` | Auto-generated | Volume display name |
| `--block-size <BYTES>` | `1048576` (1 MB) | Target block size |

**Interactive prompts:** Password + confirmation.

**Output:** Volume ID, display name, and 22-word sentence reference.

#### tgcryptfs volume mount

Mount an encrypted volume as a FUSE filesystem.

```
tgcryptfs volume mount <VOLUME> <MOUNTPOINT>
```

| Argument | Description |
|----------|-------------|
| `VOLUME` | Volume name or UUID |
| `MOUNTPOINT` | Directory to mount at (created if needed) |

**Interactive prompt:** Volume password.

Blocks until Ctrl+C or `volume unmount`. Sets `auto_unmount` so the filesystem unmounts if the process dies.

#### tgcryptfs volume unmount

Unmount a mounted volume.

```
tgcryptfs volume unmount <TARGET>
```

| Argument | Description |
|----------|-------------|
| `TARGET` | Mount point path or volume name |

Uses `umount` on macOS, `fusermount -u` on Linux.

#### tgcryptfs volume list

List all configured volumes.

```
tgcryptfs volume list
```

Displays a table of volume IDs, names, and mount status.

#### tgcryptfs volume info

Show detailed volume information.

```
tgcryptfs volume info <VOLUME>
```

| Argument | Description |
|----------|-------------|
| `VOLUME` | Volume name or UUID |

Displays volume ID, name, block size, compression algorithm, and KDF parameters.

#### tgcryptfs volume delete

Permanently delete a volume and all its local data.

```
tgcryptfs volume delete <VOLUME> [OPTIONS]
```

| Argument | Description |
|----------|-------------|
| `VOLUME` | Volume name or UUID |

| Option | Description |
|--------|-------------|
| `--force` | Skip confirmation prompt |

---

### tgcryptfs key

Encryption key management.

#### tgcryptfs key rotate

Rotate to a new encryption epoch for forward secrecy.

```
tgcryptfs key rotate <VOLUME>
```

| Argument | Description |
|----------|-------------|
| `VOLUME` | Volume name or UUID |

**Interactive prompt:** Volume password.

Re-encrypts block metadata under a new epoch key. Updates the volume config with the new epoch number.

#### tgcryptfs key export

Export the volume's root key as a 22-word sentence reference.

```
tgcryptfs key export <VOLUME>
```

| Argument | Description |
|----------|-------------|
| `VOLUME` | Volume name or UUID |

**Interactive prompt:** Volume password.

#### tgcryptfs key import

Decode a 22-word sentence reference back to a root key.

```
tgcryptfs key import <SENTENCE>
```

| Argument | Description |
|----------|-------------|
| `SENTENCE` | 22 space-separated words |

---

### tgcryptfs share

Volume sharing and access control.

#### tgcryptfs share create

Grant a user access to a volume.

```
tgcryptfs share create --volume <VOLUME> --user <USER> [OPTIONS]
```

| Option | Default | Description |
|--------|---------|-------------|
| `--volume <VOLUME>` | Required | Volume name or UUID |
| `--user <USER>` | Required | User identifier (Telegram UID or username) |
| `--access <LEVEL>` | `read-write` | Access level |

**Interactive prompt:** Volume password.

Access levels: `read-only` (`readonly`, `ro`), `read-write` (`readwrite`, `rw`), `admin`.

#### tgcryptfs share list

List all active shares for a volume.

```
tgcryptfs share list --volume <VOLUME>
```

| Option | Description |
|--------|-------------|
| `--volume <VOLUME>` | Volume name or UUID |

**Interactive prompt:** Volume password.

#### tgcryptfs share revoke

Revoke a user's access to a volume.

```
tgcryptfs share revoke --volume <VOLUME> --user <USER>
```

| Option | Description |
|--------|-------------|
| `--volume <VOLUME>` | Volume name or UUID |
| `--user <USER>` | User identifier to revoke |

**Interactive prompt:** Volume password.

#### tgcryptfs share invite

Create a shareable invite with optional limits.

```
tgcryptfs share invite --volume <VOLUME> [OPTIONS]
```

| Option | Default | Description |
|--------|---------|-------------|
| `--volume <VOLUME>` | Required | Volume name or UUID |
| `--access <LEVEL>` | `read-only` | Access level for the invite |
| `--max-uses <N>` | Unlimited | Maximum number of uses |
| `--expires-in <HOURS>` | Never | Hours until expiry |

**Interactive prompt:** Volume password.

#### tgcryptfs share accept

Accept an invite code to gain access to a shared volume.

```
tgcryptfs share accept <INVITE_CODE>
```

| Argument | Description |
|----------|-------------|
| `INVITE_CODE` | Invite code or ID |

---

### tgcryptfs deadman

Deadman switch management.

#### tgcryptfs deadman configure

Load a deadman switch configuration from a JSON file.

```
tgcryptfs deadman configure <CONFIG_PATH>
```

| Argument | Description |
|----------|-------------|
| `CONFIG_PATH` | Path to JSON config file |

Copies the config to `~/.config/tgcryptfs/deadman.json`.

**Example config:**
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

#### tgcryptfs deadman arm

Arm the deadman switch and start the daemon loop.

```
tgcryptfs deadman arm
```

Runs in the foreground. Press Ctrl+C to disarm and stop. If triggers fire and the grace period expires without disarming, destruction phases execute.

#### tgcryptfs deadman disarm

Disarm the deadman switch.

```
tgcryptfs deadman disarm
```

Prints a disarm notice. To stop a running daemon, send Ctrl+C to the `deadman arm` process.

#### tgcryptfs deadman status

Show deadman configuration and trigger status.

```
tgcryptfs deadman status
```

---

### tgcryptfs status

Show overall system status.

```
tgcryptfs status
```

Displays version, volume count, Telegram session status, cache state, deadman state, and data directory path.

---

### tgcryptfs serve

Start the REST API HTTP server.

```
tgcryptfs serve [OPTIONS]
```

| Option | Default | Description |
|--------|---------|-------------|
| `--bind <ADDR>` | `127.0.0.1:8080` | Bind address (host:port) |

Runs in the foreground. See [API Reference](API_REFERENCE.md) for endpoint documentation.

---

## Exit Codes

| Code | Meaning |
|------|---------|
| `0` | Success |
| `1` | Error (with message printed to stderr) |

All errors include a human-readable message and a suggestion for resolution.
