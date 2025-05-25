use alloc::sync::Arc;

use async_trait::async_trait;
use mutex::SpinNoIrqLock;
use systype::{SysResult, SyscallResult};
use vfs::{
    dentry::Dentry,
    file::{File, FileMeta},
};

use super::ioctl::TtyInner;
use crate::dev::tty::{
    inode::TtyInode,
    ioctl::{Pid, Termios, TtyIoctlCmd, WinSize},
};
use alloc::boxed::Box;
pub struct TtyFile {
    // buf: SpinNoIrqLock<QueueBuffer>,
    meta: FileMeta,
    pub(crate) inner: SpinNoIrqLock<TtyInner>,
}

impl TtyFile {
    pub fn new(dentry: Arc<dyn Dentry>) -> Arc<Self> {
        Arc::new(Self {
            // buf: SpinNoIrqLock::new(QueueBuffer::new()),
            meta: FileMeta::new(dentry),
            inner: SpinNoIrqLock::new(TtyInner {
                fg_pgid: 1,
                win_size: WinSize::new(),
                termios: Termios::new(),
            }),
        })
    }
}

#[async_trait]
impl File for TtyFile {
    fn meta(&self) -> &FileMeta {
        &self.meta
    }

    async fn base_read(&self, buf: &mut [u8], _pos: usize) -> SysResult<usize> {
        let dev = &self
            .meta
            .dentry
            .inode()
            .unwrap()
            .downcast_arc::<TtyInode>()
            .unwrap_or_else(|_| unreachable!())
            .char_dev;
        let rlen = dev.read(buf);

        let termios = self.inner.lock().termios;
        if termios.is_icrnl() {
            for i in 0..rlen {
                if buf[i] == '\r' as u8 {
                    buf[i] = '\n' as u8;
                }
            }
        }
        if termios.is_echo() {
            self.base_write(buf, 0).await?;
        }
        Ok(rlen)
    }

    async fn base_write(&self, buf: &[u8], _offset: usize) -> SysResult<usize> {
        let dev = &self
            .meta
            .dentry
            .inode()
            .unwrap()
            .downcast_arc::<TtyInode>()
            .unwrap_or_else(|_| unreachable!())
            .char_dev;
        dev.write(buf);

        Ok(buf.len())
    }

    fn ioctl(&self, cmd: usize, arg: usize) -> SyscallResult {
        use TtyIoctlCmd::*;
        let Some(cmd) = TtyIoctlCmd::from_repr(cmd) else {
            log::error!("[TtyFile::ioctl] cmd {cmd} not included");
            unimplemented!()
        };
        log::info!("[TtyFile::ioctl] cmd {:?}, value {:#x}", cmd, arg);
        match cmd {
            TCGETS | TCGETA => {
                unsafe {
                    *(arg as *mut Termios) = self.inner.lock().termios;
                }
                Ok(0)
            }
            TCSETS | TCSETSW | TCSETSF => {
                unsafe {
                    self.inner.lock().termios = *(arg as *const Termios);
                    // log::info!("termios {:#x?}", self.inner.lock().termios);
                }
                Ok(0)
            }
            TIOCGPGRP => {
                let fg_pgid = self.inner.lock().fg_pgid;
                log::info!("[TtyFile::ioctl] get fg pgid {fg_pgid}");
                unsafe {
                    *(arg as *mut Pid) = fg_pgid;
                }
                Ok(0)
            }
            TIOCSPGRP => {
                unsafe {
                    self.inner.lock().fg_pgid = *(arg as *const Pid);
                }
                let fg_pgid = self.inner.lock().fg_pgid;
                log::info!("[TtyFile::ioctl] set fg pgid {fg_pgid}");
                Ok(0)
            }
            TIOCGWINSZ => {
                let win_size = self.inner.lock().win_size;
                log::info!("[TtyFile::ioctl] get window size {win_size:?}",);
                unsafe {
                    *(arg as *mut WinSize) = win_size;
                }
                Ok(0)
            }
            TIOCSWINSZ => {
                unsafe {
                    self.inner.lock().win_size = *(arg as *const WinSize);
                }
                Ok(0)
            }
            TCSBRK => Ok(0),
            _ => todo!(),
        }
    }
}
