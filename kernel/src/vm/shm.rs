use alloc::{
    sync::{Arc, Weak},
    vec::Vec,
};
use mm::{address::VirtAddr, page_cache::page::Page};
use systype::{SysError, SysResult};

use super::{addr_space::AddrSpace, mem_perm::MemPerm};

impl AddrSpace {
    /// Attach given `pages` to the AddrSpace. If pages is not given, it will
    /// create pages according to the `size` and map them to the AddrSpace.
    /// if `shmaddr` is set to `0`, it will chooses a suitable page-aligned
    /// address to attach.
    ///
    /// `size` and `shmaddr` need to be page-aligned.
    pub fn attach_shm(
        self: Arc<Self>,
        size: usize,
        shmaddr: VirtAddr,
        map_perm: MemPerm,
        pages: &mut Vec<Weak<Page>>,
    ) -> SysResult<VirtAddr> {
        return Err(SysError::EBUSY);
    }

    /// `shmaddr` must be the return value of shmget (i.e. `shmaddr` is page
    /// aligned and in the beginning of the vm_area with type Shm). The
    /// check should be done at the caller who call `detach_shm`.
    pub fn detach_shm(self: Arc<Self>, shmaddr: VirtAddr) {}
}
