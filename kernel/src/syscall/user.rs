use systype::SyscallResult;

pub fn sys_getuid() -> SyscallResult {
    Ok(1000)
}