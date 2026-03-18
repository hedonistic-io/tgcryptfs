# TGCryptFS Twitter/X Launch Thread

---

**1/**
I built an encrypted filesystem that uses Telegram as cloud storage.

Mount it like a normal folder. Files get split, encrypted with post-quantum crypto, and stored as opaque blobs in your Telegram account.

Open source, Rust, MIT licensed.

https://github.com/hedonistic-io/tgcryptfs

---

**2/**
Why Telegram? You already have an account. Storage is free. And you control it -- there's no third-party service between you and your data.

Telegram sees encrypted blobs. They can't tell if it's a photo, a document, or random noise.

---

**3/**
The crypto: XChaCha20-Poly1305 for symmetric encryption (192-bit nonces, no nonce management headaches). ML-KEM-768 for post-quantum key exchange when sharing volumes with other users.

If quantum computers arrive, your captured ciphertext stays safe.

---

**4/**
Every file block, metadata record, and policy object gets AAD-bound encryption. An attacker who controls Telegram's storage can't swap blocks between contexts without authentication failure.

Your inode ciphertext only decrypts as an inode.

---

**5/**
The local metadata database uses BLAKE3-hashed table and column names. Without the schema key, the SQLite file is structurally opaque -- you can't even tell what kinds of data are stored, let alone read them.

---

**6/**
Dead man's switch: set a check-in interval. If you don't check in, the system triggers automatic data destruction. For when you need assurance that data doesn't outlive your ability to manage it.

---

**7/**
Your encryption keys can be backed up as a 22-word sentence (like a crypto seed phrase). Four rotating 4096-word lists, 12 bits per word. Write it on paper, put it in a safe. No digital trail.

---

**8/**
9 Rust crates, 471 tests, v0.1.0. The crypto layer has zero I/O dependencies so every round-trip path is property-tested.

It works, it's tested, and it's early. No GUI yet. Contributions welcome.

cargo install tgcryptfs-cli

https://tgcryptfs.hedonistic.io

---

(c) 2026 Hedonistic, LLC
