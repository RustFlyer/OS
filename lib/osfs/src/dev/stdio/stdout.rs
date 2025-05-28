use alloc::boxed::Box;
use alloc::sync::{Arc, Weak};
use async_trait::async_trait;
use config::inode::InodeType;
use driver::print;
use systype::error::SysResult;
use vfs::stat::Stat;
use vfs::{
    dentry::{Dentry, DentryMeta},
    file::{File, FileMeta},
    inode::{Inode, InodeMeta},
    inoid::alloc_ino,
    superblock::SuperBlock,
};
pub struct StdOutDentry {
    meta: DentryMeta,
}

pub struct StdOutInode {
    meta: InodeMeta,
}

pub struct StdOutFile {
    meta: FileMeta,
}

impl StdOutDentry {
    pub fn new(
        name: &str,
        inode: Option<Arc<dyn Inode>>,
        parent: Option<Weak<dyn Dentry>>,
    ) -> Arc<Self> {
        Arc::new(Self {
            meta: DentryMeta::new(name, inode, parent),
        })
    }
}

impl StdOutFile {
    pub fn new(sb: Arc<dyn SuperBlock>) -> Arc<Self> {
        let inode = Arc::new(StdOutInode {
            meta: InodeMeta::new(alloc_ino(), sb),
        });
        inode.set_inotype(InodeType::CharDevice);
        let dentry = StdOutDentry::new("StdOut", Some(inode), None);
        Arc::new(Self {
            meta: FileMeta::new(dentry),
        })
    }
}

impl Dentry for StdOutDentry {
    fn get_meta(&self) -> &DentryMeta {
        &self.meta
    }

    fn base_create(&self, _dentry: &dyn Dentry, _mode: config::inode::InodeMode) -> SysResult<()> {
        todo!()
    }

    fn base_link(&self, _dentry: &dyn Dentry, _old_dentry: &dyn Dentry) -> SysResult<()> {
        todo!()
    }

    fn base_lookup(&self, _dentry: &dyn Dentry) -> SysResult<()> {
        todo!()
    }

    fn base_new_neg_child(self: Arc<Self>, _name: &str) -> Arc<dyn Dentry> {
        todo!()
    }

    fn base_open(self: Arc<Self>) -> SysResult<Arc<dyn File>> {
        todo!()
    }

    fn base_unlink(&self, _dentry: &dyn Dentry) -> SysResult<()> {
        todo!()
    }

    fn base_rename(
        &self,
        _dentry: &dyn Dentry,
        _new_dir: &dyn Dentry,
        _new_dentry: &dyn Dentry,
    ) -> SysResult<()> {
        todo!()
    }
}

impl Inode for StdOutInode {
    fn get_meta(&self) -> &InodeMeta {
        &self.meta
    }
    fn get_attr(&self) -> SysResult<Stat> {
        todo!()
    }
}

#[async_trait]
impl File for StdOutFile {
    fn meta(&self) -> &FileMeta {
        &self.meta
    }

    async fn base_write(&self, buf: &[u8], _offset: usize) -> SysResult<usize> {
        if let Ok(data) = core::str::from_utf8(buf) {
            print!("{}", data);
        } else {
            (0..buf.len()).for_each(|i| {
                log::warn!("User stderr (non-utf8): {} ", buf[i]);
            });
        }
        Ok(buf.len())
    }
}
