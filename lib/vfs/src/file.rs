use alloc::ffi::CString;
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicUsize, Ordering};
use log::{debug, info, trace};

use downcast_rs::{DowncastSync, impl_downcast};

use config::{
    inode::{InodeState, InodeType},
    mm::PAGE_SIZE,
    vfs::{OpenFlags, PollEvents, SeekFrom},
};
use mm::vm::page_cache::page::Page;
use mutex::SpinNoIrqLock;
use systype::{SysError, SysResult};

use crate::{dentry::Dentry, direntry::DirEntry, inode::Inode, superblock::SuperBlock};

/// Data that is common to all files.
pub struct FileMeta {
    /// The dentry that this file is associated with.
    pub dentry: Arc<dyn Dentry>,
    /// The current position in the file.
    pub pos: AtomicUsize,
    /// The flags that are set for this file.
    pub flags: SpinNoIrqLock<OpenFlags>,
}

impl FileMeta {
    /// Creates a new `FileMeta` with the given dentry. Position is set to 0.
    /// The flags are set to empty.
    pub fn new(dentry: Arc<dyn Dentry>) -> Self {
        Self {
            dentry,
            pos: AtomicUsize::new(0),
            flags: SpinNoIrqLock::new(OpenFlags::empty()),
        }
    }
}

pub trait File: Send + Sync + DowncastSync {
    /// Returns the metadata of this file.
    fn meta(&self) -> &FileMeta;

    /// Reads data from this file from the given position into the provided buffer.
    /// If the range of the position to read is partly or completely beyond the end
    /// of the file, the part that is beyond it is filled with zeroes.
    ///
    /// This function should be implemented by an underlying file system for each file
    /// type it supports. For example, a file system that supports regular files should
    /// implement this function to read data from a regular file.
    ///
    /// This function should not be implemented for directory files, as they are not
    /// readable in the same way as regular files. Instead, implement [`File::base_read_dir`]
    /// for directory files.
    fn base_read(&self, _buf: &mut [u8], _pos: usize) -> SysResult<usize> {
        panic!(
            "`base_read` is not supported for this file: {}",
            self.dentry().path()
        );
    }

    fn base_write(&self, _buf: &[u8], _offset: usize) -> SysResult<usize> {
        panic!(
            "`base_write` is not supported for this file: {}",
            self.dentry().path()
        );
    }

    /// Read directory entries. This is called by the getdents(2) system call.
    ///
    /// For every call, this function will return an valid entry, or an error.
    /// If it read to the end of directory, it will return an empty entry.
    #[deprecated = "Legacy function from Phoenix OS."]
    fn base_read_dir(&self) -> SysResult<Option<DirEntry>> {
        panic!(
            "`base_read_dir` is not supported for this file: {}",
            self.dentry().path()
        );
    }

    /// Load all dentry and inodes in a directory. Will not advance dir offset.
    #[deprecated = "Legacy function from Phoenix OS."]
    fn base_load_dir(&self) -> SysResult<()> {
        panic!(
            "`base_load_dir` is not supported for this file: {}",
            self.dentry().path()
        );
    }

    #[deprecated = "Legacy function from Phoenix OS."]
    fn base_ls(&self, path: String) {
        todo!()
    }

    fn flush(&self) -> SysResult<usize> {
        todo!()
    }

    fn ioctl(&self, _cmd: usize, _arg: usize) -> SysResult<()> {
        Err(SysError::ENOTTY)
    }

    fn readlink(&self, buf: &mut [u8]) -> SysResult<usize> {
        todo!()
    }

    /// Given interested events, keep track of these events and return events
    /// that is ready.
    fn base_poll(&self, events: PollEvents) -> PollEvents {
        unimplemented!();
        let mut res = PollEvents::empty();
        if events.contains(PollEvents::POLLIN) {
            res |= PollEvents::POLLIN;
        }
        if events.contains(PollEvents::POLLOUT) {
            res |= PollEvents::POLLOUT;
        }
        res
    }

    fn inode(&self) -> Arc<dyn Inode> {
        self.meta().dentry.inode().unwrap()
    }

    /// Called when the VFS needs to move the file position index.
    ///
    /// Return the result offset.
    ///
    /// lseek() allows the file offset to be set beyond the end of the file (but
    /// this does not change the size of the file). If data is later written at
    /// this point, subsequent reads of the data in the gap (a "hole") return
    /// null bytes ('\0') until data is actually written into the gap.
    // TODO: On Linux, using lseek() on a terminal device fails with the error
    // ESPIPE. However, many function will use this Seek.
    fn seek(&self, pos: SeekFrom) -> SysResult<usize> {
        let mut res_pos = self.pos();
        match pos {
            SeekFrom::Current(off) => {
                if off < 0 {
                    if res_pos as i64 - off.abs() < 0 {
                        return Err(SysError::EINVAL);
                    }
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
        self.meta().pos.load(Ordering::Relaxed)
    }

    fn set_pos(&self, pos: usize) {
        self.meta().pos.store(pos, Ordering::Relaxed)
    }

    fn dentry(&self) -> Arc<dyn Dentry> {
        self.meta().dentry.clone()
    }

    fn superblock(&self) -> Arc<dyn SuperBlock> {
        self.meta().dentry.superblock().unwrap()
    }

    fn size(&self) -> usize {
        self.meta().dentry.inode().unwrap().size()
    }

    fn flags(&self) -> OpenFlags {
        self.meta().flags.lock().clone()
    }

    fn set_flags(&self, flags: OpenFlags) {
        *self.meta().flags.lock() = flags;
    }
}

impl dyn File {
    /// Creates a `File` object pointing to dentry `self` and returns it.
    ///
    /// Returns an `ENOENT` error if this dentry is a negative dentry.
    pub fn open(dentry: Arc<dyn Dentry>) -> SysResult<Arc<dyn File>> {
        if dentry.is_negative() {
            return Err(SysError::ENOENT);
        }
        Arc::clone(&dentry).base_open()
    }

    /// Reads data from the file.
    ///
    /// This function will read data from the file starting at the current position,
    /// tring to read as much data as possible into the provided buffer.
    ///
    /// This function will update the file position to the end of the data read.
    ///
    /// `self` must not be a directory file. Instead, call [`File::base_read_dir`] to
    /// read directory entries.
    ///
    /// Returns the number of bytes read.
    pub fn read(&self, buf: &mut [u8]) -> SysResult<usize> {
        let inode = self.inode();
        let position = self.pos();

        let bytes_read = match inode.inotype() {
            InodeType::File => self.read_through_page_cache(buf, position)?,
            _ => self.base_read(buf, position)?,
        };

        trace!("read len = {}", bytes_read);
        self.set_pos(position + bytes_read);
        return Ok(bytes_read);
    }

    /// A helper function which reads data starting from the given position from a file that
    /// has a page cache.
    ///
    /// This function will try to read data by page from the page cache. If the page is
    /// not cached, it will create a new [`Page`] in the page cache and try to read data from
    /// the underlying file system into the page.
    ///
    /// This function does not update the file position.
    ///
    /// Returns the number of bytes read.
    fn read_through_page_cache(&self, mut buf: &mut [u8], pos: usize) -> SysResult<usize> {
        let inode = self.inode();
        let page_cache = inode.page_cache();

        let size = self.size();
        let mut cur_pos = pos;
        while !buf.is_empty() && cur_pos < size {
            let page_pos = cur_pos / PAGE_SIZE * PAGE_SIZE;
            let page_offset = cur_pos % PAGE_SIZE;
            let page = match page_cache.get_page(page_pos) {
                Some(page) => page,
                None => {
                    let page = Arc::new(Page::build()?);
                    self.base_read(page.as_mut_slice(), page_pos)?;
                    page_cache.insert_page(page_pos, Arc::clone(&page));
                    page
                }
            };
            let len = buf.len().min(size - cur_pos).min(PAGE_SIZE - page_offset);
            buf[0..len].copy_from_slice(&page.as_slice()[page_offset..page_offset + len]);
            cur_pos += len;
            buf = &mut buf[len..];
        }
        Ok(cur_pos - pos)
    }

    /// Writes data to the file.
    ///
    /// This function will write data to the file starting at the current position,
    /// trying to write as much data as possible from the provided buffer. If `O_APPEND`
    /// is set for the file, the file position will be set to the end of the file before
    /// writing.
    ///
    /// This function will update the file position to the end of the data written,
    /// and will update the file size if the data written extends beyond the current end
    /// of the file.
    ///
    /// `self` must not be a directory file. Instead, call [`File::todo`] to
    /// write data to a directory file.
    ///
    /// Returns the number of bytes written.
    pub fn write(&self, buf: &[u8]) -> SysResult<usize> {
        if self.flags().contains(OpenFlags::O_APPEND) {
            self.set_pos(self.size());
        }

        let inode = self.inode();
        let size = self.size();
        let position = self.pos();

        if position > size {
            todo!("Holes are not supported yet");
        }

        let bytes_written = match inode.inotype() {
            InodeType::File => self.write_through_page_cache(buf, position)?,
            _ => self.base_write(buf, position)?,
        };
        let new_position = position + bytes_written;
        self.set_pos(new_position);
        inode.set_size(usize::max(inode.size(), new_position));
        inode.set_state(InodeState::DirtyAll);
        Ok(bytes_written)
    }

    /// A helper function which writes data starting from the given position to a file that
    /// has a page cache.
    ///
    /// This function will try to write data by page to the page cache. If the page is
    /// not cached, it will create a new [`Page`] in the page cache and try to write data
    /// to the underlying file system into the page. If the page does not exist, it will
    /// create a new zeroed page and write data to it.
    ///
    /// This function does not update the file position and the file size.
    ///
    /// Returns the number of bytes written.
    fn write_through_page_cache(&self, mut buf: &[u8], pos: usize) -> SysResult<usize> {
        let inode = self.inode();
        let page_cache = inode.page_cache();

        let mut cur_pos = pos;
        while !buf.is_empty() {
            let page_pos = cur_pos / PAGE_SIZE * PAGE_SIZE;
            let page_offset = cur_pos % PAGE_SIZE;
            let page = match page_cache.get_page(page_pos) {
                Some(page) => page,
                None => {
                    let page = Arc::new(Page::build()?);
                    self.base_read(page.as_mut_slice(), page_pos)?;
                    page_cache.insert_page(page_pos, Arc::clone(&page));
                    page
                }
            };
            let len = buf.len().min(PAGE_SIZE - page_offset);
            page.as_mut_slice()[page_offset..page_offset + len].copy_from_slice(&buf[0..len]);
            cur_pos += len;
            buf = &buf[len..];
        }
        Ok(cur_pos - pos)
    }

    /// Given interested events, keep track of these events and return events
    /// that is ready.
    pub fn poll(&self, events: PollEvents) -> PollEvents {
        unimplemented!();
        log::info!("[File::poll] path:{}", self.dentry().path());
        self.base_poll(events)
    }

    pub fn load_dir(&self) -> SysResult<()> {
        let inode = self.inode();
        if inode.state() == InodeState::Uninit {
            self.base_load_dir()?;
            inode.set_state(InodeState::Synced)
        }
        Ok(())
    }

    pub fn read_dir(&self, buf: &mut [u8]) -> SysResult<usize> {
        self.load_dir()?;

        #[derive(Debug, Clone, Copy)]
        #[repr(C)]
        struct LinuxDirent64 {
            d_ino: u64,
            d_off: u64,

            d_reclen: u16,
            d_type: u8,
            // d_name follows here, which will be written later
        }
        let buf_len = buf.len();
        // NOTE: Considering C struct align, we can not use `size_of` directly, because
        // `size_of::<LinuxDirent64>` equals 24, which is not what we want.
        const LEN_BEFORE_NAME: usize = 19;
        let mut writen_len = 0;
        let mut buf_it = buf;
        for dentry in self.dentry().children().values().skip(self.pos()) {
            if dentry.is_negative() {
                self.seek(SeekFrom::Current(1))?;
                continue;
            }
            // align to 8 bytes
            let c_name_len = dentry.name().len() + 1;
            let rec_len = (LEN_BEFORE_NAME + c_name_len + 7) & !0x7;
            let inode = dentry.inode().unwrap();
            let linux_dirent = LinuxDirent64 {
                d_ino: inode.ino() as u64,
                d_off: self.pos() as u64,
                d_type: inode.inotype() as u8,
                d_reclen: rec_len as u16,
            };

            log::debug!("[sys_getdents64] linux dirent {linux_dirent:?}");
            if writen_len + rec_len > buf_len {
                break;
            }

            self.seek(SeekFrom::Current(1))?;
            let ptr = buf_it.as_mut_ptr() as *mut LinuxDirent64;
            unsafe {
                ptr.copy_from_nonoverlapping(&linux_dirent, 1);
            }
            buf_it[LEN_BEFORE_NAME..LEN_BEFORE_NAME + c_name_len - 1]
                .copy_from_slice(dentry.name().as_bytes());
            buf_it[LEN_BEFORE_NAME + c_name_len - 1] = b'\0';
            buf_it = &mut buf_it[rec_len..];
            writen_len += rec_len;
        }
        Ok(writen_len)
    }

    /// Reads all data from this file and returns them as a vector of bytes.
    ///
    /// `self` must not be a directory file.
    #[deprecated = "Legacy function from Phoenix OS."]
    pub fn read_all(&self) -> SysResult<Vec<u8>> {
        let size = self.size();
        let mut buf = vec![0; size];

        let _ulen = self.read(&mut buf)?;
        Ok(buf)
    }

    #[deprecated = "Legacy function from Phoenix OS."]
    pub fn readlink_string(&self) -> SysResult<String> {
        let mut path_buf: Vec<u8> = vec![0; 512];
        let len = self.readlink(&mut path_buf)?;
        path_buf.truncate(len + 1);
        let path = CString::from_vec_with_nul(path_buf)
            .unwrap()
            .into_string()
            .unwrap();
        log::debug!("[File::readlink_string] read link returns {path}");
        Ok(path)
    }

    #[deprecated = "Legacy function from Phoenix OS."]
    pub fn ls(&self, path: String) {
        self.base_ls(path);
    }
}

impl_downcast!(sync File);
