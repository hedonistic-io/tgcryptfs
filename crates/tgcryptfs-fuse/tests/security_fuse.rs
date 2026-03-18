//! Security tests for the FUSE layer.
//!
//! Validates that wrong keys, tampered data, and invalid states
//! are handled safely without panics or data leaks.

use std::sync::Arc;

use rusqlite::Connection;

use tgcryptfs_core::crypto::keys::SymmetricKey;
use tgcryptfs_core::metadata::inode::Inode;
use tgcryptfs_fuse::fs::CryptFs;
use tgcryptfs_store::inode_store::InodeStore;
use tgcryptfs_store::migrations::initialize_database;
use tgcryptfs_store::opaque_schema::OpaqueSchema;
use tgcryptfs_store::schema::logical_tables;
use tgcryptfs_telegram::mock::MockTransport;

fn setup_db_with_keys(
    schema_key: [u8; 32],
    meta_key: [u8; 32],
) -> (Connection, OpaqueSchema, SymmetricKey) {
    let conn = Connection::open_in_memory().unwrap();
    let sk = SymmetricKey::from_bytes(schema_key);
    let mk = SymmetricKey::from_bytes(meta_key);
    let schema = OpaqueSchema::new(sk, &logical_tables());
    initialize_database(&conn, &schema).unwrap();
    (conn, schema, mk)
}

// ---- Wrong meta_key cannot decrypt inodes ----

#[test]
fn wrong_meta_key_cannot_read_inodes() {
    // Create a database and store an inode with key A
    let conn = Connection::open_in_memory().unwrap();
    let schema_key = SymmetricKey::from_bytes([0x42; 32]);
    let meta_key_a = SymmetricKey::from_bytes([0x43; 32]);
    let schema = OpaqueSchema::new(schema_key, &logical_tables());
    initialize_database(&conn, &schema).unwrap();

    let store_a = InodeStore::new(&conn, &schema, &meta_key_a);
    store_a.insert(&Inode::root()).unwrap();

    let ino = store_a.next_ino().unwrap();
    let file = Inode::new_file(ino, 1, "secret.txt".into(), 0o644, 1000, 1000);
    store_a.insert(&file).unwrap();

    // Verify it can be read with correct key
    assert!(store_a.get(ino).unwrap().is_some());

    // Try to read with wrong meta key
    let meta_key_b = SymmetricKey::from_bytes([0xFF; 32]);
    let store_b = InodeStore::new(&conn, &schema, &meta_key_b);

    // Wrong key → decryption fails, returns None or Err
    let result = store_b.get(ino);
    assert!(
        result.is_err() || result.unwrap().is_none(),
        "wrong key should not decrypt inode"
    );
}

// ---- Disconnected transport returns None ----

#[test]
fn disconnected_transport_download_returns_none() {
    let conn = Connection::open_in_memory().unwrap();
    let schema_key = SymmetricKey::from_bytes([0x42; 32]);
    let meta_key = SymmetricKey::from_bytes([0x43; 32]);
    let data_key = SymmetricKey::from_bytes([0x44; 32]);
    let schema = OpaqueSchema::new(schema_key, &logical_tables());
    initialize_database(&conn, &schema).unwrap();

    let transport = Arc::new(MockTransport::new());
    let rt = tokio::runtime::Runtime::new().unwrap();

    let fs = CryptFs::new(conn, schema, meta_key, data_key, 1000, 1000)
        .with_transport(transport.clone())
        .with_runtime(rt.handle().clone());

    // Upload a block
    let data = vec![0xAA; 256];
    let block = fs.flush_to_transport(42, &data).unwrap();

    // Disconnect transport
    transport.set_connected(false);

    // Download without cache → None (not a panic)
    let result = fs.download_block(block.message_id, &block.rid);
    assert!(result.is_none());
}

// ---- No transport → flush returns None ----

#[test]
fn no_transport_flush_returns_none() {
    let conn = Connection::open_in_memory().unwrap();
    let schema_key = SymmetricKey::from_bytes([0x42; 32]);
    let meta_key = SymmetricKey::from_bytes([0x43; 32]);
    let data_key = SymmetricKey::from_bytes([0x44; 32]);
    let schema = OpaqueSchema::new(schema_key, &logical_tables());
    initialize_database(&conn, &schema).unwrap();

    // No transport, no runtime
    let fs = CryptFs::new(conn, schema, meta_key, data_key, 1000, 1000);

    let result = fs.flush_to_transport(42, &[0xBB; 100]);
    assert!(result.is_none());
}

// ---- Download nonexistent block returns None ----

#[test]
fn download_nonexistent_block_returns_none() {
    let conn = Connection::open_in_memory().unwrap();
    let schema_key = SymmetricKey::from_bytes([0x42; 32]);
    let meta_key = SymmetricKey::from_bytes([0x43; 32]);
    let data_key = SymmetricKey::from_bytes([0x44; 32]);
    let schema = OpaqueSchema::new(schema_key, &logical_tables());
    initialize_database(&conn, &schema).unwrap();

    let transport = Arc::new(MockTransport::new());
    let rt = tokio::runtime::Runtime::new().unwrap();

    let fs = CryptFs::new(conn, schema, meta_key, data_key, 1000, 1000)
        .with_transport(transport.clone())
        .with_runtime(rt.handle().clone());

    // Try to download a message ID that doesn't exist
    let fake_rid = [0xFF; 32];
    let result = fs.download_block(99999, &fake_rid);
    assert!(result.is_none());
}

// ---- Handle operations on invalid fh ----

#[test]
fn invalid_handle_operations_safe() {
    let conn = Connection::open_in_memory().unwrap();
    let schema_key = SymmetricKey::from_bytes([0x42; 32]);
    let meta_key = SymmetricKey::from_bytes([0x43; 32]);
    let data_key = SymmetricKey::from_bytes([0x44; 32]);
    let schema = OpaqueSchema::new(schema_key, &logical_tables());
    initialize_database(&conn, &schema).unwrap();

    let fs = CryptFs::new(conn, schema, meta_key, data_key, 1000, 1000);

    // Get with invalid handle
    assert!(fs.handles.get(99999).is_none());

    // Update with invalid handle
    let updated = fs.handles.update(99999, |_h| {});
    assert!(!updated);

    // Close with invalid handle
    assert!(fs.handles.close(99999).is_none());

    // Count should be 0
    assert_eq!(fs.handles.count(), 0);
}

// ---- Inode store: duplicate ino insert ----

#[test]
fn duplicate_inode_insert_fails() {
    let (conn, schema, meta_key) = setup_db_with_keys([0x42; 32], [0x43; 32]);
    let store = InodeStore::new(&conn, &schema, &meta_key);

    store.insert(&Inode::root()).unwrap();

    // Insert a second root with same ino=1 → should fail
    let duplicate = Inode::root();
    let result = store.insert(&duplicate);
    assert!(result.is_err(), "duplicate ino insert should fail");
}

// ---- Inode store: get nonexistent returns None ----

#[test]
fn get_nonexistent_inode_returns_none() {
    let (conn, schema, meta_key) = setup_db_with_keys([0x42; 32], [0x43; 32]);
    let store = InodeStore::new(&conn, &schema, &meta_key);

    let result = store.get(999999).unwrap();
    assert!(result.is_none());
}

// ---- Inode store: lookup in empty dir returns None ----

#[test]
fn lookup_in_empty_dir_returns_none() {
    let (conn, schema, meta_key) = setup_db_with_keys([0x42; 32], [0x43; 32]);
    let store = InodeStore::new(&conn, &schema, &meta_key);

    store.insert(&Inode::root()).unwrap();

    let result = store.lookup(1, "nonexistent.txt").unwrap();
    assert!(result.is_none());
}

// ---- Inode store: delete then get returns None ----

#[test]
fn delete_then_get_returns_none() {
    let (conn, schema, meta_key) = setup_db_with_keys([0x42; 32], [0x43; 32]);
    let store = InodeStore::new(&conn, &schema, &meta_key);

    store.insert(&Inode::root()).unwrap();

    let ino = store.next_ino().unwrap();
    store
        .insert(&Inode::new_file(
            ino,
            1,
            "temp.txt".into(),
            0o644,
            1000,
            1000,
        ))
        .unwrap();

    store.delete(ino).unwrap();
    assert!(store.get(ino).unwrap().is_none());
}
