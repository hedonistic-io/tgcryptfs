use thiserror::Error;

#[derive(Debug, Error)]
pub enum FuseError {
    #[error("inode not found: {0}")]
    InodeNotFound(u64),

    #[error("not a directory: {0}")]
    NotDirectory(u64),

    #[error("not a file: {0}")]
    NotFile(u64),

    #[error("file exists: {0}")]
    Exists(String),

    #[error("directory not empty: {0}")]
    NotEmpty(u64),

    #[error("permission denied")]
    PermissionDenied,

    #[error("store error: {0}")]
    Store(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

impl FuseError {
    /// Convert to libc errno for FUSE responses.
    pub fn to_errno(&self) -> i32 {
        match self {
            FuseError::InodeNotFound(_) => libc::ENOENT,
            FuseError::NotDirectory(_) => libc::ENOTDIR,
            FuseError::NotFile(_) => libc::EISDIR,
            FuseError::Exists(_) => libc::EEXIST,
            FuseError::NotEmpty(_) => libc::ENOTEMPTY,
            FuseError::PermissionDenied => libc::EACCES,
            FuseError::Store(_) => libc::EIO,
            FuseError::Io(_) => libc::EIO,
        }
    }

    /// Returns a user-facing suggestion for how to resolve this error.
    pub fn suggestion(&self) -> &'static str {
        match self {
            FuseError::InodeNotFound(_) => "The file or directory does not exist; check the path",
            FuseError::NotDirectory(_) => "The path component is not a directory",
            FuseError::NotFile(_) => "Cannot perform file operation on a directory",
            FuseError::Exists(_) => "A file or directory with that name already exists",
            FuseError::NotEmpty(_) => "Remove all contents before deleting the directory",
            FuseError::PermissionDenied => {
                "Check volume mount permissions or try mounting with `allow_other`"
            }
            FuseError::Store(_) => "Metadata store may be corrupted; try unmounting and remounting",
            FuseError::Io(_) => "Check available disk space and file permissions",
        }
    }
}

pub type Result<T> = std::result::Result<T, FuseError>;
