use rusqlite::{params, Connection};

use tgcryptfs_core::crypto::{aead, blake3 as b3, keys::SymmetricKey};
use tgcryptfs_core::metadata::inode::Inode;

use crate::opaque_schema::OpaqueSchema;

/// Encrypted inode storage backed by SQLite with opaque schema.
pub struct InodeStore<'a> {
    conn: &'a Connection,
    schema: &'a OpaqueSchema,
    meta_key: &'a SymmetricKey,
}

impl<'a> InodeStore<'a> {
    pub fn new(conn: &'a Connection, schema: &'a OpaqueSchema, meta_key: &'a SymmetricKey) -> Self {
        Self {
            conn,
            schema,
            meta_key,
        }
    }

    /// Insert a new inode. The inode data is serialized and encrypted.
    pub fn insert(&self, inode: &Inode) -> rusqlite::Result<()> {
        let t = self.schema.require_table("inodes")?;
        let c_ino = self.schema.require_column("inodes", "ino")?;
        let c_parent = self.schema.require_column("inodes", "parent")?;
        let c_nh = self.schema.require_column("inodes", "name_hash")?;
        let c_data = self.schema.require_column("inodes", "data")?;
        let c_ver = self.schema.require_column("inodes", "version")?;

        let name_hash = b3::hash(inode.name.as_bytes());
        let serialized = postcard::to_allocvec(inode).map_err(|e| {
            rusqlite::Error::ToSqlConversionFailure(Box::new(std::io::Error::other(e.to_string())))
        })?;
        let aad = format!("inode:{}", inode.ino);
        let encrypted = aead::encrypt(self.meta_key, &serialized, aad.as_bytes()).map_err(|e| {
            rusqlite::Error::ToSqlConversionFailure(Box::new(std::io::Error::other(e.to_string())))
        })?;

        let sql = format!(
            "INSERT INTO {t} ({c_ino}, {c_parent}, {c_nh}, {c_data}, {c_ver}) VALUES (?1, ?2, ?3, ?4, ?5)"
        );
        self.conn.execute(
            &sql,
            params![
                inode.ino as i64,
                inode.parent as i64,
                name_hash.as_slice(),
                encrypted,
                0i64
            ],
        )?;
        Ok(())
    }

    /// Get an inode by its inode number.
    pub fn get(&self, ino: u64) -> rusqlite::Result<Option<Inode>> {
        let t = self.schema.require_table("inodes")?;
        let c_ino = self.schema.require_column("inodes", "ino")?;
        let c_data = self.schema.require_column("inodes", "data")?;

        let sql = format!("SELECT {c_data} FROM {t} WHERE {c_ino} = ?1");
        let mut stmt = self.conn.prepare(&sql)?;
        let mut rows = stmt.query(params![ino as i64])?;

        match rows.next()? {
            Some(row) => {
                let encrypted: Vec<u8> = row.get(0)?;
                let aad = format!("inode:{ino}");
                let decrypted =
                    aead::decrypt(self.meta_key, &encrypted, aad.as_bytes()).map_err(|e| {
                        rusqlite::Error::FromSqlConversionFailure(
                            0,
                            rusqlite::types::Type::Blob,
                            Box::new(std::io::Error::other(e.to_string())),
                        )
                    })?;
                let inode: Inode = postcard::from_bytes(&decrypted).map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        0,
                        rusqlite::types::Type::Blob,
                        Box::new(std::io::Error::other(e.to_string())),
                    )
                })?;
                Ok(Some(inode))
            }
            None => Ok(None),
        }
    }

    /// List children of a directory by parent inode number.
    pub fn list_children(&self, parent_ino: u64) -> rusqlite::Result<Vec<Inode>> {
        let t = self.schema.require_table("inodes")?;
        let c_parent = self.schema.require_column("inodes", "parent")?;
        let c_ino = self.schema.require_column("inodes", "ino")?;
        let c_data = self.schema.require_column("inodes", "data")?;

        let sql = format!("SELECT {c_ino}, {c_data} FROM {t} WHERE {c_parent} = ?1");
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map(params![parent_ino as i64], |row| {
            let ino: i64 = row.get(0)?;
            let encrypted: Vec<u8> = row.get(1)?;
            Ok((ino as u64, encrypted))
        })?;

        let mut inodes = Vec::new();
        for row in rows {
            let (ino, encrypted) = row?;
            let aad = format!("inode:{ino}");
            let decrypted =
                aead::decrypt(self.meta_key, &encrypted, aad.as_bytes()).map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        0,
                        rusqlite::types::Type::Blob,
                        Box::new(std::io::Error::other(e.to_string())),
                    )
                })?;
            let inode: Inode = postcard::from_bytes(&decrypted).map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    0,
                    rusqlite::types::Type::Blob,
                    Box::new(std::io::Error::other(e.to_string())),
                )
            })?;
            inodes.push(inode);
        }
        Ok(inodes)
    }

    /// Look up a child inode by parent and filename.
    pub fn lookup(&self, parent_ino: u64, name: &str) -> rusqlite::Result<Option<Inode>> {
        let t = self.schema.require_table("inodes")?;
        let c_parent = self.schema.require_column("inodes", "parent")?;
        let c_nh = self.schema.require_column("inodes", "name_hash")?;
        let c_ino = self.schema.require_column("inodes", "ino")?;
        let c_data = self.schema.require_column("inodes", "data")?;

        let name_hash = b3::hash(name.as_bytes());
        let sql =
            format!("SELECT {c_ino}, {c_data} FROM {t} WHERE {c_parent} = ?1 AND {c_nh} = ?2");
        let mut stmt = self.conn.prepare(&sql)?;
        let mut rows = stmt.query(params![parent_ino as i64, name_hash.as_slice()])?;

        match rows.next()? {
            Some(row) => {
                let ino: i64 = row.get(0)?;
                let encrypted: Vec<u8> = row.get(1)?;
                let aad = format!("inode:{ino}");
                let decrypted =
                    aead::decrypt(self.meta_key, &encrypted, aad.as_bytes()).map_err(|e| {
                        rusqlite::Error::FromSqlConversionFailure(
                            0,
                            rusqlite::types::Type::Blob,
                            Box::new(std::io::Error::other(e.to_string())),
                        )
                    })?;
                let inode: Inode = postcard::from_bytes(&decrypted).map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        0,
                        rusqlite::types::Type::Blob,
                        Box::new(std::io::Error::other(e.to_string())),
                    )
                })?;
                Ok(Some(inode))
            }
            None => Ok(None),
        }
    }

    /// Update an inode (re-encrypt with new data).
    pub fn update(&self, inode: &Inode) -> rusqlite::Result<()> {
        let t = self.schema.require_table("inodes")?;
        let c_ino = self.schema.require_column("inodes", "ino")?;
        let c_parent = self.schema.require_column("inodes", "parent")?;
        let c_nh = self.schema.require_column("inodes", "name_hash")?;
        let c_data = self.schema.require_column("inodes", "data")?;
        let c_ver = self.schema.require_column("inodes", "version")?;

        let name_hash = b3::hash(inode.name.as_bytes());
        let serialized = postcard::to_allocvec(inode).map_err(|e| {
            rusqlite::Error::ToSqlConversionFailure(Box::new(std::io::Error::other(e.to_string())))
        })?;
        let aad = format!("inode:{}", inode.ino);
        let encrypted = aead::encrypt(self.meta_key, &serialized, aad.as_bytes()).map_err(|e| {
            rusqlite::Error::ToSqlConversionFailure(Box::new(std::io::Error::other(e.to_string())))
        })?;

        let sql = format!(
            "UPDATE {t} SET {c_parent} = ?1, {c_nh} = ?2, {c_data} = ?3, {c_ver} = {c_ver} + 1 WHERE {c_ino} = ?4"
        );
        self.conn.execute(
            &sql,
            params![
                inode.parent as i64,
                name_hash.as_slice(),
                encrypted,
                inode.ino as i64
            ],
        )?;
        Ok(())
    }

    /// Delete an inode by inode number.
    pub fn delete(&self, ino: u64) -> rusqlite::Result<()> {
        let t = self.schema.require_table("inodes")?;
        let c_ino = self.schema.require_column("inodes", "ino")?;

        let sql = format!("DELETE FROM {t} WHERE {c_ino} = ?1");
        self.conn.execute(&sql, params![ino as i64])?;
        Ok(())
    }

    /// Get the next available inode number.
    pub fn next_ino(&self) -> rusqlite::Result<u64> {
        let t = self.schema.require_table("inodes")?;
        let c_ino = self.schema.require_column("inodes", "ino")?;

        let sql = format!("SELECT COALESCE(MAX({c_ino}), 0) + 1 FROM {t}");
        let next: i64 = self.conn.query_row(&sql, [], |row| row.get(0))?;
        Ok(next as u64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::migrations::initialize_database;
    use crate::opaque_schema::OpaqueSchema;
    use crate::schema::logical_tables;

    fn setup() -> (Connection, OpaqueSchema, SymmetricKey) {
        let conn = Connection::open_in_memory().unwrap();
        let schema_key = SymmetricKey::from_bytes([0x42; 32]);
        let schema = OpaqueSchema::new(schema_key, &logical_tables());
        initialize_database(&conn, &schema).unwrap();
        let meta_key = SymmetricKey::from_bytes([0x99; 32]);
        (conn, schema, meta_key)
    }

    #[test]
    fn insert_and_get() {
        let (conn, schema, meta_key) = setup();
        let store = InodeStore::new(&conn, &schema, &meta_key);

        let inode = Inode::new_file(2, 1, "test.txt".into(), 0o644, 1000, 1000);
        store.insert(&inode).unwrap();

        let retrieved = store.get(2).unwrap().unwrap();
        assert_eq!(retrieved.ino, 2);
        assert_eq!(retrieved.name, "test.txt");
        assert_eq!(retrieved.mode, 0o644);
    }

    #[test]
    fn get_nonexistent_returns_none() {
        let (conn, schema, meta_key) = setup();
        let store = InodeStore::new(&conn, &schema, &meta_key);
        assert!(store.get(999).unwrap().is_none());
    }

    #[test]
    fn list_children() {
        let (conn, schema, meta_key) = setup();
        let store = InodeStore::new(&conn, &schema, &meta_key);

        let root = Inode::root();
        store.insert(&root).unwrap();

        let f1 = Inode::new_file(2, 1, "a.txt".into(), 0o644, 1000, 1000);
        let f2 = Inode::new_file(3, 1, "b.txt".into(), 0o644, 1000, 1000);
        let f3 = Inode::new_file(4, 999, "other.txt".into(), 0o644, 1000, 1000);
        store.insert(&f1).unwrap();
        store.insert(&f2).unwrap();
        store.insert(&f3).unwrap();

        let children = store.list_children(1).unwrap();
        assert_eq!(children.len(), 2);
    }

    #[test]
    fn lookup_by_name() {
        let (conn, schema, meta_key) = setup();
        let store = InodeStore::new(&conn, &schema, &meta_key);

        let root = Inode::root();
        store.insert(&root).unwrap();

        let f = Inode::new_file(2, 1, "hello.txt".into(), 0o644, 1000, 1000);
        store.insert(&f).unwrap();

        let found = store.lookup(1, "hello.txt").unwrap().unwrap();
        assert_eq!(found.ino, 2);
        assert_eq!(found.name, "hello.txt");

        assert!(store.lookup(1, "nonexistent.txt").unwrap().is_none());
    }

    #[test]
    fn update_inode() {
        let (conn, schema, meta_key) = setup();
        let store = InodeStore::new(&conn, &schema, &meta_key);

        let mut inode = Inode::new_file(2, 1, "test.txt".into(), 0o644, 1000, 1000);
        store.insert(&inode).unwrap();

        inode.size = 12345;
        inode.mode = 0o755;
        store.update(&inode).unwrap();

        let retrieved = store.get(2).unwrap().unwrap();
        assert_eq!(retrieved.size, 12345);
        assert_eq!(retrieved.mode, 0o755);
    }

    #[test]
    fn delete_inode() {
        let (conn, schema, meta_key) = setup();
        let store = InodeStore::new(&conn, &schema, &meta_key);

        let inode = Inode::new_file(2, 1, "test.txt".into(), 0o644, 1000, 1000);
        store.insert(&inode).unwrap();
        assert!(store.get(2).unwrap().is_some());

        store.delete(2).unwrap();
        assert!(store.get(2).unwrap().is_none());
    }

    #[test]
    fn next_ino_increments() {
        let (conn, schema, meta_key) = setup();
        let store = InodeStore::new(&conn, &schema, &meta_key);

        assert_eq!(store.next_ino().unwrap(), 1);

        let root = Inode::root();
        store.insert(&root).unwrap();
        assert_eq!(store.next_ino().unwrap(), 2);

        let f = Inode::new_file(5, 1, "test.txt".into(), 0o644, 1000, 1000);
        store.insert(&f).unwrap();
        assert_eq!(store.next_ino().unwrap(), 6);
    }

    #[test]
    fn data_is_encrypted_on_disk() {
        let (conn, schema, meta_key) = setup();
        let store = InodeStore::new(&conn, &schema, &meta_key);

        let inode = Inode::new_file(2, 1, "secret.txt".into(), 0o644, 1000, 1000);
        store.insert(&inode).unwrap();

        // Read raw data from SQLite - should be encrypted blob, not readable
        let t = schema.table("inodes").unwrap();
        let c_data = schema.column("inodes", "data").unwrap();
        let c_ino = schema.column("inodes", "ino").unwrap();
        let sql = format!("SELECT {c_data} FROM {t} WHERE {c_ino} = 2");
        let raw: Vec<u8> = conn.query_row(&sql, [], |row| row.get(0)).unwrap();

        // Raw data should not contain the plaintext filename
        let raw_str = String::from_utf8_lossy(&raw);
        assert!(
            !raw_str.contains("secret.txt"),
            "data should be encrypted, not plaintext"
        );
    }

    #[test]
    fn wrong_key_cannot_decrypt() {
        let (conn, schema, meta_key) = setup();
        let store = InodeStore::new(&conn, &schema, &meta_key);

        let inode = Inode::new_file(2, 1, "test.txt".into(), 0o644, 1000, 1000);
        store.insert(&inode).unwrap();

        // Try reading with a different key
        let wrong_key = SymmetricKey::from_bytes([0xAA; 32]);
        let wrong_store = InodeStore::new(&conn, &schema, &wrong_key);
        assert!(wrong_store.get(2).is_err());
    }

    #[test]
    fn directory_with_children() {
        let (conn, schema, meta_key) = setup();
        let store = InodeStore::new(&conn, &schema, &meta_key);

        let root = Inode::root();
        store.insert(&root).unwrap();

        let dir = Inode::new_dir(2, 1, "subdir".into(), 0o755, 1000, 1000);
        store.insert(&dir).unwrap();

        let f = Inode::new_file(3, 2, "nested.txt".into(), 0o644, 1000, 1000);
        store.insert(&f).unwrap();

        let children = store.list_children(2).unwrap();
        assert_eq!(children.len(), 1);
        assert_eq!(children[0].name, "nested.txt");
    }
}
