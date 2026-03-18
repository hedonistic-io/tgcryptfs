# SentenceRefs v1 --- 256-bit Block Reference Specification

## Status

Normative specification suitable for implementation.

------------------------------------------------------------------------

# 1. Overview

SentenceRefs v1 defines:

-   A 256-bit block reference core
-   Optional immutability with atomic mutability via reference cells
-   A deterministic English-slang sentence encoding of references
-   Post-quantum--ready transport compatibility
-   Migration-safe versioning
-   Full validation and canonicalization rules

This specification is implementation-grade and deterministic.

------------------------------------------------------------------------

# 2. Cryptographic Architecture

## 2.1 Required Primitives

-   CSPRNG
-   AEAD: XChaCha20-Poly1305 (preferred) or AES-256-GCM-SIV
-   Keyed hash: BLAKE3 keyed mode (preferred) or HMAC-SHA-256
-   HKDF (SHA-256 or BLAKE3-based derivation)

## 2.2 Key Derivation

Given 256-bit `Kroot`:

Kdata = HKDF(Kroot, "SentenceRefs.v1:data")\
Kmeta = HKDF(Kroot, "SentenceRefs.v1:meta")\
Kih = HKDF(Kroot, "SentenceRefs.v1:ih")\
Kwrap = HKDF(Kroot, "SentenceRefs.v1:wrapping")

Epoch derivation:

Kdata_epoch = HKDF(Kdata, "epoch:" \|\| u32be(epoch))\
Kih_epoch = HKDF(Kih, "epoch:" \|\| u32be(epoch))

------------------------------------------------------------------------

# 3. Block Record Format (SRB1)

Binary layout:

magic\[4\] = "SRB1"\
version\[1\] = 0x01\
epoch\[4\] = u32be\
flags\[2\]\
nonce\[24\]\
aadLen\[4\]\
aad\[aadLen\]\
ctLen\[8\]\
ciphertext\[ctLen\]\
ih\[32\]

Integrity Hash:

IH = KeyedHash(Kih_epoch, headerAndBody)

Where headerAndBody includes everything except ih.

RID = random(32 bytes)

------------------------------------------------------------------------

# 4. Optional Immutability Model

Blocks are immutable once written.

Mutability is achieved via RefCells:

RefCellV1:

magic "SRC1"\
version 0x01\
epoch u32\
currentRid\[32\]\
currentIh\[32\]\
policyId u32\
scriptHash\[32\]

Atomic primitive:

CompareAndSwap(cellId, expectedRef, newRef)

Policy hook:

canUpdate(cellId, oldRef, newRef, context)

------------------------------------------------------------------------

# 5. Sentence Encoding (256-bit → English Sentence)

## 5.1 Wordlists

Four wordlists, each size exactly 4096:

Adj\[4096\]\
Noun\[4096\]\
Verb\[4096\]\
Adv\[4096\]

Each contributes 12 bits.

## 5.2 Check Byte

check8 = first_byte(BLAKE3("SentenceRefs.v1:check" \|\| X))

Total bits encoded = 256 + 8 = 264 bits

Split into 22 chunks of 12 bits.

## 5.3 Template

Canonical template:

{A0} {N0} {V0} {D0}, while {A1} {N1} {V1} {D1}; beneath {A2} {N2}, {V2}
{N3} {D2}; and {A3} {N4} {V3} {N5} {D3}. {A4} {N6} {V4} {D4}.

Lowercase only. Exact punctuation. Single spaces.

Prefix:

sr1.e{epoch_base32}:

------------------------------------------------------------------------

# 6. Reference Validation

Validation steps:

1.  Parse prefix sr1.eXXXXXXX:
2.  Decode epoch via Crockford Base32.
3.  Enforce canonical lowercase + spacing.
4.  Extract 22 content words.
5.  Validate word membership in correct list.
6.  Reconstruct 264-bit stream.
7.  Split into X (256 bits) + check8.
8.  Recompute check8 and verify match.

Errors:

ERR_PREFIX\
ERR_EPOCH_DECODE\
ERR_TEMPLATE_MISMATCH\
ERR_UNKNOWN_WORD\
ERR_CHECKSUM

------------------------------------------------------------------------

# 7. Normative Appendix A --- Crockford Base32

Alphabet:

0123456789ABCDEFGHJKMNPQRSTVWXYZ

Rules:

-   Case-insensitive during decode.
-   No padding.
-   Reject characters I, L, O, U.
-   Fixed-width 7-character encoding recommended for epoch (32-bit).
-   Big-endian integer encoding.

Encoding:

value → base32 string without padding.

Decoding:

base32 string → integer; reject invalid characters.

------------------------------------------------------------------------

# 8. Normative Appendix B --- BLAKE3 Input Rules

Domain separation MUST be explicit.

For check byte:

BLAKE3( "SentenceRefs.v1:check" \|\| X )

For integrity hash:

BLAKE3_keyed( Kih_epoch, headerAndBody )

Concatenation is raw byte concatenation.

Strings MUST be UTF-8 encoded without null termination.

All domain labels MUST match exactly (case-sensitive).

------------------------------------------------------------------------

# 9. Normative Appendix C --- Recommended AAD Schema

AAD structure (CBOR or TLV recommended):

{ "blockType": u8, \# 0=data, 1=metadata "inode": u64, \# logical inode
ID "logicalOffset": u64, \# optional "policyId": u32, \# mutability
policy binding "timestamp": u64, \# optional "flags": u16 \# optional }

Purpose:

-   Cryptographically bind block to logical role
-   Prevent cross-object substitution
-   Prove "this block is metadata type X for inode Y"

AAD MUST be serialized deterministically.

------------------------------------------------------------------------

# 10. Post-Quantum Transport Guidance

Data at rest:

Use 256-bit symmetric keys (AES-256 or XChaCha20).

Transport:

Use TLS 1.3 hybrid key exchange:

ECDHE + ML-KEM (e.g., X25519MLKEM768)

Key sharing:

Wrap volume keys using ML-KEM when distributing to peers.

------------------------------------------------------------------------

# 11. Migration Strategy

Version prefix sr1 allows future sr2 evolution.

On-disk record magic and version allow cryptographic upgrades.

Epoch supports key rotation without invalidating old data.

------------------------------------------------------------------------

# End of Specification
