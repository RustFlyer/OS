use alloc::sync::Arc;
use config::vfs::OpenFlags;
use osfs::special::io_uring::{
    dentry::IoUringDentry,
    event::IoUringParams,
    file::IoUringFile,
    flags::{IoUringEnterFlags, IoUringRegisterOp, IoUringSetupFlags},
    inode::IoUringInode,
};
use systype::error::{SysError, SyscallResult};
use vfs::sys_root_dentry;

use crate::{
    processor::current_task,
    vm::user_ptr::{UserReadPtr, UserWritePtr},
};
use vfs::inode::Inode;

/// io_uring_setup system call
pub fn sys_io_uring_setup(entries: u32, params_ptr: usize) -> SyscallResult {
    let task = current_task();

    // Validate entries count
    if entries == 0 || entries > 32768 {
        return Err(SysError::EINVAL);
    }

    // Read parameters from user space
    let addr_space = task.addr_space();
    let mut params_reader = UserReadPtr::<IoUringParams>::new(params_ptr, &addr_space);
    let mut params = unsafe { params_reader.read() }?;

    // Validate and sanitize flags
    let setup_flags = IoUringSetupFlags::from_bits(params.flags).ok_or(SysError::EINVAL)?;

    // Validate entries is power of 2 or clamp it
    let actual_entries = if setup_flags.contains(IoUringSetupFlags::IORING_SETUP_CLAMP) {
        entries.next_power_of_two().min(32768)
    } else if !entries.is_power_of_two() {
        return Err(SysError::EINVAL);
    } else {
        entries
    };

    log::debug!(
        "[sys_io_uring_setup] entries: {}, flags: {:?}",
        actual_entries,
        setup_flags
    );

    // Create io_uring inode
    let inode = IoUringInode::new(actual_entries, setup_flags, task.pid() as u32)?;
    inode.set_mode(config::inode::InodeMode::REG);

    // Get updated parameters
    params = inode.get_params();

    // Write parameters back to user space
    let mut params_writer = UserWritePtr::<IoUringParams>::new(params_ptr, &addr_space);
    unsafe { params_writer.write(params) }?;

    // Create dentry and file
    let dentry = IoUringDentry::new(
        "io_uring",
        Some(inode),
        Some(Arc::downgrade(&sys_root_dentry())),
    );
    sys_root_dentry().add_child(dentry.clone());

    let file = IoUringFile::new(dentry);

    // Set file flags
    let file_flags = OpenFlags::O_RDWR;
    // io_turing doesn't use standard CLOEXEC, it's managed internally

    // Allocate file descriptor
    task.with_mut_fdtable(|ft| ft.alloc(file, file_flags))
}

/// io_uring_enter system call
pub fn sys_io_uring_enter(
    fd: usize,
    to_submit: u32,
    min_complete: u32,
    flags: u32,
    sig: usize, // sigset_t pointer
) -> SyscallResult {
    let task = current_task();

    // Get io_uring file
    let file = task.with_mut_fdtable(|ft| ft.get_file(fd))?;
    let io_uring_file = file
        .as_any()
        .downcast_ref::<IoUringFile>()
        .ok_or(SysError::EINVAL)?;

    // Validate flags
    let enter_flags = IoUringEnterFlags::from_bits(flags).ok_or(SysError::EINVAL)?;

    log::debug!(
        "[sys_io_uring_enter] fd: {}, to_submit: {}, min_complete: {}, flags: {:?}",
        fd,
        to_submit,
        min_complete,
        enter_flags
    );

    // Handle signal mask if provided
    let sig_ptr = if sig != 0 && enter_flags.contains(IoUringEnterFlags::IORING_ENTER_EXT_ARG) {
        Some(sig as u64)
    } else {
        None
    };

    // Perform enter operation
    let result = io_uring_file.enter(to_submit, min_complete, enter_flags, sig_ptr)?;

    Ok(result as usize)
}

/// io_uring_register system call
pub fn sys_io_uring_register(fd: usize, opcode: u32, arg: usize, nr_args: u32) -> SyscallResult {
    let task = current_task();

    // Get io_uring file
    let file = task.with_mut_fdtable(|ft| ft.get_file(fd))?;
    let io_uring_file = file
        .as_any()
        .downcast_ref::<IoUringFile>()
        .ok_or(SysError::EINVAL)?;

    // Validate opcode
    let register_op = IoUringRegisterOp::from_bits(1u32 << opcode).ok_or(SysError::EINVAL)?;

    log::debug!(
        "[sys_io_uring_register] fd: {}, opcode: {:?}, arg: 0x{:x}, nr_args: {}",
        fd,
        register_op,
        arg,
        nr_args
    );

    // Perform register operation
    let result = io_uring_file.register(register_op, arg as u64, nr_args)?;

    Ok(result as usize)
}
