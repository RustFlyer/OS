use alloc::{sync::Arc, vec};
use config::inode::InodeType;
use fatfs::{Read, Seek, Write};
use mutex::ShareMutex;
use systype::{SysError, SysResult};
use vfs::{
    direntry::DirEntry,
    file::{File, FileMeta},
};

use crate::{FatFile, as_sys_err, dentry::FatDentry, inode::file::FatFileInode};

pub struct FatFileFile {
    meta: FileMeta,
    file: ShareMutex<FatFile>,
}

impl FatFileFile {
    pub fn new(dentry: Arc<FatDentry>, inode: Arc<FatFileInode>) -> Arc<Self> {
        Arc::new(Self {
            meta: FileMeta::new(dentry.clone()),
            file: inode.file.clone(),
        })
    }
}

impl File for FatFileFile {
    fn meta(&self) -> &FileMeta {
        &self.meta
    }

    fn base_read(&self, buf: &mut [u8], pos: usize) -> SysResult<usize> {
        match self.inode().inotype() {
            InodeType::File => {
                let mut file = self.file.lock();
                let fat_offset = file.offset() as usize;
                if pos != fat_offset {
                    file.seek(fatfs::SeekFrom::Start(pos as u64))
                        .map_err(as_sys_err)?;
                }
                let count = file.read(buf).map_err(as_sys_err)?;
                log::trace!("[FatFileFile::base_read] count {count}");
                Ok(count)
            }
            InodeType::Dir => Err(SysError::EISDIR),
            _ => unreachable!(),
        }
    }

    fn base_write(&self, buf: &[u8], offset: usize) -> SysResult<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        match self.inode().inotype() {
            InodeType::File => {
                let mut file = self.file.lock();
                let size = self.inode().size();

                if offset > size {
                    let empty = vec![0; offset - size];
                    file.seek(fatfs::SeekFrom::Start(size as u64))
                        .map_err(as_sys_err)?;
                    file.write_all(&empty).map_err(as_sys_err)?;
                }

                let fat_offset = file.offset() as usize;
                if offset != fat_offset {
                    file.seek(fatfs::SeekFrom::Start(offset as u64))
                        .map_err(as_sys_err)?;
                }
                file.write_all(buf).map_err(as_sys_err)?;
                if offset + buf.len() > size {
                    let new_size = offset + buf.len();
                    self.inode().set_size(new_size);
                }
                Ok(buf.len())
            }
            InodeType::Dir => Err(SysError::EISDIR),
            _ => unreachable!(),
        }
    }

    fn flush(&self) -> SysResult<usize> {
        todo!()
    }

    fn base_read_dir(&self) -> SysResult<Option<DirEntry>> {
        todo!()
    }
}
