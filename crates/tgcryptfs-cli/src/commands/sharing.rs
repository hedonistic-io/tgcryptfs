use anyhow::Result;
use std::io::Write;

use tgcryptfs_core::volume::manager;
use tgcryptfs_sharing::access::{AccessLevel, ShareRecord};
use tgcryptfs_sharing::invite::Invite;
use tgcryptfs_store::migrations::initialize_database;
use tgcryptfs_store::opaque_schema::OpaqueSchema;
use tgcryptfs_store::schema::logical_tables;
use tgcryptfs_store::sharing_store::SharingStore;

use super::utils;

pub fn parse_access_level(s: &str) -> Result<AccessLevel> {
    match s.to_lowercase().as_str() {
        "read-only" | "readonly" | "ro" => Ok(AccessLevel::ReadOnly),
        "read-write" | "readwrite" | "rw" => Ok(AccessLevel::ReadWrite),
        "admin" => Ok(AccessLevel::Admin),
        _ => anyhow::bail!("invalid access level '{s}'. Use: read-only, read-write, or admin"),
    }
}

fn open_volume_db(
    volume: &str,
) -> Result<(
    rusqlite::Connection,
    OpaqueSchema,
    tgcryptfs_core::crypto::keys::SymmetricKey,
    String,
)> {
    let volumes_dir = manager::default_volumes_dir();
    let volumes = manager::list_volumes(&volumes_dir).map_err(utils::core_err)?;

    let config = volumes
        .into_iter()
        .find(|v| v.volume_id.to_string() == volume || v.display_name == volume)
        .ok_or_else(|| anyhow::anyhow!("Volume '{volume}' not found"))?;

    eprint!("Enter volume password: ");
    std::io::stderr().flush()?;
    let mut password = String::new();
    std::io::stdin().read_line(&mut password)?;
    let password = password.trim_end();

    let result = manager::open_volume(
        &config.volume_id.to_string(),
        password.as_bytes(),
        &volumes_dir,
    )
    .map_err(utils::core_err)?;

    let conn = rusqlite::Connection::open(&result.paths.db_path)
        .map_err(|e| anyhow::anyhow!("Failed to open database: {e}"))?;

    let schema = OpaqueSchema::new(result.hierarchy.schema.clone(), &logical_tables());
    initialize_database(&conn, &schema)
        .map_err(|e| anyhow::anyhow!("Failed to initialize database: {e}"))?;

    let vol_id = config.volume_id.to_string();
    Ok((conn, schema, result.hierarchy.meta.clone(), vol_id))
}

pub async fn create_share(volume: &str, user: &str, access: &str) -> Result<()> {
    let access_level = parse_access_level(access)?;
    let (conn, schema, meta_key, vol_id) = open_volume_db(volume)?;
    let store = SharingStore::new(&conn, &schema, &meta_key);

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| anyhow::anyhow!("System clock error: {e}"))?
        .as_secs() as i64;

    let share = ShareRecord {
        user_id: user.to_string(),
        telegram_user_id: 0, // will be resolved when user connects
        display_name: user.to_string(),
        access_level,
        wrapped_key: Vec::new(), // will be set during key exchange
        granted_at: now,
        active: true,
    };

    store
        .insert_share(&vol_id, &share)
        .map_err(|e| anyhow::anyhow!("Failed to create share: {e}"))?;

    println!("Share created.");
    println!("  Volume:  {vol_id}");
    println!("  User:    {user}");
    println!("  Access:  {access}");

    Ok(())
}

pub async fn list_shares(volume: &str) -> Result<()> {
    let (conn, schema, meta_key, vol_id) = open_volume_db(volume)?;
    let store = SharingStore::new(&conn, &schema, &meta_key);

    let shares = store
        .list_shares(&vol_id)
        .map_err(|e| anyhow::anyhow!("Failed to list shares: {e}"))?;

    if shares.is_empty() {
        println!("No active shares for this volume.");
        return Ok(());
    }

    println!("{:<20}  {:<12}  {:<20}", "USER", "ACCESS", "GRANTED");
    println!("{}", "-".repeat(55));
    for s in &shares {
        let access = format!("{:?}", s.access_level);
        println!("{:<20}  {:<12}  {}", s.user_id, access, s.granted_at);
    }
    println!();
    println!("{} share(s) total.", shares.len());

    Ok(())
}

pub async fn revoke_share(volume: &str, user: &str) -> Result<()> {
    let (conn, schema, meta_key, _vol_id) = open_volume_db(volume)?;
    let store = SharingStore::new(&conn, &schema, &meta_key);

    store
        .revoke_share(user)
        .map_err(|e| anyhow::anyhow!("Failed to revoke share: {e}"))?;

    println!("Share revoked for user '{user}'.");

    Ok(())
}

pub async fn create_invite(
    volume: &str,
    access: &str,
    max_uses: Option<u32>,
    expires_hours: Option<u64>,
) -> Result<()> {
    let access_level = parse_access_level(access)?;
    let (conn, schema, meta_key, vol_id) = open_volume_db(volume)?;
    let store = SharingStore::new(&conn, &schema, &meta_key);

    let expires_at = match expires_hours {
        Some(h) => {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(|e| anyhow::anyhow!("System clock error: {e}"))?
                .as_secs() as i64;
            now + (h as i64 * 3600)
        }
        None => 0,
    };

    let invite = Invite::new(
        vol_id,
        "owner".into(),
        access_level,
        expires_at,
        max_uses.unwrap_or(0),
    );

    let invite_id = invite.invite_id.clone();
    store
        .insert_invite(&invite)
        .map_err(|e| anyhow::anyhow!("Failed to create invite: {e}"))?;

    println!("Invite created.");
    println!("  Invite ID: {invite_id}");
    println!("  Access:    {access}");
    if let Some(n) = max_uses {
        println!("  Max uses:  {n}");
    } else {
        println!("  Max uses:  unlimited");
    }
    if let Some(h) = expires_hours {
        println!("  Expires:   in {h} hours");
    } else {
        println!("  Expires:   never");
    }

    Ok(())
}

pub async fn accept_invite(invite_code: &str) -> Result<()> {
    println!("Accepting invite: {invite_code}");
    println!();
    println!("Invite acceptance requires connecting to the volume's database.");
    println!("This will be fully functional when the volume can be resolved");
    println!("from the invite code.");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_access_level_read_only() {
        assert_eq!(
            parse_access_level("read-only").unwrap(),
            AccessLevel::ReadOnly
        );
        assert_eq!(
            parse_access_level("readonly").unwrap(),
            AccessLevel::ReadOnly
        );
        assert_eq!(parse_access_level("ro").unwrap(), AccessLevel::ReadOnly);
        assert_eq!(parse_access_level("RO").unwrap(), AccessLevel::ReadOnly);
    }

    #[test]
    fn parse_access_level_read_write() {
        assert_eq!(
            parse_access_level("read-write").unwrap(),
            AccessLevel::ReadWrite
        );
        assert_eq!(
            parse_access_level("readwrite").unwrap(),
            AccessLevel::ReadWrite
        );
        assert_eq!(parse_access_level("rw").unwrap(), AccessLevel::ReadWrite);
    }

    #[test]
    fn parse_access_level_admin() {
        assert_eq!(parse_access_level("admin").unwrap(), AccessLevel::Admin);
        assert_eq!(parse_access_level("ADMIN").unwrap(), AccessLevel::Admin);
    }

    #[test]
    fn parse_access_level_invalid() {
        assert!(parse_access_level("owner").is_err());
        assert!(parse_access_level("").is_err());
        assert!(parse_access_level("r").is_err());
    }
}
