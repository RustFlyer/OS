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

/// 还有bug，不要用
pub fn backtrace() {
    let mut pc;
    let mut fp;

    log::debug!("==================== backtrace begin ====================");
    unsafe {
        core::arch::asm!("mv {}, ra", out(reg) pc);
        core::arch::asm!("mv {}, s0", out(reg) fp);

        while pc >= config::mm::VIRT_START && pc < config::mm::kernel_end() {
            log::debug!("before pc: {:#018x}", pc);
            log::debug!("before fp: {:#018x}", fp);

            log::debug!("回溯函数地址：{:#018x}", pc - size_of::<usize>());
            if fp > config::mm::VIRT_START && fp < config::mm::kernel_end() {
                fp = *(fp as *const usize).offset(-2);
                pc = *(fp as *const usize).offset(-1);

                log::debug!("after pc: {:#018x}", pc);
                log::debug!("after fp: {:#018x}", fp);
            } else {
                log::debug!("invalid fp: {:#018x}", fp);
                break;
            }
        }
    }
    log::debug!("====================  backtrace end  ====================");
}

pub fn backtrace_test() {
    pub fn a() {
        backtrace();
    }

    pub fn b() {
        a();
    }

    pub fn c() {
        b();
    }

    c();
}

/// When you want to stop in functions, call it and make breakpoints in gdb.
#[unsafe(no_mangle)]
fn stop() {
    let a = 1 + 1;
}
