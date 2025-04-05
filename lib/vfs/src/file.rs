use core::{
    cmp,
    sync::atomic::{AtomicUsize, Ordering},
};

use crate::{dentry::Dentry, direntry::DirEntry, inode::Inode, superblock::SuperBlock};
use alloc::vec::Vec;
use alloc::{boxed::Box, string::String};
use alloc::{string::ToString, sync::Arc};
use async_trait::async_trait;
use config::{
    inode::{InodeState, InodeType},
    mm::PAGE_SIZE,
    vfs::{OpenFlags, PollEvents, SeekFrom},
};
use downcast_rs::{DowncastSync, impl_downcast};
use log::debug;
use mm::vm::page_cache::page::Page;
use mutex::SpinNoIrqLock;
use systype::{SysError, SysResult, SyscallResult};

pub struct FileMeta {
    pub dentry: Arc<dyn Dentry>,
    pub inode: Arc<dyn Inode>,

    pub pos: AtomicUsize,
    pub flags: SpinNoIrqLock<OpenFlags>,
}

impl FileMeta {
    pub fn new(dentry: Arc<dyn Dentry>, inode: Arc<dyn Inode>) -> Self {
        Self {
            dentry,
            inode,
            pos: AtomicUsize::new(0),
            flags: SpinNoIrqLock::new(OpenFlags::empty()),
        }
    }
}

#[async_trait]
pub trait File: Send + Sync + DowncastSync {
    fn get_meta(&self) -> &FileMeta;

    fn base_read_at(&self, offset: usize, buf: &mut [u8]) -> SyscallResult {
        todo!()
    }

    fn base_write_at(&self, offset: usize, buf: &[u8]) -> SyscallResult {
        todo!()
    }

    /// Read directory entries. This is called by the getdents(2) system call.
    ///
    /// For every call, this function will return an valid entry, or an error.
    /// If it read to the end of directory, it will return an empty entry.
    fn base_read_dir(&self) -> SysResult<Option<DirEntry>> {
        todo!()
    }

    /// Load all dentry and inodes in a directory. Will not advance dir offset.
    fn base_load_dir(&self) -> SysResult<()> {
        todo!()
    }

    fn base_read_link(&self, buf: &mut [u8]) -> SyscallResult {
        todo!()
    }

    fn flush(&self) -> SysResult<usize> {
        todo!()
    }

    fn ioctl(&self, _cmd: usize, _arg: usize) -> SyscallResult {
        Err(SysError::ENOTTY)
    }

    async fn poll(&self, events: PollEvents) -> SysResult<PollEvents> {
        let mut res = PollEvents::empty();
        if events.contains(PollEvents::POLLIN) {
            res |= PollEvents::POLLIN;
        }
        if events.contains(PollEvents::POLLOUT) {
            res |= PollEvents::POLLOUT;
        }
        Ok(res)
    }

    fn inode(&self) -> Arc<dyn Inode> {
        self.get_meta().inode.clone()
    }

    fn itype(&self) -> InodeType {
        self.get_meta().inode.inotype()
    }

    /// Called when the VFS needs to move the file position index.
    ///
    /// Return the result offset.
    fn seek(&self, pos: SeekFrom) -> SyscallResult {
        let mut res_pos = self.pos();
        match pos {
            SeekFrom::Current(off) => {
                if off < 0 {
                    res_pos -= off.abs() as usize;
                } else {
                    res_pos += off as usize;
                }
            }
            SeekFrom::Start(off) => {
                res_pos = off as usize;
            }
            SeekFrom::End(off) => {
                let size = self.size();
                if off < 0 {
                    res_pos = size - off.abs() as usize;
                } else {
                    res_pos = size + off as usize;
                }
            }
        }
        self.set_pos(res_pos);
        Ok(res_pos)
    }

    fn pos(&self) -> usize {
        self.get_meta().pos.load(Ordering::Relaxed)
    }

    fn set_pos(&self, pos: usize) {
        self.get_meta().pos.store(pos, Ordering::Relaxed)
    }

    fn dentry(&self) -> Arc<dyn Dentry> {
        self.get_meta().dentry.clone()
    }

    fn super_block(&self) -> Arc<dyn SuperBlock> {
        self.get_meta().dentry.superblock().expect("fix me")
    }

    fn size(&self) -> usize {
        self.get_meta().inode.size()
    }

    async fn readlink(&self, buf: &mut [u8]) -> SyscallResult {
        self.base_read_link(buf)
    }

    fn base_ls(&self, path: String) {
        todo!()
    }
}

impl dyn File {
    pub fn flags(&self) -> OpenFlags {
        *self.get_meta().flags.lock()
    }

    pub fn set_flags(&self, flags: OpenFlags) {
        *self.get_meta().flags.lock() = flags;
    }

    /// Read at an `offset`, and will fill `buf` until `buf` is full or eof is
    /// reached. Will not advance offset.
    ///
    /// Returns count of bytes actually read or an error.
    pub fn read_at(&self, offset: usize, buf: &mut [u8]) -> SyscallResult {
        log::info!(
            "[File::read] file {}, offset {offset}, buf len {}",
            self.dentry().path(),
            buf.len()
        );
        let mut buf = buf;
        let mut count = 0;
        let mut offset = offset;
        let inode = self.inode();

        log::debug!("[File::read] read without pages");
        let count = self.base_read_at(offset, buf)?;
        return Ok(count);

        if !(inode.inotype().is_reg() || inode.inotype().is_block_device()) {
            log::debug!("[File::read] read without pages");
            let count = self.base_read_at(offset, buf)?;
            return Ok(count);
        };
        let pages = inode.page_cache();
        log::debug!("[File::read] read with address_space");
        while !buf.is_empty() && offset < self.size() {
            let offset_aligned = offset & !(PAGE_SIZE - 1);
            let offset_in_page = offset - offset_aligned;
            let len = if let Some(page) = pages.get_page(offset_aligned) {
                log::trace!("[File::read] offset {offset_aligned} cached in address space");
                let len = cmp::min(buf.len(), PAGE_SIZE - offset_in_page).min(self.size() - offset);
                buf[0..len]
                    .copy_from_slice(&page.as_mut_slice()[offset_in_page..offset_in_page + len]);
                len
            } else {
                log::trace!("[File::read] offset {offset_aligned} not cached in address space");
                let page = Page::build()?;
                let len = self.base_read_at(offset_aligned, page.as_mut_slice())?;
                if len == 0 {
                    log::warn!("[File::read] reach file end");
                    break;
                }
                let len = cmp::min(buf.len(), len);
                buf[0..len]
                    .copy_from_slice(&page.as_mut_slice()[offset_in_page..offset_in_page + len]);
                pages.insert_page(offset_aligned, page);
                len
            };
            log::trace!("[File::read] read count {len}, buf len {}", buf.len());
            count += len;
            offset += len;
            buf = &mut buf[len..];
        }
        log::info!("[File::read] read count {count}");
        Ok(count)
    }

    /// Called by write(2) and related system calls.
    ///
    /// On success, the number of bytes written is returned, and the file offset
    /// is incremented by the number of bytes actually written.
    pub fn write_at(&self, offset: usize, buf: &[u8]) -> SyscallResult {
        self.base_write_at(offset, buf)
    }

    /// Read from offset in self, and will fill `buf` until `buf` is full or eof
    /// is reached. Will advance offset.
    pub fn read(&self, buf: &mut [u8]) -> SyscallResult {
        let pos = self.pos();
        let ret = self.read_at(pos, buf)?;
        self.set_pos(pos + ret);
        Ok(ret)
    }

    pub fn write(&self, buf: &[u8]) -> SyscallResult {
        let pos = self.pos();
        debug!("write at pos [{}]", self.pos());
        let ret = self.write_at(pos, buf)?;
        self.set_pos(pos + ret);
        Ok(ret)
    }

    pub fn load_dir(&self) -> SysResult<()> {
        let inode = self.inode();
        if inode.state() == InodeState::Uninit {
            self.base_load_dir()?;
            inode.set_state(InodeState::Synced)
        }
        Ok(())
    }

    pub fn read_dir(&self) -> SysResult<Option<DirEntry>> {
        self.load_dir()?;
        if let Some(sub_dentry) = self
            .dentry()
            .get_meta()
            .children
            .lock()
            .values()
            .filter(|c| !c.is_negative())
            .nth(self.pos())
        {
            self.seek(SeekFrom::Current(1))?;
            let inode = sub_dentry.inode().expect("check me");
            let dirent = DirEntry {
                ino: inode.ino() as u64,
                off: self.pos() as u64,
                itype: inode.inotype(),
                name: sub_dentry.name().to_string(),
            };
            Ok(Some(dirent))
        } else {
            Ok(None)
        }
    }

    /// Read all data from this file synchronously.
    pub fn read_all(&self) -> SysResult<Vec<u8>> {
        log::info!("[File::read_all] file size {}", self.size());
        let mut buf = Vec::with_capacity(self.size());
        self.read_at(0, &mut buf)?;
        Ok(buf)
    }

    pub fn ls(&self, path: String) {
        self.base_ls(path);
    }
}

impl_downcast!(sync File);
