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
