use core::ops::ControlFlow;

use alloc::sync::Arc;
use mm::address::VirtAddr;
use systype::{SysError, SysResult};

use crate::{processor::current_task, task::Task, vm::user_ptr::{PageFaultAccessType, SumGuard}};

pub struct FutexAddr {
    pub addr: VirtAddr,
    _guard: SumGuard,
}

impl FutexAddr {
    pub fn raw(&self) -> usize {
        self.addr.into()
    }
    pub fn check(&self) -> SysResult<()> {
        current_task().just_ensure_user_area(self.addr, size_of::<VirtAddr>(), PageFaultAccessType::RO)
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

impl Task {
    fn just_ensure_user_area(
        &self,
        begin: VirtAddr,
        len: usize,
        access: PageFaultAccessType,
    ) -> SysResult<()> {
        self.ensure_user_area(begin, len, access, |_, _| ControlFlow::Continue(()))
    }

    /// Ensure that the whole range is accessible, or return an error.
    fn ensure_user_area(
        &self,
        begin: VirtAddr,
        len: usize,
        access: PageFaultAccessType,
        mut f: impl FnMut(VirtAddr, usize) -> ControlFlow<Option<SysError>>,
    ) -> SysResult<()> {
        if len == 0 {
            return Ok(());
        }

        unsafe { set_kernel_user_rw_trap() };

        let test_fn = match access {
            PageFaultAccessType::RO => will_read_fail,
            PageFaultAccessType::RW => will_write_fail,
            _ => panic!("invalid access type"),
        };

        let mut curr_vaddr = begin;
        let mut readable_len = 0;
        while readable_len < len {
            if test_fn(curr_vaddr.0) {
                self.with_mut_memory_space(|m| m.handle_page_fault(curr_vaddr, access))?
            }

            let next_page_beg: VirtAddr = VirtAddr::from(curr_vaddr.floor().next());
            let len = next_page_beg - curr_vaddr;

            match f(curr_vaddr, len) {
                ControlFlow::Continue(_) => {}
                ControlFlow::Break(None) => {
                    unsafe { set_kernel_trap() };
                    return Ok(());
                }
                ControlFlow::Break(Some(e)) => {
                    unsafe { set_kernel_trap() };
                    return Err(e);
                }
            }

            readable_len += len;
            curr_vaddr = next_page_beg;
        }

        unsafe { set_kernel_trap() };
        Ok(())
    }
}