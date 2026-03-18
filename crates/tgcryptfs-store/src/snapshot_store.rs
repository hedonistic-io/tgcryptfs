use rusqlite::{params, Connection};

use tgcryptfs_core::crypto::{aead, keys::SymmetricKey};
use tgcryptfs_core::snapshot::entry::SnapshotEntry;

use crate::opaque_schema::OpaqueSchema;

/// Encrypted snapshot log storage.
pub struct SnapshotStore<'a> {
    conn: &'a Connection,
    schema: &'a OpaqueSchema,
    meta_key: &'a SymmetricKey,
}

impl<'a> SnapshotStore<'a> {
    pub fn new(conn: &'a Connection, schema: &'a OpaqueSchema, meta_key: &'a SymmetricKey) -> Self {
        Self {
            conn,
            schema,
            meta_key,
        }
    }

    /// Append a snapshot entry.
    ///
    /// AAD is bound to the inode: `"snapshot:ino:{ino}"`. This allows retrieval
    /// without needing to know the snapshot_id in advance while still preventing
    /// cross-inode ciphertext substitution.
    pub fn append(&self, entry: &SnapshotEntry) -> rusqlite::Result<i64> {
        let t = self.schema.require_table("snapshots")?;
        let c_ts = self.schema.require_column("snapshots", "timestamp")?;
        let c_data = self.schema.require_column("snapshots", "data")?;
        let c_ino = self.schema.require_column("snapshots", "ino")?;

        let serialized = postcard::to_allocvec(entry).map_err(|e| {
            rusqlite::Error::ToSqlConversionFailure(Box::new(std::io::Error::other(e.to_string())))
        })?;
        let aad = format!("snapshot:ino:{}", entry.inode);
        let encrypted = aead::encrypt(self.meta_key, &serialized, aad.as_bytes()).map_err(|e| {
            rusqlite::Error::ToSqlConversionFailure(Box::new(std::io::Error::other(e.to_string())))
        })?;

        let ts = (entry.timestamp / 1_000_000_000) as i64; // ns to seconds for index
        let sql = format!("INSERT INTO {t} ({c_ts}, {c_data}, {c_ino}) VALUES (?1, ?2, ?3)");
        self.conn
            .execute(&sql, params![ts, encrypted, entry.inode as i64])?;
        Ok(self.conn.last_insert_rowid())
    }

    /// Get a snapshot entry by its row ID and inode.
    pub fn get(&self, row_id: i64, ino: u64) -> rusqlite::Result<Option<SnapshotEntry>> {
        let t = self.schema.require_table("snapshots")?;
        let c_sid = self.schema.require_column("snapshots", "sid")?;
        let c_data = self.schema.require_column("snapshots", "data")?;

        let sql = format!("SELECT {c_data} FROM {t} WHERE {c_sid} = ?1");
        let mut stmt = self.conn.prepare(&sql)?;
        let mut rows = stmt.query(params![row_id])?;

        match rows.next()? {
            Some(row) => {
                let encrypted: Vec<u8> = row.get(0)?;
                let entry = self.try_decrypt_entry(&encrypted, ino)?;
                Ok(Some(entry))
            }
            None => Ok(None),
        }
    }

    /// List snapshot entries for a given inode.
    pub fn list_for_inode(&self, ino: u64) -> rusqlite::Result<Vec<SnapshotEntry>> {
        let t = self.schema.require_table("snapshots")?;
        let c_ino = self.schema.require_column("snapshots", "ino")?;
        let c_data = self.schema.require_column("snapshots", "data")?;
        let c_ts = self.schema.require_column("snapshots", "timestamp")?;

        let sql = format!("SELECT {c_data} FROM {t} WHERE {c_ino} = ?1 ORDER BY {c_ts} DESC");
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map(params![ino as i64], |row| {
            let encrypted: Vec<u8> = row.get(0)?;
            Ok(encrypted)
        })?;

        let mut entries = Vec::new();
        for row in rows {
            let encrypted = row?;
            let entry = self.try_decrypt_entry(&encrypted, ino)?;
            entries.push(entry);
        }
        Ok(entries)
    }

    /// List recent snapshot entries.
    pub fn list_recent(&self, limit: u32) -> rusqlite::Result<Vec<SnapshotEntry>> {
        let t = self.schema.require_table("snapshots")?;
        let c_data = self.schema.require_column("snapshots", "data")?;
        let c_ino = self.schema.require_column("snapshots", "ino")?;
        let c_ts = self.schema.require_column("snapshots", "timestamp")?;

        let sql = format!("SELECT {c_data}, {c_ino} FROM {t} ORDER BY {c_ts} DESC LIMIT ?1");
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map(params![limit], |row| {
            let encrypted: Vec<u8> = row.get(0)?;
            let ino: i64 = row.get(1)?;
            Ok((encrypted, ino as u64))
        })?;

        let mut entries = Vec::new();
        for row in rows {
            let (encrypted, ino) = row?;
            let entry = self.try_decrypt_entry(&encrypted, ino)?;
            entries.push(entry);
        }
        Ok(entries)
    }

    fn try_decrypt_entry(&self, encrypted: &[u8], ino: u64) -> rusqlite::Result<SnapshotEntry> {
        let aad = format!("snapshot:ino:{ino}");
        let decrypted = aead::decrypt(self.meta_key, encrypted, aad.as_bytes()).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(
                0,
                rusqlite::types::Type::Blob,
                Box::new(std::io::Error::other(e.to_string())),
            )
        })?;
        postcard::from_bytes(&decrypted).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(
                0,
                rusqlite::types::Type::Blob,
                Box::new(std::io::Error::other(e.to_string())),
            )
        })
    }

}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::migrations::initialize_database;
    use crate::opaque_schema::OpaqueSchema;
    use crate::schema::logical_tables;
    use tgcryptfs_core::snapshot::entry::SnapshotOperation;
    use tgcryptfs_core::snapshot::log::create_entry;

    fn setup() -> (Connection, OpaqueSchema, SymmetricKey) {
        let conn = Connection::open_in_memory().unwrap();
        let schema_key = SymmetricKey::from_bytes([0x42; 32]);
        let schema = OpaqueSchema::new(schema_key, &logical_tables());
        initialize_database(&conn, &schema).unwrap();
        let meta_key = SymmetricKey::from_bytes([0x99; 32]);
        (conn, schema, meta_key)
    }

    #[test]
    fn append_and_list() {
        let (conn, schema, meta_key) = setup();
        let store = SnapshotStore::new(&conn, &schema, &meta_key);

        let entry = create_entry(1, SnapshotOperation::Create, 2, None, None, None);
        store.append(&entry).unwrap();

        let entries = store.list_for_inode(2).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].inode, 2);
    }

    #[test]
    fn list_recent() {
        let (conn, schema, meta_key) = setup();
        let store = SnapshotStore::new(&conn, &schema, &meta_key);

        for i in 0..5 {
            let entry = create_entry(i, SnapshotOperation::Write, 10, None, None, None);
            store.append(&entry).unwrap();
        }

        let recent = store.list_recent(3).unwrap();
        assert_eq!(recent.len(), 3);
    }

    #[test]
    fn snapshots_filtered_by_inode() {
        let (conn, schema, meta_key) = setup();
        let store = SnapshotStore::new(&conn, &schema, &meta_key);

        let e1 = create_entry(1, SnapshotOperation::Create, 10, None, None, None);
        let e2 = create_entry(2, SnapshotOperation::Write, 10, None, None, None);
        let e3 = create_entry(3, SnapshotOperation::Create, 20, None, None, None);
        store.append(&e1).unwrap();
        store.append(&e2).unwrap();
        store.append(&e3).unwrap();

        let ino10 = store.list_for_inode(10).unwrap();
        assert_eq!(ino10.len(), 2);

        let ino20 = store.list_for_inode(20).unwrap();
        assert_eq!(ino20.len(), 1);
    }
}
