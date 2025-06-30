#![no_std]
#![no_main]
#![allow(unknown_lints)]

/// Conditional compilation macro for debug code
///
/// # Examples
/// ```
/// when_debug!({
///     println!("Debug info: {}", value);
///     stop_at("checkpoint_1");
/// });
/// ```
#[macro_export]
macro_rules! when_debug {
    ($blk:expr) => {
        cfg_if::cfg_if! {
            if #[cfg(feature = "debug")] {
                $blk
            }
        }
    };
}

/// Enhanced macro for conditional debug with custom features
#[macro_export]
macro_rules! when_feature {
    ($feature:literal, $blk:expr) => {
        cfg_if::cfg_if! {
            if #[cfg(feature = $feature)] {
                $blk
            }
        }
    };
}

/// Debug breakpoint with context information
///
/// This function serves as a breakpoint target for GDB. The compiler
/// won't optimize it away due to the `#[inline(never)]` attribute.
#[inline(never)]
#[unsafe(no_mangle)]
pub fn debug_break() {
    // Use a volatile operation to prevent optimization
    unsafe {
        core::ptr::write_volatile(&mut 0u32 as *mut u32, 42);
    }
}

/// Named breakpoint for easier debugging
///
/// # Arguments
/// * `name` - A string identifier for this breakpoint
#[inline(never)]
#[allow(static_mut_refs)]
#[unsafe(no_mangle)]
pub fn stop_at(name: &'static str) {
    // Store the name in a static to make it visible in debugger
    static mut CURRENT_STOP: &'static str = "";
    unsafe {
        core::ptr::write_volatile(&mut CURRENT_STOP, name);
    }
    debug_break();
}

/// Numbered breakpoints for quick debugging
#[inline(never)]
#[unsafe(no_mangle)]
pub fn stop_0() {
    debug_break();
}

#[inline(never)]
#[unsafe(no_mangle)]
pub fn stop_1() {
    debug_break();
}

#[inline(never)]
#[unsafe(no_mangle)]
pub fn stop_2() {
    debug_break();
}

#[inline(never)]
#[unsafe(no_mangle)]
pub fn stop_3() {
    debug_break();
}

#[inline(never)]
#[unsafe(no_mangle)]
pub fn stop_4() {
    debug_break();
}

/// Debug assertion that only works in debug builds
#[macro_export]
macro_rules! debug_assert_eq_stop {
    ($left:expr, $right:expr) => {
        when_debug!({
            if $left != $right {
                stop_at("assertion_failed");
            }
        });
    };
}

/// Print and stop - useful for tracing execution flow
#[macro_export]
macro_rules! trace_stop {
    ($msg:expr) => {
        when_debug!({
            // Assuming you have some print mechanism
            // print!("TRACE: {}", $msg);
            stop_at("trace_point");
        });
    };
}

/// Memory debug helper - check if address is valid
#[inline(never)]
#[unsafe(no_mangle)]
pub fn check_addr(addr: usize, size: usize) -> bool {
    // This is a placeholder - implement actual memory validation
    // based on your kernel's memory management
    addr != 0 && size > 0
}

/// Conditional breakpoint based on value
#[inline(never)]
#[unsafe(no_mangle)]
pub fn stop_if_eq(value: usize, target: usize) {
    if value == target {
        debug_break();
    }
}

// / Panic hook for debug builds
// #[cfg(feature = "debug")]
// #[panic_handler]
// fn panic_handler(_info: &core::panic::PanicInfo) -> ! {
//     stop_at("panic");
//     loop {}
// }
