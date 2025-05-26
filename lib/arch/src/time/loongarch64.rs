use core::time::Duration;

use loongArch64::{
    register::{
        ecfg::{self, LineBasedInterrupt},
        tcfg,
    },
    time::{Time, get_timer_freq},
};
use spin::Lazy;

use config::board::INTERRUPTS_PER_SEC;

// On LoongArch, we can get the timer frequency via the CPUCFG instruction.
static FREQ: Lazy<usize> = Lazy::new(get_timer_freq);

pub fn get_time() -> usize {
    Time::read()
}

pub fn get_time_ms() -> usize {
    Time::read() / (*FREQ / 1000)
}

pub fn get_time_us() -> usize {
    Time::read() / (*FREQ / 1_000_000)
}

pub fn get_time_duration() -> Duration {
    Duration::from_micros(get_time_us() as u64)
}

/// Set the next timer interrupt.
///
/// This function sets the next timer interrupt to occur after a specified number of
/// clock ticks.
pub fn set_nx_timer_irq() {
    // This function does nothing on LoongArch, as timer interrupts can be set to be
    // raised periodically.
}

/// Initialize the timer.
///
/// This function must be called exactly once to set up the timer.
pub fn init_timer() {
    let ticks = *FREQ / INTERRUPTS_PER_SEC;
    tcfg::set_init_val(ticks);
    tcfg::set_periodic(true);
    tcfg::set_en(true);

    let intr = LineBasedInterrupt::TIMER
        | LineBasedInterrupt::SWI0
        | LineBasedInterrupt::SWI1
        | LineBasedInterrupt::HWI0;
    ecfg::set_lie(intr);
}
