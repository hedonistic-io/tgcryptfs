use std::ffi::OsStr;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use fuser::{
    FileAttr, FileType as FuseFileType, Filesystem, ReplyAttr, ReplyData, ReplyDirectory,
    ReplyEmpty, ReplyEntry, ReplyOpen, ReplyWrite, Request, TimeOrNow,
};
use rusqlite::Connection;
use tgcryptfs_core::crypto::keys::SymmetricKey;
use tgcryptfs_core::metadata::inode::Inode;
use tgcryptfs_core::metadata::types::FileType;

use tgcryptfs_cache::block_cache::BlockCache;
use tgcryptfs_core::block::pointer::{BlockPointer, FileManifest};
use tgcryptfs_store::block_store::{BlockRecord, BlockStore};
use tgcryptfs_store::inode_store::InodeStore;
use tgcryptfs_store::opaque_schema::OpaqueSchema;
use tgcryptfs_telegram::client::BlockTransport;

use crate::handle::HandleTable;

const TTL: Duration = Duration::from_secs(1);
const BLOCK_SIZE: u32 = 4096;

/// TGCryptFS FUSE filesystem.
///
/// Bridges between the FUSE kernel module and our encrypted storage layer.
/// Uses tokio for async dispatch to avoid blocking FUSE threads (fixes v1 100% CPU bug).
pub struct CryptFs {
    conn: Connection,
    schema: OpaqueSchema,
    meta_key: SymmetricKey,
    #[allow(dead_code)] // Reserved for epoch-aware block encryption
    data_key: SymmetricKey,
    pub handles: HandleTable,
    uid: u32,
    gid: u32,
    /// Optional block transport for uploading/downloading blocks to Telegram.
    transport: Option<Arc<dyn BlockTransport>>,
    /// Optional block cache for caching downloaded blocks locally.
    cache: Option<Arc<BlockCache>>,
    /// Tokio runtime handle for async operations from sync FUSE callbacks.
    rt: Option<tokio::runtime::Handle>,
}

impl CryptFs {
    pub fn new(
        conn: Connection,
        schema: OpaqueSchema,
        meta_key: SymmetricKey,
        data_key: SymmetricKey,
        uid: u32,
        gid: u32,
    ) -> Self {
        Self {
            conn,
            schema,
            meta_key,
            data_key,
            handles: HandleTable::new(),
            uid,
            gid,
            transport: None,
            cache: None,
            rt: None,
        }
    }

    /// Set the block transport (enables Telegram I/O).
    pub fn with_transport(mut self, transport: Arc<dyn BlockTransport>) -> Self {
        self.transport = Some(transport);
        self
    }

    /// Set the block cache.
    pub fn with_cache(mut self, cache: Arc<BlockCache>) -> Self {
        self.cache = Some(cache);
        self
    }

    /// Set the tokio runtime handle for async dispatch.
    pub fn with_runtime(mut self, rt: tokio::runtime::Handle) -> Self {
        self.rt = Some(rt);
        self
    }

    pub fn store(&self) -> InodeStore<'_> {
        InodeStore::new(&self.conn, &self.schema, &self.meta_key)
    }

    pub fn block_store(&self) -> BlockStore<'_> {
        BlockStore::new(&self.conn, &self.schema)
    }

    fn inode_to_attr(inode: &Inode) -> FileAttr {
        let kind = match inode.file_type {
            FileType::RegularFile => FuseFileType::RegularFile,
            FileType::Directory => FuseFileType::Directory,
            FileType::Symlink => FuseFileType::Symlink,
        };

        let ts = &inode.timestamps;
        let atime = ns_to_system_time(ts.atime_ns);
        let mtime = ns_to_system_time(ts.mtime_ns);
        let ctime = ns_to_system_time(ts.ctime_ns);
        let crtime = ns_to_system_time(ts.crtime_ns);

        FileAttr {
            ino: inode.ino,
            size: inode.size,
            blocks: inode.size.div_ceil(512),
            atime,
            mtime,
            ctime,
            crtime,
            kind,
            perm: inode.mode as u16,
            nlink: inode.nlink,
            uid: inode.uid,
            gid: inode.gid,
            rdev: 0,
            blksize: BLOCK_SIZE,
            flags: 0,
        }
    }

    /// Upload dirty write buffer as a block via transport.
    /// Returns the BlockRecord for the uploaded block.
    pub fn flush_to_transport(&self, ino: u64, data: &[u8]) -> Option<BlockRecord> {
        let transport = self.transport.as_ref()?;
        let rt = self.rt.as_ref()?;

        // Compute content hash for dedup
        let content_hash = blake3::hash(data);
        let content_hash_bytes: [u8; 32] = *content_hash.as_bytes();

        // Check for dedup
        let block_store = self.block_store();
        if let Ok(Some(existing)) = block_store.find_by_content_hash(&content_hash_bytes) {
            let _ = block_store.increment_ref(&existing.rid);
            return Some(existing);
        }

        // Upload the data
        let filename = format!("block_{ino}_{}", hex::encode(&content_hash_bytes[..8]));
        let upload_result = rt.block_on(transport.upload_block(data, &filename));

        match upload_result {
            Ok(result) => {
                let mut rid = [0u8; 32];
                use rand::RngCore;
                rand::rngs::OsRng.fill_bytes(&mut rid);

                let block = BlockRecord {
                    rid,
                    content_hash: content_hash_bytes,
                    message_id: result.message_id,
                    encrypted_size: result.size as i64,
                    epoch: 0,
                    ref_count: 1,
                    compressed: false,
                };

                if let Err(e) = block_store.insert(&block) {
                    tracing::error!(error = %e, "failed to insert block record");
                    return None;
                }

                // Cache the block
                if let Some(cache) = &self.cache {
                    let _ = cache.put(&rid, data);
                }

                Some(block)
            }
            Err(e) => {
                tracing::error!(error = %e, "failed to upload block");
                None
            }
        }
    }

    /// Download a block by message_id via transport.
    pub fn download_block(&self, message_id: i64, rid: &[u8; 32]) -> Option<Vec<u8>> {
        // Try cache first
        if let Some(cache) = &self.cache {
            if let Ok(data) = cache.get(rid) {
                return Some(data);
            }
        }

        // Download from transport
        let transport = self.transport.as_ref()?;
        let rt = self.rt.as_ref()?;

        match rt.block_on(transport.download_block(message_id)) {
            Ok(result) => {
                // Cache for future reads
                if let Some(cache) = &self.cache {
                    let _ = cache.put(rid, &result.data);
                }
                Some(result.data)
            }
            Err(e) => {
                tracing::error!(error = %e, "failed to download block");
                None
            }
        }
    }
}

fn ns_to_system_time(ns: i128) -> SystemTime {
    if ns >= 0 {
        UNIX_EPOCH + Duration::from_nanos(ns as u64)
    } else {
        UNIX_EPOCH
    }
}

fn system_time_to_ns(t: SystemTime) -> i128 {
    t.duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as i128)
        .unwrap_or(0)
}

fn time_or_now_to_ns(t: TimeOrNow) -> i128 {
    match t {
        TimeOrNow::SpecificTime(st) => system_time_to_ns(st),
        TimeOrNow::Now => system_time_to_ns(SystemTime::now()),
    }
}

impl Filesystem for CryptFs {
    fn init(
        &mut self,
        _req: &Request<'_>,
        _config: &mut fuser::KernelConfig,
    ) -> std::result::Result<(), libc::c_int> {
        tracing::info!("CryptFs initialized");

        // Ensure root inode exists
        let store = self.store();
        match store.get(1) {
            Ok(Some(_)) => {}
            Ok(None) => {
                let mut root = Inode::root();
                root.uid = self.uid;
                root.gid = self.gid;
                if let Err(e) = store.insert(&root) {
                    tracing::error!(error = %e, "failed to create root inode");
                    return Err(libc::EIO);
                }
            }
            Err(e) => {
                tracing::error!(error = %e, "failed to check root inode");
                return Err(libc::EIO);
            }
        }

        Ok(())
    }

    fn lookup(&mut self, _req: &Request<'_>, parent: u64, name: &OsStr, reply: ReplyEntry) {
        let name_str = match name.to_str() {
            Some(s) => s,
            None => {
                reply.error(libc::EINVAL);
                return;
            }
        };

        let store = self.store();
        match store.lookup(parent, name_str) {
            Ok(Some(inode)) => {
                let attr = Self::inode_to_attr(&inode);
                reply.entry(&TTL, &attr, 0);
            }
            Ok(None) => {
                reply.error(libc::ENOENT);
            }
            Err(e) => {
                tracing::error!(error = %e, "lookup failed");
                reply.error(libc::EIO);
            }
        }
    }

    fn getattr(&mut self, _req: &Request<'_>, ino: u64, reply: ReplyAttr) {
        let store = self.store();
        match store.get(ino) {
            Ok(Some(inode)) => {
                let attr = Self::inode_to_attr(&inode);
                reply.attr(&TTL, &attr);
            }
            Ok(None) => {
                reply.error(libc::ENOENT);
            }
            Err(e) => {
                tracing::error!(error = %e, "getattr failed");
                reply.error(libc::EIO);
            }
        }
    }

    fn setattr(
        &mut self,
        _req: &Request<'_>,
        ino: u64,
        mode: Option<u32>,
        uid: Option<u32>,
        gid: Option<u32>,
        size: Option<u64>,
        atime: Option<TimeOrNow>,
        mtime: Option<TimeOrNow>,
        _ctime: Option<SystemTime>,
        _fh: Option<u64>,
        _crtime: Option<SystemTime>,
        _chgtime: Option<SystemTime>,
        _bkuptime: Option<SystemTime>,
        _flags: Option<u32>,
        reply: ReplyAttr,
    ) {
        let store = self.store();
        match store.get(ino) {
            Ok(Some(mut inode)) => {
                if let Some(m) = mode {
                    inode.mode = m;
                }
                if let Some(u) = uid {
                    inode.uid = u;
                }
                if let Some(g) = gid {
                    inode.gid = g;
                }
                if let Some(s) = size {
                    inode.size = s;
                }
                if let Some(a) = atime {
                    inode.timestamps.atime_ns = time_or_now_to_ns(a);
                }
                if let Some(m) = mtime {
                    inode.timestamps.mtime_ns = time_or_now_to_ns(m);
                }
                inode.timestamps.ctime_ns = system_time_to_ns(SystemTime::now());

                match store.update(&inode) {
                    Ok(()) => {
                        let attr = Self::inode_to_attr(&inode);
                        reply.attr(&TTL, &attr);
                    }
                    Err(e) => {
                        tracing::error!(error = %e, "setattr update failed");
                        reply.error(libc::EIO);
                    }
                }
            }
            Ok(None) => reply.error(libc::ENOENT),
            Err(e) => {
                tracing::error!(error = %e, "setattr get failed");
                reply.error(libc::EIO);
            }
        }
    }

    fn mkdir(
        &mut self,
        req: &Request<'_>,
        parent: u64,
        name: &OsStr,
        mode: u32,
        _umask: u32,
        reply: ReplyEntry,
    ) {
        let name_str = match name.to_str() {
            Some(s) => s,
            None => {
                reply.error(libc::EINVAL);
                return;
            }
        };

        let store = self.store();

        // Check parent exists and is a directory
        let mut parent_inode = match store.get(parent) {
            Ok(Some(i)) if i.is_dir() => i,
            Ok(Some(_)) => {
                reply.error(libc::ENOTDIR);
                return;
            }
            Ok(None) => {
                reply.error(libc::ENOENT);
                return;
            }
            Err(e) => {
                tracing::error!(error = %e, "mkdir parent lookup failed");
                reply.error(libc::EIO);
                return;
            }
        };

        // Check name doesn't already exist
        if let Ok(Some(_)) = store.lookup(parent, name_str) {
            reply.error(libc::EEXIST);
            return;
        }

        // Allocate new inode number
        let ino = match store.next_ino() {
            Ok(ino) => ino,
            Err(e) => {
                tracing::error!(error = %e, "mkdir next_ino failed");
                reply.error(libc::EIO);
                return;
            }
        };

        let dir = Inode::new_dir(
            ino,
            parent,
            name_str.to_string(),
            mode,
            req.uid(),
            req.gid(),
        );

        if let Err(e) = store.insert(&dir) {
            tracing::error!(error = %e, "mkdir insert failed");
            reply.error(libc::EIO);
            return;
        }

        // Update parent children list
        parent_inode.children.push(ino);
        parent_inode.nlink += 1;
        if let Err(e) = store.update(&parent_inode) {
            tracing::error!(error = %e, "mkdir parent update failed");
            reply.error(libc::EIO);
            return;
        }

        let attr = Self::inode_to_attr(&dir);
        reply.entry(&TTL, &attr, 0);
    }

    fn create(
        &mut self,
        req: &Request<'_>,
        parent: u64,
        name: &OsStr,
        mode: u32,
        _umask: u32,
        flags: i32,
        reply: fuser::ReplyCreate,
    ) {
        let name_str = match name.to_str() {
            Some(s) => s,
            None => {
                reply.error(libc::EINVAL);
                return;
            }
        };

        let store = self.store();

        // Check parent
        let mut parent_inode = match store.get(parent) {
            Ok(Some(i)) if i.is_dir() => i,
            Ok(Some(_)) => {
                reply.error(libc::ENOTDIR);
                return;
            }
            Ok(None) => {
                reply.error(libc::ENOENT);
                return;
            }
            Err(_) => {
                reply.error(libc::EIO);
                return;
            }
        };

        // Check name doesn't exist
        if let Ok(Some(_)) = store.lookup(parent, name_str) {
            reply.error(libc::EEXIST);
            return;
        }

        let ino = match store.next_ino() {
            Ok(ino) => ino,
            Err(_) => {
                reply.error(libc::EIO);
                return;
            }
        };

        let file = Inode::new_file(
            ino,
            parent,
            name_str.to_string(),
            mode,
            req.uid(),
            req.gid(),
        );

        if let Err(e) = store.insert(&file) {
            tracing::error!(error = %e, "create insert failed");
            reply.error(libc::EIO);
            return;
        }

        parent_inode.children.push(ino);
        if let Err(e) = store.update(&parent_inode) {
            tracing::error!(error = %e, "create parent update failed");
            reply.error(libc::EIO);
            return;
        }

        let fh = self.handles.open(ino, flags);
        let attr = Self::inode_to_attr(&file);
        reply.created(&TTL, &attr, 0, fh, 0);
    }

    fn open(&mut self, _req: &Request<'_>, ino: u64, flags: i32, reply: ReplyOpen) {
        let store = self.store();
        match store.get(ino) {
            Ok(Some(_)) => {
                let fh = self.handles.open(ino, flags);
                reply.opened(fh, 0);
            }
            Ok(None) => reply.error(libc::ENOENT),
            Err(_) => reply.error(libc::EIO),
        }
    }

    fn release(
        &mut self,
        _req: &Request<'_>,
        ino: u64,
        fh: u64,
        _flags: i32,
        _lock_owner: Option<u64>,
        _flush: bool,
        reply: ReplyEmpty,
    ) {
        // If handle is dirty and we have a transport, flush to Telegram
        if let Some(handle) = self.handles.get(fh) {
            if handle.dirty && !handle.write_buffer.is_empty() && self.transport.is_some() {
                let data = &handle.write_buffer;
                if let Some(block) = self.flush_to_transport(ino, data) {
                    // Build manifest with the uploaded block
                    let bp = BlockPointer {
                        rid: block.rid,
                        message_id: block.message_id,
                        file_offset: 0,
                        length: data.len() as u64,
                        block_data_offset: 0,
                        encrypted_size: block.encrypted_size as u64,
                        compressed: block.compressed,
                        content_hash: block.content_hash,
                        epoch: block.epoch,
                    };

                    let file_hash = *blake3::hash(data).as_bytes();
                    let manifest = FileManifest {
                        inode: ino,
                        version: 1,
                        total_size: data.len() as u64,
                        file_hash,
                        blocks: vec![bp],
                    };

                    // Update inode with manifest
                    let store = self.store();
                    if let Ok(Some(mut inode)) = store.get(ino) {
                        inode.manifest = Some(manifest);
                        inode.size = data.len() as u64;
                        let _ = store.update(&inode);
                    }

                    tracing::info!(ino, "flushed dirty buffer to transport");
                }
            }
        }

        self.handles.close(fh);
        reply.ok();
    }

    fn read(
        &mut self,
        _req: &Request<'_>,
        ino: u64,
        fh: u64,
        offset: i64,
        size: u32,
        _flags: i32,
        _lock_owner: Option<u64>,
        reply: ReplyData,
    ) {
        let store = self.store();
        match store.get(ino) {
            Ok(Some(inode)) => {
                if !inode.is_file() {
                    reply.error(libc::EISDIR);
                    return;
                }

                // Try reading from handle's write buffer first (in-memory data)
                if let Some(handle) = self.handles.get(fh) {
                    if !handle.write_buffer.is_empty() {
                        let offset = offset as usize;
                        let end = std::cmp::min(offset + size as usize, handle.write_buffer.len());
                        if offset >= handle.write_buffer.len() {
                            reply.data(&[]);
                        } else {
                            reply.data(&handle.write_buffer[offset..end]);
                        }
                        return;
                    }
                }

                // No in-memory data — try reading from manifest blocks
                if let Some(ref manifest) = inode.manifest {
                    if !manifest.blocks.is_empty() {
                        let read_offset = offset as u64;
                        let read_len = u64::from(size);
                        let needed = manifest.blocks_in_range(read_offset, read_len);

                        let mut result_buf = vec![0u8; size as usize];
                        let mut ok = true;

                        for bp in &needed {
                            if let Some(data) = self.download_block(bp.message_id, &bp.rid) {
                                // Calculate which portion of this block falls in our range
                                let block_start = bp.file_offset;
                                let block_end = bp.file_offset + bp.length;
                                let read_start = read_offset.max(block_start);
                                let read_end = (read_offset + read_len).min(block_end);

                                if read_start < read_end {
                                    let src_off = (read_start - block_start) as usize;
                                    let dst_off = (read_start - read_offset) as usize;
                                    let copy_len = (read_end - read_start) as usize;

                                    let src_end = src_off + copy_len;
                                    if src_end <= data.len()
                                        && dst_off + copy_len <= result_buf.len()
                                    {
                                        result_buf[dst_off..dst_off + copy_len]
                                            .copy_from_slice(&data[src_off..src_end]);
                                    }
                                }
                            } else {
                                ok = false;
                                break;
                            }
                        }

                        if ok && !needed.is_empty() {
                            let actual_len = std::cmp::min(
                                u64::from(size),
                                inode.size.saturating_sub(read_offset),
                            ) as usize;
                            reply.data(&result_buf[..actual_len]);
                            return;
                        }
                    }
                }

                reply.data(&[]);
            }
            Ok(None) => reply.error(libc::ENOENT),
            Err(_) => reply.error(libc::EIO),
        }
    }

    fn write(
        &mut self,
        _req: &Request<'_>,
        ino: u64,
        fh: u64,
        offset: i64,
        data: &[u8],
        _write_flags: u32,
        _flags: i32,
        _lock_owner: Option<u64>,
        reply: ReplyWrite,
    ) {
        let offset = offset as usize;

        let updated = self.handles.update(fh, |h| {
            // Extend buffer if needed
            if offset + data.len() > h.write_buffer.len() {
                h.write_buffer.resize(offset + data.len(), 0);
            }
            h.write_buffer[offset..offset + data.len()].copy_from_slice(data);
            h.dirty = true;
        });

        if updated {
            // Update inode size
            if let Some(handle) = self.handles.get(fh) {
                let store = self.store();
                if let Ok(Some(mut inode)) = store.get(ino) {
                    inode.size = handle.write_buffer.len() as u64;
                    inode.timestamps.mtime_ns = system_time_to_ns(SystemTime::now());
                    let _ = store.update(&inode);
                }
            }
            reply.written(data.len() as u32);
        } else {
            reply.error(libc::EBADF);
        }
    }

    fn unlink(&mut self, _req: &Request<'_>, parent: u64, name: &OsStr, reply: ReplyEmpty) {
        let name_str = match name.to_str() {
            Some(s) => s,
            None => {
                reply.error(libc::EINVAL);
                return;
            }
        };

        let store = self.store();

        let inode = match store.lookup(parent, name_str) {
            Ok(Some(i)) => i,
            Ok(None) => {
                reply.error(libc::ENOENT);
                return;
            }
            Err(_) => {
                reply.error(libc::EIO);
                return;
            }
        };

        if inode.is_dir() {
            reply.error(libc::EISDIR);
            return;
        }

        // Remove from parent's children
        if let Ok(Some(mut parent_inode)) = store.get(parent) {
            parent_inode.children.retain(|&c| c != inode.ino);
            let _ = store.update(&parent_inode);
        }

        // Delete the inode
        if let Err(e) = store.delete(inode.ino) {
            tracing::error!(error = %e, "unlink delete failed");
            reply.error(libc::EIO);
            return;
        }

        reply.ok();
    }

    fn rmdir(&mut self, _req: &Request<'_>, parent: u64, name: &OsStr, reply: ReplyEmpty) {
        let name_str = match name.to_str() {
            Some(s) => s,
            None => {
                reply.error(libc::EINVAL);
                return;
            }
        };

        let store = self.store();

        let inode = match store.lookup(parent, name_str) {
            Ok(Some(i)) => i,
            Ok(None) => {
                reply.error(libc::ENOENT);
                return;
            }
            Err(_) => {
                reply.error(libc::EIO);
                return;
            }
        };

        if !inode.is_dir() {
            reply.error(libc::ENOTDIR);
            return;
        }

        if !inode.children.is_empty() {
            reply.error(libc::ENOTEMPTY);
            return;
        }

        // Remove from parent's children
        if let Ok(Some(mut parent_inode)) = store.get(parent) {
            parent_inode.children.retain(|&c| c != inode.ino);
            parent_inode.nlink = parent_inode.nlink.saturating_sub(1);
            let _ = store.update(&parent_inode);
        }

        if let Err(e) = store.delete(inode.ino) {
            tracing::error!(error = %e, "rmdir delete failed");
            reply.error(libc::EIO);
            return;
        }

        reply.ok();
    }

    fn readdir(
        &mut self,
        _req: &Request<'_>,
        ino: u64,
        _fh: u64,
        offset: i64,
        mut reply: ReplyDirectory,
    ) {
        let store = self.store();

        let inode = match store.get(ino) {
            Ok(Some(i)) if i.is_dir() => i,
            Ok(Some(_)) => {
                reply.error(libc::ENOTDIR);
                return;
            }
            Ok(None) => {
                reply.error(libc::ENOENT);
                return;
            }
            Err(_) => {
                reply.error(libc::EIO);
                return;
            }
        };

        let mut entries: Vec<(u64, FuseFileType, String)> = Vec::new();
        entries.push((ino, FuseFileType::Directory, ".".to_string()));
        entries.push((
            inode.parent.max(1),
            FuseFileType::Directory,
            "..".to_string(),
        ));

        // Add child entries
        match store.list_children(ino) {
            Ok(children) => {
                for child in children {
                    let kind = match child.file_type {
                        FileType::RegularFile => FuseFileType::RegularFile,
                        FileType::Directory => FuseFileType::Directory,
                        FileType::Symlink => FuseFileType::Symlink,
                    };
                    entries.push((child.ino, kind, child.name.clone()));
                }
            }
            Err(e) => {
                tracing::error!(error = %e, "readdir list_children failed");
                reply.error(libc::EIO);
                return;
            }
        }

        for (i, (ino, kind, name)) in entries.iter().enumerate().skip(offset as usize) {
            if reply.add(*ino, (i + 1) as i64, *kind, name) {
                break;
            }
        }

        reply.ok();
    }

    fn rename(
        &mut self,
        _req: &Request<'_>,
        parent: u64,
        name: &OsStr,
        newparent: u64,
        newname: &OsStr,
        _flags: u32,
        reply: ReplyEmpty,
    ) {
        let name_str = match name.to_str() {
            Some(s) => s,
            None => {
                reply.error(libc::EINVAL);
                return;
            }
        };
        let newname_str = match newname.to_str() {
            Some(s) => s,
            None => {
                reply.error(libc::EINVAL);
                return;
            }
        };

        let store = self.store();

        // Find source inode
        let mut inode = match store.lookup(parent, name_str) {
            Ok(Some(i)) => i,
            Ok(None) => {
                reply.error(libc::ENOENT);
                return;
            }
            Err(_) => {
                reply.error(libc::EIO);
                return;
            }
        };

        // Check if destination already exists and remove it
        if let Ok(Some(existing)) = store.lookup(newparent, newname_str) {
            if existing.is_dir() && !existing.children.is_empty() {
                reply.error(libc::ENOTEMPTY);
                return;
            }
            let _ = store.delete(existing.ino);
            if let Ok(Some(mut np)) = store.get(newparent) {
                np.children.retain(|&c| c != existing.ino);
                let _ = store.update(&np);
            }
        }

        // Remove from old parent
        if let Ok(Some(mut old_parent)) = store.get(parent) {
            old_parent.children.retain(|&c| c != inode.ino);
            if inode.is_dir() {
                old_parent.nlink = old_parent.nlink.saturating_sub(1);
            }
            let _ = store.update(&old_parent);
        }

        // Update inode
        inode.name = newname_str.to_string();
        inode.parent = newparent;
        inode.timestamps.ctime_ns = system_time_to_ns(SystemTime::now());
        let _ = store.update(&inode);

        // Add to new parent
        if let Ok(Some(mut new_parent)) = store.get(newparent) {
            new_parent.children.push(inode.ino);
            if inode.is_dir() {
                new_parent.nlink += 1;
            }
            let _ = store.update(&new_parent);
        }

        reply.ok();
    }

    fn readlink(&mut self, _req: &Request<'_>, ino: u64, reply: ReplyData) {
        let store = self.store();
        match store.get(ino) {
            Ok(Some(inode)) => {
                if let Some(target) = &inode.symlink_target {
                    reply.data(target.as_bytes());
                } else {
                    reply.error(libc::EINVAL);
                }
            }
            Ok(None) => reply.error(libc::ENOENT),
            Err(_) => reply.error(libc::EIO),
        }
    }

    fn symlink(
        &mut self,
        req: &Request<'_>,
        parent: u64,
        link_name: &OsStr,
        target: &Path,
        reply: ReplyEntry,
    ) {
        let name_str = match link_name.to_str() {
            Some(s) => s,
            None => {
                reply.error(libc::EINVAL);
                return;
            }
        };
        let target_str = match target.to_str() {
            Some(s) => s,
            None => {
                reply.error(libc::EINVAL);
                return;
            }
        };

        let store = self.store();

        let mut parent_inode = match store.get(parent) {
            Ok(Some(i)) if i.is_dir() => i,
            _ => {
                reply.error(libc::ENOENT);
                return;
            }
        };

        let ino = match store.next_ino() {
            Ok(ino) => ino,
            Err(_) => {
                reply.error(libc::EIO);
                return;
            }
        };

        let symlink = Inode::new_symlink(
            ino,
            parent,
            name_str.to_string(),
            target_str.to_string(),
            req.uid(),
            req.gid(),
        );

        if let Err(e) = store.insert(&symlink) {
            tracing::error!(error = %e, "symlink insert failed");
            reply.error(libc::EIO);
            return;
        }

        parent_inode.children.push(ino);
        let _ = store.update(&parent_inode);

        let attr = Self::inode_to_attr(&symlink);
        reply.entry(&TTL, &attr, 0);
    }

    fn statfs(&mut self, _req: &Request<'_>, _ino: u64, reply: fuser::ReplyStatfs) {
        // Report virtual filesystem stats
        reply.statfs(
            0, // blocks
            0, // bfree
            0, // bavail
            0, // files
            0, // ffree
            BLOCK_SIZE, 256, // namelen
            BLOCK_SIZE,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tgcryptfs_store::migrations::initialize_database;
    use tgcryptfs_store::schema::logical_tables;

    #[test]
    fn ns_time_conversion() {
        let now = SystemTime::now();
        let ns = system_time_to_ns(now);
        assert!(ns > 0);

        let converted = ns_to_system_time(ns);
        let diff = now.duration_since(converted).unwrap_or_default();
        assert!(diff.as_nanos() < 1000); // Within 1 microsecond
    }

    #[test]
    fn inode_to_file_attr() {
        let inode = Inode::new_file(42, 1, "test.txt".into(), 0o644, 1000, 1000);
        let attr = CryptFs::inode_to_attr(&inode);
        assert_eq!(attr.ino, 42);
        assert_eq!(attr.kind, FuseFileType::RegularFile);
        assert_eq!(attr.perm, 0o644);
        assert_eq!(attr.uid, 1000);
        assert_eq!(attr.nlink, 1);
    }

    #[test]
    fn dir_inode_to_attr() {
        let inode = Inode::new_dir(2, 1, "docs".into(), 0o755, 1000, 1000);
        let attr = CryptFs::inode_to_attr(&inode);
        assert_eq!(attr.kind, FuseFileType::Directory);
        assert_eq!(attr.perm, 0o755);
        assert_eq!(attr.nlink, 2);
    }

    #[test]
    fn symlink_inode_to_attr() {
        let inode = Inode::new_symlink(3, 1, "link".into(), "/target".into(), 1000, 1000);
        let attr = CryptFs::inode_to_attr(&inode);
        assert_eq!(attr.kind, FuseFileType::Symlink);
        assert_eq!(attr.size, 7);
    }

    fn setup_cryptfs_with_transport() -> (CryptFs, Arc<tgcryptfs_telegram::mock::MockTransport>) {
        let conn = Connection::open_in_memory().unwrap();
        let schema_key = SymmetricKey::from_bytes([0x42; 32]);
        let meta_key = SymmetricKey::from_bytes([0x43; 32]);
        let data_key = SymmetricKey::from_bytes([0x44; 32]);
        let schema = OpaqueSchema::new(schema_key, &logical_tables());
        initialize_database(&conn, &schema).unwrap();

        let transport = Arc::new(tgcryptfs_telegram::mock::MockTransport::new());
        let rt = tokio::runtime::Runtime::new().unwrap();

        let fs = CryptFs::new(conn, schema, meta_key, data_key, 1000, 1000)
            .with_transport(transport.clone())
            .with_runtime(rt.handle().clone());

        (fs, transport)
    }

    #[test]
    fn flush_to_transport_uploads_block() {
        let (fs, transport) = setup_cryptfs_with_transport();

        let data = vec![0xAA; 1024];
        let block = fs.flush_to_transport(42, &data);

        assert!(block.is_some());
        let block = block.unwrap();
        assert_eq!(block.ref_count, 1);
        assert_eq!(transport.message_count(), 1);

        // Verify the data was stored in transport
        let raw = transport.get_raw(block.message_id);
        assert!(raw.is_some());
        assert_eq!(raw.unwrap(), data);
    }

    #[test]
    fn flush_to_transport_dedup() {
        let (fs, transport) = setup_cryptfs_with_transport();

        let data = vec![0xBB; 512];

        // First upload
        let block1 = fs.flush_to_transport(42, &data).unwrap();

        // Second upload of same data — should dedup
        let block2 = fs.flush_to_transport(43, &data).unwrap();

        assert_eq!(block1.rid, block2.rid);
        assert_eq!(transport.message_count(), 1); // Only one upload happened

        // Ref count should have been incremented
        let stored = fs.block_store().get(&block1.rid).unwrap().unwrap();
        assert_eq!(stored.ref_count, 2);
    }

    #[test]
    fn download_block_via_transport() {
        let (fs, _transport) = setup_cryptfs_with_transport();

        let data = vec![0xCC; 2048];
        let block = fs.flush_to_transport(42, &data).unwrap();

        // Download the block by message_id and rid
        let downloaded = fs.download_block(block.message_id, &block.rid);
        assert!(downloaded.is_some());
        assert_eq!(downloaded.unwrap(), data);
    }

    #[test]
    fn flush_no_transport_returns_none() {
        let conn = Connection::open_in_memory().unwrap();
        let schema_key = SymmetricKey::from_bytes([0x42; 32]);
        let meta_key = SymmetricKey::from_bytes([0x43; 32]);
        let data_key = SymmetricKey::from_bytes([0x44; 32]);
        let schema = OpaqueSchema::new(schema_key, &logical_tables());
        initialize_database(&conn, &schema).unwrap();

        let fs = CryptFs::new(conn, schema, meta_key, data_key, 1000, 1000);

        // Without transport, flush returns None
        let block = fs.flush_to_transport(42, &[0xAA; 100]);
        assert!(block.is_none());
    }

    #[test]
    fn download_block_cache_hit() {
        let (_fs, _transport) = setup_cryptfs_with_transport();

        // Set up cache
        let cache_dir = tempfile::TempDir::new().unwrap();
        let cache_config = tgcryptfs_cache::block_cache::CacheConfig {
            cache_dir: cache_dir.path().to_path_buf(),
            max_size: 10 * 1024 * 1024,
            encrypt_at_rest: false,
        };
        let cache_key = SymmetricKey::from_bytes([0x55; 32]);
        let cache = Arc::new(
            tgcryptfs_cache::block_cache::BlockCache::new(cache_config, cache_key).unwrap(),
        );

        // Rebuild CryptFs with cache
        let conn = Connection::open_in_memory().unwrap();
        let schema_key = SymmetricKey::from_bytes([0x42; 32]);
        let meta_key = SymmetricKey::from_bytes([0x43; 32]);
        let data_key = SymmetricKey::from_bytes([0x44; 32]);
        let schema = OpaqueSchema::new(schema_key, &logical_tables());
        initialize_database(&conn, &schema).unwrap();

        let transport = Arc::new(tgcryptfs_telegram::mock::MockTransport::new());
        let rt = tokio::runtime::Runtime::new().unwrap();

        let fs = CryptFs::new(conn, schema, meta_key, data_key, 1000, 1000)
            .with_transport(transport.clone())
            .with_cache(cache.clone())
            .with_runtime(rt.handle().clone());

        // Upload data
        let data = vec![0xDD; 256];
        let block = fs.flush_to_transport(42, &data).unwrap();

        // Data should be cached from the flush
        assert!(cache.contains(&block.rid));

        // Download should hit cache (even if transport is disconnected)
        transport.set_connected(false);
        let downloaded = fs.download_block(block.message_id, &block.rid);
        assert!(downloaded.is_some());
        assert_eq!(downloaded.unwrap(), data);
    }
}
