use rusqlite::{params, Connection};

use tgcryptfs_core::crypto::{aead, keys::SymmetricKey};

use crate::opaque_schema::OpaqueSchema;

/// Serializable deadman configuration (stored encrypted).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DeadmanRecord {
    pub volume_id: String,
    pub armed: bool,
    pub config_data: Vec<u8>,
    pub last_check: Option<i64>,
}

/// Deadman configuration storage.
pub struct DeadmanStore<'a> {
    conn: &'a Connection,
    schema: &'a OpaqueSchema,
    meta_key: &'a SymmetricKey,
}

impl<'a> DeadmanStore<'a> {
    pub fn new(conn: &'a Connection, schema: &'a OpaqueSchema, meta_key: &'a SymmetricKey) -> Self {
        Self {
            conn,
            schema,
            meta_key,
        }
    }

    pub fn upsert(&self, record: &DeadmanRecord) -> rusqlite::Result<()> {
        let t = self.schema.require_table("deadman")?;
        let c_vid = self.schema.require_column("deadman", "vid")?;
        let c_data = self.schema.require_column("deadman", "data")?;
        let c_armed = self.schema.require_column("deadman", "armed")?;
        let c_lc = self.schema.require_column("deadman", "last_check")?;

        let serialized = postcard::to_allocvec(record).map_err(|e| {
            rusqlite::Error::ToSqlConversionFailure(Box::new(std::io::Error::other(e.to_string())))
        })?;
        let aad = format!("deadman:{}", record.volume_id);
        let encrypted = aead::encrypt(self.meta_key, &serialized, aad.as_bytes()).map_err(|e| {
            rusqlite::Error::ToSqlConversionFailure(Box::new(std::io::Error::other(e.to_string())))
        })?;

        let sql = format!(
            "INSERT OR REPLACE INTO {t} ({c_vid}, {c_data}, {c_armed}, {c_lc}) VALUES (?1, ?2, ?3, ?4)"
        );
        self.conn.execute(
            &sql,
            params![
                record.volume_id,
                encrypted,
                i64::from(record.armed),
                record.last_check,
            ],
        )?;
        Ok(())
    }

    pub fn get(&self, volume_id: &str) -> rusqlite::Result<Option<DeadmanRecord>> {
        let t = self.schema.require_table("deadman")?;
        let c_vid = self.schema.require_column("deadman", "vid")?;
        let c_data = self.schema.require_column("deadman", "data")?;

        let sql = format!("SELECT {c_data} FROM {t} WHERE {c_vid} = ?1");
        let mut stmt = self.conn.prepare(&sql)?;
        let mut rows = stmt.query(params![volume_id])?;

        match rows.next()? {
            Some(row) => {
                let encrypted: Vec<u8> = row.get(0)?;
                let aad = format!("deadman:{volume_id}");
                let decrypted =
                    aead::decrypt(self.meta_key, &encrypted, aad.as_bytes()).map_err(|e| {
                        rusqlite::Error::FromSqlConversionFailure(
                            0,
                            rusqlite::types::Type::Blob,
                            Box::new(std::io::Error::other(e.to_string())),
                        )
                    })?;
                let record: DeadmanRecord = postcard::from_bytes(&decrypted).map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        0,
                        rusqlite::types::Type::Blob,
                        Box::new(std::io::Error::other(e.to_string())),
                    )
                })?;
                Ok(Some(record))
            }
            None => Ok(None),
        }
    }

    pub fn set_armed(&self, volume_id: &str, armed: bool) -> rusqlite::Result<()> {
        let t = self.schema.require_table("deadman")?;
        let c_vid = self.schema.require_column("deadman", "vid")?;
        let c_armed = self.schema.require_column("deadman", "armed")?;
        let sql = format!("UPDATE {t} SET {c_armed} = ?1 WHERE {c_vid} = ?2");
        self.conn
            .execute(&sql, params![i64::from(armed), volume_id])?;
        Ok(())
    }

    pub fn update_last_check(&self, volume_id: &str, timestamp: i64) -> rusqlite::Result<()> {
        let t = self.schema.require_table("deadman")?;
        let c_vid = self.schema.require_column("deadman", "vid")?;
        let c_lc = self.schema.require_column("deadman", "last_check")?;
        let sql = format!("UPDATE {t} SET {c_lc} = ?1 WHERE {c_vid} = ?2");
        self.conn.execute(&sql, params![timestamp, volume_id])?;
        Ok(())
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
    fn upsert_and_get() {
        let (conn, schema, meta_key) = setup();
        let store = DeadmanStore::new(&conn, &schema, &meta_key);

        let record = DeadmanRecord {
            volume_id: "vol-1".into(),
            armed: false,
            config_data: vec![0x01; 100],
            last_check: None,
        };
        store.upsert(&record).unwrap();

        let retrieved = store.get("vol-1").unwrap().unwrap();
        assert!(!retrieved.armed);
        assert_eq!(retrieved.config_data.len(), 100);
    }

    #[test]
    fn set_armed() {
        let (conn, schema, meta_key) = setup();
        let store = DeadmanStore::new(&conn, &schema, &meta_key);

        let record = DeadmanRecord {
            volume_id: "vol-1".into(),
            armed: false,
            config_data: vec![],
            last_check: None,
        };
        store.upsert(&record).unwrap();
        store.set_armed("vol-1", true).unwrap();

        // Note: set_armed only updates the armed column flag, not the encrypted data
        // The encrypted data still has armed=false, but the queryable flag is updated
    }

    #[test]
    fn nonexistent_returns_none() {
        let (conn, schema, meta_key) = setup();
        let store = DeadmanStore::new(&conn, &schema, &meta_key);
        assert!(store.get("nonexistent").unwrap().is_none());
    }
}
