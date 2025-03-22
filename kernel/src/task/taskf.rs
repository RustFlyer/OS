extern crate alloc;
use alloc::sync::Arc;

use super::future::{self};
use super::manager::TASK_MANAGER;
use super::task::*;

use crate::vm::addr_space::AddrSpace;
use crate::vm::addr_space::switch_to;
use arch::riscv64::time::get_time_duration;
use core::time::Duration;
use timer::{TIMER_MANAGER, Timer};

impl Task {
    /// Switchse Task to User
    pub fn enter_user_mode(&mut self) {
        self.timer_mut().switch_to_user();
    }

    /// Switches Task to Kernel
    pub fn enter_kernel_mode(&mut self) {
        self.timer_mut().switch_to_kernel();
    }

    /// Suspends the Task until it is waken or time out
    pub async fn suspend_timeout(&self, limit: Duration) -> Duration {
        let expire = get_time_duration() + limit;
        let mut timer = Timer::new(expire);
        timer.set_callback(self.get_waker().clone());
        TIMER_MANAGER.add_timer(timer);
        future::suspend_now().await;
        let now = get_time_duration();
        if expire > now {
            expire - now
        } else {
            Duration::ZERO
        }
    }

    /// Spawns app from elf
    pub fn spawn_from_elf(elf_data: &'static [u8]) {
        let mut addrspace = AddrSpace::build_user().unwrap();
        let entry_point = addrspace.load_elf(elf_data).unwrap();
        let stack = addrspace.map_stack().unwrap();
        let task = Arc::new(Task::new(
            entry_point.to_usize(),
            stack.to_usize(),
            addrspace,
        ));

        TASK_MANAGER.add_task(&task);
        future::spawn_user_task(task);
    }

    /// Switches to the address space of the task.
    ///
    /// # Safety
    /// This function must be called before the current page table is dropped, or the kernel
    /// may lose its memory mappings.
    pub fn switch_addr_space(&self) {
        unsafe {
            switch_to(&self.addr_space_mut().lock());
        }
    }

    pub fn exit(&self) {}

    pub fn clear(&self) {}
}
