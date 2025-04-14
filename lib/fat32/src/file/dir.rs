use alloc::sync::Arc;
use config::vfs::SeekFrom;
use mutex::{ShareMutex, new_share_mutex};
use systype::{SysError, SysResult};
use vfs::{
    direntry::DirEntry,
    file::{File, FileMeta},
    inode::Inode,
};

use crate::{
    FatDir, FatDirIter,
    dentry::FatDentry,
    inode::{dir::FatDirInode, file::FatFileInode},
};

pub struct FatDirFile {
    meta: FileMeta,
    dir: ShareMutex<FatDir>,
    iter_cache: ShareMutex<FatDirIter>,
}

impl FatDirFile {
    pub fn new(dentry: Arc<FatDentry>, inode: Arc<FatDirInode>) -> Arc<Self> {
        Arc::new(Self {
            meta: FileMeta::new(dentry.clone()),
            dir: inode.dir.clone(),
            iter_cache: new_share_mutex(inode.dir.lock().iter()),
        })
    }
}

impl File for FatDirFile {
    fn meta(&self) -> &FileMeta {
        &self.meta
    }

    fn base_read_dir(&self) -> SysResult<Option<DirEntry>> {
        let entry = self.iter_cache.lock().next();
        let Some(entry) = entry else {
            return Ok(None);
        };
        let Ok(entry) = entry else {
            return Err(SysError::EIO);
        };
        let name = entry.file_name();
        self.seek(SeekFrom::Current(1))?;

        let sub_dentry = self.dentry().get_child(&name).ok_or(SysError::ENOENT)?;
        let new_inode: Arc<dyn Inode> = if entry.is_dir() {
            let new_dir = entry.to_dir();
            FatDirInode::new(self.superblock(), new_dir)
        } else {
            let new_file = entry.to_file();
            FatFileInode::new(self.superblock(), new_file)
        };
        let itype = new_inode.inotype();
        sub_dentry.set_inode(new_inode);
        let entry = DirEntry {
            ino: 1,                 // Fat32 does not support ino on disk
            off: self.pos() as u64, // off should not be used
            itype,
            name,
        };
        Ok(Some(entry))
    }

    fn base_load_dir(&self) -> SysResult<()> {
        let mut iter = self.dir.lock().iter();
        while let Some(entry) = iter.next() {
            let Ok(entry) = entry else {
                return Err(SysError::EIO);
            };
            let name = entry.file_name();
            let sub_dentry = self.dentry().get_child(&name).ok_or(SysError::ENOENT)?;
            let new_inode: Arc<dyn Inode> = if entry.is_dir() {
                let new_dir = entry.to_dir();
                FatDirInode::new(self.superblock(), new_dir)
            } else {
                let new_file = entry.to_file();
                FatFileInode::new(self.superblock(), new_file)
            };
            sub_dentry.set_inode(new_inode);
        }
        Ok(())
    }
}
