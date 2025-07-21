use alloc::sync::Arc;

use config::vfs::{AtFd, OpenFlags};
use osfs::fd_table::Fd;
use systype::error::{SysError, SyscallResult};
use vfs::fanotify::{
    FanotifyGroup, FsObject, FsObjectId,
    fs::{create_group_file, file::FanotifyGroupFile},
    types::{FanEventFileFlags, FanEventMask, FanInitFlags, FanMarkFlags},
};

use crate::{processor::current_task, vm::user_ptr::UserReadPtr};

pub fn sys_fanotify_init(flags: u32, event_f_flags: u32) -> SyscallResult {
    let flags = FanInitFlags::from_bits(flags).ok_or(SysError::EINVAL)?;
    let event_f_flags = FanEventFileFlags::from_bits(event_f_flags).ok_or(SysError::EINVAL)?;

    log::info!("sys_fanotify_init: flags={flags:?}, event_f_flags={event_f_flags:?}");

    if flags.contains(FanInitFlags::CLASS_PRE_CONTENT | FanInitFlags::CLASS_CONTENT) {
        return Err(SysError::EINVAL);
    }
    if flags.intersects(FanInitFlags::CLASS_PRE_CONTENT | FanInitFlags::CLASS_CONTENT)
        && flags.contains(FanInitFlags::REPORT_FID)
    {
        return Err(SysError::EINVAL);
    }
    if flags.contains(FanInitFlags::REPORT_NAME) && !flags.contains(FanInitFlags::REPORT_DIR_FID) {
        return Err(SysError::EINVAL);
    }
    if flags.contains(FanInitFlags::REPORT_TARGET_FID)
        && !flags.contains(
            FanInitFlags::REPORT_FID | FanInitFlags::REPORT_DIR_FID | FanInitFlags::REPORT_NAME,
        )
    {
        return Err(SysError::EINVAL);
    }
    if flags.contains(FanInitFlags::REPORT_PIDFD | FanInitFlags::REPORT_TID) {
        return Err(SysError::EINVAL);
    }

    if flags.intersects(
            FanInitFlags::CLASS_PRE_CONTENT
                | FanInitFlags::CLASS_CONTENT
                | FanInitFlags::UNLIMITED_QUEUE
                | FanInitFlags::UNLIMITED_MARKS
                | FanInitFlags::ENABLE_AUDIT
                | FanInitFlags::REPORT_TARGET_FID
                | FanInitFlags::REPORT_PIDFD,
    ) {
        unimplemented!("Unsupported fanotify flags: {flags:?}");
    }

    let task = current_task();
    let group = Arc::new(FanotifyGroup::new(flags, event_f_flags));
    let group_file = create_group_file(&group)?;
    let group_open_flags = OpenFlags::from(flags);
    let fd = task.with_mut_fdtable(|fdtable| fdtable.alloc(group_file, group_open_flags))?;

    Ok(fd)
}

pub fn sys_fanotify_mark(
    fanotify_fd: i32,
    flags: u32,
    mask: u64,
    dirfd: i32,
    pathname: usize,
) -> SyscallResult {
    // Get the fanotify group from the file descriptor.
    let group = current_task()
        .with_mut_fdtable(|fdtable| {
            let fd_info = fdtable.get(fanotify_fd as Fd)?;
            Ok(fd_info.file())
        })?
        .downcast_arc::<FanotifyGroupFile>()
        .or(Err(SysError::EBADF))?
        .group();

    // Check the validity of `flags` and `mask`.
    let flags = FanMarkFlags::from_bits(flags).ok_or(SysError::EINVAL)?;
    let mask = FanEventMask::from_bits(mask).ok_or(SysError::EINVAL)?;

    log::info!(
        "sys_fanotify_mark: flags={flags:?}, mask={mask:?}, dirfd={dirfd}, pathname={pathname:#x}"
    );

    if flags
        .intersection(FanMarkFlags::ADD | FanMarkFlags::REMOVE | FanMarkFlags::FLUSH)
        .bits()
        != 1
    {
        return Err(SysError::EINVAL);
    }
    if flags.contains(FanMarkFlags::FLUSH)
        && flags != FanMarkFlags::FLUSH
        && flags != FanMarkFlags::FLUSH | FanMarkFlags::MOUNT
        && flags != FanMarkFlags::FLUSH | FanMarkFlags::FILESYSTEM
    {
        return Err(SysError::EINVAL);
    }
    if flags.contains(FanMarkFlags::MOUNT)
        && mask.intersects(
                FanEventMask::ATTRIB
                    | FanEventMask::CREATE
                    | FanEventMask::DELETE
                    | FanEventMask::DELETE_SELF
                    | FanEventMask::FS_ERROR
                    | FanEventMask::MOVED_FROM
                    | FanEventMask::MOVED_TO
                    | FanEventMask::RENAME
                    | FanEventMask::MOVE_SELF,
            )
    {
        return Err(SysError::EINVAL);
    }

    if flags.intersects(
        FanMarkFlags::FLUSH
            | FanMarkFlags::MOUNT
            | FanMarkFlags::FILESYSTEM
            | FanMarkFlags::DONT_FOLLOW
            | FanMarkFlags::ONLYDIR
            | FanMarkFlags::IGNORED_MASK
            | FanMarkFlags::IGNORE
            | FanMarkFlags::IGNORED_SURV_MODIFY
            | FanMarkFlags::EVICTABLE,
    ) {
        unimplemented!("Unsupported fanotify flags: {flags:?}");
    }
    if mask.contains(
        FanEventMask::ACCESS
            | FanEventMask::OPEN
            | FanEventMask::OPEN_EXEC
            | FanEventMask::ATTRIB
            | FanEventMask::DELETE
            | FanEventMask::DELETE_SELF
            | FanEventMask::FS_ERROR
            | FanEventMask::RENAME
            | FanEventMask::MOVED_FROM
            | FanEventMask::MOVED_TO
            | FanEventMask::MOVE_SELF
            | FanEventMask::MODIFY
            | FanEventMask::CLOSE_WRITE
            | FanEventMask::CLOSE_NOWRITE
            | FanEventMask::Q_OVERFLOW
            | FanEventMask::ACCESS_PERM
            | FanEventMask::OPEN_PERM
            | FanEventMask::OPEN_EXEC_PERM
            | FanEventMask::ONDIR
            | FanEventMask::EVENT_ON_CHILD,
    ) {
        unimplemented!("Unsupported fanotify mask: {mask:?}");
    }

    // Get the object to be marked.
    let task = current_task();

    let path = if pathname == 0 {
        None
    } else {
        let addr_space = task.addr_space();
        let mut user_ptr = UserReadPtr::<u8>::new(pathname, &addr_space);
        let path = user_ptr.read_c_string(256)?;
        let path = path.into_string().or(Err(SysError::EINVAL))?;
        Some(path)
    };

    let dirfd = AtFd::from(dirfd as isize);
    let dir = match dirfd {
        AtFd::FdCwd => task.cwd().lock().clone(),
        AtFd::Normal(dirfd) => task
            .with_mut_fdtable(|fdtable| {
                let fd_info = fdtable.get(dirfd)?;
                Ok(fd_info.file())
            })?
            .dentry(),
    };

    let object_inode = {
        let dentry = if let Some(path) = path {
            task.walk_at(dirfd, path)?
        } else {
            dir
        };
        dentry.inode().ok_or(SysError::ENOENT)?
    };
    let (object_id, object) = (
        FsObjectId::Inode(object_inode.ino()),
        FsObject::Inode(Arc::downgrade(&object_inode)),
    );

    // Analyze `flags` and `mask`.
    let (mark, ignore) = (mask, FanEventMask::empty());

    if let Some(entry) = group.get_entry(object_id) {
        // If the object already has an entry, update its mark mask and ignore mask.
        if flags.contains(FanMarkFlags::ADD) {
            entry.add_mark(mark);
            entry.add_ignore(ignore);
        } else {
            entry.remove_mark(mark);
            entry.remove_ignore(ignore);
        }
    } else {
        // Create a new entry for the object in the fanotify group.
        if flags.contains(FanMarkFlags::ADD) {
            group.add_entry(object, mark, ignore);
        } else {
            return Err(SysError::ENOENT);
        }
    }

    Ok(0)
}
