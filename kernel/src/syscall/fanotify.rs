use alloc::sync::Arc;

use config::vfs::OpenFlags;
use systype::error::{SysError, SyscallResult};
use vfs::fanotify::{
    FanotifyGroup,
    fs::create_group_file,
    types::{FanEventFileFlags, FanInitFlags},
};

use crate::processor::current_task;

pub fn sys_fanotify_init(flags: u32, event_f_flags: u32) -> SyscallResult {
    let flags = FanInitFlags::from_bits(flags).ok_or(SysError::EINVAL)?;
    let event_f_flags = FanEventFileFlags::from_bits(event_f_flags).ok_or(SysError::EINVAL)?;

    let task = current_task();
    let group = Arc::new(FanotifyGroup::new(flags, event_f_flags));
    let group_file = create_group_file(&group)?;
    let group_open_flags = OpenFlags::from(flags);
    let fd = task.with_mut_fdtable(|fdtable| fdtable.alloc(group_file, group_open_flags))?;

    Ok(fd)
}
