use rusqlite::{params, Connection};

use tgcryptfs_core::crypto::{aead, keys::SymmetricKey};
use tgcryptfs_core::volume::config::VolumeConfig;

use crate::opaque_schema::OpaqueSchema;

/// Volume configuration storage (encrypted).
pub struct VolumeStore<'a> {
    conn: &'a Connection,
    schema: &'a OpaqueSchema,
    meta_key: &'a SymmetricKey,
}

impl<'a> VolumeStore<'a> {
    pub fn new(conn: &'a Connection, schema: &'a OpaqueSchema, meta_key: &'a SymmetricKey) -> Self {
        Self {
            conn,
            schema,
            meta_key,
        }
    }

    pub fn save_config(&self, config: &VolumeConfig) -> rusqlite::Result<()> {
        let t = self.schema.require_table("volume")?;
        let c_vid = self.schema.require_column("volume", "volume_id")?;
        let c_data = self.schema.require_column("volume", "data")?;
        let c_cat = self.schema.require_column("volume", "created_at")?;
        let c_uat = self.schema.require_column("volume", "updated_at")?;

        let serialized = postcard::to_allocvec(config).map_err(|e| {
            rusqlite::Error::ToSqlConversionFailure(Box::new(std::io::Error::other(e.to_string())))
        })?;
        let aad = format!("volume:{}", config.volume_id);
        let encrypted = aead::encrypt(self.meta_key, &serialized, aad.as_bytes()).map_err(|e| {
            rusqlite::Error::ToSqlConversionFailure(Box::new(std::io::Error::other(e.to_string())))
        })?;

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        let sql = format!(
            "INSERT OR REPLACE INTO {t} ({c_vid}, {c_data}, {c_cat}, {c_uat}) VALUES (?1, ?2, ?3, ?4)"
        );
        self.conn.execute(
            &sql,
            params![config.volume_id.to_string(), encrypted, now, now],
        )?;
        Ok(())
    }

    pub fn load_config(&self, volume_id: &str) -> rusqlite::Result<Option<VolumeConfig>> {
        let t = self.schema.require_table("volume")?;
        let c_vid = self.schema.require_column("volume", "volume_id")?;
        let c_data = self.schema.require_column("volume", "data")?;

        let sql = format!("SELECT {c_data} FROM {t} WHERE {c_vid} = ?1");
        let mut stmt = self.conn.prepare(&sql)?;
        let mut rows = stmt.query(params![volume_id])?;

        match rows.next()? {
            Some(row) => {
                let encrypted: Vec<u8> = row.get(0)?;
                let aad = format!("volume:{volume_id}");
                let decrypted =
                    aead::decrypt(self.meta_key, &encrypted, aad.as_bytes()).map_err(|e| {
                        rusqlite::Error::FromSqlConversionFailure(
                            0,
                            rusqlite::types::Type::Blob,
                            Box::new(std::io::Error::other(e.to_string())),
                        )
                    })?;
                let config: VolumeConfig = postcard::from_bytes(&decrypted).map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        0,
                        rusqlite::types::Type::Blob,
                        Box::new(std::io::Error::other(e.to_string())),
                    )
                })?;
                Ok(Some(config))
            }
            None => Ok(None),
        }
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
    fn save_and_load_config() {
        let (conn, schema, meta_key) = setup();
        let store = VolumeStore::new(&conn, &schema, &meta_key);

        let config = VolumeConfig::new("test-vol".into(), "noun verb adj".into());
        let vol_id = config.volume_id.to_string();

        store.save_config(&config).unwrap();
        let loaded = store.load_config(&vol_id).unwrap().unwrap();
        assert_eq!(loaded.display_name, "test-vol");
        assert_eq!(loaded.volume_id, config.volume_id);
    }

    #[test]
    fn load_nonexistent() {
        let (conn, schema, meta_key) = setup();
        let store = VolumeStore::new(&conn, &schema, &meta_key);
        assert!(store.load_config("nonexistent").unwrap().is_none());
    }
}
