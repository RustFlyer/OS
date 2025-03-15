pub mod trap_env;
pub mod kernel_trap_handler;
pub mod sum;
pub mod trap_context;
pub mod trap_handler;
pub mod trap_return;

pub use trap_handler::trap_handler;
pub use trap_return::trap_return;
pub use arch::riscv64::{
    interrupt::{disable_interrupt, enable_interrupt},
    time::{get_time_duration, set_nx_timer_irq},
};

pub unsafe fn load_trap_handler() {
    unsafe {
        trap_env::set_kernel_stvec();
        enable_interrupt();
    }
}