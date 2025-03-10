use config::board::CLOCK_FREQ;
use riscv::register::time;

pub fn get_time() -> usize {
    time::read()
}

pub fn get_time_ms() -> usize {
    time::read() / (CLOCK_FREQ / 1000)
}

pub fn get_time_us() -> usize {
    time::read() / (CLOCK_FREQ / 1_000_000)
}
