use serde::{Deserialize, Serialize};

/// POSIX file type.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum FileType {
    RegularFile,
    Directory,
    Symlink,
}

/// Timestamps stored as nanoseconds since UNIX epoch.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct Timestamps {
    pub atime_ns: i128,
    pub mtime_ns: i128,
    pub ctime_ns: i128,
    pub crtime_ns: i128,
}

impl Timestamps {
    pub fn now() -> Self {
        let ns = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as i128;
        Self {
            atime_ns: ns,
            mtime_ns: ns,
            ctime_ns: ns,
            crtime_ns: ns,
        }
    }
}
