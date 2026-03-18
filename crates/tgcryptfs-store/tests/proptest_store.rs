use proptest::prelude::*;

use tgcryptfs_core::crypto::keys::SymmetricKey;
use tgcryptfs_store::opaque_schema::{OpaqueSchema, TableDef};

fn arb_key() -> impl Strategy<Value = SymmetricKey> {
    prop::array::uniform32(any::<u8>()).prop_map(SymmetricKey::from_bytes)
}

fn test_tables() -> Vec<TableDef> {
    vec![
        TableDef {
            name: "users".into(),
            columns: vec!["id".into(), "name".into(), "email".into()],
            indexes: vec!["idx_users_name".into()],
        },
        TableDef {
            name: "blocks".into(),
            columns: vec!["rid".into(), "epoch".into(), "size".into()],
            indexes: vec!["idx_blocks_epoch".into()],
        },
    ]
}

proptest! {
    /// Opaque schema is deterministic: same key + tables -> same names.
    #[test]
    fn opaque_schema_deterministic(
        key in arb_key(),
    ) {
        let tables = test_tables();
        let schema1 = OpaqueSchema::new(key.clone(), &tables);
        let schema2 = OpaqueSchema::new(key, &tables);

        let t1 = schema1.table("users").unwrap();
        let t2 = schema2.table("users").unwrap();
        prop_assert_eq!(t1, t2);

        let c1 = schema1.column("users", "name").unwrap();
        let c2 = schema2.column("users", "name").unwrap();
        prop_assert_eq!(c1, c2);
    }

    /// Different keys produce different opaque names.
    #[test]
    fn opaque_schema_different_keys(
        key1 in arb_key(),
        key2 in arb_key(),
    ) {
        prop_assume!(key1.as_bytes() != key2.as_bytes());
        let tables = test_tables();
        let schema1 = OpaqueSchema::new(key1, &tables);
        let schema2 = OpaqueSchema::new(key2, &tables);

        let t1 = schema1.table("users").unwrap();
        let t2 = schema2.table("users").unwrap();
        prop_assert_ne!(t1, t2);
    }

    /// Table names start with "t_" prefix.
    #[test]
    fn opaque_table_prefix(
        key in arb_key(),
    ) {
        let tables = test_tables();
        let schema = OpaqueSchema::new(key, &tables);

        for td in &tables {
            let opaque = schema.table(&td.name).unwrap();
            prop_assert!(opaque.starts_with("t_"),
                "Table '{}' -> '{}' doesn't start with t_", td.name, opaque);
        }
    }

    /// Column names start with "c_" prefix.
    #[test]
    fn opaque_column_prefix(
        key in arb_key(),
    ) {
        let tables = test_tables();
        let schema = OpaqueSchema::new(key, &tables);

        for td in &tables {
            for col in &td.columns {
                let opaque = schema.column(&td.name, col).unwrap();
                prop_assert!(opaque.starts_with("c_"),
                    "Column '{}.{}' -> '{}' doesn't start with c_", td.name, col, opaque);
            }
        }
    }

    /// Unknown table returns None.
    #[test]
    fn opaque_unknown_table_returns_none(
        key in arb_key(),
    ) {
        let tables = test_tables();
        let schema = OpaqueSchema::new(key, &tables);
        prop_assert!(schema.table("nonexistent").is_none());
    }

    /// Different columns in the same table produce different opaque names.
    #[test]
    fn opaque_columns_unique(
        key in arb_key(),
    ) {
        let tables = test_tables();
        let schema = OpaqueSchema::new(key, &tables);

        let c_id = schema.column("users", "id").unwrap().to_string();
        let c_name = schema.column("users", "name").unwrap().to_string();
        let c_email = schema.column("users", "email").unwrap().to_string();
        prop_assert_ne!(&c_id, &c_name);
        prop_assert_ne!(&c_id, &c_email);
        prop_assert_ne!(&c_name, &c_email);
    }
}
