use crate::println;
use config::device::MAX_HARTS;
use config::mm::HART_START_ADDR;
use driver::sbi;

pub fn start_harts(hart_id: usize) {
    for i in 0..MAX_HARTS {
        if i == hart_id {
            continue;
        }
        let status: isize = sbi::hart_start(i, HART_START_ADDR) as _;
        // println!("[kernel] start to wake up hart {i}... status {status}");
    }
}
