pub mod csr_env;
pub mod kernel_trap_handler;
pub mod sum;
pub mod trap_context;
pub mod trap_handler;
pub mod trap_return;

pub use trap_handler::trap_handler;
pub use trap_return::trap_return;

pub fn init() {
    csr_env::set_kernel_trap();
}
