use core::sync::atomic::AtomicU64;

use alloc::boxed::Box;
use async_trait::async_trait;
use systype::error::{SysError, SysResult};
use vfs::{
    direntry::DirEntry,
    file::{File, FileMeta},
};

pub struct UrandomFile {
    pub(crate) meta: FileMeta,
    seed: AtomicU64,
}

impl UrandomFile {
    pub fn new(meta: FileMeta) -> Self {
        Self {
            meta,
            seed: AtomicU64::new(0x12345678abcdef),
        }
    }

    fn next(&self) -> u8 {
        let mut x = self.seed.load(core::sync::atomic::Ordering::Relaxed);
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.seed.store(x, core::sync::atomic::Ordering::Relaxed);
        (x & 0xFF) as u8
    }
}

#[async_trait]
impl File for UrandomFile {
    fn meta(&self) -> &FileMeta {
        &self.meta
    }

    async fn base_read(&self, buf: &mut [u8], _pos: usize) -> SysResult<usize> {
        for b in buf.iter_mut() {
            *b = self.next();
        }
        // log::error!("read random: {:?}", buf.len());
        Ok(buf.len())
    }

    async fn base_write(&self, _buf: &[u8], _pos: usize) -> SysResult<usize> {
        Ok(_buf.len())
    }

    fn base_read_dir(&self) -> SysResult<Option<DirEntry>> {
        Err(SysError::ENOTDIR)
    }

    fn flush(&self) -> SysResult<usize> {
        todo!()
    }
}
