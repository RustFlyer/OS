#![no_std]
#![no_main]

#[macro_export]
macro_rules! when_debug {
    ($blk:expr) => {
        cfg_if::cfg_if! {
            if #[cfg(ondebug)] {
                $blk
            }
        }
    };
}

/// When you want to stop in functions, call it and make breakpoints in gdb.
#[unsafe(no_mangle)]
pub fn stop() {
    let _a = 1 + 1;
}

#[unsafe(no_mangle)]
pub fn stop1() {
    let _a = 1 + 1;
}

#[unsafe(no_mangle)]
pub fn stop2() {
    let _a = 1 + 1;
}
