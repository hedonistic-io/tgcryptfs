use rusqlite::{params, Connection};

use tgcryptfs_core::crypto::{aead, keys::SymmetricKey};

use crate::opaque_schema::OpaqueSchema;

/// Serializable user record (stored encrypted).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct UserRecord {
    pub user_id: String,
    pub telegram_user_id: i64,
    pub display_name: String,
    pub access_level: String,
    pub mlkem_public_key: Vec<u8>,
    pub wrapped_keys: Vec<u8>,
    pub granted_at: i64,
    pub last_validated: Option<i64>,
    pub active: bool,
}

/// Encrypted user storage.
pub struct UserStore<'a> {
    conn: &'a Connection,
    schema: &'a OpaqueSchema,
    meta_key: &'a SymmetricKey,
}

impl<'a> UserStore<'a> {
    pub fn new(conn: &'a Connection, schema: &'a OpaqueSchema, meta_key: &'a SymmetricKey) -> Self {
        Self {
            conn,
            schema,
            meta_key,
        }
    }

    pub fn insert(&self, user: &UserRecord) -> rusqlite::Result<()> {
        let t = self.schema.require_table("users")?;
        let c_uid = self.schema.require_column("users", "uid")?;
        let c_data = self.schema.require_column("users", "data")?;
        let c_active = self.schema.require_column("users", "active")?;

        let serialized = postcard::to_allocvec(user).map_err(|e| {
            rusqlite::Error::ToSqlConversionFailure(Box::new(std::io::Error::other(e.to_string())))
        })?;
        let aad = format!("user:{}", user.user_id);
        let encrypted = aead::encrypt(self.meta_key, &serialized, aad.as_bytes()).map_err(|e| {
            rusqlite::Error::ToSqlConversionFailure(Box::new(std::io::Error::other(e.to_string())))
        })?;

        let sql = format!(
            "INSERT OR REPLACE INTO {t} ({c_uid}, {c_data}, {c_active}) VALUES (?1, ?2, ?3)"
        );
        self.conn.execute(
            &sql,
            params![user.user_id, encrypted, i64::from(user.active)],
        )?;
        Ok(())
    }

    pub fn get(&self, user_id: &str) -> rusqlite::Result<Option<UserRecord>> {
        let t = self.schema.require_table("users")?;
        let c_uid = self.schema.require_column("users", "uid")?;
        let c_data = self.schema.require_column("users", "data")?;

        let sql = format!("SELECT {c_data} FROM {t} WHERE {c_uid} = ?1");
        let mut stmt = self.conn.prepare(&sql)?;
        let mut rows = stmt.query(params![user_id])?;

        match rows.next()? {
            Some(row) => {
                let encrypted: Vec<u8> = row.get(0)?;
                let aad = format!("user:{user_id}");
                let decrypted =
                    aead::decrypt(self.meta_key, &encrypted, aad.as_bytes()).map_err(|e| {
                        rusqlite::Error::FromSqlConversionFailure(
                            0,
                            rusqlite::types::Type::Blob,
                            Box::new(std::io::Error::other(e.to_string())),
                        )
                    })?;
                let user: UserRecord = postcard::from_bytes(&decrypted).map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        0,
                        rusqlite::types::Type::Blob,
                        Box::new(std::io::Error::other(e.to_string())),
                    )
                })?;
                Ok(Some(user))
            }
            None => Ok(None),
        }
    }

    pub fn list_active(&self) -> rusqlite::Result<Vec<UserRecord>> {
        let t = self.schema.require_table("users")?;
        let c_uid = self.schema.require_column("users", "uid")?;
        let c_data = self.schema.require_column("users", "data")?;
        let c_active = self.schema.require_column("users", "active")?;

        let sql = format!("SELECT {c_uid}, {c_data} FROM {t} WHERE {c_active} = 1");
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map([], |row| {
            let uid: String = row.get(0)?;
            let encrypted: Vec<u8> = row.get(1)?;
            Ok((uid, encrypted))
        })?;

        let mut users = Vec::new();
        for row in rows {
            let (uid, encrypted) = row?;
            let aad = format!("user:{uid}");
            let decrypted =
                aead::decrypt(self.meta_key, &encrypted, aad.as_bytes()).map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        0,
                        rusqlite::types::Type::Blob,
                        Box::new(std::io::Error::other(e.to_string())),
                    )
                })?;
            let user: UserRecord = postcard::from_bytes(&decrypted).map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    0,
                    rusqlite::types::Type::Blob,
                    Box::new(std::io::Error::other(e.to_string())),
                )
            })?;
            users.push(user);
        }
        Ok(users)
    }

    pub fn deactivate(&self, user_id: &str) -> rusqlite::Result<()> {
        let t = self.schema.require_table("users")?;
        let c_uid = self.schema.require_column("users", "uid")?;
        let c_active = self.schema.require_column("users", "active")?;
        let sql = format!("UPDATE {t} SET {c_active} = 0 WHERE {c_uid} = ?1");
        self.conn.execute(&sql, params![user_id])?;
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

    fn test_user(id: &str) -> UserRecord {
        UserRecord {
            user_id: id.to_string(),
            telegram_user_id: 123456,
            display_name: "Test User".into(),
            access_level: "ReadWrite".into(),
            mlkem_public_key: vec![0x01; 100],
            wrapped_keys: vec![0x02; 64],
            granted_at: 1000000,
            last_validated: None,
            active: true,
        }
    }

    #[test]
    fn insert_and_get() {
        let (conn, schema, meta_key) = setup();
        let store = UserStore::new(&conn, &schema, &meta_key);
        store.insert(&test_user("user-1")).unwrap();

        let u = store.get("user-1").unwrap().unwrap();
        assert_eq!(u.display_name, "Test User");
    }

    #[test]
    fn list_active_filters_inactive() {
        let (conn, schema, meta_key) = setup();
        let store = UserStore::new(&conn, &schema, &meta_key);
        store.insert(&test_user("user-1")).unwrap();
        store.insert(&test_user("user-2")).unwrap();
        store.deactivate("user-2").unwrap();

        let active = store.list_active().unwrap();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].user_id, "user-1");
    }
}
