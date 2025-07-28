use alloc::sync::Arc;
use config::{inode::InodeMode, vfs::OpenFlags};
use systype::error::SysResult;
use vfs::dentry::Dentry;

use crate::simple::dentry::SimpleDentry;

pub mod fs;
pub mod superblock;

pub fn init_etcfs(root_dentry: Arc<dyn Dentry>) -> SysResult<()> {
    let passwd_dentry =
        SimpleDentry::new("passwd", None, Some(Arc::downgrade(&root_dentry.clone())));
    root_dentry.add_child(passwd_dentry.clone());
    root_dentry.create(&passwd_dentry.clone().into_dyn(), InodeMode::REG)?;
    log::info!("[init_procfs] add passwd_dentry");
    let passwd_file = passwd_dentry.base_open()?;
    passwd_file.set_flags(OpenFlags::O_WRONLY);

    Ok(())
}
