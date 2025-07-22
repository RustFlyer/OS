use core::time::Duration;

use riscv::register::time;

use config::board::{CLOCK_FREQ, INTERRUPTS_PER_SEC};

pub fn get_time() -> usize {
    time::read()
}

pub fn get_time_s() -> usize {
    time::read() / unsafe { CLOCK_FREQ }
}

pub fn get_time_ms() -> usize {
    time::read() / (unsafe { CLOCK_FREQ } / 1000)
}

pub fn get_time_us() -> usize {
    time::read() / (unsafe { CLOCK_FREQ } / 1_000_000)
}

pub fn get_time_duration() -> Duration {
    Duration::from_micros(get_time_us() as u64)
}

/// Set the next timer interrupt.
///
/// This function sets the next timer interrupt to occur after a specified number of
/// clock ticks.
pub fn set_nx_timer_irq() {
    let next_trigger: u64 = (time::read() + unsafe { CLOCK_FREQ } / INTERRUPTS_PER_SEC)
        .try_into()
        .unwrap();
    sbi_rt::set_timer(next_trigger);
}

/// Initialize the timer.
///
/// This function must be called once to set up the timer.
pub fn init_timer() {
    set_nx_timer_irq();
    // This function does nothing on RISC-V, as there is nothing to do before
    // calls to `set_nx_timer_irq`.
}
