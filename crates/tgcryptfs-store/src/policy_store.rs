use rusqlite::{params, Connection};

use tgcryptfs_core::crypto::{aead, keys::SymmetricKey};
use tgcryptfs_core::policy::types::MutabilityPolicy;

use crate::opaque_schema::OpaqueSchema;

/// Encrypted policy storage.
pub struct PolicyStore<'a> {
    conn: &'a Connection,
    schema: &'a OpaqueSchema,
    meta_key: &'a SymmetricKey,
}

impl<'a> PolicyStore<'a> {
    pub fn new(conn: &'a Connection, schema: &'a OpaqueSchema, meta_key: &'a SymmetricKey) -> Self {
        Self {
            conn,
            schema,
            meta_key,
        }
    }

    pub fn insert(&self, policy: &MutabilityPolicy) -> rusqlite::Result<()> {
        let t = self.schema.require_table("policies")?;
        let c_pid = self.schema.require_column("policies", "pid")?;
        let c_data = self.schema.require_column("policies", "data")?;

        let serialized = postcard::to_allocvec(policy).map_err(|e| {
            rusqlite::Error::ToSqlConversionFailure(Box::new(std::io::Error::other(e.to_string())))
        })?;
        let aad = format!("policy:{}", policy.policy_id);
        let encrypted = aead::encrypt(self.meta_key, &serialized, aad.as_bytes()).map_err(|e| {
            rusqlite::Error::ToSqlConversionFailure(Box::new(std::io::Error::other(e.to_string())))
        })?;

        let sql = format!("INSERT OR REPLACE INTO {t} ({c_pid}, {c_data}) VALUES (?1, ?2)");
        self.conn
            .execute(&sql, params![policy.policy_id, encrypted])?;
        Ok(())
    }

    pub fn get(&self, policy_id: u32) -> rusqlite::Result<Option<MutabilityPolicy>> {
        let t = self.schema.require_table("policies")?;
        let c_pid = self.schema.require_column("policies", "pid")?;
        let c_data = self.schema.require_column("policies", "data")?;

        let sql = format!("SELECT {c_data} FROM {t} WHERE {c_pid} = ?1");
        let mut stmt = self.conn.prepare(&sql)?;
        let mut rows = stmt.query(params![policy_id])?;

        match rows.next()? {
            Some(row) => {
                let encrypted: Vec<u8> = row.get(0)?;
                let aad = format!("policy:{policy_id}");
                let decrypted =
                    aead::decrypt(self.meta_key, &encrypted, aad.as_bytes()).map_err(|e| {
                        rusqlite::Error::FromSqlConversionFailure(
                            0,
                            rusqlite::types::Type::Blob,
                            Box::new(std::io::Error::other(e.to_string())),
                        )
                    })?;
                let policy: MutabilityPolicy = postcard::from_bytes(&decrypted).map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        0,
                        rusqlite::types::Type::Blob,
                        Box::new(std::io::Error::other(e.to_string())),
                    )
                })?;
                Ok(Some(policy))
            }
            None => Ok(None),
        }
    }

    pub fn list_all(&self) -> rusqlite::Result<Vec<MutabilityPolicy>> {
        let t = self.schema.require_table("policies")?;
        let c_pid = self.schema.require_column("policies", "pid")?;
        let c_data = self.schema.require_column("policies", "data")?;

        let sql = format!("SELECT {c_pid}, {c_data} FROM {t}");
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map([], |row| {
            let pid: i64 = row.get(0)?;
            let encrypted: Vec<u8> = row.get(1)?;
            Ok((pid as u32, encrypted))
        })?;

        let mut policies = Vec::new();
        for row in rows {
            let (pid, encrypted) = row?;
            let aad = format!("policy:{pid}");
            let decrypted =
                aead::decrypt(self.meta_key, &encrypted, aad.as_bytes()).map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        0,
                        rusqlite::types::Type::Blob,
                        Box::new(std::io::Error::other(e.to_string())),
                    )
                })?;
            let policy: MutabilityPolicy = postcard::from_bytes(&decrypted).map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    0,
                    rusqlite::types::Type::Blob,
                    Box::new(std::io::Error::other(e.to_string())),
                )
            })?;
            policies.push(policy);
        }
        Ok(policies)
    }

    pub fn delete(&self, policy_id: u32) -> rusqlite::Result<()> {
        let t = self.schema.require_table("policies")?;
        let c_pid = self.schema.require_column("policies", "pid")?;
        let sql = format!("DELETE FROM {t} WHERE {c_pid} = ?1");
        self.conn.execute(&sql, params![policy_id])?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::migrations::initialize_database;
    use crate::opaque_schema::OpaqueSchema;
    use crate::schema::logical_tables;
    use tgcryptfs_core::policy::types::{DeleteBehavior, ExpiryBehavior, PolicyRule};

    fn setup() -> (Connection, OpaqueSchema, SymmetricKey) {
        let conn = Connection::open_in_memory().unwrap();
        let schema_key = SymmetricKey::from_bytes([0x42; 32]);
        let schema = OpaqueSchema::new(schema_key, &logical_tables());
        initialize_database(&conn, &schema).unwrap();
        let meta_key = SymmetricKey::from_bytes([0x99; 32]);
        (conn, schema, meta_key)
    }

    fn test_policy() -> MutabilityPolicy {
        MutabilityPolicy {
            policy_id: 1,
            name: "default".into(),
            rules: vec![PolicyRule {
                path_pattern: "**".into(),
                on_delete: DeleteBehavior::Soft,
                record_changes: true,
                mutable: true,
                retention_secs: Some(86400 * 30),
                on_expiry: ExpiryBehavior::Hold,
            }],
        }
    }

    #[test]
    fn insert_and_get() {
        let (conn, schema, meta_key) = setup();
        let store = PolicyStore::new(&conn, &schema, &meta_key);
        store.insert(&test_policy()).unwrap();

        let p = store.get(1).unwrap().unwrap();
        assert_eq!(p.name, "default");
        assert_eq!(p.rules.len(), 1);
    }

    #[test]
    fn list_all() {
        let (conn, schema, meta_key) = setup();
        let store = PolicyStore::new(&conn, &schema, &meta_key);
        store.insert(&test_policy()).unwrap();

        let mut p2 = test_policy();
        p2.policy_id = 2;
        p2.name = "strict".into();
        store.insert(&p2).unwrap();

        let all = store.list_all().unwrap();
        assert_eq!(all.len(), 2);
    }

    #[test]
    fn delete_policy() {
        let (conn, schema, meta_key) = setup();
        let store = PolicyStore::new(&conn, &schema, &meta_key);
        store.insert(&test_policy()).unwrap();
        store.delete(1).unwrap();
        assert!(store.get(1).unwrap().is_none());
    }
}
