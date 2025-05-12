pub mod kernel_trap_handler;
pub mod trap_context;
pub mod trap_env;
pub mod trap_handler;
pub mod trap_return;
pub mod trap_syscall;

use arch::trap::enable_interrupt;

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
