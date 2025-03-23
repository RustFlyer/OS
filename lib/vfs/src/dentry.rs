extern crate alloc;

use alloc::string::String;
use alloc::sync::Arc;
use systype::SysResult;

use crate::file::File;

pub struct DentryMeta {
    pub name: String,
}

pub trait Dentry: Send + Sync {
    fn get_meta(&self) -> &DentryMeta;

    fn open(self: Arc<Self>, name: &str) -> SysResult<Arc<dyn File>>;
}
