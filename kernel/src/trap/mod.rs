pub mod trap_context;
pub mod trap_env;
pub mod trap_handler;
pub mod trap_return;
pub mod trap_syscall;

#[allow(unused)]
pub use arch::riscv64::{
    interrupt::{disable_interrupt, enable_interrupt},
    time::{get_time_duration, set_nx_timer_irq},
};
pub use trap_handler::trap_handler;
pub use trap_return::trap_return;

/*
    before handling trap, should
    1. load __trap_from_kernel into stvec, in case of trap occurs within handling trap
    2. enables interrupt in case of it is a interrupt type trap
*/
pub unsafe fn load_trap_handler() {
    trap_env::set_kernel_stvec();
    enable_interrupt();
}

pub fn init() {
    unsafe {
        riscv::register::sie::set_stimer();
        riscv::register::sstatus::set_sie();
    }
}
