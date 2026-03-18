use rusqlite::{params, Connection};

use tgcryptfs_core::crypto::{aead, keys::SymmetricKey};

use crate::opaque_schema::OpaqueSchema;

// Re-export the types from the sharing crate for convenience
pub use tgcryptfs_sharing::access::ShareRecord;
pub use tgcryptfs_sharing::invite::Invite;

/// Encrypted storage for share records and invites.
pub struct SharingStore<'a> {
    conn: &'a Connection,
    schema: &'a OpaqueSchema,
    meta_key: &'a SymmetricKey,
}

impl<'a> SharingStore<'a> {
    pub fn new(conn: &'a Connection, schema: &'a OpaqueSchema, meta_key: &'a SymmetricKey) -> Self {
        Self {
            conn,
            schema,
            meta_key,
        }
    }

    // ---- Share operations ----

    /// Insert or update a share record.
    pub fn insert_share(&self, volume_id: &str, share: &ShareRecord) -> rusqlite::Result<()> {
        let t = self.schema.require_table("shares")?;
        let c_sid = self.schema.require_column("shares", "sid")?;
        let c_vid = self.schema.require_column("shares", "volume_id")?;
        let c_uid = self.schema.require_column("shares", "uid")?;
        let c_data = self.schema.require_column("shares", "data")?;
        let c_active = self.schema.require_column("shares", "active")?;

        let serialized = postcard::to_allocvec(share).map_err(|e| {
            rusqlite::Error::ToSqlConversionFailure(Box::new(std::io::Error::other(e.to_string())))
        })?;
        let aad = format!("share:{}", share.user_id);
        let encrypted = aead::encrypt(self.meta_key, &serialized, aad.as_bytes()).map_err(|e| {
            rusqlite::Error::ToSqlConversionFailure(Box::new(std::io::Error::other(e.to_string())))
        })?;

        let sql = format!(
            "INSERT OR REPLACE INTO {t} ({c_sid}, {c_vid}, {c_uid}, {c_data}, {c_active})
             VALUES (?1, ?2, ?3, ?4, ?5)"
        );
        self.conn.execute(
            &sql,
            params![
                share.user_id,
                volume_id,
                share.user_id,
                encrypted,
                i64::from(share.active),
            ],
        )?;
        Ok(())
    }

    /// Get a share by user_id.
    pub fn get_share(&self, user_id: &str) -> rusqlite::Result<Option<ShareRecord>> {
        let t = self.schema.require_table("shares")?;
        let c_sid = self.schema.require_column("shares", "sid")?;
        let c_data = self.schema.require_column("shares", "data")?;

        let sql = format!("SELECT {c_data} FROM {t} WHERE {c_sid} = ?1");
        let mut stmt = self.conn.prepare(&sql)?;
        let mut rows = stmt.query(params![user_id])?;

        match rows.next()? {
            Some(row) => {
                let encrypted: Vec<u8> = row.get(0)?;
                let aad = format!("share:{user_id}");
                let decrypted =
                    aead::decrypt(self.meta_key, &encrypted, aad.as_bytes()).map_err(|e| {
                        rusqlite::Error::FromSqlConversionFailure(
                            0,
                            rusqlite::types::Type::Blob,
                            Box::new(std::io::Error::other(e.to_string())),
                        )
                    })?;
                let share: ShareRecord = postcard::from_bytes(&decrypted).map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        0,
                        rusqlite::types::Type::Blob,
                        Box::new(std::io::Error::other(e.to_string())),
                    )
                })?;
                Ok(Some(share))
            }
            None => Ok(None),
        }
    }

    /// List all active shares for a volume.
    pub fn list_shares(&self, volume_id: &str) -> rusqlite::Result<Vec<ShareRecord>> {
        let t = self.schema.require_table("shares")?;
        let c_vid = self.schema.require_column("shares", "volume_id")?;
        let c_sid = self.schema.require_column("shares", "sid")?;
        let c_data = self.schema.require_column("shares", "data")?;
        let c_active = self.schema.require_column("shares", "active")?;

        let sql =
            format!("SELECT {c_sid}, {c_data} FROM {t} WHERE {c_vid} = ?1 AND {c_active} = 1");
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map(params![volume_id], |row| {
            let uid: String = row.get(0)?;
            let encrypted: Vec<u8> = row.get(1)?;
            Ok((uid, encrypted))
        })?;

        let mut shares = Vec::new();
        for row in rows {
            let (uid, encrypted) = row?;
            let aad = format!("share:{uid}");
            let decrypted =
                aead::decrypt(self.meta_key, &encrypted, aad.as_bytes()).map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        0,
                        rusqlite::types::Type::Blob,
                        Box::new(std::io::Error::other(e.to_string())),
                    )
                })?;
            let share: ShareRecord = postcard::from_bytes(&decrypted).map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    0,
                    rusqlite::types::Type::Blob,
                    Box::new(std::io::Error::other(e.to_string())),
                )
            })?;
            shares.push(share);
        }
        Ok(shares)
    }

    /// Revoke a share (set active=false).
    pub fn revoke_share(&self, user_id: &str) -> rusqlite::Result<()> {
        let t = self.schema.require_table("shares")?;
        let c_sid = self.schema.require_column("shares", "sid")?;
        let c_active = self.schema.require_column("shares", "active")?;
        let sql = format!("UPDATE {t} SET {c_active} = 0 WHERE {c_sid} = ?1");
        self.conn.execute(&sql, params![user_id])?;
        Ok(())
    }

    // ---- Invite operations ----

    /// Insert a new invite.
    pub fn insert_invite(&self, invite: &Invite) -> rusqlite::Result<()> {
        let t = self.schema.require_table("invites")?;
        let c_iid = self.schema.require_column("invites", "invite_id")?;
        let c_vid = self.schema.require_column("invites", "volume_id")?;
        let c_data = self.schema.require_column("invites", "data")?;
        let c_active = self.schema.require_column("invites", "active")?;

        let serialized = postcard::to_allocvec(invite).map_err(|e| {
            rusqlite::Error::ToSqlConversionFailure(Box::new(std::io::Error::other(e.to_string())))
        })?;
        let aad = format!("invite:{}", invite.invite_id);
        let encrypted = aead::encrypt(self.meta_key, &serialized, aad.as_bytes()).map_err(|e| {
            rusqlite::Error::ToSqlConversionFailure(Box::new(std::io::Error::other(e.to_string())))
        })?;

        let sql = format!(
            "INSERT OR REPLACE INTO {t} ({c_iid}, {c_vid}, {c_data}, {c_active})
             VALUES (?1, ?2, ?3, ?4)"
        );
        self.conn.execute(
            &sql,
            params![
                invite.invite_id,
                invite.volume_id,
                encrypted,
                i64::from(!invite.revoked),
            ],
        )?;
        Ok(())
    }

    /// Get an invite by ID.
    pub fn get_invite(&self, invite_id: &str) -> rusqlite::Result<Option<Invite>> {
        let t = self.schema.require_table("invites")?;
        let c_iid = self.schema.require_column("invites", "invite_id")?;
        let c_data = self.schema.require_column("invites", "data")?;

        let sql = format!("SELECT {c_data} FROM {t} WHERE {c_iid} = ?1");
        let mut stmt = self.conn.prepare(&sql)?;
        let mut rows = stmt.query(params![invite_id])?;

        match rows.next()? {
            Some(row) => {
                let encrypted: Vec<u8> = row.get(0)?;
                let aad = format!("invite:{invite_id}");
                let decrypted =
                    aead::decrypt(self.meta_key, &encrypted, aad.as_bytes()).map_err(|e| {
                        rusqlite::Error::FromSqlConversionFailure(
                            0,
                            rusqlite::types::Type::Blob,
                            Box::new(std::io::Error::other(e.to_string())),
                        )
                    })?;
                let invite: Invite = postcard::from_bytes(&decrypted).map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        0,
                        rusqlite::types::Type::Blob,
                        Box::new(std::io::Error::other(e.to_string())),
                    )
                })?;
                Ok(Some(invite))
            }
            None => Ok(None),
        }
    }

    /// Use an invite: increment use_count and re-persist.
    pub fn use_invite(&self, invite_id: &str) -> rusqlite::Result<Option<Invite>> {
        let invite = self.get_invite(invite_id)?;
        match invite {
            Some(mut inv) => {
                if inv.try_use().is_err() {
                    return Ok(None); // invalid invite
                }
                // Re-persist the updated invite
                self.insert_invite(&inv)?;
                Ok(Some(inv))
            }
            None => Ok(None),
        }
    }

    /// Revoke an invite.
    pub fn revoke_invite(&self, invite_id: &str) -> rusqlite::Result<()> {
        let t = self.schema.require_table("invites")?;
        let c_iid = self.schema.require_column("invites", "invite_id")?;
        let c_active = self.schema.require_column("invites", "active")?;
        let sql = format!("UPDATE {t} SET {c_active} = 0 WHERE {c_iid} = ?1");
        self.conn.execute(&sql, params![invite_id])?;

        // Also update the encrypted data to mark as revoked
        if let Ok(Some(mut invite)) = self.get_invite(invite_id) {
            invite.revoke();
            let _ = self.insert_invite(&invite);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::migrations::initialize_database;
    use crate::opaque_schema::OpaqueSchema;
    use crate::schema::logical_tables;
    use tgcryptfs_sharing::access::AccessLevel;

    fn setup() -> (Connection, OpaqueSchema, SymmetricKey) {
        let conn = Connection::open_in_memory().unwrap();
        let schema_key = SymmetricKey::from_bytes([0x42; 32]);
        let schema = OpaqueSchema::new(schema_key, &logical_tables());
        initialize_database(&conn, &schema).unwrap();
        let meta_key = SymmetricKey::from_bytes([0x99; 32]);
        (conn, schema, meta_key)
    }

    fn test_share(uid: &str) -> ShareRecord {
        ShareRecord {
            user_id: uid.to_string(),
            telegram_user_id: 123456,
            display_name: "Test User".into(),
            access_level: AccessLevel::ReadWrite,
            wrapped_key: vec![0x42; 32],
            granted_at: 1000000,
            active: true,
        }
    }

    #[test]
    fn insert_and_get_share() {
        let (conn, schema, meta_key) = setup();
        let store = SharingStore::new(&conn, &schema, &meta_key);

        store.insert_share("vol-1", &test_share("user-1")).unwrap();
        let share = store.get_share("user-1").unwrap().unwrap();
        assert_eq!(share.display_name, "Test User");
        assert_eq!(share.access_level, AccessLevel::ReadWrite);
    }

    #[test]
    fn list_shares_for_volume() {
        let (conn, schema, meta_key) = setup();
        let store = SharingStore::new(&conn, &schema, &meta_key);

        store.insert_share("vol-1", &test_share("user-1")).unwrap();
        store.insert_share("vol-1", &test_share("user-2")).unwrap();
        store.insert_share("vol-2", &test_share("user-3")).unwrap();

        let shares = store.list_shares("vol-1").unwrap();
        assert_eq!(shares.len(), 2);

        let shares = store.list_shares("vol-2").unwrap();
        assert_eq!(shares.len(), 1);
    }

    #[test]
    fn revoke_share() {
        let (conn, schema, meta_key) = setup();
        let store = SharingStore::new(&conn, &schema, &meta_key);

        store.insert_share("vol-1", &test_share("user-1")).unwrap();
        store.insert_share("vol-1", &test_share("user-2")).unwrap();

        store.revoke_share("user-1").unwrap();

        // Revoked shares are excluded from list_shares
        let shares = store.list_shares("vol-1").unwrap();
        assert_eq!(shares.len(), 1);
        assert_eq!(shares[0].user_id, "user-2");
    }

    #[test]
    fn insert_and_get_invite() {
        let (conn, schema, meta_key) = setup();
        let store = SharingStore::new(&conn, &schema, &meta_key);

        let invite = Invite::new("vol-1".into(), "owner".into(), AccessLevel::ReadOnly, 0, 5);
        let invite_id = invite.invite_id.clone();

        store.insert_invite(&invite).unwrap();
        let retrieved = store.get_invite(&invite_id).unwrap().unwrap();
        assert_eq!(retrieved.volume_id, "vol-1");
        assert_eq!(retrieved.max_uses, 5);
    }

    #[test]
    fn use_invite_increments_count() {
        let (conn, schema, meta_key) = setup();
        let store = SharingStore::new(&conn, &schema, &meta_key);

        let invite = Invite::new("vol-1".into(), "owner".into(), AccessLevel::ReadWrite, 0, 2);
        let invite_id = invite.invite_id.clone();
        store.insert_invite(&invite).unwrap();

        // First use
        let used = store.use_invite(&invite_id).unwrap().unwrap();
        assert_eq!(used.use_count, 1);

        // Second use
        let used = store.use_invite(&invite_id).unwrap().unwrap();
        assert_eq!(used.use_count, 2);

        // Third use should fail (max_uses=2)
        let used = store.use_invite(&invite_id).unwrap();
        assert!(used.is_none());
    }

    #[test]
    fn revoke_invite() {
        let (conn, schema, meta_key) = setup();
        let store = SharingStore::new(&conn, &schema, &meta_key);

        let invite = Invite::new("vol-1".into(), "owner".into(), AccessLevel::ReadOnly, 0, 0);
        let invite_id = invite.invite_id.clone();
        store.insert_invite(&invite).unwrap();

        store.revoke_invite(&invite_id).unwrap();

        let retrieved = store.get_invite(&invite_id).unwrap().unwrap();
        assert!(retrieved.revoked);
    }

    #[test]
    fn nonexistent_share_returns_none() {
        let (conn, schema, meta_key) = setup();
        let store = SharingStore::new(&conn, &schema, &meta_key);
        assert!(store.get_share("nobody").unwrap().is_none());
    }

    #[test]
    fn nonexistent_invite_returns_none() {
        let (conn, schema, meta_key) = setup();
        let store = SharingStore::new(&conn, &schema, &meta_key);
        assert!(store.get_invite("no-invite").unwrap().is_none());
    }
}
