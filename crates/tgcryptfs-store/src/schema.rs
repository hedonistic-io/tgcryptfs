use crate::opaque_schema::TableDef;

/// All logical table definitions for the TGCryptFS metadata database.
/// These names are never stored directly - OpaqueSchema maps them to opaque identifiers.
pub fn logical_tables() -> Vec<TableDef> {
    vec![
        TableDef {
            name: "volume".into(),
            columns: vec![
                "volume_id".into(),
                "data".into(),
                "created_at".into(),
                "updated_at".into(),
            ],
            indexes: vec![],
        },
        TableDef {
            name: "inodes".into(),
            columns: vec![
                "ino".into(),
                "parent".into(),
                "name_hash".into(),
                "data".into(),
                "version".into(),
            ],
            indexes: vec!["inodes_parent".into(), "inodes_parent_name".into()],
        },
        TableDef {
            name: "blocks".into(),
            columns: vec![
                "rid".into(),
                "content_hash".into(),
                "message_id".into(),
                "encrypted_size".into(),
                "epoch".into(),
                "ref_count".into(),
                "compressed".into(),
            ],
            indexes: vec!["blocks_content_hash".into(), "blocks_message_id".into()],
        },
        TableDef {
            name: "manifests".into(),
            columns: vec![
                "ino".into(),
                "version".into(),
                "data".into(),
                "created_at".into(),
            ],
            indexes: vec![],
        },
        TableDef {
            name: "snapshots".into(),
            columns: vec![
                "sid".into(),
                "timestamp".into(),
                "data".into(),
                "ino".into(),
            ],
            indexes: vec!["snapshots_ts".into(), "snapshots_ino".into()],
        },
        TableDef {
            name: "policies".into(),
            columns: vec!["pid".into(), "data".into()],
            indexes: vec![],
        },
        TableDef {
            name: "path_policies".into(),
            columns: vec!["path_pattern".into(), "pid".into(), "priority".into()],
            indexes: vec!["path_policies_pattern".into()],
        },
        TableDef {
            name: "users".into(),
            columns: vec!["uid".into(), "data".into(), "active".into()],
            indexes: vec![],
        },
        TableDef {
            name: "audit_log".into(),
            columns: vec![
                "id".into(),
                "timestamp".into(),
                "uid".into(),
                "action".into(),
                "details".into(),
            ],
            indexes: vec!["audit_ts".into()],
        },
        TableDef {
            name: "deadman".into(),
            columns: vec![
                "vid".into(),
                "data".into(),
                "armed".into(),
                "last_check".into(),
            ],
            indexes: vec![],
        },
        TableDef {
            name: "shares".into(),
            columns: vec![
                "sid".into(),
                "volume_id".into(),
                "uid".into(),
                "data".into(),
                "active".into(),
            ],
            indexes: vec!["shares_volume".into()],
        },
        TableDef {
            name: "invites".into(),
            columns: vec![
                "invite_id".into(),
                "volume_id".into(),
                "data".into(),
                "active".into(),
            ],
            indexes: vec!["invites_volume".into()],
        },
    ]
}
