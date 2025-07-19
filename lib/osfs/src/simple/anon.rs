use alloc::sync::Arc;
use vfs::dentry::{Dentry, DentryMeta};

pub struct AnonDentry {
    meta: DentryMeta,
}

impl AnonDentry {
    pub fn new(name: &str) -> Arc<Self> {
        Arc::new(Self {
            meta: DentryMeta::new(name, None, None),
        })
    }
}

impl Dentry for AnonDentry {
    fn get_meta(&self) -> &DentryMeta {
        &self.meta
    }

    fn base_open(self: Arc<Self>) -> systype::error::SysResult<Arc<dyn vfs::file::File>> {
        todo!()
    }

    fn base_create(
        &self,
        _dentry: &dyn Dentry,
        _mode: config::inode::InodeMode,
    ) -> systype::error::SysResult<()> {
        todo!()
    }

    fn base_lookup(&self, _dentry: &dyn Dentry) -> systype::error::SysResult<()> {
        todo!()
    }

    fn base_link(
        &self,
        _dentry: &dyn Dentry,
        _old_dentry: &dyn Dentry,
    ) -> systype::error::SysResult<()> {
        todo!()
    }

    fn base_unlink(&self, _dentry: &dyn Dentry) -> systype::error::SysResult<()> {
        todo!()
    }

    fn base_rename(
        &self,
        _dentry: &dyn Dentry,
        _new_dir: &dyn Dentry,
        _new_dentry: &dyn Dentry,
    ) -> systype::error::SysResult<()> {
        todo!()
    }

    fn base_new_neg_child(self: Arc<Self>, _name: &str) -> Arc<dyn Dentry> {
        todo!()
    }
}
