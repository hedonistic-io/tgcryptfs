use rusqlite::{params, Connection};

use crate::opaque_schema::OpaqueSchema;

/// Block reference record for tracking uploaded blocks on Telegram.
pub struct BlockRecord {
    pub rid: [u8; 32],
    pub content_hash: [u8; 32],
    pub message_id: i64,
    pub encrypted_size: i64,
    pub epoch: u32,
    pub ref_count: i64,
    pub compressed: bool,
}

/// Block reference storage.
pub struct BlockStore<'a> {
    conn: &'a Connection,
    schema: &'a OpaqueSchema,
}

impl<'a> BlockStore<'a> {
    pub fn new(conn: &'a Connection, schema: &'a OpaqueSchema) -> Self {
        Self { conn, schema }
    }

    /// Insert a new block reference.
    pub fn insert(&self, block: &BlockRecord) -> rusqlite::Result<()> {
        let t = self.schema.require_table("blocks")?;
        let c_rid = self.schema.require_column("blocks", "rid")?;
        let c_ch = self.schema.require_column("blocks", "content_hash")?;
        let c_mid = self.schema.require_column("blocks", "message_id")?;
        let c_es = self.schema.require_column("blocks", "encrypted_size")?;
        let c_ep = self.schema.require_column("blocks", "epoch")?;
        let c_rc = self.schema.require_column("blocks", "ref_count")?;
        let c_comp = self.schema.require_column("blocks", "compressed")?;

        let sql = format!(
            "INSERT INTO {t} ({c_rid}, {c_ch}, {c_mid}, {c_es}, {c_ep}, {c_rc}, {c_comp})
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)"
        );
        self.conn.execute(
            &sql,
            params![
                block.rid.as_slice(),
                block.content_hash.as_slice(),
                block.message_id,
                block.encrypted_size,
                block.epoch,
                block.ref_count,
                i64::from(block.compressed),
            ],
        )?;
        Ok(())
    }

    /// Get a block by its random ID.
    pub fn get(&self, rid: &[u8; 32]) -> rusqlite::Result<Option<BlockRecord>> {
        let t = self.schema.require_table("blocks")?;
        let c_rid = self.schema.require_column("blocks", "rid")?;
        let c_ch = self.schema.require_column("blocks", "content_hash")?;
        let c_mid = self.schema.require_column("blocks", "message_id")?;
        let c_es = self.schema.require_column("blocks", "encrypted_size")?;
        let c_ep = self.schema.require_column("blocks", "epoch")?;
        let c_rc = self.schema.require_column("blocks", "ref_count")?;
        let c_comp = self.schema.require_column("blocks", "compressed")?;

        let sql = format!(
            "SELECT {c_ch}, {c_mid}, {c_es}, {c_ep}, {c_rc}, {c_comp}
             FROM {t} WHERE {c_rid} = ?1"
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let mut rows = stmt.query(params![rid.as_slice()])?;

        match rows.next()? {
            Some(row) => {
                let ch: Vec<u8> = row.get(0)?;
                let mut content_hash = [0u8; 32];
                content_hash.copy_from_slice(&ch);

                Ok(Some(BlockRecord {
                    rid: *rid,
                    content_hash,
                    message_id: row.get(1)?,
                    encrypted_size: row.get(2)?,
                    epoch: row.get::<_, i64>(3)? as u32,
                    ref_count: row.get(4)?,
                    compressed: row.get::<_, i64>(5)? != 0,
                }))
            }
            None => Ok(None),
        }
    }

    /// Find a block by content hash (for deduplication).
    pub fn find_by_content_hash(
        &self,
        content_hash: &[u8; 32],
    ) -> rusqlite::Result<Option<BlockRecord>> {
        let t = self.schema.require_table("blocks")?;
        let c_rid = self.schema.require_column("blocks", "rid")?;
        let c_ch = self.schema.require_column("blocks", "content_hash")?;
        let c_mid = self.schema.require_column("blocks", "message_id")?;
        let c_es = self.schema.require_column("blocks", "encrypted_size")?;
        let c_ep = self.schema.require_column("blocks", "epoch")?;
        let c_rc = self.schema.require_column("blocks", "ref_count")?;
        let c_comp = self.schema.require_column("blocks", "compressed")?;

        let sql = format!(
            "SELECT {c_rid}, {c_mid}, {c_es}, {c_ep}, {c_rc}, {c_comp}
             FROM {t} WHERE {c_ch} = ?1 LIMIT 1"
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let mut rows = stmt.query(params![content_hash.as_slice()])?;

        match rows.next()? {
            Some(row) => {
                let rid_bytes: Vec<u8> = row.get(0)?;
                let mut rid = [0u8; 32];
                rid.copy_from_slice(&rid_bytes);

                Ok(Some(BlockRecord {
                    rid,
                    content_hash: *content_hash,
                    message_id: row.get(1)?,
                    encrypted_size: row.get(2)?,
                    epoch: row.get::<_, i64>(3)? as u32,
                    ref_count: row.get(4)?,
                    compressed: row.get::<_, i64>(5)? != 0,
                }))
            }
            None => Ok(None),
        }
    }

    /// Increment the reference count for a block.
    pub fn increment_ref(&self, rid: &[u8; 32]) -> rusqlite::Result<()> {
        let t = self.schema.require_table("blocks")?;
        let c_rid = self.schema.require_column("blocks", "rid")?;
        let c_rc = self.schema.require_column("blocks", "ref_count")?;

        let sql = format!("UPDATE {t} SET {c_rc} = {c_rc} + 1 WHERE {c_rid} = ?1");
        self.conn.execute(&sql, params![rid.as_slice()])?;
        Ok(())
    }

    /// Decrement the reference count. Returns true if the block should be garbage collected (ref_count <= 0).
    pub fn decrement_ref(&self, rid: &[u8; 32]) -> rusqlite::Result<bool> {
        let t = self.schema.require_table("blocks")?;
        let c_rid = self.schema.require_column("blocks", "rid")?;
        let c_rc = self.schema.require_column("blocks", "ref_count")?;

        let sql = format!("UPDATE {t} SET {c_rc} = {c_rc} - 1 WHERE {c_rid} = ?1");
        self.conn.execute(&sql, params![rid.as_slice()])?;

        let sql = format!("SELECT {c_rc} FROM {t} WHERE {c_rid} = ?1");
        let rc: i64 = self
            .conn
            .query_row(&sql, params![rid.as_slice()], |r| r.get(0))?;
        Ok(rc <= 0)
    }

    /// List all blocks at a given epoch (for key rotation).
    pub fn list_by_epoch(&self, epoch: u32) -> rusqlite::Result<Vec<BlockRecord>> {
        let t = self.schema.require_table("blocks")?;
        let c_rid = self.schema.require_column("blocks", "rid")?;
        let c_ch = self.schema.require_column("blocks", "content_hash")?;
        let c_mid = self.schema.require_column("blocks", "message_id")?;
        let c_es = self.schema.require_column("blocks", "encrypted_size")?;
        let c_ep = self.schema.require_column("blocks", "epoch")?;
        let c_rc = self.schema.require_column("blocks", "ref_count")?;
        let c_comp = self.schema.require_column("blocks", "compressed")?;

        let sql = format!(
            "SELECT {c_rid}, {c_ch}, {c_mid}, {c_es}, {c_rc}, {c_comp}
             FROM {t} WHERE {c_ep} = ?1"
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map(params![epoch], |row| {
            let rid_bytes: Vec<u8> = row.get(0)?;
            let ch_bytes: Vec<u8> = row.get(1)?;
            let mut rid = [0u8; 32];
            let mut content_hash = [0u8; 32];
            rid.copy_from_slice(&rid_bytes);
            content_hash.copy_from_slice(&ch_bytes);

            Ok(BlockRecord {
                rid,
                content_hash,
                message_id: row.get(2)?,
                encrypted_size: row.get(3)?,
                epoch,
                ref_count: row.get(4)?,
                compressed: row.get::<_, i64>(5)? != 0,
            })
        })?;

        let mut blocks = Vec::new();
        for row in rows {
            blocks.push(row?);
        }
        Ok(blocks)
    }

    /// Update a block's epoch and message_id after re-encryption.
    pub fn update_block_epoch(
        &self,
        rid: &[u8; 32],
        new_epoch: u32,
        new_message_id: i64,
        new_encrypted_size: i64,
    ) -> rusqlite::Result<()> {
        let t = self.schema.require_table("blocks")?;
        let c_rid = self.schema.require_column("blocks", "rid")?;
        let c_ep = self.schema.require_column("blocks", "epoch")?;
        let c_mid = self.schema.require_column("blocks", "message_id")?;
        let c_es = self.schema.require_column("blocks", "encrypted_size")?;

        let sql =
            format!("UPDATE {t} SET {c_ep} = ?1, {c_mid} = ?2, {c_es} = ?3 WHERE {c_rid} = ?4");
        self.conn.execute(
            &sql,
            params![
                new_epoch,
                new_message_id,
                new_encrypted_size,
                rid.as_slice()
            ],
        )?;
        Ok(())
    }

    /// Delete a block reference (used after garbage collection).
    pub fn delete(&self, rid: &[u8; 32]) -> rusqlite::Result<()> {
        let t = self.schema.require_table("blocks")?;
        let c_rid = self.schema.require_column("blocks", "rid")?;
        let sql = format!("DELETE FROM {t} WHERE {c_rid} = ?1");
        self.conn.execute(&sql, params![rid.as_slice()])?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::migrations::initialize_database;
    use crate::opaque_schema::OpaqueSchema;
    use crate::schema::logical_tables;
    use tgcryptfs_core::crypto::keys::SymmetricKey;

    fn setup() -> (Connection, OpaqueSchema) {
        let conn = Connection::open_in_memory().unwrap();
        let key = SymmetricKey::from_bytes([0x42; 32]);
        let schema = OpaqueSchema::new(key, &logical_tables());
        initialize_database(&conn, &schema).unwrap();
        (conn, schema)
    }

    fn test_block() -> BlockRecord {
        BlockRecord {
            rid: [0x01; 32],
            content_hash: [0xAA; 32],
            message_id: 12345,
            encrypted_size: 50000,
            epoch: 0,
            ref_count: 1,
            compressed: true,
        }
    }

    #[test]
    fn insert_and_get() {
        let (conn, schema) = setup();
        let store = BlockStore::new(&conn, &schema);
        let block = test_block();
        store.insert(&block).unwrap();

        let retrieved = store.get(&[0x01; 32]).unwrap().unwrap();
        assert_eq!(retrieved.message_id, 12345);
        assert_eq!(retrieved.encrypted_size, 50000);
        assert!(retrieved.compressed);
    }

    #[test]
    fn find_by_content_hash() {
        let (conn, schema) = setup();
        let store = BlockStore::new(&conn, &schema);
        store.insert(&test_block()).unwrap();

        let found = store.find_by_content_hash(&[0xAA; 32]).unwrap().unwrap();
        assert_eq!(found.rid, [0x01; 32]);

        assert!(store.find_by_content_hash(&[0xBB; 32]).unwrap().is_none());
    }

    #[test]
    fn ref_counting() {
        let (conn, schema) = setup();
        let store = BlockStore::new(&conn, &schema);
        store.insert(&test_block()).unwrap();

        store.increment_ref(&[0x01; 32]).unwrap();
        let b = store.get(&[0x01; 32]).unwrap().unwrap();
        assert_eq!(b.ref_count, 2);

        assert!(!store.decrement_ref(&[0x01; 32]).unwrap()); // 2 -> 1, not GC-able
        assert!(store.decrement_ref(&[0x01; 32]).unwrap()); // 1 -> 0, GC-able
    }

    #[test]
    fn delete_block() {
        let (conn, schema) = setup();
        let store = BlockStore::new(&conn, &schema);
        store.insert(&test_block()).unwrap();
        store.delete(&[0x01; 32]).unwrap();
        assert!(store.get(&[0x01; 32]).unwrap().is_none());
    }

    #[test]
    fn list_by_epoch() {
        let (conn, schema) = setup();
        let store = BlockStore::new(&conn, &schema);

        // Insert blocks at different epochs
        store.insert(&test_block()).unwrap(); // epoch 0
        store
            .insert(&BlockRecord {
                rid: [0x02; 32],
                content_hash: [0xBB; 32],
                message_id: 22222,
                encrypted_size: 60000,
                epoch: 0,
                ref_count: 1,
                compressed: false,
            })
            .unwrap();
        store
            .insert(&BlockRecord {
                rid: [0x03; 32],
                content_hash: [0xCC; 32],
                message_id: 33333,
                encrypted_size: 70000,
                epoch: 1,
                ref_count: 1,
                compressed: true,
            })
            .unwrap();

        let epoch0 = store.list_by_epoch(0).unwrap();
        assert_eq!(epoch0.len(), 2);

        let epoch1 = store.list_by_epoch(1).unwrap();
        assert_eq!(epoch1.len(), 1);
        assert_eq!(epoch1[0].message_id, 33333);

        let epoch2 = store.list_by_epoch(2).unwrap();
        assert!(epoch2.is_empty());
    }

    #[test]
    fn update_block_epoch() {
        let (conn, schema) = setup();
        let store = BlockStore::new(&conn, &schema);
        store.insert(&test_block()).unwrap();

        // Update to new epoch with new message_id
        store
            .update_block_epoch(&[0x01; 32], 1, 99999, 55000)
            .unwrap();

        let block = store.get(&[0x01; 32]).unwrap().unwrap();
        assert_eq!(block.epoch, 1);
        assert_eq!(block.message_id, 99999);
        assert_eq!(block.encrypted_size, 55000);

        // Verify it's listed under the new epoch
        let epoch0 = store.list_by_epoch(0).unwrap();
        assert!(epoch0.is_empty());
        let epoch1 = store.list_by_epoch(1).unwrap();
        assert_eq!(epoch1.len(), 1);
    }
}
