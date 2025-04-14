use core::fmt::{Debug, write};

use alloc::{sync::Arc, vec::Vec};
use config::{fs::MAX_FDS, vfs::OpenFlags};
use log::{debug, info};
use systype::{SysError, SysResult};
use vfs::file::File;

use crate::{dev::tty::TTY, simplefile::SFile};

pub type Fd = usize;

#[derive(Clone)]
pub struct FdInfo {
    file: Arc<dyn File>,
    flags: OpenFlags,
}

#[derive(Clone)]
pub struct FdTable {
    table: Vec<Option<FdInfo>>,
}

impl FdInfo {
    pub fn new(file: Arc<dyn File>, flags: OpenFlags) -> Self {
        Self { file, flags }
    }

    pub fn file(&self) -> Arc<dyn File> {
        self.file.clone()
    }

    pub fn flags(&self) -> OpenFlags {
        self.flags
    }

    pub fn set_flags(&mut self, flags: OpenFlags) {
        self.flags = flags;
    }

    pub fn close(&mut self) {
        self.flags = OpenFlags::O_CLOEXEC;
    }
}

impl FdTable {
    pub fn new() -> Self {
        let mut table: Vec<Option<FdInfo>> = Vec::with_capacity(MAX_FDS);

        let fdinfo = FdInfo::new(TTY.get().unwrap().clone(), OpenFlags::empty());
        table.push(Some(fdinfo));

        let fdinfo = FdInfo::new(TTY.get().unwrap().clone(), OpenFlags::empty());
        table.push(Some(fdinfo));

        let fdinfo = FdInfo::new(TTY.get().unwrap().clone(), OpenFlags::empty());
        table.push(Some(fdinfo));

        Self { table }
    }

    fn get_available_slot(&mut self) -> Option<usize> {
        let inner_slot = self
            .table
            .iter()
            .enumerate()
            .find(|(_i, e)| e.is_none())
            .map(|(i, _)| i);
        if inner_slot.is_some() {
            return inner_slot;
        } else if inner_slot.is_none() && self.table.len() < MAX_FDS {
            self.table.push(None);
            return Some(self.table.len() - 1);
        } else {
            return None;
        }
    }

    pub fn alloc(&mut self, file: Arc<dyn File>, flags: OpenFlags) -> SysResult<Fd> {
        let fdinfo = FdInfo::new(file, flags);
        if let Some(fd) = self.get_available_slot() {
            info!("alloc fd [{}]", fd);
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
                if fd_info.flags().contains(OpenFlags::O_CLOEXEC) {
                    *slot = None;
                }
            }
        }
    }

    pub fn remove(&mut self, fd: Fd) -> SysResult<()> {
        assert!(fd < self.table.len());
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
                Some(file) => write!(f, "{}: {:?}\n", i, file),
                None => write!(f, "{}: <closed>\n", i),
            })
    }
}
