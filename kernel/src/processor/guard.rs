use crate::processor::hart::current_hart;
use pps::ProcessorPrivilegeState;

pub struct Guard;

impl Guard {
    pub fn new() {
        current_hart().get_mut_pps().inc_sum_cnt();
    }
}

impl Drop for Guard {
    fn drop(&mut self) {
        current_hart().get_mut_pps().dec_sum_cnt();
    }
}

pub fn guard_function<F>(f: impl FnOnce() -> F) -> F {
    let guard = Guard::new();
    let ret = f();
    ret
}
