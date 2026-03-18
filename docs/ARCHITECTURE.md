# TGCryptFS v2 Architecture

## System Overview

TGCryptFS v2 is a 9-crate Rust workspace that implements an encrypted filesystem backed by Telegram's cloud storage. The system separates concerns into distinct layers:

```
+--------------------+
|   tgcryptfs-cli    |  User-facing CLI
+--------------------+
         |
+--------------------+
|   tgcryptfs-api    |  Service orchestration
+--------------------+
         |
+--------+--------+--------+--------+--------+--------+
| core   | store  | tg     | cache  | fuse   | share  | deadman
+--------+--------+--------+--------+--------+--------+
```

## Crate Responsibilities

### tgcryptfs-core
The foundation crate with zero external service dependencies.

- **crypto/**: XChaCha20-Poly1305 AEAD, BLAKE3 hashing, HKDF-SHA256 key derivation, Argon2id password hashing, ML-KEM-768 post-quantum key exchange
- **block/**: Content-defined chunking (CDC), SRB1 block format, LZ4/Zstd compression, content-addressable dedup
- **metadata/**: Inode structures, file types, timestamps
- **policy/**: Path-based mutability policies with glob matching
- **sentence/**: 22-word mnemonic encoding for 256-bit keys (4 x 4096-word lists, 12 bits per word)
- **snapshot/**: Point-in-time inode state capture for versioning
- **volume/**: Volume configuration, lifecycle management, name generation

### tgcryptfs-store
Encrypted SQLite persistence layer.

- **OpaqueSchema**: Derives all table/column/index names from Kschema via BLAKE3
- **InodeStore**: CRUD for encrypted inode metadata
- **BlockStore**: Content-addressable block records with reference counting
- **SnapshotStore**: Encrypted snapshot entries with timestamp indexing
- **PolicyStore**: Mutability policy storage
- **UserStore**: User record management for shared volumes
- **VolumeStore**: Encrypted volume configuration persistence
- **DeadmanStore**: Deadman switch state persistence

### tgcryptfs-telegram
Telegram MTProto transport layer.

- **BlockTransport trait**: Async interface for upload/download/delete
- **TelegramClient**: Production implementation using grammers-client 0.6
  - Session management (file-based persistence)
  - Upload via `upload_stream()` + `send_message()` to Saved Messages
  - Download via `iter_messages()` + `iter_download()`
  - Delete via `delete_messages()`
  - Retry with exponential backoff, concurrency limiting via semaphores
- **MockTransport**: In-memory mock for testing

### tgcryptfs-cache
Disk-persistent encrypted block cache.

- LRU eviction with configurable size limit
- Optional AEAD encryption of cached blocks at rest
- Subdirectory sharding for filesystem performance
- Cache statistics tracking

### tgcryptfs-fuse
FUSE filesystem implementation using the `fuser` crate.

- **CryptFs**: Implements `fuser::Filesystem` with all standard operations
- **HandleTable**: File handle allocation and tracking
- Operations: lookup, getattr, setattr, mkdir, create, open, release, read, write, unlink, rmdir, readdir, rename, readlink, symlink, statfs

### tgcryptfs-sharing
Multi-user volume sharing.

- **AccessLevel**: ReadOnly, ReadWrite, Admin permission hierarchy
- **Invite**: Time-limited, use-limited invite tokens with revocation
- **InviteCode**: Base64url-encoded invite codes for out-of-band sharing
- **Key exchange**: ML-KEM-768 encapsulation + AEAD key wrapping

### tgcryptfs-deadman
Deadman switch subsystem.

- **TriggerEvaluator**: Evaluates 5 trigger types (heartbeat, network, OS, RPC, custom)
- **DeadmanController**: Arm/disarm state machine with check scheduling
- **DestructionExecutor**: Multi-phase data destruction (shred files, wipe dirs, custom commands)

### tgcryptfs-api
Service orchestration and REST API layer.

- **VolumeService**: Create/open/list/delete volumes through the full stack
- **AuthService**: Telegram session management
- **SystemService**: System status and uptime tracking
- **REST Server**: axum 0.7-based HTTP API with 17 endpoints (see API Server section below)
- **Error Handling**: All errors implement `suggestion()` and API errors implement `IntoResponse` with structured JSON

### tgcryptfs-cli
User-facing binary with clap-derived command structure. 8 subcommands: `auth`, `volume`, `key`, `status`, `sharing`, `deadman`, `configure`, `serve`.

## Data Flow

### Write Path
```
User writes to mounted file
  -> FUSE intercepts write()
  -> Data buffered in HandleTable
  -> On flush/close: CDC chunks data into blocks
  -> Each block: compress (LZ4/Zstd) -> SRB1 encode (AEAD encrypt) -> BLAKE3 hash
  -> Dedup check: if hash exists, increment ref count
  -> Otherwise: upload to Telegram via BlockTransport
  -> Update inode manifest in InodeStore
  -> Cache block locally in BlockCache
```

### Read Path
```
User reads from mounted file
  -> FUSE intercepts read()
  -> Check BlockCache for cached block
  -> If miss: download from Telegram via BlockTransport
  -> SRB1 decode (AEAD decrypt) -> decompress
  -> Cache decrypted block for future reads
  -> Return data to user
```

### Volume Creation
```
tgcryptfs volume create
  -> Generate VolumeConfig (UUID, salt, KDF params)
  -> Derive root key: Argon2id(password, salt)
  -> Derive hierarchy: HKDF(root, "data"|"meta"|"schema"|...)
  -> Create volume directory structure
  -> Save config JSON (no keys!) to disk
  -> Initialize SQLite with opaque schema
  -> Display sentence reference for backup
```

## API Server Architecture

The REST API is served by the `tgcryptfs serve` CLI subcommand using axum 0.7.

### Components

```
tgcryptfs serve --bind 127.0.0.1:8080
  |
  +-- AppState (Arc<AppStateInner>)
  |     +-- SystemService
  |     +-- VolumeService
  |     +-- AuthService
  |
  +-- Middleware Stack
  |     +-- CorsLayer::permissive()
  |     +-- TraceLayer (tower-http)
  |     +-- request_logger (custom)
  |
  +-- Router (/api/v1)
        +-- /status, /version        -> system handlers
        +-- /auth/*                   -> auth handlers
        +-- /volumes/*                -> volume handlers
        +-- /shares/*                 -> sharing handlers
        +-- /deadman/*                -> deadman handlers
```

### Request Flow

1. Request enters via axum listener
2. CORS layer applies permissive headers
3. TraceLayer logs request/response at tower-http level
4. Custom `request_logger` middleware logs method, path, and timing
5. Router dispatches to handler based on method + path
6. Handler extracts `State<AppState>` and request body/params
7. Handler delegates to the appropriate service layer
8. Errors are converted via `IntoResponse` to structured JSON

### Error Handling

Every error type across all 7 crates implements a `suggestion()` method that returns a user-facing recovery hint. The API layer's `ApiError` additionally implements:

- `status_code()` — maps to HTTP status (400, 401, 404, 409, 500, 503)
- `error_code()` — machine-readable code string (e.g. `"VOLUME_NOT_FOUND"`)
- `IntoResponse` — serializes to `{ error, code, suggestion }` JSON

CLI commands use helper functions (`core_err`, `telegram_err`, `api_err`, etc.) that format errors with their suggestions for terminal display.

## Security Properties

1. **Confidentiality**: All data encrypted with XChaCha20-Poly1305 before leaving the device
2. **Integrity**: Every block includes AEAD authentication tags; BLAKE3 content hashes verify dedup
3. **Authenticity**: AAD binding prevents block reuse across contexts (different inodes, volumes)
4. **Forward secrecy**: Epoch-based key rotation ensures old keys can be destroyed
5. **Post-quantum**: ML-KEM-768 key exchange resistant to quantum computing attacks
6. **Schema privacy**: Database structure reveals no information without the key
7. **Key separation**: HKDF ensures compromise of one key doesn't compromise others
