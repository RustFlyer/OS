#![no_std]
#![no_main]

extern crate alloc;

pub mod console;

pub use console::*;
use crate_interface::call_interface;
use log::{Level, LevelFilter};

struct SimpleLogger;

/// 日志记录器
/// 实现 log::Log 接口
/// 作用是提供一个日志记录器，可以记录日志
/// call_interface! 宏：动态查找已注册的接口实现
impl log::Log for SimpleLogger {
    fn enabled(&self, _metadata: &log::Metadata) -> bool {
        true
    }
    fn log(&self, record: &log::Record) {
        if !self.enabled(record.metadata()) {
            return;
        }
        call_interface!(LogInterface::print_log(record));
    }
    fn flush(&self) {}
}

/// 日志接口
/// 作用是提供一个接口，让日志记录器可以调用
/// Send + Sync 表示可以跨线程调用
/// crate_interface 是一个用于实现跨 crate 接口抽象的 Rust 库（常见于操作系统开发场景）
/// ，它通过编译时的接口绑定机制，帮助实现模块解耦和接口动态分发。
/// def_interface 宏：定义了一个可被其他模块实现的接口
/// 其他模块可以通过 impl_interface 实现该接口
#[crate_interface::def_interface]
pub trait LogInterface: Send + Sync {
    fn print_log(record: &log::Record);
}

/// 初始化日志记录器
///
/// 设置日志记录器和日志级别
pub fn init() {
    static LOGGER: SimpleLogger = SimpleLogger;
    log::set_logger(&LOGGER).ok();
    log::set_max_level(match option_env!("LOG") {
        Some("trace") => LevelFilter::Trace,
        Some("debug") => LevelFilter::Debug,
        Some("info") => LevelFilter::Info,
        Some("warn") => LevelFilter::Warn,
        Some("error") => LevelFilter::Error,
        _ => LevelFilter::Off,
    });
}

pub fn level2color(level: Level) -> u8 {
    match level {
        Level::Error => 31, // Red
        Level::Warn => 93,  // BrightYellow
        Level::Info => 36,  // Blue
        Level::Debug => 32, // Green
        Level::Trace => 90, // BrightBlack
    }
}

#[macro_export]
macro_rules! lprint {
    ($fmt: literal $(, $($arg: tt)+)?) => {
        $crate::console_print(format_args!($fmt $(, $($arg)+)?))
    }
}

#[macro_export]
macro_rules! lprintln {
    ($fmt: literal $(, $($arg: tt)+)?) => {
        $crate::console_print(format_args!(concat!($fmt, "\n") $(, $($arg)+)?))
    }
}
