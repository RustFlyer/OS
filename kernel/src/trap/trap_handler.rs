use alloc::sync::Arc;

use riscv::register::{
    sepc,
    stval,
    scause::{self, Exception, Interrupt, Trap},
    sstatus::FS,
};

pub async fn trap_handler(task: &Arc<Task>) -> bool {
    unsafe { set_sepc() };

    let cx = task.trap_context_mut();
    let stval = stval::read();
    let scause = scause::read();
    let sepc = sepc::read();
    let cause = scause.cause();
    
}