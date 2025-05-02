use alloc::sync::Arc;
use mm::address::VirtAddr;
use systype::SysResult;

use crate::{task::Task, vm::user_ptr::SumGuard};

pub struct FutexAddr {
    pub addr: VirtAddr,
    _guard: SumGuard,
}

impl FutexAddr {
    pub fn raw(&self) -> usize {
        self.addr.into()
    }
    pub fn check(&self, task: &Arc<Task>) -> SysResult<()> {
        task.just_ensure_user_area(self.addr, size_of::<VirtAddr>(), PageFaultAccessType::RO)
    }
    pub fn read(&self) -> u32 {
        unsafe { atomic_load_acquire(self.addr.0 as *const u32) }
    }
}

impl From<usize> for FutexAddr {
    fn from(a: usize) -> Self {
        Self {
            addr: a.into(),
            _guard: SumGuard::new(),
        }
    }
}