/// Integration tests for the encrypted SQLite store layer.
use rusqlite::Connection;

use tgcryptfs_core::crypto::keys::SymmetricKey;
use tgcryptfs_core::metadata::inode::Inode;
use tgcryptfs_core::metadata::types::FileType;
use tgcryptfs_core::snapshot::entry::SnapshotOperation;
use tgcryptfs_core::snapshot::log;

use tgcryptfs_store::inode_store::InodeStore;
use tgcryptfs_store::migrations::initialize_database;
use tgcryptfs_store::opaque_schema::OpaqueSchema;
use tgcryptfs_store::schema::logical_tables;
use tgcryptfs_store::snapshot_store::SnapshotStore;

fn setup() -> (Connection, OpaqueSchema, SymmetricKey) {
    let conn = Connection::open_in_memory().unwrap();
    let schema_key = SymmetricKey::from_bytes([0x42; 32]);
    let schema = OpaqueSchema::new(schema_key, &logical_tables());
    initialize_database(&conn, &schema).unwrap();
    let meta_key = SymmetricKey::from_bytes([0x99; 32]);
    (conn, schema, meta_key)
}

/// Test: Full inode lifecycle: create → read → update → delete.
#[test]
fn inode_full_lifecycle() {
    let (conn, schema, meta_key) = setup();
    let store = InodeStore::new(&conn, &schema, &meta_key);

    // Create root dir
    let root = Inode::root();
    store.insert(&root).unwrap();

    // Create a file in root
    let mut file = Inode::new_file(2, 1, "secret.txt".into(), 0o644, 1000, 1000);
    file.size = 1024;
    store.insert(&file).unwrap();

    // Read back
    let loaded = store.get(2).unwrap().unwrap();
    assert_eq!(loaded.name, "secret.txt");
    assert_eq!(loaded.size, 1024);
    assert_eq!(loaded.file_type, FileType::RegularFile);

    // Update
    let mut updated = loaded;
    updated.size = 2048;
    store.update(&updated).unwrap();
    let reloaded = store.get(2).unwrap().unwrap();
    assert_eq!(reloaded.size, 2048);

    // List children of root
    let children = store.list_children(1).unwrap();
    assert_eq!(children.len(), 1);
    assert_eq!(children[0].name, "secret.txt");

    // Delete
    store.delete(2).unwrap();
    assert!(store.get(2).unwrap().is_none());
}

/// Test: Inode data is encrypted at rest.
#[test]
fn inode_data_encrypted_at_rest() {
    let (conn, schema, meta_key) = setup();
    let store = InodeStore::new(&conn, &schema, &meta_key);

    let file = Inode::new_file(2, 1, "classified.doc".into(), 0o600, 0, 0);
    store.insert(&file).unwrap();

    let t = schema.table("inodes").unwrap();
    let c_data = schema.column("inodes", "data").unwrap();
    let sql = format!("SELECT {c_data} FROM {t}");
    let raw: Vec<u8> = conn.query_row(&sql, [], |row| row.get(0)).unwrap();

    let raw_str = String::from_utf8_lossy(&raw);
    assert!(
        !raw_str.contains("classified.doc"),
        "filename found in raw data!"
    );
}

/// Test: Wrong key cannot decrypt inode data.
#[test]
fn wrong_key_cannot_decrypt() {
    let (conn, schema, meta_key) = setup();
    let store = InodeStore::new(&conn, &schema, &meta_key);

    let file = Inode::new_file(2, 1, "secret.txt".into(), 0o644, 0, 0);
    store.insert(&file).unwrap();

    let wrong_key = SymmetricKey::from_bytes([0x00; 32]);
    let wrong_store = InodeStore::new(&conn, &schema, &wrong_key);
    let result = wrong_store.get(2);
    assert!(result.is_err(), "should fail to decrypt with wrong key");
}

/// Test: Opaque schema produces non-guessable names.
#[test]
fn opaque_schema_not_guessable() {
    let schema_key = SymmetricKey::from_bytes([0x42; 32]);
    let schema = OpaqueSchema::new(schema_key, &logical_tables());

    let table = schema.table("inodes").unwrap();
    let column = schema.column("inodes", "ino").unwrap();

    assert!(!table.contains("inodes"), "table name should be opaque");
    assert!(!column.contains("ino"), "column name should be opaque");
}

/// Test: Snapshot entries for file versioning.
#[test]
fn snapshot_versioning() {
    let (conn, schema, meta_key) = setup();
    let inode_store = InodeStore::new(&conn, &schema, &meta_key);
    let snap_store = SnapshotStore::new(&conn, &schema, &meta_key);

    let file = Inode::new_file(2, 1, "doc.txt".into(), 0o644, 0, 0);
    inode_store.insert(&file).unwrap();

    // Capture snapshot (create)
    let state = log::capture_state(&file);
    let entry1 = log::create_entry(1, SnapshotOperation::Create, 2, None, Some(state), None);
    snap_store.append(&entry1).unwrap();

    // Modify the file
    let mut file2 = file.clone();
    file2.size = 5000;
    inode_store.update(&file2).unwrap();

    let before = log::capture_state(&file);
    let after = log::capture_state(&file2);
    let entry2 = log::create_entry(
        2,
        SnapshotOperation::Write,
        2,
        Some(before),
        Some(after),
        None,
    );
    snap_store.append(&entry2).unwrap();

    let snapshots = snap_store.list_for_inode(2).unwrap();
    assert_eq!(snapshots.len(), 2);
}

/// Test: Directory hierarchy with nested children.
#[test]
fn directory_hierarchy() {
    let (conn, schema, meta_key) = setup();
    let store = InodeStore::new(&conn, &schema, &meta_key);

    store.insert(&Inode::root()).unwrap();
    store
        .insert(&Inode::new_dir(2, 1, "documents".into(), 0o755, 0, 0))
        .unwrap();
    store
        .insert(&Inode::new_file(3, 2, "secret.txt".into(), 0o644, 0, 0))
        .unwrap();
    store
        .insert(&Inode::new_file(4, 2, "notes.txt".into(), 0o644, 0, 0))
        .unwrap();
    store
        .insert(&Inode::new_dir(5, 1, "photos".into(), 0o755, 0, 0))
        .unwrap();

    // Root has 2 children
    let root_children = store.list_children(1).unwrap();
    assert_eq!(root_children.len(), 2);

    // /documents has 2 children
    let doc_children = store.list_children(2).unwrap();
    assert_eq!(doc_children.len(), 2);

    // /photos has 0 children
    let photo_children = store.list_children(5).unwrap();
    assert_eq!(photo_children.len(), 0);

    // Lookup by name
    let found = store.lookup(2, "secret.txt").unwrap();
    assert!(found.is_some());
    assert_eq!(found.unwrap().ino, 3);
}
