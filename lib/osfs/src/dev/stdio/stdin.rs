use alloc::boxed::Box;
use alloc::sync::{Arc, Weak};
use arch::console::console_getchar;
use async_trait::async_trait;
use config::inode::InodeType;
use systype::error::SysResult;
use vfs::stat::Stat;
use vfs::{
    dentry::{Dentry, DentryMeta},
    file::{File, FileMeta},
    inode::{Inode, InodeMeta},
    inoid::alloc_ino,
    superblock::SuperBlock,
};
pub struct StdInDentry {
    meta: DentryMeta,
}

pub struct StdInInode {
    meta: InodeMeta,
}

pub struct StdInFile {
    meta: FileMeta,
}

impl StdInDentry {
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

impl StdInFile {
    pub fn new(sb: Arc<dyn SuperBlock>) -> Arc<Self> {
        let inode = Arc::new(StdInInode {
            meta: InodeMeta::new(alloc_ino(), sb),
        });
        inode.set_inotype(InodeType::CharDevice);
        let dentry = StdInDentry::new("stdin", Some(inode), None);
        Arc::new(Self {
            meta: FileMeta::new(dentry),
        })
    }
}

impl Dentry for StdInDentry {
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

impl Inode for StdInInode {
    fn get_meta(&self) -> &InodeMeta {
        &self.meta
    }
    fn get_attr(&self) -> SysResult<Stat> {
        todo!()
    }
}

#[async_trait]
impl File for StdInFile {
    fn meta(&self) -> &FileMeta {
        &self.meta
    }

    async fn base_read(&self, buf: &mut [u8], _pos: usize) -> SysResult<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        let mut cnt = 0;
        while cnt < buf.len() {
            let c = console_getchar();
            buf[cnt] = c;
            cnt += 1;
            // yield_now().await;
        }
        Ok(cnt)
    }
}
