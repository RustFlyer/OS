use core::sync::atomic::{AtomicU32, Ordering};

use alloc::sync::Arc;
use alloc::{boxed::Box, vec::Vec};
use async_trait::async_trait;
use systype::error::{SysError, SysResult};
use vfs::{
    dentry::Dentry,
    file::{File, FileMeta},
};

use super::{
    event::{BpfMap, BpfProgram},
    inode::{BpfInode, BpfMapInfo, BpfProgInfo, BpfStats},
};

pub struct BpfFile {
    meta: FileMeta,
    /// 内部程序 ID (如果是程序文件描述符)
    prog_id: AtomicU32,
    /// 内部映射 ID (如果是映射文件描述符)
    map_id: AtomicU32,
}

impl BpfFile {
    pub fn new(dentry: Arc<dyn Dentry>) -> Arc<Self> {
        Arc::new(Self {
            meta: FileMeta::new(dentry),
            prog_id: AtomicU32::new(0),
            map_id: AtomicU32::new(0),
        })
    }

    pub fn into_dyn_ref(&self) -> &dyn File {
        self
    }

    /// set program ID
    pub fn set_prog_id(&self, id: u32) -> SysResult<()> {
        self.prog_id.store(id, Ordering::Relaxed);
        Ok(())
    }

    /// get program ID
    pub fn get_prog_id(&self) -> SysResult<u32> {
        let id = self.prog_id.load(Ordering::Relaxed);
        if id == 0 {
            Err(SysError::EINVAL)
        } else {
            Ok(id)
        }
    }

    /// set mapping ID
    pub fn set_map_id(&self, id: u32) -> SysResult<()> {
        self.map_id.store(id, Ordering::Relaxed);
        Ok(())
    }

    /// get mapping ID
    pub fn get_map_id(&self) -> SysResult<u32> {
        let id = self.map_id.load(Ordering::Relaxed);
        if id == 0 {
            Err(SysError::EINVAL)
        } else {
            Ok(id)
        }
    }

    /// Load a BPF program
    pub fn load_program(&self, program: BpfProgram) -> SysResult<u32> {
        let inode = self.inode();
        let bpf_inode = inode
            .downcast_arc::<BpfInode>()
            .map_err(|_| SysError::EINVAL)?;

        bpf_inode.load_program(program)
    }

    /// Create a BPF map
    pub fn create_map(&self, map: BpfMap) -> SysResult<u32> {
        let inode = self.inode();
        let bpf_inode = inode
            .downcast_arc::<BpfInode>()
            .map_err(|_| SysError::EINVAL)?;

        bpf_inode.create_map(map)
    }

    /// Map lookup element
    pub fn map_lookup_elem(&self, map_fd: u32, key: &[u8]) -> SysResult<Option<Vec<u8>>> {
        let inode = self.inode();
        let bpf_inode = inode
            .downcast_arc::<BpfInode>()
            .map_err(|_| SysError::EINVAL)?;

        bpf_inode.map_lookup_elem(map_fd, key)
    }

    /// Map update element
    pub fn map_update_elem(
        &self,
        map_fd: u32,
        key: &[u8],
        value: &[u8],
        flags: u64,
    ) -> SysResult<()> {
        let inode = self.inode();
        let bpf_inode = inode
            .downcast_arc::<BpfInode>()
            .map_err(|_| SysError::EINVAL)?;

        bpf_inode.map_update_elem(map_fd, key, value, flags)
    }

    /// Map delete element
    pub fn map_delete_elem(&self, map_fd: u32, key: &[u8]) -> SysResult<()> {
        let inode = self.inode();
        let bpf_inode = inode
            .downcast_arc::<BpfInode>()
            .map_err(|_| SysError::EINVAL)?;

        bpf_inode.map_delete_elem(map_fd, key)
    }

    /// Map get next key
    pub fn map_get_next_key(&self, map_fd: u32, key: Option<&[u8]>) -> SysResult<Option<Vec<u8>>> {
        let inode = self.inode();
        let bpf_inode = inode
            .downcast_arc::<BpfInode>()
            .map_err(|_| SysError::EINVAL)?;

        bpf_inode.map_get_next_key(map_fd, key)
    }

    /// Attach program
    pub fn prog_attach(
        &self,
        prog_fd: u32,
        target_fd: i32,
        attach_type: u32,
        flags: u32,
    ) -> SysResult<u32> {
        let inode = self.inode();
        let bpf_inode = inode
            .downcast_arc::<BpfInode>()
            .map_err(|_| SysError::EINVAL)?;

        bpf_inode.prog_attach(prog_fd, target_fd, attach_type, flags)
    }

    /// Detach program
    pub fn prog_detach(&self, target_fd: i32, attach_type: u32) -> SysResult<()> {
        let inode = self.inode();
        let bpf_inode = inode
            .downcast_arc::<BpfInode>()
            .map_err(|_| SysError::EINVAL)?;

        bpf_inode.prog_detach(target_fd, attach_type)
    }

    /// Test run program
    pub fn prog_test_run(&self, prog_fd: u32, data_in: &[u8]) -> SysResult<(Vec<u8>, u32, u32)> {
        let inode = self.inode();
        let bpf_inode = inode
            .downcast_arc::<BpfInode>()
            .map_err(|_| SysError::EINVAL)?;

        bpf_inode.prog_test_run(prog_fd, data_in)
    }

    /// Get program info
    pub fn get_prog_info(&self, prog_fd: u32) -> SysResult<BpfProgInfo> {
        let inode = self.inode();
        let bpf_inode = inode
            .downcast_arc::<BpfInode>()
            .map_err(|_| SysError::EINVAL)?;

        bpf_inode.get_prog_info(prog_fd)
    }

    /// Get map info
    pub fn get_map_info(&self, map_fd: u32) -> SysResult<BpfMapInfo> {
        let inode = self.inode();
        let bpf_inode = inode
            .downcast_arc::<BpfInode>()
            .map_err(|_| SysError::EINVAL)?;

        bpf_inode.get_map_info(map_fd)
    }

    /// Get BPF statistics
    pub fn get_stats(&self) -> SysResult<BpfStats> {
        let inode = self.inode();
        let bpf_inode = inode
            .downcast_arc::<BpfInode>()
            .map_err(|_| SysError::EINVAL)?;

        Ok(bpf_inode.get_stats())
    }
}

#[async_trait]
impl File for BpfFile {
    fn meta(&self) -> &FileMeta {
        &self.meta
    }

    async fn base_read(&self, _buf: &mut [u8], _pos: usize) -> SysResult<usize> {
        // BPF files are not readable in the traditional sense
        Err(SysError::EINVAL)
    }

    async fn base_write(&self, _buf: &[u8], _offset: usize) -> SysResult<usize> {
        // BPF files are not writable in the traditional sense
        Err(SysError::EINVAL)
    }
}
