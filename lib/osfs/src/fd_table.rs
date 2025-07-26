use alloc::{sync::Arc, vec::Vec};
use core::fmt::Debug;

use config::{fs::MAX_FDS, vfs::OpenFlags};
use systype::{
    error::{SysError, SysResult},
    rlimit::RLimit,
};
use vfs::{fanotify::types::FanEventMask, file::File};

use crate::dev::tty::{TTY0, TTY1, TTY2};

pub type Fd = usize;

#[derive(Clone)]
pub struct FdInfo {
    file: Arc<dyn File>,
    flags: FdFlags,
}

#[derive(Clone)]
pub struct FdTable {
    table: Vec<Option<FdInfo>>,
    rlimit: RLimit,
}

impl FdInfo {
    pub fn new(file: Arc<dyn File>, flags: FdFlags) -> Self {
        Self { file, flags }
    }

    pub fn file(&self) -> Arc<dyn File> {
        Arc::clone(&self.file)
    }

    pub fn flags(&self) -> FdFlags {
        self.flags
    }

    pub fn set_flags(&mut self, flags: FdFlags) {
        self.flags = flags;
    }

    pub fn close(&mut self) {
        self.flags = FdFlags::CLOEXEC;
    }

    fn fanotify_close(&self) {
        let event = if self.file.flags().writable() {
            FanEventMask::CLOSE_WRITE
        } else {
            FanEventMask::CLOSE_NOWRITE
        };
        self.file.fanotify_publish(event);
    }
}

impl FdTable {
    pub fn new() -> Self {
        let mut table: Vec<Option<FdInfo>> = Vec::with_capacity(MAX_FDS);

        let fdinfo = FdInfo::new(TTY0.get().unwrap().clone(), FdFlags::empty());
        table.push(Some(fdinfo));

        let fdinfo = FdInfo::new(TTY1.get().unwrap().clone(), FdFlags::empty());
        table.push(Some(fdinfo));

        let fdinfo = FdInfo::new(TTY2.get().unwrap().clone(), FdFlags::empty());
        table.push(Some(fdinfo));

        Self {
            table,
            rlimit: RLimit {
                rlim_cur: MAX_FDS,
                rlim_max: MAX_FDS,
            },
        }
    }

    fn get_available_slot(&mut self, start: usize) -> Option<usize> {
        while start >= self.table.len() && start < self.rlimit.rlim_cur {
            self.table.push(None);
        }

        let inner_slot = self
            .table
            .iter()
            .enumerate()
            .skip_while(|(i, _e)| *i < start)
            .find(|(_i, e)| e.is_none())
            .map(|(i, _)| i);

        if inner_slot.is_some() {
            inner_slot
        } else if inner_slot.is_none() && self.table.len() < self.rlimit.rlim_cur {
            self.table.push(None);
            return Some(self.table.len() - 1);
        } else {
            return None;
        }
    }

    pub fn alloc(&mut self, file: Arc<dyn File>, flags: OpenFlags) -> SysResult<Fd> {
        if let Some(fd) = self.get_available_slot(0) {
            log::info!("alloc fd [{}]", fd);
            crate::proc::fd::create_self_fd_file(fd)?;
            file.fanotify_publish(FanEventMask::OPEN);
            self.table[fd] = Some(FdInfo::new(file, flags.into()));
            Ok(fd)
        } else {
            Err(SysError::EMFILE)
        }
    }

    pub fn get(&self, fd: Fd) -> SysResult<&FdInfo> {
        self.table
            .get(fd)
            .ok_or(SysError::EBADF)?
            .as_ref()
            .ok_or(SysError::EBADF)
    }

    pub fn get_mut(&mut self, fd: Fd) -> SysResult<&mut FdInfo> {
        self.table
            .get_mut(fd)
            .unwrap()
            .as_mut()
            .ok_or(SysError::EBADF)
    }

    pub fn get_file(&self, fd: Fd) -> SysResult<Arc<dyn File>> {
        Ok(self.get(fd)?.file())
    }

    pub fn clear(&mut self) {
        for slot in self.table.iter_mut() {
            if let Some(fd_info) = slot {
                fd_info.fanotify_close();
                *slot = None;
            }
        }
    }

    pub fn close_cloexec(&mut self) {
        for (cnt, slot) in self.table.iter_mut().enumerate() {
            if let Some(fd_info) = slot {
                // log::debug!(
                //     "fdinfo ino {} type {:?} fd {} close",
                //     fd_info.file.inode().get_meta().ino,
                //     fd_info.file.inode().inotype(),
                //     cnt
                // );
                // log::debug!(
                //     "fd {} remained: {}",
                //     cnt,
                //     Arc::strong_count(&fd_info.file) - 1
                // );
                if fd_info.flags().contains(FdFlags::CLOEXEC) {
                    fd_info.fanotify_close();
                    *slot = None;
                }
            }
        }
    }

    pub fn remove_with_range(&mut self, first: Fd, last: Fd, flags: usize) -> SysResult<()> {
        const CLOSE_RANGE_CLOEXEC: usize = 1 << 2;

        let existing: Vec<usize> = self
            .table
            .iter()
            .enumerate()
            .filter(|(_fd, info)| info.is_some())
            .map(|(fd, _info)| fd)
            .filter(|fd| *fd >= first && *fd <= last)
            .collect();

        for fd in existing {
            log::info!("[remove_with_range] close fd: {}", fd);
            if (flags & CLOSE_RANGE_CLOEXEC) != 0 {
                if let Ok(info) = self.get_mut(fd) {
                    info.set_flags(FdFlags::CLOEXEC);
                }
            } else {
                let _ = self.remove(fd);
            }
        }

        Ok(())
    }

    pub fn remove(&mut self, fd: Fd) -> SysResult<()> {
        // assert!(fd < self.table.len());
        if fd >= self.table.len() {
            return Err(SysError::EBADF);
        }
        if self.table[fd].is_none() {
            return Err(SysError::EBADF);
        }
        log::debug!(
            "fd {} remained: {}",
            fd,
            Arc::strong_count(&self.table[fd].as_ref().unwrap().file) - 1
        );
        let fdinfo = self.get_mut(fd)?;
        fdinfo.fanotify_close();
        self.table[fd] = None;
        Ok(())
    }

    fn extend_to(&mut self, len: usize) -> SysResult<()> {
        if len > MAX_FDS {
            return Err(SysError::EBADF);
        }
        if self.table.len() < len {
            for _ in self.table.len()..len {
                self.table.push(None)
            }
        }
        Ok(())
    }

    pub fn put(&mut self, fd: Fd, fd_info: FdInfo) -> SysResult<()> {
        self.extend_to(fd + 1)?;
        self.table[fd] = Some(fd_info);
        Ok(())
    }

    pub fn dup(&mut self, old_fd: Fd) -> SysResult<Fd> {
        let file = self.get_file(old_fd)?;
        self.alloc(file, OpenFlags::empty())
    }

    pub fn dup3(&mut self, old_fd: Fd, new_fd: Fd, flags: OpenFlags) -> SysResult<Fd> {
        let file = self.get_file(old_fd)?;
        let fd_info = FdInfo::new(file, flags.into());
        self.put(new_fd, fd_info)?;
        Ok(new_fd)
    }

    pub fn dup3_with_flags(&mut self, old_fd: Fd, new_fd: Fd) -> SysResult<Fd> {
        let old_fd_info = self.get(old_fd)?;
        self.put(new_fd, old_fd_info.clone())?;
        Ok(new_fd)
    }

    pub fn dup_with_bound(
        &mut self,
        old_fd: Fd,
        lower_bound: usize,
        flags: OpenFlags,
    ) -> SysResult<Fd> {
        let file = self.get_file(old_fd)?;
        let new_fd = self
            .get_available_slot(lower_bound)
            .ok_or(SysError::EMFILE)?;
        log::debug!(
            "[dup_with_bound] old fd {}, lowerbound {}, new fd {}",
            old_fd,
            lower_bound,
            new_fd
        );
        let fd_info = FdInfo::new(file, flags.into());
        self.put(new_fd, fd_info)?;
        debug_assert!(new_fd >= lower_bound);
        Ok(new_fd)
    }

    pub fn set_rlimit(&mut self, rlimit: RLimit) {
        self.rlimit = rlimit;
    }

    pub fn get_rlimit(&self) -> RLimit {
        self.rlimit
    }
}

impl Default for FdTable {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for FdTable {
    fn drop(&mut self) {
        // Call `clear` rather than simply dropping them to send fanotify events.
        self.clear();
    }
}

impl Debug for FdInfo {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "File [{}] with flags [{:?}]",
            self.file.dentry().get_meta().name,
            self.flags
        )
    }
}

impl Debug for FdTable {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        self.table
            .iter()
            .enumerate()
            .try_for_each(|(i, entry)| match entry {
                Some(file) => writeln!(f, "{}: {:?}", i, file),
                None => writeln!(f, "{}: <closed>", i),
            })
    }
}

#[derive(Debug, Copy, Clone)]
#[repr(C)]
pub struct FdSet {
    fds_bits: [u64; 16],
}

impl FdSet {
    pub fn zero() -> Self {
        Self { fds_bits: [0; 16] }
    }

    pub fn clear(&mut self) {
        for i in 0..self.fds_bits.len() {
            self.fds_bits[i] = 0;
        }
    }

    /// Add the given file descriptor to the collection. Calculate the index and
    /// corresponding bit of the file descriptor in the array, and set the bit
    /// to 1
    pub fn set(&mut self, fd: usize) {
        let idx = fd / 64;
        let bit = fd % 64;
        let mask = 1 << bit;
        log::debug!("set fd {fd} = mask {mask}");
        self.fds_bits[idx] |= mask;
    }

    /// Check if the given file descriptor is in the collection. Calculate the
    /// index and corresponding bit of the file descriptor in the array, and
    /// check if the bit is 1
    pub fn is_set(&self, fd: usize) -> bool {
        let idx = fd / 64;
        let bit = fd % 64;
        let mask = 1 << bit;
        let ret = self.fds_bits[idx] & mask != 0;

        // ret.then(|| log::warn!("[FdSet::is_set] fd {} set", fd));

        ret
    }
}

bitflags::bitflags! {
    // Defined in <bits/fcntl-linux.h>.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct FdFlags: u8 {
        const CLOEXEC = 1;
    }
}

impl From<OpenFlags> for FdFlags {
    fn from(value: OpenFlags) -> Self {
        if value.contains(OpenFlags::O_CLOEXEC) {
            FdFlags::CLOEXEC
        } else {
            FdFlags::empty()
        }
    }
}
