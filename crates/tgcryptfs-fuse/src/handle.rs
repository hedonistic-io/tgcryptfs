use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use zeroize::Zeroize;

/// Tracks open file handles for the FUSE filesystem.
pub struct HandleTable {
    next_fh: AtomicU64,
    handles: Mutex<HashMap<u64, FileHandle>>,
}

/// A single open file handle.
#[derive(Debug, Clone)]
pub struct FileHandle {
    /// File handle ID.
    pub fh: u64,
    /// Inode number this handle points to.
    pub ino: u64,
    /// Open flags.
    pub flags: i32,
    /// Write buffer for coalescing writes.
    pub write_buffer: Vec<u8>,
    /// Whether this handle has been modified (dirty).
    pub dirty: bool,
}

impl HandleTable {
    pub fn new() -> Self {
        Self {
            next_fh: AtomicU64::new(1),
            handles: Mutex::new(HashMap::new()),
        }
    }

    /// Open a new file handle.
    pub fn open(&self, ino: u64, flags: i32) -> u64 {
        let fh = self.next_fh.fetch_add(1, Ordering::Relaxed);
        let handle = FileHandle {
            fh,
            ino,
            flags,
            write_buffer: Vec::new(),
            dirty: false,
        };
        self.handles.lock().unwrap().insert(fh, handle);
        fh
    }

    /// Get a file handle by ID.
    pub fn get(&self, fh: u64) -> Option<FileHandle> {
        self.handles.lock().unwrap().get(&fh).cloned()
    }

    /// Update a file handle.
    pub fn update<F>(&self, fh: u64, f: F) -> bool
    where
        F: FnOnce(&mut FileHandle),
    {
        let mut handles = self.handles.lock().unwrap();
        if let Some(handle) = handles.get_mut(&fh) {
            f(handle);
            true
        } else {
            false
        }
    }

    /// Close (release) a file handle. Zeroizes the write buffer before dropping.
    /// Returns the handle if it existed.
    pub fn close(&self, fh: u64) -> Option<FileHandle> {
        let mut handle = self.handles.lock().unwrap().remove(&fh)?;
        handle.write_buffer.zeroize();
        Some(handle)
    }

    /// Get all open handles for an inode.
    pub fn handles_for_ino(&self, ino: u64) -> Vec<u64> {
        self.handles
            .lock()
            .unwrap()
            .iter()
            .filter(|(_, h)| h.ino == ino)
            .map(|(fh, _)| *fh)
            .collect()
    }

    /// Count open handles.
    pub fn count(&self) -> usize {
        self.handles.lock().unwrap().len()
    }
}

impl Default for HandleTable {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_and_get() {
        let table = HandleTable::new();
        let fh = table.open(42, libc::O_RDONLY);
        let handle = table.get(fh).unwrap();
        assert_eq!(handle.ino, 42);
        assert_eq!(handle.flags, libc::O_RDONLY);
        assert!(!handle.dirty);
    }

    #[test]
    fn close_handle() {
        let table = HandleTable::new();
        let fh = table.open(42, 0);
        assert_eq!(table.count(), 1);

        let handle = table.close(fh).unwrap();
        assert_eq!(handle.ino, 42);
        assert_eq!(table.count(), 0);
        assert!(table.get(fh).is_none());
    }

    #[test]
    fn update_handle() {
        let table = HandleTable::new();
        let fh = table.open(42, 0);

        table.update(fh, |h| {
            h.dirty = true;
            h.write_buffer = vec![1, 2, 3];
        });

        let handle = table.get(fh).unwrap();
        assert!(handle.dirty);
        assert_eq!(handle.write_buffer, vec![1, 2, 3]);
    }

    #[test]
    fn unique_fh_ids() {
        let table = HandleTable::new();
        let fh1 = table.open(1, 0);
        let fh2 = table.open(2, 0);
        let fh3 = table.open(3, 0);
        assert_ne!(fh1, fh2);
        assert_ne!(fh2, fh3);
    }

    #[test]
    fn handles_for_ino() {
        let table = HandleTable::new();
        let fh1 = table.open(42, libc::O_RDONLY);
        let fh2 = table.open(42, libc::O_WRONLY);
        let _fh3 = table.open(99, libc::O_RDONLY);

        let handles = table.handles_for_ino(42);
        assert_eq!(handles.len(), 2);
        assert!(handles.contains(&fh1));
        assert!(handles.contains(&fh2));
    }

    #[test]
    fn close_nonexistent() {
        let table = HandleTable::new();
        assert!(table.close(999).is_none());
    }
}
