use crate::{
    processor::current_task,
    vm::user_ptr::{UserReadPtr, UserWritePtr},
};
use alloc::{
    string::{String, ToString},
    sync::Arc,
    vec::Vec,
};
use config::vfs::OpenFlags;
use osfs::special::bpf::{BpfCommand, BpfDentry, BpfFile, BpfInode, BpfInsn, BpfMap, BpfProgram};
use systype::error::{SysError, SyscallResult};
use vfs::{inode::Inode, sys_root_dentry};

static mut BPF_SUBSYSTEM: Option<Arc<BpfInode>> = None;
static BPF_INIT: spin::Once = spin::Once::new();

#[allow(static_mut_refs)]
fn get_bpf_subsystem() -> Arc<BpfInode> {
    BPF_INIT.call_once(|| unsafe {
        BPF_SUBSYSTEM = Some(BpfInode::new());
    });
    unsafe { BPF_SUBSYSTEM.as_ref().unwrap().clone() }
}

fn create_bpf_fd(bpf_type: &str) -> SyscallResult {
    let task = current_task();
    let inode = get_bpf_subsystem();
    inode.set_mode(config::inode::InodeMode::REG);

    let dentry = BpfDentry::new(
        bpf_type,
        Some(inode.clone()),
        Some(Arc::downgrade(&sys_root_dentry())),
    );
    sys_root_dentry().add_child(dentry.clone());

    let file = BpfFile::new(dentry);
    let file_flags = OpenFlags::O_RDWR;

    task.with_mut_fdtable(|ft| ft.alloc(file, file_flags))
}

pub fn sys_bpf(cmd: u32, attr_ptr: usize, size: u32) -> SyscallResult {
    let task = current_task();

    // Parse command
    let command = match cmd {
        0 => BpfCommand::BpfMapCreate,
        1 => BpfCommand::BpfMapLookupElem,
        2 => BpfCommand::BpfMapUpdateElem,
        3 => BpfCommand::BpfMapDeleteElem,
        4 => BpfCommand::BpfMapGetNextKey,
        5 => BpfCommand::BpfProgLoad,
        6 => BpfCommand::BpfObjPin,
        7 => BpfCommand::BpfObjGet,
        8 => BpfCommand::BpfProgAttach,
        9 => BpfCommand::BpfProgDetach,
        10 => BpfCommand::BpfProgTestRun,
        16 => BpfCommand::BpfObjGetInfoByFd,
        _ => return Err(SysError::EINVAL),
    };

    log::error!("[sys_bpf] cmd: {:?}, size: {}", command, size);

    // Get BPF subsystem
    let bpf_inode = get_bpf_subsystem();

    // Read attribute data from user space
    let addr_space = task.addr_space();
    let mut data_ptr = UserReadPtr::<u8>::new(attr_ptr, &addr_space);
    let attr_data = unsafe { data_ptr.read_array(size as usize) }?;

    match command {
        BpfCommand::BpfMapCreate => {
            if size < 48 {
                return Err(SysError::EINVAL);
            }

            // Parse map creation attributes
            let map_type =
                u32::from_ne_bytes([attr_data[0], attr_data[1], attr_data[2], attr_data[3]]);
            let key_size =
                u32::from_ne_bytes([attr_data[4], attr_data[5], attr_data[6], attr_data[7]]);
            let value_size =
                u32::from_ne_bytes([attr_data[8], attr_data[9], attr_data[10], attr_data[11]]);
            let max_entries =
                u32::from_ne_bytes([attr_data[12], attr_data[13], attr_data[14], attr_data[15]]);
            let map_flags =
                u32::from_ne_bytes([attr_data[16], attr_data[17], attr_data[18], attr_data[19]]);

            let map_name = String::from("bpf_map");
            let map = BpfMap::new(
                map_type,
                map_name,
                key_size,
                value_size,
                max_entries,
                map_flags,
            );
            let internal_map_id = bpf_inode.create_map(map)?;
            let fd = create_bpf_fd("bpf_map")?;

            let file = task.with_mut_fdtable(|ft| ft.get_file(fd))?;
            let bpf_file = file
                .as_any()
                .downcast_ref::<BpfFile>()
                .ok_or(SysError::EINVAL)?;
            bpf_file.set_map_id(internal_map_id)?;

            Ok(fd)
        }

        BpfCommand::BpfProgLoad => {
            if size < 72 {
                return Err(SysError::EINVAL);
            }

            // Parse program load attributes
            let prog_type =
                u32::from_ne_bytes([attr_data[0], attr_data[1], attr_data[2], attr_data[3]]);
            let insn_cnt =
                u32::from_ne_bytes([attr_data[4], attr_data[5], attr_data[6], attr_data[7]]);
            let insns_ptr = u64::from_ne_bytes([
                attr_data[8],
                attr_data[9],
                attr_data[10],
                attr_data[11],
                attr_data[12],
                attr_data[13],
                attr_data[14],
                attr_data[15],
            ]);
            let license_ptr = u64::from_ne_bytes([
                attr_data[16],
                attr_data[17],
                attr_data[18],
                attr_data[19],
                attr_data[20],
                attr_data[21],
                attr_data[22],
                attr_data[23],
            ]);

            // Read instructions from user space
            let mut insns_user_ptr = UserReadPtr::<u8>::new(insns_ptr as usize, &addr_space);
            let insns_data = unsafe {
                insns_user_ptr.read_array((insn_cnt as usize) * core::mem::size_of::<BpfInsn>())
            }?;

            // Parse instructions
            let mut insns = Vec::new();
            for chunk in insns_data.chunks_exact(core::mem::size_of::<BpfInsn>()) {
                let code = chunk[0];
                let dst_src = chunk[1];
                let off = i16::from_ne_bytes([chunk[2], chunk[3]]);
                let imm = i32::from_ne_bytes([chunk[4], chunk[5], chunk[6], chunk[7]]);
                insns.push(BpfInsn {
                    code,
                    dst_src,
                    off,
                    imm,
                });
            }

            // Read license
            let mut license_user_ptr = UserReadPtr::<u8>::new(license_ptr as usize, &addr_space);
            let license_data = unsafe { license_user_ptr.read_array(128) }?;
            let license = String::from_utf8_lossy(&license_data)
                .trim_end_matches('\0')
                .to_string();

            let prog_name = String::from("bpf_prog");
            let program = BpfProgram::new(prog_type, prog_name, insns, license);

            let internal_prog_id = bpf_inode.load_program(program)?;
            let fd = create_bpf_fd("bpf_prog")?;

            let file = task.with_mut_fdtable(|ft| ft.get_file(fd))?;
            let bpf_file = file
                .as_any()
                .downcast_ref::<BpfFile>()
                .ok_or(SysError::EINVAL)?;
            bpf_file.set_prog_id(internal_prog_id)?;

            log::debug!(
                "[sys_bpf] Loaded program with fd: {}, internal_id: {}",
                fd,
                internal_prog_id
            );

            Ok(fd as usize)
        }

        BpfCommand::BpfMapLookupElem => {
            if size < 32 {
                return Err(SysError::EINVAL);
            }

            let map_fd =
                u32::from_ne_bytes([attr_data[0], attr_data[1], attr_data[2], attr_data[3]]);
            let key_ptr = u64::from_ne_bytes([
                attr_data[4],
                attr_data[5],
                attr_data[6],
                attr_data[7],
                attr_data[8],
                attr_data[9],
                attr_data[10],
                attr_data[11],
            ]);
            let value_ptr = u64::from_ne_bytes([
                attr_data[12],
                attr_data[13],
                attr_data[14],
                attr_data[15],
                attr_data[16],
                attr_data[17],
                attr_data[18],
                attr_data[19],
            ]);

            // Get map info to determine key size
            let map_info = bpf_inode.get_map_info(map_fd)?;

            // Read key from user space
            let mut key_user_ptr = UserReadPtr::<u8>::new(key_ptr as usize, &addr_space);
            let key_data = unsafe { key_user_ptr.read_array(map_info.key_size as usize) }?;

            // Lookup value
            match bpf_inode.map_lookup_elem(map_fd, &key_data)? {
                Some(value) => {
                    // Write value back to user space
                    let mut value_user_ptr =
                        UserWritePtr::<u8>::new(value_ptr as usize, &addr_space);
                    unsafe { value_user_ptr.write_array(&value) }?;
                    Ok(0)
                }
                None => Err(SysError::ENOENT),
            }
        }

        BpfCommand::BpfMapUpdateElem => {
            if size < 32 {
                return Err(SysError::EINVAL);
            }

            let map_fd =
                u32::from_ne_bytes([attr_data[0], attr_data[1], attr_data[2], attr_data[3]]);
            let key_ptr = u64::from_ne_bytes([
                attr_data[4],
                attr_data[5],
                attr_data[6],
                attr_data[7],
                attr_data[8],
                attr_data[9],
                attr_data[10],
                attr_data[11],
            ]);
            let value_ptr = u64::from_ne_bytes([
                attr_data[12],
                attr_data[13],
                attr_data[14],
                attr_data[15],
                attr_data[16],
                attr_data[17],
                attr_data[18],
                attr_data[19],
            ]);
            let flags = u64::from_ne_bytes([
                attr_data[20],
                attr_data[21],
                attr_data[22],
                attr_data[23],
                attr_data[24],
                attr_data[25],
                attr_data[26],
                attr_data[27],
            ]);

            // Get map info
            let map_info = bpf_inode.get_map_info(map_fd)?;

            // Read key and value from user space
            let mut key_user_ptr = UserReadPtr::<u8>::new(key_ptr as usize, &addr_space);
            let key_data = unsafe { key_user_ptr.read_array(map_info.key_size as usize) }?;

            let mut value_user_ptr = UserReadPtr::<u8>::new(value_ptr as usize, &addr_space);
            let value_data = unsafe { value_user_ptr.read_array(map_info.value_size as usize) }?;

            // Update map
            bpf_inode.map_update_elem(map_fd, &key_data, &value_data, flags)?;
            Ok(0)
        }

        BpfCommand::BpfMapDeleteElem => {
            if size < 16 {
                return Err(SysError::EINVAL);
            }

            let map_fd =
                u32::from_ne_bytes([attr_data[0], attr_data[1], attr_data[2], attr_data[3]]);
            let key_ptr = u64::from_ne_bytes([
                attr_data[4],
                attr_data[5],
                attr_data[6],
                attr_data[7],
                attr_data[8],
                attr_data[9],
                attr_data[10],
                attr_data[11],
            ]);

            // Get map info
            let map_info = bpf_inode.get_map_info(map_fd)?;

            // Read key from user space
            let mut key_user_ptr = UserReadPtr::<u8>::new(key_ptr as usize, &addr_space);
            let key_data = unsafe { key_user_ptr.read_array(map_info.key_size as usize) }?;

            // Delete element
            bpf_inode.map_delete_elem(map_fd, &key_data)?;
            Ok(0)
        }

        BpfCommand::BpfMapGetNextKey => {
            if size < 24 {
                return Err(SysError::EINVAL);
            }

            let map_fd =
                u32::from_ne_bytes([attr_data[0], attr_data[1], attr_data[2], attr_data[3]]);
            let key_ptr = u64::from_ne_bytes([
                attr_data[4],
                attr_data[5],
                attr_data[6],
                attr_data[7],
                attr_data[8],
                attr_data[9],
                attr_data[10],
                attr_data[11],
            ]);
            let next_key_ptr = u64::from_ne_bytes([
                attr_data[12],
                attr_data[13],
                attr_data[14],
                attr_data[15],
                attr_data[16],
                attr_data[17],
                attr_data[18],
                attr_data[19],
            ]);

            // Get map info
            let map_info = bpf_inode.get_map_info(map_fd)?;

            // Read current key from user space (if provided)
            let current_key = if key_ptr != 0 {
                let mut key_user_ptr = UserReadPtr::<u8>::new(key_ptr as usize, &addr_space);
                let key_data = unsafe { key_user_ptr.read_array(map_info.key_size as usize) }?;
                Some(key_data)
            } else {
                None
            };

            // Get next key
            match bpf_inode.map_get_next_key(map_fd, current_key.as_deref())? {
                Some(next_key) => {
                    // Write next key back to user space
                    let mut next_key_user_ptr =
                        UserWritePtr::<u8>::new(next_key_ptr as usize, &addr_space);
                    unsafe { next_key_user_ptr.write_array(&next_key) }?;
                    Ok(0)
                }
                None => Err(SysError::ENOENT),
            }
        }

        BpfCommand::BpfProgAttach => {
            if size < 20 {
                return Err(SysError::EINVAL);
            }

            let target_fd =
                i32::from_ne_bytes([attr_data[0], attr_data[1], attr_data[2], attr_data[3]]);
            let attach_bpf_fd =
                u32::from_ne_bytes([attr_data[4], attr_data[5], attr_data[6], attr_data[7]]);
            let attach_type =
                u32::from_ne_bytes([attr_data[8], attr_data[9], attr_data[10], attr_data[11]]);
            let attach_flags =
                u32::from_ne_bytes([attr_data[12], attr_data[13], attr_data[14], attr_data[15]]);

            let link_id =
                bpf_inode.prog_attach(attach_bpf_fd, target_fd, attach_type, attach_flags)?;
            Ok(link_id as usize)
        }

        BpfCommand::BpfProgDetach => {
            if size < 12 {
                return Err(SysError::EINVAL);
            }

            let target_fd =
                i32::from_ne_bytes([attr_data[0], attr_data[1], attr_data[2], attr_data[3]]);
            let attach_type =
                u32::from_ne_bytes([attr_data[4], attr_data[5], attr_data[6], attr_data[7]]);

            bpf_inode.prog_detach(target_fd, attach_type)?;
            Ok(0)
        }

        BpfCommand::BpfProgTestRun => {
            if size < 40 {
                return Err(SysError::EINVAL);
            }

            let prog_fd =
                u32::from_ne_bytes([attr_data[0], attr_data[1], attr_data[2], attr_data[3]]);
            let data_size_in =
                u32::from_ne_bytes([attr_data[4], attr_data[5], attr_data[6], attr_data[7]]);
            let data_size_out =
                u32::from_ne_bytes([attr_data[8], attr_data[9], attr_data[10], attr_data[11]]);
            let data_in_ptr = u64::from_ne_bytes([
                attr_data[12],
                attr_data[13],
                attr_data[14],
                attr_data[15],
                attr_data[16],
                attr_data[17],
                attr_data[18],
                attr_data[19],
            ]);
            let data_out_ptr = u64::from_ne_bytes([
                attr_data[20],
                attr_data[21],
                attr_data[22],
                attr_data[23],
                attr_data[24],
                attr_data[25],
                attr_data[26],
                attr_data[27],
            ]);

            // Read input data
            let mut data_in = vec![0u8; data_size_in as usize];
            if data_size_in > 0 {
                let mut data_in_user_ptr =
                    UserReadPtr::<u8>::new(data_in_ptr as usize, &addr_space);
                data_in = unsafe { data_in_user_ptr.read_array(data_size_in as usize) }?;
            }

            // Run program
            let (data_out, retval, _duration) = bpf_inode.prog_test_run(prog_fd, &data_in)?;

            // Write output data back
            if data_size_out > 0 && !data_out.is_empty() {
                let write_size = core::cmp::min(data_size_out as usize, data_out.len());
                let mut data_out_user_ptr =
                    UserWritePtr::<u8>::new(data_out_ptr as usize, &addr_space);
                unsafe { data_out_user_ptr.write_array(&data_out[..write_size]) }?;
            }

            // Write back results (simplified - in real implementation would write to specific fields)
            Ok(retval as usize)
        }

        BpfCommand::BpfObjGetInfoByFd => {
            if size < 16 {
                return Err(SysError::EINVAL);
            }

            let bpf_fd =
                u32::from_ne_bytes([attr_data[0], attr_data[1], attr_data[2], attr_data[3]]);
            let info_len =
                u32::from_ne_bytes([attr_data[4], attr_data[5], attr_data[6], attr_data[7]]);
            let info_ptr = u64::from_ne_bytes([
                attr_data[8],
                attr_data[9],
                attr_data[10],
                attr_data[11],
                attr_data[12],
                attr_data[13],
                attr_data[14],
                attr_data[15],
            ]);

            // Try to get program info first, then map info
            let info_data = if let Ok(prog_info) = bpf_inode.get_prog_info(bpf_fd) {
                // Serialize program info (simplified)
                let mut data = Vec::new();
                data.extend_from_slice(&prog_info.id.to_ne_bytes());
                data.extend_from_slice(&prog_info.type_.to_ne_bytes());
                data.extend_from_slice(prog_info.name.as_bytes());
                data.resize(info_len as usize, 0);
                data
            } else if let Ok(map_info) = bpf_inode.get_map_info(bpf_fd) {
                // Serialize map info (simplified)
                let mut data = Vec::new();
                data.extend_from_slice(&map_info.id.to_ne_bytes());
                data.extend_from_slice(&map_info.type_.to_ne_bytes());
                data.extend_from_slice(&map_info.key_size.to_ne_bytes());
                data.extend_from_slice(&map_info.value_size.to_ne_bytes());
                data.extend_from_slice(&map_info.max_entries.to_ne_bytes());
                data.extend_from_slice(map_info.name.as_bytes());
                data.resize(info_len as usize, 0);
                data
            } else {
                return Err(SysError::EBADF);
            };

            // Write info back to user space
            let write_size = core::cmp::min(info_len as usize, info_data.len());
            let mut info_user_ptr = UserWritePtr::<u8>::new(info_ptr as usize, &addr_space);
            unsafe { info_user_ptr.write_array(&info_data[..write_size]) }?;

            Ok(0)
        }

        _ => {
            log::warn!("[sys_bpf] Unsupported command: {:?}", command);
            Err(SysError::ENOSYS)
        }
    }
}
