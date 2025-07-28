use alloc::borrow::ToOwned;
use alloc::sync::Arc;
use alloc::vec::Vec;
use alloc::{collections::BTreeMap, string::String};
use core::sync::atomic::{AtomicU32, Ordering};
use mutex::SpinNoIrqLock;
use systype::error::{SysError, SysResult};
use vfs::{
    inode::{Inode, InodeMeta},
    inoid::alloc_ino,
    stat::Stat,
    sys_root_dentry,
};

use super::{
    event::{BpfMap, BpfProgram},
    flags::BpfCommand,
};

pub struct BpfInode {
    meta: InodeMeta,
    /// Loaded programs (fd -> program)
    programs: SpinNoIrqLock<BTreeMap<u32, Arc<BpfProgram>>>,
    /// Created maps (fd -> map)
    maps: SpinNoIrqLock<BTreeMap<u32, Arc<SpinNoIrqLock<BpfMap>>>>,
    /// Next program FD
    next_prog_fd: AtomicU32,
    /// Next map FD
    next_map_fd: AtomicU32,
    /// Links (program attachments)
    links: SpinNoIrqLock<BTreeMap<u32, BpfLink>>,
    /// Next link ID
    next_link_id: AtomicU32,
}

#[derive(Debug, Clone)]
pub struct BpfLink {
    pub id: u32,
    pub prog_id: u32,
    pub attach_type: u32,
    pub target_fd: i32,
    pub flags: u32,
}

impl BpfInode {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            meta: InodeMeta::new(alloc_ino(), sys_root_dentry().superblock().unwrap()),
            programs: SpinNoIrqLock::new(BTreeMap::new()),
            maps: SpinNoIrqLock::new(BTreeMap::new()),
            next_prog_fd: AtomicU32::new(1),
            next_map_fd: AtomicU32::new(1),
            links: SpinNoIrqLock::new(BTreeMap::new()),
            next_link_id: AtomicU32::new(1),
        })
    }

    /// Load a BPF program
    pub fn load_program(&self, program: BpfProgram) -> SysResult<u32> {
        // Validate program
        program.validate().map_err(|_| SysError::EINVAL)?;

        let prog_fd = self.next_prog_fd.fetch_add(1, Ordering::SeqCst);
        let prog_arc = Arc::new(program);

        self.programs.lock().insert(prog_fd, prog_arc);

        log::debug!("[BPF] Loaded program with FD: {}", prog_fd);
        Ok(prog_fd)
    }

    /// Create a BPF map
    pub fn create_map(&self, map: BpfMap) -> SysResult<u32> {
        // Validate map
        map.validate().map_err(|_| SysError::EINVAL)?;

        let map_fd = self.next_map_fd.fetch_add(1, Ordering::SeqCst);
        let map_arc = Arc::new(SpinNoIrqLock::new(map));

        self.maps.lock().insert(map_fd, map_arc);

        log::debug!("[BPF] Created map with FD: {}", map_fd);
        Ok(map_fd)
    }

    /// Get program by FD
    pub fn get_program(&self, prog_fd: u32) -> SysResult<Arc<BpfProgram>> {
        self.programs
            .lock()
            .get(&prog_fd)
            .cloned()
            .ok_or(SysError::EBADF)
    }

    /// Get map by FD
    pub fn get_map(&self, map_fd: u32) -> SysResult<Arc<SpinNoIrqLock<BpfMap>>> {
        self.maps
            .lock()
            .get(&map_fd)
            .cloned()
            .ok_or(SysError::EBADF)
    }

    /// Map lookup element
    pub fn map_lookup_elem(&self, map_fd: u32, key: &[u8]) -> SysResult<Option<Vec<u8>>> {
        let map = self.get_map(map_fd)?;
        let map_guard = map.lock();
        Ok(map_guard.data.lookup(key))
    }

    /// Map update element
    pub fn map_update_elem(
        &self,
        map_fd: u32,
        key: &[u8],
        value: &[u8],
        flags: u64,
    ) -> SysResult<()> {
        let map = self.get_map(map_fd)?;
        let mut map_guard = map.lock();
        map_guard
            .data
            .update(key, value, flags)
            .map_err(|_| SysError::EINVAL)
    }

    /// Map delete element
    pub fn map_delete_elem(&self, map_fd: u32, key: &[u8]) -> SysResult<()> {
        let map = self.get_map(map_fd)?;
        let mut map_guard = map.lock();
        map_guard.data.delete(key).map_err(|_| SysError::ENOENT)
    }

    /// Map get next key
    pub fn map_get_next_key(&self, map_fd: u32, key: Option<&[u8]>) -> SysResult<Option<Vec<u8>>> {
        let map = self.get_map(map_fd)?;
        let map_guard = map.lock();
        Ok(map_guard.data.get_next_key(key))
    }

    /// Attach program
    pub fn prog_attach(
        &self,
        prog_fd: u32,
        target_fd: i32,
        attach_type: u32,
        flags: u32,
    ) -> SysResult<u32> {
        // Verify program exists
        let _program = self.get_program(prog_fd)?;

        let link_id = self.next_link_id.fetch_add(1, Ordering::SeqCst);
        let link = BpfLink {
            id: link_id,
            prog_id: prog_fd,
            attach_type,
            target_fd,
            flags,
        };

        self.links.lock().insert(link_id, link);

        log::debug!(
            "[BPF] Attached program {} to target {} with link {}",
            prog_fd,
            target_fd,
            link_id
        );
        Ok(link_id)
    }

    /// Detach program
    pub fn prog_detach(&self, target_fd: i32, attach_type: u32) -> SysResult<()> {
        let mut links = self.links.lock();
        let to_remove: Vec<u32> = links
            .iter()
            .filter(|(_, link)| link.target_fd == target_fd && link.attach_type == attach_type)
            .map(|(id, _)| *id)
            .collect();

        for id in to_remove {
            links.remove(&id);
        }

        Ok(())
    }

    /// Test run program (simplified)
    pub fn prog_test_run(&self, prog_fd: u32, data_in: &[u8]) -> SysResult<(Vec<u8>, u32, u32)> {
        let _program = self.get_program(prog_fd)?;

        // todo!
        // In a real implementation, this would execute the BPF program
        // For now, return dummy data
        let data_out = data_in.to_vec();
        let retval = 0;
        let duration = 1000; // microseconds

        Ok((data_out, retval, duration))
    }

    /// Get program info
    pub fn get_prog_info(&self, prog_fd: u32) -> SysResult<BpfProgInfo> {
        let program = self.get_program(prog_fd)?;

        Ok(BpfProgInfo {
            id: program.id,
            type_: program.prog_type,
            name: program.name.clone(),
            tag: [0u8; 8], // Program tag (hash)
            jited_prog_len: 0,
            xlated_prog_len: (program.insns.len() * core::mem::size_of::<super::event::BpfInsn>())
                as u32,
            nr_map_ids: 0,
            creation_time: 0,
            load_time: 0,
            uid: 0,
            gid: 0,
            jited_prog_insns: 0,
            xlated_prog_insns: program.insns.len() as u64
                * core::mem::size_of::<super::event::BpfInsn>() as u64,
        })
    }

    /// Get map info
    pub fn get_map_info(&self, map_fd: u32) -> SysResult<BpfMapInfo> {
        let map = self.get_map(map_fd)?;
        let map_guard = map.lock();

        Ok(BpfMapInfo {
            id: map_guard.id,
            type_: map_guard.map_type,
            name: map_guard.name.clone(),
            key_size: map_guard.key_size,
            value_size: map_guard.value_size,
            max_entries: map_guard.max_entries,
            map_flags: map_guard.map_flags,
            ifindex: 0,
            btf_id: 0,
            btf_key_type_id: map_guard.btf_key_type_id,
            btf_value_type_id: map_guard.btf_value_type_id,
        })
    }

    /// Get statistics
    pub fn get_stats(&self) -> BpfStats {
        let programs = self.programs.lock();
        let maps = self.maps.lock();
        let links = self.links.lock();

        BpfStats {
            prog_count: programs.len() as u32,
            map_count: maps.len() as u32,
            link_count: links.len() as u32,
        }
    }
}

#[derive(Debug, Clone)]
pub struct BpfProgInfo {
    pub id: u32,
    pub type_: u32,
    pub name: String,
    pub tag: [u8; 8],
    pub jited_prog_len: u32,
    pub xlated_prog_len: u32,
    pub nr_map_ids: u32,
    pub creation_time: u64,
    pub load_time: u64,
    pub uid: u32,
    pub gid: u32,
    pub jited_prog_insns: u64,
    pub xlated_prog_insns: u64,
}

#[derive(Debug, Clone)]
pub struct BpfMapInfo {
    pub id: u32,
    pub type_: u32,
    pub name: String,
    pub key_size: u32,
    pub value_size: u32,
    pub max_entries: u32,
    pub map_flags: u32,
    pub ifindex: u32,
    pub btf_id: u32,
    pub btf_key_type_id: u32,
    pub btf_value_type_id: u32,
}

#[derive(Debug, Clone)]
pub struct BpfStats {
    pub prog_count: u32,
    pub map_count: u32,
    pub link_count: u32,
}

impl Inode for BpfInode {
    fn get_meta(&self) -> &InodeMeta {
        &self.meta
    }

    fn get_attr(&self) -> SysResult<Stat> {
        let inner = self.meta.inner.lock();
        Ok(Stat {
            st_dev: 0,
            st_ino: self.meta.ino as u64,
            st_mode: config::inode::InodeMode::REG.bits(),
            st_nlink: 1,
            st_uid: 0,
            st_gid: 0,
            st_rdev: 0,
            __pad: 0,
            st_size: 0,
            st_blksize: 512,
            __pad2: 0,
            st_blocks: 0,
            st_atime: inner.atime,
            st_mtime: inner.mtime,
            st_ctime: inner.ctime,
            unused: 0,
        })
    }

    fn set_size(&self, _size: usize) -> SysResult<()> {
        Err(SysError::EINVAL)
    }
}
