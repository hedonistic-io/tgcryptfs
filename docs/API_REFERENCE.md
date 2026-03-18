# TGCryptFS REST API Reference

## Overview

The TGCryptFS REST API is served by the `tgcryptfs serve` command. All endpoints are under the `/api/v1` prefix. The server uses axum 0.7 with CORS enabled (permissive).

**Start the server:**
```bash
tgcryptfs serve --bind 127.0.0.1:8080
```

**Base URL:** `http://127.0.0.1:8080/api/v1`

---

## Error Format

All errors return JSON with this structure:

```json
{
  "error": "Human-readable error description",
  "code": "MACHINE_READABLE_CODE",
  "suggestion": "What to do to fix this"
}
```

### Error Codes

| Code | HTTP Status | Description |
|------|-------------|-------------|
| `VOLUME_NOT_FOUND` | 404 | Volume does not exist |
| `VOLUME_EXISTS` | 409 | Volume name already in use |
| `VOLUME_MOUNTED` | 409 | Volume is currently mounted |
| `VOLUME_NOT_MOUNTED` | 400 | Volume is not mounted |
| `AUTH_REQUIRED` | 401 | Telegram authentication needed |
| `TELEGRAM_ERROR` | 502 | Telegram connection or API error |
| `CRYPTO_ERROR` | 500 | Cryptographic operation failed |
| `STORAGE_ERROR` | 500 | Database or storage error |
| `INVALID_ARGUMENT` | 400 | Missing or invalid request parameter |
| `INTERNAL_ERROR` | 500 | Unexpected server error |
| `IO_ERROR` | 500 | Filesystem I/O error |

---

## System Endpoints

### GET /api/v1/status

Returns system health and aggregate statistics.

**Response** `200 OK`:
```json
{
  "version": "0.1.0",
  "telegram_connected": false,
  "volumes_mounted": 0,
  "total_volumes": 3,
  "cache_entries": 0,
  "cache_size_bytes": 0,
  "deadman_armed": false,
  "uptime_secs": 120
}
```

| Field | Type | Description |
|-------|------|-------------|
| `version` | string | Server version |
| `telegram_connected` | bool | Whether a Telegram session is active |
| `volumes_mounted` | integer | Number of currently mounted volumes |
| `total_volumes` | integer | Total configured volumes |
| `cache_entries` | integer | Cached blocks |
| `cache_size_bytes` | integer | Cache size in bytes |
| `deadman_armed` | bool | Whether deadman switch is armed |
| `uptime_secs` | integer | Server uptime in seconds |

---

### GET /api/v1/version

Returns the server version string.

**Response** `200 OK`:
```json
{
  "version": "0.1.0"
}
```

---

## Authentication Endpoints

### POST /api/v1/auth/session

Check for an existing Telegram session. Interactive authentication (phone/code/2FA) requires the CLI.

**Request body:** None.

**Response** `200 OK` (session exists):
```json
{
  "status": "authenticated",
  "session_path": "/home/user/.local/share/tgcryptfs/session"
}
```

**Response** `200 OK` (no session):
```json
{
  "status": "not_authenticated",
  "message": "Use the CLI `tgcryptfs auth login` for interactive authentication"
}
```

---

### GET /api/v1/auth/status

Check current authentication state.

**Response** `200 OK`:
```json
{
  "authenticated": true,
  "session_path": "/home/user/.local/share/tgcryptfs/session"
}
```

| Field | Type | Description |
|-------|------|-------------|
| `authenticated` | bool | Whether a session file exists |
| `session_path` | string | Path to the session file |

---

### DELETE /api/v1/auth/session

Remove the Telegram session (logout).

**Response** `200 OK`:
```json
{
  "status": "logged_out"
}
```

**Errors:**
- `500 INTERNAL_ERROR` — Failed to remove session file.

---

## Volume Endpoints

### POST /api/v1/volumes

Create a new encrypted volume.

**Request body:**
```json
{
  "name": "my-vault",
  "password": "secure_password",
  "block_size": 1048576
}
```

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `name` | string | No | Display name (auto-generated if omitted) |
| `password` | string | Yes | Volume password |
| `block_size` | integer | No | Target block size in bytes (default: 1 MB) |

**Response** `201 Created`:
```json
{
  "volume_id": "a1b2c3d4-e5f6-7890-abcd-ef1234567890",
  "display_name": "my-vault",
  "sentence_ref": "correct horse battery staple ... (22 words)"
}
```

**Errors:**
- `400 INVALID_ARGUMENT` — Missing password.
- `409 VOLUME_EXISTS` — Volume name already in use.

---

### GET /api/v1/volumes

List all configured volumes.

**Response** `200 OK`:
```json
[
  {
    "volume_id": "a1b2c3d4-e5f6-7890-abcd-ef1234567890",
    "display_name": "my-vault",
    "created_at": 1739836800,
    "mounted": false,
    "mount_point": null,
    "block_count": 0,
    "total_size": 0
  }
]
```

Returns an empty array if no volumes exist.

---

### GET /api/v1/volumes/:id

Get detailed information about a specific volume.

**Path parameter:** `id` — Volume UUID.

**Response** `200 OK`:
```json
{
  "volume_id": "a1b2c3d4-e5f6-7890-abcd-ef1234567890",
  "display_name": "my-vault",
  "created_at": 1739836800,
  "mounted": true,
  "mount_point": "/home/user/vault",
  "block_count": 42,
  "total_size": 44040192
}
```

| Field | Type | Description |
|-------|------|-------------|
| `volume_id` | string | Volume UUID |
| `display_name` | string | Human-readable name |
| `created_at` | integer | Unix timestamp |
| `mounted` | bool | Whether the volume is currently mounted |
| `mount_point` | string or null | Mount directory path |
| `block_count` | integer | Number of stored blocks |
| `total_size` | integer | Total data size in bytes |

**Errors:**
- `404 VOLUME_NOT_FOUND` — Volume does not exist.

---

### DELETE /api/v1/volumes/:id

Delete a volume and all its local data.

**Path parameter:** `id` — Volume UUID.

**Response** `200 OK`:
```json
{
  "status": "deleted",
  "volume_id": "a1b2c3d4-e5f6-7890-abcd-ef1234567890"
}
```

**Errors:**
- `404 VOLUME_NOT_FOUND` — Volume does not exist.
- `409 VOLUME_MOUNTED` — Volume is currently mounted; unmount first.

---

### POST /api/v1/volumes/:id/mount

Mount a volume. Returns immediately; mount happens asynchronously.

**Path parameter:** `id` — Volume UUID.

**Request body:**
```json
{
  "password": "secure_password",
  "mount_point": "/home/user/vault"
}
```

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `password` | string | Yes | Volume password |
| `mount_point` | string | Yes | Filesystem path to mount at |

**Response** `202 Accepted`:
```json
{
  "status": "mounting",
  "volume_id": "a1b2c3d4-e5f6-7890-abcd-ef1234567890",
  "message": "Mount operation started. Use GET /api/v1/volumes/:id to check status."
}
```

**Errors:**
- `404 VOLUME_NOT_FOUND` — Volume does not exist.
- `400 INVALID_ARGUMENT` — Missing password or mount_point.

---

### POST /api/v1/volumes/:id/unmount

Unmount a mounted volume.

**Path parameter:** `id` — Volume UUID.

**Request body:** None.

**Response** `200 OK`:
```json
{
  "status": "unmounting",
  "volume_id": "a1b2c3d4-e5f6-7890-abcd-ef1234567890",
  "message": "Unmount operation started."
}
```

**Errors:**
- `404 VOLUME_NOT_FOUND` — Volume does not exist.
- `400 VOLUME_NOT_MOUNTED` — Volume is not currently mounted.

---

## Sharing Endpoints

### GET /api/v1/shares/volume/:volume_id

List all shares for a volume.

**Path parameter:** `volume_id` — Volume UUID.

**Response** `200 OK`:
```json
{
  "volume_id": "a1b2c3d4-e5f6-7890-abcd-ef1234567890",
  "shares": [],
  "message": "Sharing list requires an authenticated volume session"
}
```

---

### POST /api/v1/shares

Create a new share granting a user access to a volume.

**Request body:**
```json
{
  "volume_id": "a1b2c3d4-e5f6-7890-abcd-ef1234567890",
  "user_id": "alice",
  "access_level": "read-write"
}
```

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `volume_id` | string | Yes | Volume UUID |
| `user_id` | string | Yes | User identifier |
| `access_level` | string | No | `read-only` (default), `read-write`, or `admin` |

**Response** `201 Created`:
```json
{
  "status": "created",
  "volume_id": "a1b2c3d4-e5f6-7890-abcd-ef1234567890",
  "user_id": "alice",
  "access_level": "read-write"
}
```

**Errors:**
- `400 INVALID_ARGUMENT` — Missing volume_id or user_id.

---

### DELETE /api/v1/shares/:id

Revoke a share.

**Path parameter:** `id` — Share ID.

**Response** `200 OK`:
```json
{
  "status": "revoked",
  "share_id": "share-abc123"
}
```

---

## Deadman Switch Endpoints

### GET /api/v1/deadman/status

Check deadman switch configuration and armed state.

**Response** `200 OK`:
```json
{
  "configured": true,
  "armed": false,
  "config_path": "/home/user/.config/tgcryptfs/deadman.json"
}
```

| Field | Type | Description |
|-------|------|-------------|
| `configured` | bool | Whether a deadman config file exists |
| `armed` | bool | Whether the switch is currently armed |
| `config_path` | string | Path to the config file |

---

### POST /api/v1/deadman/arm

Request to arm the deadman switch.

**Request body:** None.

**Response** `200 OK`:
```json
{
  "status": "arm_requested",
  "message": "Use the CLI `tgcryptfs deadman arm` to start the daemon loop"
}
```

The daemon loop must be started via the CLI for actual trigger evaluation.

---

### POST /api/v1/deadman/disarm

Request to disarm the deadman switch.

**Request body:** None.

**Response** `200 OK`:
```json
{
  "status": "disarm_requested",
  "message": "Disarm signal sent. The daemon will stop at the next check interval."
}
```

---

## Data Types

### VolumeSummary

```json
{
  "volume_id": "string (UUID)",
  "display_name": "string",
  "created_at": "integer (Unix timestamp)",
  "mounted": "boolean",
  "mount_point": "string or null",
  "block_count": "integer",
  "total_size": "integer (bytes)"
}
```

### SystemStatus

```json
{
  "version": "string",
  "telegram_connected": "boolean",
  "volumes_mounted": "integer",
  "total_volumes": "integer",
  "cache_entries": "integer",
  "cache_size_bytes": "integer",
  "deadman_armed": "boolean",
  "uptime_secs": "integer"
}
```

### ErrorResponse

```json
{
  "error": "string",
  "code": "string",
  "suggestion": "string"
}
```

---

## Examples

### Create a volume and mount it via the API

```bash
# Create
curl -X POST http://localhost:8080/api/v1/volumes \
  -H "Content-Type: application/json" \
  -d '{"name": "test-vol", "password": "my-password"}'

# Check status
curl http://localhost:8080/api/v1/volumes

# Mount
curl -X POST http://localhost:8080/api/v1/volumes/VOLUME_ID/mount \
  -H "Content-Type: application/json" \
  -d '{"password": "my-password", "mount_point": "/tmp/mnt"}'

# Unmount
curl -X POST http://localhost:8080/api/v1/volumes/VOLUME_ID/unmount

# Delete
curl -X DELETE http://localhost:8080/api/v1/volumes/VOLUME_ID
```

### Share a volume

```bash
# Create share
curl -X POST http://localhost:8080/api/v1/shares \
  -H "Content-Type: application/json" \
  -d '{"volume_id": "VOLUME_ID", "user_id": "alice", "access_level": "read-write"}'

# List shares
curl http://localhost:8080/api/v1/shares/volume/VOLUME_ID

# Revoke
curl -X DELETE http://localhost:8080/api/v1/shares/SHARE_ID
```
