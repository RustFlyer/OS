use crate::{processor::current_task, vm::user_ptr::UserReadPtr};
use alloc::sync::Arc;
use config::vfs::{AtFd, OpenFlags};
use osfs::special::fscontext::{
    FsConfigCmd, FsConfigCommand, FsContextDentry, FsContextFile, FsContextInode, FsParameterValue,
    FsmountFlags, FsopenFlags,
};
use systype::error::{SysError, SyscallResult};
use vfs::inode::Inode;
use vfs::sys_root_dentry;

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct FspickFlags: u32 {
        const CLOEXEC          = 0x0001;
        const SYMLINK_NOFOLLOW = 0x0002;
        const NO_AUTOMOUNT     = 0x0004;
        const EMPTY_PATH       = 0x0008;
    }
}

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct FsconfigCmd : u32 {
        const  SetFlag = 0;
        const  SetString = 1;
        const  SetBinary = 2;
        const  SetPath = 3;
        const  SetPathEmpty = 4;
        const  SetFd = 5;
        const  CmdCreate = 6;
        const  CmdReconfigure = 7;
    }
}

pub fn sys_fspick(dirfd: usize, pathname: usize, flags: u32) -> SyscallResult {
    let task = current_task();
    let addr_space = task.addr_space();
    let cpath = UserReadPtr::<u8>::new(pathname, &addr_space).read_c_string(256)?;
    let path = cpath.into_string().unwrap();
    let flags = FspickFlags::from_bits_truncate(flags);
    let open_flags = if flags.contains(FspickFlags::CLOEXEC) {
        OpenFlags::O_CLOEXEC
    } else {
        OpenFlags::empty()
    };

    let dentry = task.walk_at(AtFd::from(dirfd), path.clone())?;
    let file = dentry.base_open()?;
    let fd = task.with_mut_fdtable(|ft| ft.alloc(file, open_flags))?;
    Ok(fd)
}

// pub fn sys_fsconfig(fs_fd: usize, cmd: u32, key: usize, value: usize, aux: usize) -> SyscallResult {
//     let task = current_task();
//     let addr_space = task.addr_space();
//     let key_str = if key != 0 {
//         Some(UserReadPtr::<u8>::new(key, &addr_space).read_c_string(256)?)
//     } else {
//         None
//     };

//     let value_str = if value != 0 {
//         Some(UserReadPtr::<u8>::new(value, &addr_space).read_c_string(256)?)
//     } else {
//         None
//     };

//     let cmd = FsconfigCmd::from_bits_truncate(cmd);

//     match cmd {
//         FsconfigCmd::SetString | FsconfigCmd::SetFlag | FsconfigCmd::CmdReconfigure => Ok(0),
//         _ => Err(SysError::EINVAL),
//     }
// }

pub fn sys_fsopen(fs_name_ptr: usize, flags: u32) -> SyscallResult {
    let task = current_task();

    // Check permissions - in real kernel this would check CAP_SYS_ADMIN or may_mount()
    // For now, simplified permission check
    if task.uid() != 0 {
        return Err(SysError::EPERM);
    }

    // Validate flags
    let fsopen_flags = FsopenFlags::from_bits(flags).ok_or(SysError::EINVAL)?;

    // Read filesystem name from user space
    let addr_space = task.addr_space();
    let mut data_ptr = UserReadPtr::<u8>::new(fs_name_ptr, &addr_space);
    let fs_name_cstring = data_ptr.read_c_string(256)?; // Reasonable limit
    let fs_name = fs_name_cstring
        .into_string()
        .map_err(|_| SysError::EINVAL)?;

    log::debug!(
        "[sys_fsopen] fs_name: {}, flags: {:?}",
        fs_name,
        fsopen_flags
    );

    /// Check if a filesystem type is valid and supported
    fn is_valid_filesystem_type(fs_name: &str) -> bool {
        // In a real implementation, this would check:
        // 1. Built-in filesystem types
        // 2. Loadable filesystem modules
        // 3. Pseudo filesystems

        match fs_name {
            // Common filesystem types
            "ext2" | "ext3" | "ext4" => true,
            "xfs" | "btrfs" | "f2fs" => true,
            "vfat" | "ntfs" | "exfat" => true,
            // Network filesystems
            "nfs" | "nfs4" | "cifs" | "9p" => true,
            // Pseudo filesystems
            "proc" | "sysfs" | "devpts" | "tmpfs" => true,
            "debugfs" | "tracefs" | "securityfs" => true,
            // Memory filesystems
            "ramfs" | "rootfs" | "hugetlbfs" => true,
            // Special filesystems
            "overlayfs" | "aufs" | "unionfs" => true,
            _ => false, // Unknown filesystem type
        }
    }
    // Check if filesystem type exists (simplified check)
    if !is_valid_filesystem_type(&fs_name) {
        return Err(SysError::ENODEV);
    }

    // Create filesystem context inode
    let inode = FsContextInode::new(fsopen_flags, fs_name.clone());
    inode.set_mode(config::inode::InodeMode::REG);

    // Create dentry
    let dentry = FsContextDentry::new(
        "fscontext",
        Some(inode),
        Some(Arc::downgrade(&sys_root_dentry())),
    );
    sys_root_dentry().add_child(dentry.clone());

    // Create file
    let file = FsContextFile::new(dentry);

    // Set file flags
    let mut file_flags = OpenFlags::O_RDWR;
    if fsopen_flags.contains(FsopenFlags::FSOPEN_CLOEXEC) {
        file_flags |= OpenFlags::O_CLOEXEC;
    }

    // Allocate file descriptor
    task.with_mut_fdtable(|ft| ft.alloc(file, file_flags))
}

/// fsconfig syscall - configure filesystem context
pub fn sys_fsconfig(
    fd: usize,
    cmd: u32,
    key_ptr: usize,
    value_ptr: usize,
    aux: i32,
) -> SyscallResult {
    let task = current_task();

    // Get filesystem context file
    let file = task.with_mut_fdtable(|ft| ft.get_file(fd))?;
    let fs_file = file
        .as_any()
        .downcast_ref::<FsContextFile>()
        .ok_or(SysError::EINVAL)?;

    let addr_space = task.addr_space();

    // Parse command
    let fs_cmd = match cmd {
        c if c == FsConfigCmd::FSCONFIG_SET_STRING.bits() => {
            // Read key and value strings
            let mut key_ptr = UserReadPtr::<u8>::new(key_ptr, &addr_space);
            let key = key_ptr
                .read_c_string(256)?
                .into_string()
                .map_err(|_| SysError::EINVAL)?;

            let mut val_ptr = UserReadPtr::<u8>::new(value_ptr, &addr_space);
            let value = val_ptr
                .read_c_string(4096)?
                .into_string()
                .map_err(|_| SysError::EINVAL)?;

            FsConfigCommand {
                cmd,
                key: Some(key),
                value: Some(FsParameterValue::String(value)),
                aux,
            }
        }
        c if c == FsConfigCmd::FSCONFIG_SET_FLAG.bits() => {
            // Read key only
            let mut key_ptr = UserReadPtr::<u8>::new(key_ptr, &addr_space);
            let key = key_ptr
                .read_c_string(256)?
                .into_string()
                .map_err(|_| SysError::EINVAL)?;

            FsConfigCommand {
                cmd,
                key: Some(key),
                value: Some(FsParameterValue::None),
                aux,
            }
        }
        c if c == FsConfigCmd::FSCONFIG_SET_BINARY.bits() => {
            // Read key and binary data
            let mut key_ptr = UserReadPtr::<u8>::new(key_ptr, &addr_space);
            let key = key_ptr
                .read_c_string(256)?
                .into_string()
                .map_err(|_| SysError::EINVAL)?;

            // aux contains the length of binary data
            if aux < 0 || aux > 65536 {
                // Reasonable limit
                return Err(SysError::EINVAL);
            }

            let mut val_ptr = UserReadPtr::<u8>::new(value_ptr, &addr_space);
            let data = unsafe { val_ptr.read_array(aux as usize)? };

            FsConfigCommand {
                cmd,
                key: Some(key),
                value: Some(FsParameterValue::Blob(data)),
                aux,
            }
        }
        c if c == FsConfigCmd::FSCONFIG_SET_PATH.bits() => {
            // Read key and path
            let mut key_ptr = UserReadPtr::<u8>::new(key_ptr, &addr_space);
            let key = key_ptr
                .read_c_string(256)?
                .into_string()
                .map_err(|_| SysError::EINVAL)?;

            let mut path_ptr = UserReadPtr::<u8>::new(value_ptr, &addr_space);
            let path = path_ptr
                .read_c_string(4096)?
                .into_string()
                .map_err(|_| SysError::EINVAL)?;

            FsConfigCommand {
                cmd,
                key: Some(key),
                value: Some(FsParameterValue::Path(path)),
                aux,
            }
        }
        c if c == FsConfigCmd::FSCONFIG_SET_FD.bits() => {
            // Read key, fd is in aux
            let mut key_ptr = UserReadPtr::<u8>::new(key_ptr, &addr_space);
            let key = key_ptr
                .read_c_string(256)?
                .into_string()
                .map_err(|_| SysError::EINVAL)?;

            FsConfigCommand {
                cmd,
                key: Some(key),
                value: Some(FsParameterValue::Fd(aux)),
                aux,
            }
        }
        c if c == FsConfigCmd::FSCONFIG_CMD_CREATE.bits() => FsConfigCommand {
            cmd,
            key: None,
            value: None,
            aux,
        },
        c if c == FsConfigCmd::FSCONFIG_CMD_RECONFIGURE.bits() => FsConfigCommand {
            cmd,
            key: None,
            value: None,
            aux,
        },
        _ => return Err(SysError::EINVAL),
    };

    log::debug!("[sys_fsconfig] fd: {}, cmd: {}, aux: {}", fd, cmd, aux);

    // Execute the command
    fs_file.execute_command(fs_cmd)?;

    Ok(0)
}

/// fsmount syscall - create a mount from filesystem context
pub fn sys_fsmount(fd: usize, flags: u32, attr_flags: u32) -> SyscallResult {
    let task = current_task();

    // Check permissions
    if task.uid() != 0 {
        return Err(SysError::EPERM);
    }

    // Validate flags
    let _mount_flags = FsmountFlags::from_bits(flags).ok_or(SysError::EINVAL)?;

    // Get filesystem context file
    let file = task.with_mut_fdtable(|ft| ft.get_file(fd))?;
    let fs_file = file
        .as_any()
        .downcast_ref::<FsContextFile>()
        .ok_or(SysError::EINVAL)?;

    // Check if context is ready for mounting
    if !fs_file.is_ready_for_mount()? {
        return Err(SysError::EBUSY);
    }

    // Get the filesystem context
    let context = fs_file.get_context()?;
    let fs_type = fs_file.get_fs_type()?;

    log::debug!(
        "[sys_fsmount] fd: {}, fs_type: {}, flags: {}",
        fd,
        fs_type,
        flags
    );

    // In a real implementation, this would:
    // 1. Create a new mount namespace entry
    // 2. Initialize the superblock if not already done
    // 3. Create a mount point file descriptor
    // 4. Return the mount fd

    todo!()
}

/// move_mount syscall - move a mount to a new location
pub fn sys_move_mount(
    from_dfd: i32,
    from_pathname_ptr: usize,
    to_dfd: i32,
    to_pathname_ptr: usize,
    flags: u32,
) -> SyscallResult {
    let task = current_task();

    // Check permissions
    if task.uid() != 0 {
        return Err(SysError::EPERM);
    }

    let addr_space = task.addr_space();

    // Read paths from user space
    let mut from_ptr = UserReadPtr::<u8>::new(from_pathname_ptr, &addr_space);
    let from_path = from_ptr
        .read_c_string(4096)?
        .into_string()
        .map_err(|_| SysError::EINVAL)?;

    let mut to_ptr = UserReadPtr::<u8>::new(to_pathname_ptr, &addr_space);
    let to_path = to_ptr
        .read_c_string(4096)?
        .into_string()
        .map_err(|_| SysError::EINVAL)?;

    log::debug!(
        "[sys_move_mount] from: {}, to: {}, flags: {}",
        from_path,
        to_path,
        flags
    );

    // In a real implementation, this would:
    // 1. Resolve the source mount point
    // 2. Resolve the destination path
    // 3. Move the mount atomically
    // 4. Update mount namespace

    todo!()
}
