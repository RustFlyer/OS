use alloc::{sync::Arc, vec::Vec};
use core::fmt::Debug;

use config::{fs::MAX_FDS, vfs::OpenFlags};
use systype::{
    error::{SysError, SysResult},
    rlimit::RLimit,
};
use vfs::file::File;

use crate::dev::tty::TTY;

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
        self.file.clone()
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
}

impl FdTable {
    pub fn new() -> Self {
        let mut table: Vec<Option<FdInfo>> = Vec::with_capacity(MAX_FDS);

        let fdinfo = FdInfo::new(TTY.get().unwrap().clone(), FdFlags::empty());
        table.push(Some(fdinfo));

        let fdinfo = FdInfo::new(TTY.get().unwrap().clone(), FdFlags::empty());
        table.push(Some(fdinfo));

        let fdinfo = FdInfo::new(TTY.get().unwrap().clone(), FdFlags::empty());
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
        let fdinfo = FdInfo::new(file, flags.into());
        if let Some(fd) = self.get_available_slot(0) {
            log::info!("alloc fd [{}]", fd);
            self.table[fd] = Some(fdinfo);
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

    pub fn close(&mut self) {
        for slot in self.table.iter_mut() {
            if let Some(fd_info) = slot {
                if fd_info.flags().contains(FdFlags::CLOEXEC) {
                    *slot = None;
                }
            }
        }
    }

    pub fn remove(&mut self, fd: Fd) -> SysResult<()> {
        // assert!(fd < self.table.len());
        if fd >= self.table.len() {
            return Err(SysError::EBADF);
        }
        if self.table[fd].is_none() {
            return Err(SysError::EBADF);
        }
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
