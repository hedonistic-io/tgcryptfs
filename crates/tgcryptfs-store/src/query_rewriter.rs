use crate::opaque_schema::OpaqueSchema;

/// Rewrite a logical SQL statement to use opaque identifiers.
///
/// This is a simple text-replacement approach. For prepared statements,
/// rewriting happens once at startup and the result is cached.
pub fn rewrite_sql(schema: &OpaqueSchema, logical_sql: &str, table: &str) -> Option<String> {
    let mut result = logical_sql.to_string();

    // Replace table name
    if let Some(opaque_table) = schema.table(table) {
        result = result.replace(table, opaque_table);
    } else {
        return None;
    }

    Some(result)
}

/// Rewrite a logical SQL statement, replacing both table and column names.
pub fn rewrite_full(
    schema: &OpaqueSchema,
    logical_sql: &str,
    table: &str,
    columns: &[&str],
) -> Option<String> {
    let mut result = logical_sql.to_string();

    // Replace column names first (before table name, to avoid partial matches)
    for col in columns {
        if let Some(opaque_col) = schema.column(table, col) {
            result = result.replace(col, opaque_col);
        }
    }

    // Replace table name
    if let Some(opaque_table) = schema.table(table) {
        result = result.replace(table, opaque_table);
    } else {
        return None;
    }

    Some(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::opaque_schema::{OpaqueSchema, TableDef};
    use tgcryptfs_core::crypto::keys::SymmetricKey;

    fn test_schema() -> OpaqueSchema {
        let key = SymmetricKey::from_bytes([0x42; 32]);
        let tables = vec![TableDef {
            name: "inodes".into(),
            columns: vec!["ino".into(), "parent".into(), "data".into()],
            indexes: vec![],
        }];
        OpaqueSchema::new(key, &tables)
    }

    #[test]
    fn rewrite_replaces_table_name() {
        let schema = test_schema();
        let result = rewrite_sql(&schema, "SELECT * FROM inodes", "inodes").unwrap();
        assert!(!result.contains("inodes"));
        assert!(result.contains("t_"));
    }

    #[test]
    fn rewrite_full_replaces_columns() {
        let schema = test_schema();
        let result = rewrite_full(
            &schema,
            "SELECT ino, data FROM inodes WHERE parent = ?",
            "inodes",
            &["ino", "data", "parent"],
        )
        .unwrap();
        assert!(!result.contains("ino"));
        assert!(!result.contains("data"));
        assert!(!result.contains("parent"));
        assert!(!result.contains("inodes"));
    }

    #[test]
    fn unknown_table_returns_none() {
        let schema = test_schema();
        assert!(rewrite_sql(&schema, "SELECT * FROM unknown", "unknown").is_none());
    }
}
