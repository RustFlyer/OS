use config::board::{CLOCK_FREQ, INTERRUPTS_PER_SEC};
use core::time::Duration;
use riscv::register::time;

pub fn get_time() -> usize {
    time::read()
}

pub fn get_time_s() -> usize {
    time::read() / CLOCK_FREQ
}

pub fn get_time_ms() -> usize {
    time::read() / (CLOCK_FREQ / 1000)
}

pub fn get_time_us() -> usize {
    time::read() / (CLOCK_FREQ / 1_000_000)
}

pub fn get_time_duration() -> Duration {
    Duration::from_micros(get_time_us() as u64)
}

pub fn set_nx_timer_irq() {
    let next_trigger: u64 = (time::read() + CLOCK_FREQ / INTERRUPTS_PER_SEC)
        .try_into()
        .unwrap();
    sbi_rt::set_timer(next_trigger);
}

pub unsafe fn set_timer_irq(times: usize) {
    let next_trigger: u64 = (time::read() + times * CLOCK_FREQ / INTERRUPTS_PER_SEC)
        .try_into()
        .unwrap();
    sbi_rt::set_timer(next_trigger);
}
