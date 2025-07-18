use alloc::{sync::Arc, vec::Vec};
use config::vfs::EpollEvents;
use systype::error::{SysError, SysResult};
use vfs::file::File;

use crate::fd_table::Fd;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct EpollEvent {
    pub events: EpollEvents, // 事件掩码，如 EPOLLIN | EPOLLOUT
    pub data: u64,           // 用户自定义数据
}

#[derive(Clone)]
pub struct EpollEntry {
    pub fd: Fd,
    pub file: Arc<dyn File>,
    pub event: EpollEvent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EpollCtlOp {
    Add,
    Mod,
    Del,
}

#[derive(Clone)]
pub struct EpollInner {
    pub entries: Vec<EpollEntry>,
    pub ready_events: Vec<EpollEvent>,
}

unsafe impl Send for EpollInner {}
unsafe impl Sync for EpollInner {}

impl EpollInner {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            ready_events: Vec::new(),
        }
    }

    pub fn ctl(
        &mut self,
        op: EpollCtlOp,
        fd: Fd,
        file: Arc<dyn File>,
        event: EpollEvent,
    ) -> SysResult<()> {
        match op {
            EpollCtlOp::Add => {
                if self.entries.iter().any(|e| e.fd == fd) {
                    return Err(SysError::EEXIST);
                }
                self.entries.push(EpollEntry { fd, file, event });
                Ok(())
            }
            EpollCtlOp::Mod => {
                let entry = self
                    .entries
                    .iter_mut()
                    .find(|e| e.fd == fd)
                    .ok_or(SysError::ENOENT)?;
                entry.event = event;
                Ok(())
            }
            EpollCtlOp::Del => {
                let idx = self
                    .entries
                    .iter()
                    .position(|e| e.fd == fd)
                    .ok_or(SysError::ENOENT)?;
                self.entries.swap_remove(idx);
                Ok(())
            }
        }
    }
}
