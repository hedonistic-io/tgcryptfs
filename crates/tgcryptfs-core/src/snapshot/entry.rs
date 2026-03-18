use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::block::pointer::FileManifest;
use crate::metadata::inode::Inode;

/// A single snapshot entry recording a filesystem operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotEntry {
    /// Unique snapshot ID (auto-incrementing)
    pub snapshot_id: u64,
    /// Timestamp (nanoseconds since epoch)
    pub timestamp: i128,
    /// Operation that triggered this snapshot
    pub operation: SnapshotOperation,
    /// Affected inode
    pub inode: u64,
    /// Previous state (for rollback)
    pub before: Option<SnapshotState>,
    /// New state
    pub after: Option<SnapshotState>,
    /// User who performed the operation (for shared volumes)
    pub user_id: Option<Uuid>,
}

/// The type of filesystem operation that was recorded.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SnapshotOperation {
    Create,
    Write,
    Delete,
    Rename { old_name: String, new_name: String },
    Mkdir,
    Rmdir,
    Link,
    Symlink,
    SetAttr,
    Restore { from_snapshot: u64 },
}

/// State captured at a point in time for rollback.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotState {
    /// Inode metadata at this point
    pub inode_snapshot: Inode,
    /// For files: the manifest at this point (block pointers)
    pub manifest: Option<FileManifest>,
}
