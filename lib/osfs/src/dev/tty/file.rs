use alloc::{sync::Arc, vec::Vec};
use driver::{print, sbi::getchar};
use mutex::SpinNoIrqLock;

use systype::SysResult;
use vfs::{
    dentry::Dentry,
    file::{File, FileMeta},
};

use super::queuebuf::QueueBuffer;

pub struct TtyFile {
    buf: SpinNoIrqLock<QueueBuffer>,
    meta: FileMeta,
}

impl TtyFile {
    pub fn new(dentry: Arc<dyn Dentry>) -> Arc<Self> {
        Arc::new(Self {
            buf: SpinNoIrqLock::new(QueueBuffer::new()),
            meta: FileMeta::new(dentry),
        })
    }
}

impl File for TtyFile {
    fn meta(&self) -> &FileMeta {
        &self.meta
    }

    fn base_read(&self, buf: &mut [u8], _pos: usize) -> SysResult<usize> {
        let mut cnt = 0;
        loop {
            let ch: u8;
            let self_buf = self.buf.lock().pop();
            if self_buf != 0xff {
                ch = self_buf;
            } else {
                ch = getchar();
                if ch == 0xff {
                    todo!();
                }
            }
            // log::debug!(
            //     "[TtyFuture::poll] recv ch {ch}, cnt {cnt}, len {}",
            //     buf.len()
            // );
            buf[cnt] = ch;

            cnt += 1;

            if cnt < buf.len() {
                log::warn!("can not async yield");
                // yield_now().await;
                continue;
            } else {
                return Ok(buf.len());
            }
        }
    }

    fn base_write(&self, buf: &[u8], _offset: usize) -> SysResult<usize> {
        let utf8_buf: Vec<u8> = buf.iter().filter(|c| c.is_ascii()).map(|c| *c).collect();
        print!("{}", unsafe { core::str::from_utf8_unchecked(&utf8_buf) });

        Ok(buf.len())
    }
}
