use rusqlite::Connection;

use crate::opaque_schema::OpaqueSchema;

/// Initialize all tables using opaque names derived from the schema key.
/// This is called once when a volume database is created.
pub fn initialize_database(conn: &Connection, schema: &OpaqueSchema) -> rusqlite::Result<()> {
    conn.execute_batch("PRAGMA journal_mode=WAL;")?;
    conn.execute_batch("PRAGMA foreign_keys=OFF;")?;

    // Volume table
    let t = schema.require_table("volume")?;
    let c_vid = schema.require_column("volume", "volume_id")?;
    let c_data = schema.require_column("volume", "data")?;
    let c_cat = schema.require_column("volume", "created_at")?;
    let c_uat = schema.require_column("volume", "updated_at")?;
    conn.execute_batch(&format!(
        "CREATE TABLE IF NOT EXISTS {t} (
            {c_vid} TEXT PRIMARY KEY,
            {c_data} BLOB NOT NULL,
            {c_cat} INTEGER NOT NULL,
            {c_uat} INTEGER NOT NULL
        );"
    ))?;

    // Inodes table
    let t = schema.require_table("inodes")?;
    let c_ino = schema.require_column("inodes", "ino")?;
    let c_parent = schema.require_column("inodes", "parent")?;
    let c_nh = schema.require_column("inodes", "name_hash")?;
    let c_data = schema.require_column("inodes", "data")?;
    let c_ver = schema.require_column("inodes", "version")?;
    let i_parent = schema.require_index("inodes_parent")?;
    let i_pn = schema.require_index("inodes_parent_name")?;
    conn.execute_batch(&format!(
        "CREATE TABLE IF NOT EXISTS {t} (
            {c_ino} INTEGER PRIMARY KEY,
            {c_parent} INTEGER NOT NULL,
            {c_nh} BLOB NOT NULL,
            {c_data} BLOB NOT NULL,
            {c_ver} INTEGER NOT NULL DEFAULT 0
        );
        CREATE INDEX IF NOT EXISTS {i_parent} ON {t}({c_parent});
        CREATE UNIQUE INDEX IF NOT EXISTS {i_pn} ON {t}({c_parent}, {c_nh});"
    ))?;

    // Blocks table
    let t = schema.require_table("blocks")?;
    let c_rid = schema.require_column("blocks", "rid")?;
    let c_ch = schema.require_column("blocks", "content_hash")?;
    let c_mid = schema.require_column("blocks", "message_id")?;
    let c_es = schema.require_column("blocks", "encrypted_size")?;
    let c_ep = schema.require_column("blocks", "epoch")?;
    let c_rc = schema.require_column("blocks", "ref_count")?;
    let c_comp = schema.require_column("blocks", "compressed")?;
    let i_ch = schema.require_index("blocks_content_hash")?;
    let i_mid = schema.require_index("blocks_message_id")?;
    conn.execute_batch(&format!(
        "CREATE TABLE IF NOT EXISTS {t} (
            {c_rid} BLOB PRIMARY KEY,
            {c_ch} BLOB NOT NULL,
            {c_mid} INTEGER NOT NULL,
            {c_es} INTEGER NOT NULL,
            {c_ep} INTEGER NOT NULL,
            {c_rc} INTEGER NOT NULL DEFAULT 1,
            {c_comp} INTEGER NOT NULL DEFAULT 0
        );
        CREATE INDEX IF NOT EXISTS {i_ch} ON {t}({c_ch});
        CREATE INDEX IF NOT EXISTS {i_mid} ON {t}({c_mid});"
    ))?;

    // Manifests table
    let t = schema.require_table("manifests")?;
    let c_ino = schema.require_column("manifests", "ino")?;
    let c_ver = schema.require_column("manifests", "version")?;
    let c_data = schema.require_column("manifests", "data")?;
    let c_cat = schema.require_column("manifests", "created_at")?;
    conn.execute_batch(&format!(
        "CREATE TABLE IF NOT EXISTS {t} (
            {c_ino} INTEGER NOT NULL,
            {c_ver} INTEGER NOT NULL,
            {c_data} BLOB NOT NULL,
            {c_cat} INTEGER NOT NULL,
            PRIMARY KEY ({c_ino}, {c_ver})
        );"
    ))?;

    // Snapshots table
    let t = schema.require_table("snapshots")?;
    let c_sid = schema.require_column("snapshots", "sid")?;
    let c_ts = schema.require_column("snapshots", "timestamp")?;
    let c_data = schema.require_column("snapshots", "data")?;
    let c_ino = schema.require_column("snapshots", "ino")?;
    let i_ts = schema.require_index("snapshots_ts")?;
    let i_ino = schema.require_index("snapshots_ino")?;
    conn.execute_batch(&format!(
        "CREATE TABLE IF NOT EXISTS {t} (
            {c_sid} INTEGER PRIMARY KEY AUTOINCREMENT,
            {c_ts} INTEGER NOT NULL,
            {c_data} BLOB NOT NULL,
            {c_ino} INTEGER NOT NULL
        );
        CREATE INDEX IF NOT EXISTS {i_ts} ON {t}({c_ts});
        CREATE INDEX IF NOT EXISTS {i_ino} ON {t}({c_ino});"
    ))?;

    // Policies table
    let t = schema.require_table("policies")?;
    let c_pid = schema.require_column("policies", "pid")?;
    let c_data = schema.require_column("policies", "data")?;
    conn.execute_batch(&format!(
        "CREATE TABLE IF NOT EXISTS {t} (
            {c_pid} INTEGER PRIMARY KEY,
            {c_data} BLOB NOT NULL
        );"
    ))?;

    // Path-policy mapping
    let t = schema.require_table("path_policies")?;
    let c_pp = schema.require_column("path_policies", "path_pattern")?;
    let c_pid = schema.require_column("path_policies", "pid")?;
    let c_pr = schema.require_column("path_policies", "priority")?;
    let i_pp = schema.require_index("path_policies_pattern")?;
    conn.execute_batch(&format!(
        "CREATE TABLE IF NOT EXISTS {t} (
            {c_pp} BLOB NOT NULL,
            {c_pid} INTEGER NOT NULL,
            {c_pr} INTEGER NOT NULL
        );
        CREATE INDEX IF NOT EXISTS {i_pp} ON {t}({c_pp});"
    ))?;

    // Users table
    let t = schema.require_table("users")?;
    let c_uid = schema.require_column("users", "uid")?;
    let c_data = schema.require_column("users", "data")?;
    let c_active = schema.require_column("users", "active")?;
    conn.execute_batch(&format!(
        "CREATE TABLE IF NOT EXISTS {t} (
            {c_uid} TEXT PRIMARY KEY,
            {c_data} BLOB NOT NULL,
            {c_active} INTEGER NOT NULL DEFAULT 1
        );"
    ))?;

    // Audit log
    let t = schema.require_table("audit_log")?;
    let c_id = schema.require_column("audit_log", "id")?;
    let c_ts = schema.require_column("audit_log", "timestamp")?;
    let c_uid = schema.require_column("audit_log", "uid")?;
    let c_act = schema.require_column("audit_log", "action")?;
    let c_det = schema.require_column("audit_log", "details")?;
    let i_ts = schema.require_index("audit_ts")?;
    conn.execute_batch(&format!(
        "CREATE TABLE IF NOT EXISTS {t} (
            {c_id} INTEGER PRIMARY KEY AUTOINCREMENT,
            {c_ts} INTEGER NOT NULL,
            {c_uid} TEXT,
            {c_act} BLOB NOT NULL,
            {c_det} BLOB
        );
        CREATE INDEX IF NOT EXISTS {i_ts} ON {t}({c_ts});"
    ))?;

    // Deadman table
    let t = schema.require_table("deadman")?;
    let c_vid = schema.require_column("deadman", "vid")?;
    let c_data = schema.require_column("deadman", "data")?;
    let c_armed = schema.require_column("deadman", "armed")?;
    let c_lc = schema.require_column("deadman", "last_check")?;
    conn.execute_batch(&format!(
        "CREATE TABLE IF NOT EXISTS {t} (
            {c_vid} TEXT PRIMARY KEY,
            {c_data} BLOB NOT NULL,
            {c_armed} INTEGER NOT NULL DEFAULT 0,
            {c_lc} INTEGER
        );"
    ))?;

    // Shares table
    let t = schema.require_table("shares")?;
    let c_sid = schema.require_column("shares", "sid")?;
    let c_vid = schema.require_column("shares", "volume_id")?;
    let c_uid = schema.require_column("shares", "uid")?;
    let c_data = schema.require_column("shares", "data")?;
    let c_active = schema.require_column("shares", "active")?;
    let i_vol = schema.require_index("shares_volume")?;
    conn.execute_batch(&format!(
        "CREATE TABLE IF NOT EXISTS {t} (
            {c_sid} TEXT PRIMARY KEY,
            {c_vid} TEXT NOT NULL,
            {c_uid} TEXT NOT NULL,
            {c_data} BLOB NOT NULL,
            {c_active} INTEGER NOT NULL DEFAULT 1
        );
        CREATE INDEX IF NOT EXISTS {i_vol} ON {t}({c_vid});"
    ))?;

    // Invites table
    let t = schema.require_table("invites")?;
    let c_iid = schema.require_column("invites", "invite_id")?;
    let c_vid = schema.require_column("invites", "volume_id")?;
    let c_data = schema.require_column("invites", "data")?;
    let c_active = schema.require_column("invites", "active")?;
    let i_vol = schema.require_index("invites_volume")?;
    conn.execute_batch(&format!(
        "CREATE TABLE IF NOT EXISTS {t} (
            {c_iid} TEXT PRIMARY KEY,
            {c_vid} TEXT NOT NULL,
            {c_data} BLOB NOT NULL,
            {c_active} INTEGER NOT NULL DEFAULT 1
        );
        CREATE INDEX IF NOT EXISTS {i_vol} ON {t}({c_vid});"
    ))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::opaque_schema::OpaqueSchema;
    use crate::schema::logical_tables;
    use tgcryptfs_core::crypto::keys::SymmetricKey;

    fn test_schema() -> OpaqueSchema {
        let key = SymmetricKey::from_bytes([0x42; 32]);
        OpaqueSchema::new(key, &logical_tables())
    }

    #[test]
    fn initialize_creates_all_tables() {
        let conn = Connection::open_in_memory().unwrap();
        let schema = test_schema();
        initialize_database(&conn, &schema).unwrap();

        // Verify tables exist by querying sqlite_master
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 12); // 12 logical tables
    }

    #[test]
    fn table_names_are_opaque() {
        let conn = Connection::open_in_memory().unwrap();
        let schema = test_schema();
        initialize_database(&conn, &schema).unwrap();

        let mut stmt = conn
            .prepare(
                "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'",
            )
            .unwrap();
        let names: Vec<String> = stmt
            .query_map([], |row| row.get(0))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();

        for name in &names {
            assert!(
                name.starts_with("t_"),
                "table name should be opaque: {name}"
            );
            assert!(
                !name.contains("inode"),
                "table name should not contain logical name: {name}"
            );
            assert!(
                !name.contains("block"),
                "table name should not contain logical name: {name}"
            );
        }
    }

    #[test]
    fn column_names_are_opaque() {
        let conn = Connection::open_in_memory().unwrap();
        let schema = test_schema();
        initialize_database(&conn, &schema).unwrap();

        let inodes_table = schema.table("inodes").unwrap();
        let mut stmt = conn
            .prepare(&format!("PRAGMA table_info({inodes_table})"))
            .unwrap();
        let col_names: Vec<String> = stmt
            .query_map([], |row| row.get::<_, String>(1))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();

        for col in &col_names {
            assert!(col.starts_with("c_"), "column name should be opaque: {col}");
        }
    }

    #[test]
    fn idempotent_initialization() {
        let conn = Connection::open_in_memory().unwrap();
        let schema = test_schema();
        initialize_database(&conn, &schema).unwrap();
        // Second call should not fail
        initialize_database(&conn, &schema).unwrap();
    }
}
