use systype::error::SyscallResult;

/// Returns the real user ID of the calling process.
pub fn sys_getuid() -> SyscallResult {
    Ok(1000)
}

/// Returns the real group ID of the calling process.
pub fn sys_getgid() -> SyscallResult {
    Ok(1000)
}