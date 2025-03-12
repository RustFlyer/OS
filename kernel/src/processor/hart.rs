use crate::task::Task;
use config::device::MAX_HARTS;

extern crate alloc;
use alloc::sync::Arc;
use alloc::vec::Vec;

use pps::ProcessorPrivilegeState;

use lazy_static::lazy_static;

use core::arch::asm;
use riscv::register::sstatus;
use riscv::register::sstatus::FS;

use arch::riscv64::interrupt::{disable_interrupt, enable_interrupt};

lazy_static! {
    pub static ref HARTS: Vec<Arc<HART>> = (0..MAX_HARTS).map(|i| Arc::new(HART::new(i))).collect();
}

pub struct HART {
    pub id: usize,
    task: Option<Arc<Task>>,
    pps: ProcessorPrivilegeState,
}

impl HART {
    pub fn new(id: usize) -> Self {
        Self {
            id,
            task: None,
            pps: ProcessorPrivilegeState::new(),
        }
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
        self.pps = pps.clone();
    }

    pub fn get_pps(&self) -> &ProcessorPrivilegeState {
        &self.pps
    }

    pub fn get_mut_pps(&mut self) -> &mut ProcessorPrivilegeState {
        &mut self.pps
    }

    pub fn user_switch_in(&mut self, new_task: Arc<Task>, pps: &mut ProcessorPrivilegeState) {
        todo!();

        disable_interrupt();
        pps.auto_sum();

        core::mem::swap(self.get_mut_pps(), pps);

        self.set_task(new_task);
        // switch pagetable...
        enable_interrupt();
    }

    pub fn user_switch_out(&mut self, pps: &mut ProcessorPrivilegeState) {
        todo!();

        disable_interrupt();
        pps.auto_sum();

        core::mem::swap(self.get_mut_pps(), pps);

        let task = self.get_task();
        // set old task record out
        self.clear_task();

        // switch pagetable...
        enable_interrupt();
    }

    pub fn kernel_switch_in(&mut self, pps: &mut ProcessorPrivilegeState) {
        disable_interrupt();
        pps.auto_sum();
        core::mem::swap(self.get_mut_pps(), pps);
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

pub fn get_hart_by_id(id: usize) -> Arc<HART> {
    HARTS.get(id).unwrap().clone()
}

pub fn current_hart() -> &'static mut HART {
    let hart_addr: usize;
    unsafe {
        asm!("mv {}, tp", out(reg) hart_addr);
        &mut *(hart_addr as *mut HART)
    }
}

pub fn set_current_hart(id: usize) {
    let hart = get_hart_by_id(id);
    let hart_addr = Arc::as_ptr(&hart) as usize;
    unsafe {
        asm!("mv tp, {}", in(reg) hart_addr);
    }
}

pub fn init_hart() {
    unsafe {
        // asm!("csrrw tp, sstatus, zero");
        sstatus::set_fs(FS::Initial);
    }
}

pub fn current_task() -> Arc<Task> {
    current_hart().get_task().clone()
}
