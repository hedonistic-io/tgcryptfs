use super::entry::{SnapshotEntry, SnapshotOperation, SnapshotState};
use crate::metadata::inode::Inode;
use uuid::Uuid;

/// Build a new snapshot entry for a filesystem operation.
pub fn create_entry(
    snapshot_id: u64,
    operation: SnapshotOperation,
    inode: u64,
    before: Option<SnapshotState>,
    after: Option<SnapshotState>,
    user_id: Option<Uuid>,
) -> SnapshotEntry {
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as i128;

    SnapshotEntry {
        snapshot_id,
        timestamp,
        operation,
        inode,
        before,
        after,
        user_id,
    }
}

/// Capture the current state of an inode for snapshot purposes.
pub fn capture_state(inode: &Inode) -> SnapshotState {
    SnapshotState {
        inode_snapshot: inode.clone(),
        manifest: inode.manifest.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_entry_sets_timestamp() {
        let entry = create_entry(1, SnapshotOperation::Create, 2, None, None, None);
        assert!(entry.timestamp > 0);
        assert_eq!(entry.snapshot_id, 1);
        assert_eq!(entry.inode, 2);
    }

    #[test]
    fn capture_state_clones_inode() {
        let inode = Inode::new_file(5, 1, "test.txt".into(), 0o644, 1000, 1000);
        let state = capture_state(&inode);
        assert_eq!(state.inode_snapshot.ino, 5);
        assert!(state.manifest.is_some());
    }
}
