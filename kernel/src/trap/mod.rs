pub mod trap_env;
pub mod kernel_trap_handler;
pub mod trap_context;
pub mod trap_handler;
pub mod trap_return;

pub unsafe fn set_trap_handler() {
    unsafe {
        trap_env::set_stvec();
        enable_interrupt();
    }
}
