use alloc::sync::Arc;
use config::{inode::InodeType, vfs::OpenFlags};
use osfuture::block_on;
use vfs::{dentry::Dentry, file::File, inode::Inode, sys_root_dentry};

use crate::simple::{dentry::SimpleDentry, inode::SimpleInode};

pub fn login_user() {
    let info =
        "root:x:0:0:root:/root:/bin/sh\nnobody:x:65534:65534:nobody:/nonexistent:/usr/sbin/nologin";

    let root_dentry = sys_root_dentry();

    let etc_inode = SimpleInode::new(root_dentry.superblock().unwrap());
    etc_inode.set_inotype(InodeType::Dir);
    let etc_dentry: Arc<dyn Dentry> =
        SimpleDentry::new("etc", Some(etc_inode), Some(Arc::downgrade(&root_dentry)));
    root_dentry.add_child(etc_dentry.clone());

    let passwd_inode = SimpleInode::new(root_dentry.superblock().unwrap());
    passwd_inode.set_inotype(InodeType::File);
    let passwd_dentry: Arc<dyn Dentry> = SimpleDentry::new(
        "passwd",
        Some(passwd_inode),
        Some(Arc::downgrade(&etc_dentry)),
    );
    etc_dentry.add_child(passwd_dentry.clone());

    let file = <dyn File>::open(passwd_dentry).unwrap();
    file.set_flags(OpenFlags::O_RDWR);
    let _ = block_on(async { file.write(info.as_bytes()).await });
    let _ = file.seek(config::vfs::SeekFrom::Start(0));
    log::debug!("login user success");
}
