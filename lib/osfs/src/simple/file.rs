use alloc::{boxed::Box, string::String, sync::Arc};

use async_trait::async_trait;

use config::mm::PAGE_SIZE;
use systype::error::{SysError, SysResult};
use vfs::{
    dentry::Dentry,
    file::{File, FileMeta},
};

pub struct SimpleDirFile {
    meta: FileMeta,
}

impl SimpleDirFile {
    pub fn new(dentry: Arc<dyn Dentry>) -> Arc<Self> {
        Arc::new(Self {
            meta: FileMeta::new(dentry),
        })
    }
}

#[async_trait]
impl File for SimpleDirFile {
    fn meta(&self) -> &FileMeta {
        &self.meta
    }

    async fn base_read(&self, _buf: &mut [u8], _pos: usize) -> SysResult<usize> {
        log::error!("[SimpleDirFile] not implement simple file");
        Err(SysError::EISDIR)
    }

    async fn base_write(&self, _buf: &[u8], _offset: usize) -> SysResult<usize> {
        log::error!("[SimpleDirFile] not implement simple file");
        Err(SysError::EISDIR)
    }

    fn base_load_dir(&self) -> SysResult<()> {
        Ok(())
    }
}

pub struct SimpleFileFile {
    meta: FileMeta,
}

impl SimpleFileFile {
    pub fn new(dentry: Arc<dyn Dentry>) -> Arc<Self> {
        Arc::new(Self {
            meta: FileMeta::new(dentry),
        })
    }

    pub fn into_dyn_ref(&self) -> &dyn File {
        self
    }
}

#[async_trait]
impl File for SimpleFileFile {
    fn meta(&self) -> &FileMeta {
        &self.meta
    }

    async fn base_read(&self, mut buf: &mut [u8], pos: usize) -> SysResult<usize> {
        let size = self.size();
        let mut cur_pos = pos;

        let inode = self.inode();
        let cache = inode.page_cache();

        log::debug!("[base_read] pos: {}, path: {}", pos, self.dentry().path());

        while !buf.is_empty() && cur_pos < size {
            let file_offset = cur_pos / PAGE_SIZE * PAGE_SIZE;
            let page_offset = cur_pos % PAGE_SIZE;
            let len = buf.len().min(size - cur_pos).min(PAGE_SIZE - page_offset);

            let page = match cache.get_page(file_offset) {
                Some(page) => page,
                None => cache.create_zeroed_page(file_offset)?,
            };

            buf[0..len].copy_from_slice(&page.as_slice()[page_offset..page_offset + len]);
            cur_pos += len;
            buf = &mut buf[len..];
        }

        let mut chs = String::new();
        buf.iter().for_each(|u| chs.push(*u as char));
        log::debug!("[base_read] output: {chs}");

        Ok(cur_pos - pos)
    }

    async fn base_write(&self, mut buf: &[u8], offset: usize) -> SysResult<usize> {
        let mut cur_pos = offset;

        let inode = self.inode();
        let cache = inode.page_cache();

        log::debug!("[base_write] simple file");

        while !buf.is_empty() {
            let file_offset = cur_pos / PAGE_SIZE * PAGE_SIZE;
            let page_offset = cur_pos % PAGE_SIZE;
            let len = buf.len().min(PAGE_SIZE - page_offset);

            let page = match cache.get_page(file_offset) {
                Some(page) => page,
                None => cache.create_zeroed_page(file_offset)?,
            };

            page.as_mut_slice()[page_offset..page_offset + len].copy_from_slice(&buf[0..len]);
            cur_pos += len;
            buf = &buf[len..];
        }
        Ok(cur_pos - offset)
        // Err(SysError::EISDIR)
    }
}
