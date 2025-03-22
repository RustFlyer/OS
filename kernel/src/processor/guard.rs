use crate::processor::hart::current_hart;

pub struct Guard;

/// 保护块结构体
///
/// 保护块，用于保护代码块，防止其他HART访问
impl Guard {
    pub fn new() {
        current_hart().get_mut_pps().inc_sum_cnt();
    }
}

/// 保护块Drop特性
///
/// 释放保护块，减少保护块计数
impl Drop for Guard {
    fn drop(&mut self) {
        current_hart().get_mut_pps().dec_sum_cnt();
    }
}

/// 保护函数
///
/// 保护函数，用于保护代码块，防止其他HART访问
pub fn guard_function<F>(f: impl FnOnce() -> F) -> F {
    let _guard = Guard::new();
    let ret = f();
    ret
}
