use crate::{task::Task, vm};
use config::device::MAX_HARTS;

extern crate alloc;
use alloc::sync::Arc;

use pps::ProcessorPrivilegeState;

use core::arch::asm;
#[cfg(target_arch = "riscv64")]
use riscv::register::sstatus;
#[cfg(target_arch = "riscv64")]
use riscv::register::sstatus::FS;

use arch::trap::{disable_interrupt, enable_interrupt};

const HART_ONE: Hart = Hart::new(0);

/// `HARTS` is a global variable vector which controls all hart objects.
///
/// # Use
/// A hart is used in UserFuture/KernelFuture poll and switches the CPU
/// environment.
pub static mut HARTS: [Hart; MAX_HARTS] = [HART_ONE; MAX_HARTS];

/// `Hart` is used to manage the state of each CPU.
///
/// # Args
/// - `id` distinguishes different cpu.
/// - `task` is a option.
///   - when hart is running user task, `task` will point to it.
///   - when hart is running kernel task, `task` will be set as None.
/// - `pps` is a specified environment with processor privilege state.
pub struct Hart {
    pub id: usize,
    task: Option<Arc<Task>>,
    pps: ProcessorPrivilegeState,
}

impl Hart {
    pub const fn new(id: usize) -> Self {
        Self {
            id,
            task: None,
            pps: ProcessorPrivilegeState::new(),
        }
    }

    pub fn set_hart_id(&mut self, hart_id: usize) {
        self.id = hart_id;
    }

    pub fn set_task(&mut self, task: Arc<Task>) {
        self.task = Some(task);
    }

    pub fn get_task(&self) -> Arc<Task> {
        self.task.clone().unwrap()
    }

    pub fn is_task_exist(&self) -> bool {
        self.task.is_some()
    }

    pub fn clear_task(&mut self) {
        self.task = None;
    }

    pub fn set_pps(&mut self, pps: &ProcessorPrivilegeState) {
        self.pps = *pps;
    }

    pub fn get_pps(&self) -> &ProcessorPrivilegeState {
        &self.pps
    }

    pub fn get_mut_pps(&mut self) -> &mut ProcessorPrivilegeState {
        &mut self.pps
    }

    pub async fn user_switch_in(
        &mut self,
        new_task: &mut Arc<Task>,
        pps: &mut ProcessorPrivilegeState,
    ) {
        log::debug!(
            "[hart] switch to task {} tid {}",
            new_task.get_name(),
            new_task.tid()
        );
        disable_interrupt();
        pps.auto_sum(); // `pps` is the user task's PPS which is to be enabled.
        core::mem::swap(self.get_mut_pps(), pps);

        new_task.switch_addr_space();
        new_task.timer_mut().record_switch_in();
        self.set_task(Arc::clone(new_task));
        enable_interrupt();
    }

    pub fn user_switch_out(&mut self, pps: &mut ProcessorPrivilegeState) {
        disable_interrupt();
        pps.auto_sum(); // `pps` is the hart's original PPS which is to be enabled.
        core::mem::swap(self.get_mut_pps(), pps);
        // let _task = self.get_task();
        unsafe {
            vm::switch_to_kernel_page_table();
        }
        self.clear_task();
        enable_interrupt();
    }

    pub fn kernel_switch_in(&mut self, pps: &mut ProcessorPrivilegeState) {
        disable_interrupt();
        pps.auto_sum();
        core::mem::swap(self.get_mut_pps(), pps);
        unsafe {
            vm::switch_to_kernel_page_table();
        }
        enable_interrupt();
    }

    pub fn kernel_switch_out(&mut self, pps: &mut ProcessorPrivilegeState) {
        disable_interrupt();
        pps.auto_sum();
        core::mem::swap(self.get_mut_pps(), pps);
        enable_interrupt();
    }

    pub fn preempt_switch_in(&mut self) {
        todo!()
    }

    pub fn preempt_switch_out(&mut self) {
        todo!()
    }
}

pub fn get_hart(hart_id: usize) -> &'static mut Hart {
    unsafe { &mut HARTS[hart_id] }
}

pub fn current_hart() -> &'static mut Hart {
    let ret;
    unsafe {
        let tp: usize;
        #[cfg(target_arch = "riscv64")]
        asm!("mv {}, tp", out(reg) tp);
        #[cfg(target_arch = "loongarch64")]
        asm!("move {}, $tp", out(reg) tp);
        ret = &mut *(tp as *mut Hart);
    }

    ret
}

pub fn set_current_hart(id: usize) {
    let hart = get_hart(id);
    hart.set_hart_id(id);
    let hart_addr = hart as *const _ as usize;
    unsafe {
        #[cfg(target_arch = "riscv64")]
        asm!("mv tp, {}", in(reg) hart_addr);
        #[cfg(target_arch = "loongarch64")]
        asm!("move $tp, {}", in(reg) hart_addr);
    }
}

pub fn get_current_hart() -> &'static mut Hart {
    let hart_ptr: *mut Hart;
    unsafe {
        #[cfg(target_arch = "riscv64")]
        asm!("mv {}, tp", out(reg) hart_ptr);
        #[cfg(target_arch = "loongarch64")]
        asm!("move {}, $tp", out(reg) hart_ptr);
        &mut *hart_ptr
    }
}

pub fn init(id: usize) {
    set_current_hart(id);
    #[cfg(target_arch = "riscv64")]
    unsafe {
        sstatus::set_fs(FS::Initial);
    }
}

pub fn current_task() -> Arc<Task> {
    current_hart().get_task().clone()
}

/// temp for test without driver
pub fn one_hart() -> &'static mut Hart {
    unsafe { &mut HARTS[0] }
}
