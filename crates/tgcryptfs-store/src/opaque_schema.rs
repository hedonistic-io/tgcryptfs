use std::collections::HashMap;

use tgcryptfs_core::crypto::blake3 as b3;
use tgcryptfs_core::crypto::keys::SymmetricKey;

/// In-memory mapping from logical names to opaque SQLite identifiers.
/// Reconstructed from Kschema on every mount. NEVER persisted.
pub struct OpaqueSchema {
    #[allow(dead_code)]
    schema_key: SymmetricKey,
    tables: HashMap<String, String>,
    columns: HashMap<(String, String), String>,
    indexes: HashMap<String, String>,
}

impl OpaqueSchema {
    /// Build a new opaque schema from a schema key and logical definitions.
    pub fn new(schema_key: SymmetricKey, logical_tables: &[TableDef]) -> Self {
        let mut tables = HashMap::new();
        let mut columns = HashMap::new();
        let mut indexes = HashMap::new();

        for table_def in logical_tables {
            let opaque_table = derive_name(&schema_key, "table", &table_def.name);
            tables.insert(table_def.name.clone(), opaque_table.clone());

            for col in &table_def.columns {
                let opaque_col = derive_name(&schema_key, &format!("col:{}", table_def.name), col);
                columns.insert((table_def.name.clone(), col.clone()), opaque_col);
            }

            for idx in &table_def.indexes {
                let opaque_idx = derive_name(&schema_key, "idx", idx);
                indexes.insert(idx.clone(), opaque_idx);
            }
        }

        Self {
            schema_key,
            tables,
            columns,
            indexes,
        }
    }

    /// Get the opaque table name for a logical table name.
    pub fn table(&self, logical: &str) -> Option<&str> {
        self.tables.get(logical).map(String::as_str)
    }

    /// Get the opaque column name for a logical table + column pair.
    pub fn column(&self, table: &str, logical_col: &str) -> Option<&str> {
        self.columns
            .get(&(table.to_string(), logical_col.to_string()))
            .map(String::as_str)
    }

    /// Get the opaque index name for a logical index name.
    pub fn index(&self, logical: &str) -> Option<&str> {
        self.indexes.get(logical).map(String::as_str)
    }

    /// Get the opaque table name, returning a `rusqlite::Error` if missing.
    pub fn require_table(&self, logical: &str) -> rusqlite::Result<&str> {
        self.table(logical).ok_or_else(|| {
            rusqlite::Error::InvalidParameterName(format!("missing schema table: {logical}"))
        })
    }

    /// Get the opaque column name, returning a `rusqlite::Error` if missing.
    pub fn require_column(&self, table: &str, logical_col: &str) -> rusqlite::Result<&str> {
        self.column(table, logical_col).ok_or_else(|| {
            rusqlite::Error::InvalidParameterName(format!(
                "missing schema column: {table}.{logical_col}"
            ))
        })
    }

    /// Get the opaque index name, returning a `rusqlite::Error` if missing.
    pub fn require_index(&self, logical: &str) -> rusqlite::Result<&str> {
        self.index(logical).ok_or_else(|| {
            rusqlite::Error::InvalidParameterName(format!("missing schema index: {logical}"))
        })
    }
}

/// Logical table definition for schema building.
pub struct TableDef {
    pub name: String,
    pub columns: Vec<String>,
    pub indexes: Vec<String>,
}

/// Derive an opaque identifier: "t_" + hex(BLAKE3(key || domain || ":" || name))[0..16]
fn derive_name(key: &SymmetricKey, domain: &str, name: &str) -> String {
    let hash = b3::derive_opaque_id(key, domain, name);
    let prefix = match domain {
        d if d.starts_with("col:") => "c_",
        "idx" => "i_",
        _ => "t_",
    };
    format!("{}{}", prefix, hex::encode(&hash[..8]))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_schema() -> OpaqueSchema {
        let key = SymmetricKey::from_bytes([0x42; 32]);
        let tables = vec![
            TableDef {
                name: "inodes".into(),
                columns: vec![
                    "ino".into(),
                    "parent".into(),
                    "name_hash".into(),
                    "data".into(),
                ],
                indexes: vec!["inodes_parent".into()],
            },
            TableDef {
                name: "blocks".into(),
                columns: vec!["rid".into(), "content_hash".into(), "message_id".into()],
                indexes: vec!["blocks_content_hash".into()],
            },
        ];
        OpaqueSchema::new(key, &tables)
    }

    #[test]
    fn table_name_is_opaque() {
        let schema = test_schema();
        let name = schema.table("inodes").unwrap();
        assert!(name.starts_with("t_"));
        assert_eq!(name.len(), 2 + 16); // "t_" + 16 hex chars
        assert!(!name.contains("inode"));
    }

    #[test]
    fn column_name_is_opaque() {
        let schema = test_schema();
        let name = schema.column("inodes", "parent").unwrap();
        assert!(name.starts_with("c_"));
        assert!(!name.contains("parent"));
    }

    #[test]
    fn index_name_is_opaque() {
        let schema = test_schema();
        let name = schema.index("inodes_parent").unwrap();
        assert!(name.starts_with("i_"));
    }

    #[test]
    fn different_tables_different_names() {
        let schema = test_schema();
        assert_ne!(schema.table("inodes"), schema.table("blocks"));
    }

    #[test]
    fn same_column_different_tables_different_names() {
        let key = SymmetricKey::from_bytes([0x42; 32]);
        let tables = vec![
            TableDef {
                name: "a".into(),
                columns: vec!["id".into()],
                indexes: vec![],
            },
            TableDef {
                name: "b".into(),
                columns: vec!["id".into()],
                indexes: vec![],
            },
        ];
        let schema = OpaqueSchema::new(key, &tables);
        assert_ne!(schema.column("a", "id"), schema.column("b", "id"));
    }

    #[test]
    fn deterministic_derivation() {
        let s1 = test_schema();
        let s2 = test_schema();
        assert_eq!(s1.table("inodes"), s2.table("inodes"));
        assert_eq!(s1.column("inodes", "ino"), s2.column("inodes", "ino"));
    }

    #[test]
    fn unknown_table_returns_none() {
        let schema = test_schema();
        assert!(schema.table("nonexistent").is_none());
    }
}
