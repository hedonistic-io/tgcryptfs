use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use super::types::{FileType, Timestamps};
use crate::block::pointer::FileManifest;

/// Filesystem inode with all metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Inode {
    /// Unique inode number
    pub ino: u64,
    /// Parent directory inode (0 for root)
    pub parent: u64,
    /// Filename
    pub name: String,
    /// File type
    pub file_type: FileType,
    /// POSIX permissions (mode bits)
    pub mode: u32,
    /// Owner user ID
    pub uid: u32,
    /// Owner group ID
    pub gid: u32,
    /// File size in bytes
    pub size: u64,
    /// Number of hard links
    pub nlink: u32,
    /// Timestamps
    pub timestamps: Timestamps,
    /// Block manifest (for regular files)
    pub manifest: Option<FileManifest>,
    /// Symlink target (for symlinks)
    pub symlink_target: Option<String>,
    /// Child inode numbers (for directories)
    pub children: Vec<u64>,
    /// Extended attributes
    pub xattrs: HashMap<String, Vec<u8>>,
    /// Active mutability policy ID
    pub policy_id: Option<u32>,
}

impl Inode {
    /// Create a root directory inode.
    pub fn root() -> Self {
        Self {
            ino: 1,
            parent: 0,
            name: String::new(),
            file_type: FileType::Directory,
            mode: 0o755,
            uid: 0,
            gid: 0,
            size: 0,
            nlink: 2,
            timestamps: Timestamps::now(),
            manifest: None,
            symlink_target: None,
            children: Vec::new(),
            xattrs: HashMap::new(),
            policy_id: None,
        }
    }

    /// Create a new regular file inode.
    pub fn new_file(ino: u64, parent: u64, name: String, mode: u32, uid: u32, gid: u32) -> Self {
        Self {
            ino,
            parent,
            name,
            file_type: FileType::RegularFile,
            mode,
            uid,
            gid,
            size: 0,
            nlink: 1,
            timestamps: Timestamps::now(),
            manifest: Some(FileManifest::new(ino)),
            symlink_target: None,
            children: Vec::new(),
            xattrs: HashMap::new(),
            policy_id: None,
        }
    }

    /// Create a new directory inode.
    pub fn new_dir(ino: u64, parent: u64, name: String, mode: u32, uid: u32, gid: u32) -> Self {
        Self {
            ino,
            parent,
            name,
            file_type: FileType::Directory,
            mode,
            uid,
            gid,
            size: 0,
            nlink: 2,
            timestamps: Timestamps::now(),
            manifest: None,
            symlink_target: None,
            children: Vec::new(),
            xattrs: HashMap::new(),
            policy_id: None,
        }
    }

    /// Create a new symlink inode.
    pub fn new_symlink(
        ino: u64,
        parent: u64,
        name: String,
        target: String,
        uid: u32,
        gid: u32,
    ) -> Self {
        Self {
            ino,
            parent,
            name,
            file_type: FileType::Symlink,
            mode: 0o777,
            uid,
            gid,
            size: target.len() as u64,
            nlink: 1,
            timestamps: Timestamps::now(),
            manifest: None,
            symlink_target: Some(target),
            children: Vec::new(),
            xattrs: HashMap::new(),
            policy_id: None,
        }
    }

    pub fn is_dir(&self) -> bool {
        self.file_type == FileType::Directory
    }

    pub fn is_file(&self) -> bool {
        self.file_type == FileType::RegularFile
    }

    pub fn is_symlink(&self) -> bool {
        self.file_type == FileType::Symlink
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn root_inode() {
        let root = Inode::root();
        assert_eq!(root.ino, 1);
        assert_eq!(root.parent, 0);
        assert!(root.is_dir());
        assert_eq!(root.nlink, 2);
    }

    #[test]
    fn new_file_inode() {
        let f = Inode::new_file(2, 1, "test.txt".into(), 0o644, 1000, 1000);
        assert!(f.is_file());
        assert_eq!(f.nlink, 1);
        assert!(f.manifest.is_some());
        assert!(f.symlink_target.is_none());
    }

    #[test]
    fn new_dir_inode() {
        let d = Inode::new_dir(3, 1, "subdir".into(), 0o755, 1000, 1000);
        assert!(d.is_dir());
        assert_eq!(d.nlink, 2);
    }

    #[test]
    fn new_symlink_inode() {
        let s = Inode::new_symlink(4, 1, "link".into(), "/target".into(), 1000, 1000);
        assert!(s.is_symlink());
        assert_eq!(s.symlink_target, Some("/target".into()));
        assert_eq!(s.size, 7);
    }
}
