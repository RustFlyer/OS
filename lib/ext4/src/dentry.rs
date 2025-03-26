extern crate alloc;
use alloc::sync::Arc;
use config::inode::InodeType;
use lwext4_rust::bindings::{ext4_dir_rm, ext4_flink, ext4_fremove};
use vfs::{
    dentry::{Dentry, DentryMeta},
    file::File,
    superblock::SuperBlock,
};

use systype::{SysResult, SyscallResult};

pub struct ExtDentry {
    meta: DentryMeta,
}

impl ExtDentry {
    pub fn new(
        name: &str,
        superblock: Arc<dyn SuperBlock>,
        parentdentry: Option<Arc<dyn Dentry>>,
    ) -> Arc<Self> {
        Arc::new(Self {
            meta: DentryMeta::new(name, superblock, parentdentry),
        })
    }
}

impl Dentry for ExtDentry {
    fn get_meta(&self) -> &DentryMeta {
        &self.meta
    }

    fn base_create(
        self: Arc<Self>,
        name: &str,
        mode: config::inode::InodeMode,
    ) -> SysResult<Arc<dyn Dentry>> {
    }
    fn base_lookup(self: Arc<Self>, name: &str) -> SysResult<Arc<dyn Dentry>> {}

    fn base_new_child(self: Arc<Self>, _name: &str) -> Arc<dyn Dentry> {}

    fn base_open(self: Arc<Self>) -> SysResult<Arc<dyn File>> {}

    fn base_rmdir(self: Arc<Self>, name: &str) -> SyscallResult {}

    fn base_link(self: Arc<Self>, new: &Arc<dyn Dentry>) -> SysResult<()> {
        let sblk = self.super_block();
        let oldpath = self.path();
        let newpath = new.path();

        unsafe {
            ext4_flink(oldpath, newpath);
        }
        new.set_inode(self.inode()?);
        Ok(())
    }

    fn base_unlink(self: Arc<Self>, name: &str) -> SyscallResult {
        let sub_dentry = self.get_child(name)?;
        let path = sub_dentry.path();
        match sub_dentry.inode()?.inotype() {
            InodeType::Dir => unsafe { ext4_dir_rm(path) },
            InodeType::File | InodeType::SymLink => unsafe { ext4_fremove(path) },
            _ => todo!(),
        }
    }
}
