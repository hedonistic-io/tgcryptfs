# Security Design

## Threat Model

TGCryptFS protects against:

1. **Server-side compromise**: Telegram cannot read file contents (encrypted before upload)
2. **Database theft**: SQLite files are encrypted with opaque schema - no useful information without the key
3. **Block reuse attacks**: AAD binding prevents cross-context block substitution
4. **Password guessing**: Argon2id with 64MB memory cost and 3 iterations
5. **Quantum computing**: ML-KEM-768 for post-quantum key exchange
6. **Key compromise propagation**: Separate keys for data, metadata, schema, integrity, wrapping, and deadman
7. **Physical seizure**: Deadman option with configurable destruction triggers

TGCryptFS does NOT protect against:

1. **Active endpoint compromise**: A compromised device with the password can read all data
2. **Rubber-hose cryptanalysis**: The password holder can always decrypt
3. **Traffic analysis**: Telegram can observe upload/download patterns and block sizes
4. **Telegram account seizure**: If the Telegram account is compromised, encrypted blocks can be deleted

## Cryptographic Primitives

| Primitive | Algorithm | Purpose |
|-----------|-----------|---------|
| AEAD | XChaCha20-Poly1305 | All data encryption |
| KDF (password) | Argon2id | Password to root key |
| KDF (hierarchy) | HKDF-SHA256 | Root key to sub-keys |
| Hash | BLAKE3 | Content addressing, integrity |
| KEM | ML-KEM-768 | Post-quantum key exchange |
| MAC | BLAKE3 keyed | Integrity verification |

## Key Material Lifecycle

### Derivation
```
Password (user input, never stored)
  + Salt (random 32 bytes, stored in volume config)
  |
  v [Argon2id: 64MB memory, 3 iterations, 4 threads]
  |
Root Key (32 bytes)
  |
  +-- HKDF(root, "SentenceRefs.v1:data")    -> Kdata
  +-- HKDF(root, "SentenceRefs.v1:meta")    -> Kmeta
  +-- HKDF(root, "SentenceRefs.v1:schema")  -> Kschema
  +-- HKDF(root, "SentenceRefs.v1:ih")      -> Kih
  +-- HKDF(root, "SentenceRefs.v1:wrapping")-> Kwrap
  +-- HKDF(root, "SentenceRefs.v1:deadman") -> Kdeadman
```

### Zeroization

All key material is wrapped in `zeroize::Zeroize` + `ZeroizeOnDrop`:
- Keys are zeroed when dropped
- No key material is serialized to disk (only the salt and KDF params are stored)
- Temporary key buffers use stack-allocated `[u8; 32]` arrays

### Epoch Keys

For forward secrecy, data encryption uses epoch-scoped keys:
```
Kdata + epoch_number -> HKDF -> Kepoch
```

When rotating to a new epoch:
1. Derive new epoch key
2. Re-encrypt all blocks with new key
3. Zero previous epoch key
4. Previous data cannot be decrypted even with the root key if the epoch key is destroyed

## AAD Binding

Every AEAD encryption includes Additional Authenticated Data (AAD) that binds the ciphertext to its context:

| Context | AAD Format |
|---------|------------|
| File block | `inode:{ino}:block:{offset}` |
| Inode metadata | `inode:{ino}` |
| Policy | `policy:{pid}` |
| Volume config | `volume:{volume_id}` |
| Snapshot | `snapshot:{snapshot_id}` |
| User record | `user:{uid}` |
| Wrapped key | `share:{volume_id}:{invite_id}` |

This prevents:
- Moving a block from one inode to another
- Swapping metadata between inodes
- Replaying encrypted data in a different context

## Opaque Schema

SQLite table and column names are derived deterministically:
```
opaque_name = hex(BLAKE3(Kschema || domain || ":" || logical_name))
```

Example:
- Logical: `inodes.ino` -> Opaque: `a7f3c2...4e.b91d0e...82`
- Without Kschema, the schema structure is indistinguishable from random

## Multi-User Key Exchange

ML-KEM-768 (Module Lattice Key Encapsulation Mechanism):
1. NIST post-quantum standard (FIPS 203)
2. 768-dimensional lattice provides 128-bit security level
3. Resistant to Shor's algorithm on quantum computers

Flow:
```
Recipient: generate(ML-KEM-768) -> (dk, ek)
           send ek to Owner

Owner:     encapsulate(ek) -> (shared_secret, ciphertext)
           encrypt(shared_secret, data_key) -> wrapped_key
           send (ciphertext, wrapped_key) to Recipient

Recipient: decapsulate(dk, ciphertext) -> shared_secret
           decrypt(shared_secret, wrapped_key) -> data_key
```
