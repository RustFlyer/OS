use core::{
    arch::asm,
    ffi::{VaList, c_char, c_int},
    hint::spin_loop,
};

use alloc::string::{String, ToString};
use config::mm::{KERNEL_MAP_OFFSET, PAGE_SIZE};
use mm::heap::allocate_align_memory;

use crate::print;

/*
// for C ffi test
unsafe extern "C" {
    pub fn ahci_mdelay(ms: u32);
    pub fn ahci_printf(fmt: *const u8, _: ...) -> i32;
    pub fn ahci_malloc_align(size: u64, align: u32) -> u64;
    pub fn ahci_sync_dcache();
    pub fn ahci_phys_to_uncached(va: u64) -> u64;
    pub fn ahci_virt_to_phys(va: u64) -> u64;
}
*/

// 这里是测试时用于调用C的printf
// 替换成OS实现的printf
#[allow(unsafe_op_in_unsafe_fn)]
pub unsafe extern "C" fn ahci_printf(fmt: *const u8, mut args: ...) -> i32 {
    if fmt.is_null() {
        return -1;
    }

    let fmt_str = match core::ffi::CStr::from_ptr(fmt as *const c_char).to_str() {
        Ok(s) => s,
        Err(_) => return -1,
    };

    let result = parse_and_print(fmt_str, args.as_va_list());

    match result {
        Ok(count) => count as c_int,
        Err(_) => -1,
    }
}

// 等待数毫秒
pub fn ahci_mdelay(ms: u32) {
    let cnt = ms * 10000;
    for i in 0..cnt {
        spin_loop();
    }
    print!("wait...");
}

// 同步dcache中所有cached和uncached访存请求
pub fn ahci_sync_dcache() {
    unsafe {
        asm!("dbar 0");
    }
}

// 分配按align字节对齐的内存
pub fn ahci_malloc_align(size: u64, align: u32) -> u64 {
    log::error!("malloc align mem");
    // mm::frame::FrameTracker::build()
    //     .unwrap()
    //     .as_mut_slice()
    //     .as_mut_ptr() as u64

    allocate_align_memory(size as usize, align as usize) as u64
}

// 物理地址转换为uncached虚拟地址
pub fn ahci_phys_to_uncached(pa: u64) -> u64 {
    pa + KERNEL_MAP_OFFSET as u64
}

// cached虚拟地址转换为物理地址
// ahci dma可以接受64位的物理地址
pub fn ahci_virt_to_phys(va: u64) -> u64 {
    va - KERNEL_MAP_OFFSET as u64
}

#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn parse_and_print(fmt: &str, mut args: VaList) -> Result<usize, ()> {
    let mut output = String::new();
    let mut chars = fmt.chars();
    let mut printed_chars = 0;

    while let Some(ch) = chars.next() {
        if ch == '%' {
            if let Some(spec) = chars.next() {
                match spec {
                    '%' => {
                        output.push('%');
                        printed_chars += 1;
                    }
                    'd' | 'i' => {
                        let val: c_int = args.arg();
                        use core::fmt::Write;
                        let _ = write!(&mut output, "{}", val);
                        printed_chars += format_args!("{}", val).to_string().len();
                    }
                    'u' => {
                        let val: u32 = args.arg();
                        use core::fmt::Write;
                        let _ = write!(&mut output, "{}", val);
                        printed_chars += format_args!("{}", val).to_string().len();
                    }
                    'x' => {
                        let val: u32 = args.arg();
                        use core::fmt::Write;
                        let _ = write!(&mut output, "{:x}", val);
                        printed_chars += format_args!("{:x}", val).to_string().len();
                    }
                    'X' => {
                        let val: u32 = args.arg();
                        use core::fmt::Write;
                        let _ = write!(&mut output, "{:X}", val);
                        printed_chars += format_args!("{:X}", val).to_string().len();
                    }
                    's' => {
                        let ptr: *const c_char = args.arg();
                        if !ptr.is_null() {
                            let c_str = core::ffi::CStr::from_ptr(ptr);
                            if let Ok(rust_str) = c_str.to_str() {
                                output.push_str(rust_str);
                                printed_chars += rust_str.len();
                            }
                        }
                    }
                    'c' => {
                        let val: c_int = args.arg();
                        if let Some(ch) = char::from_u32(val as u32) {
                            output.push(ch);
                            printed_chars += 1;
                        }
                    }
                    _ => {
                        // unsupport
                        output.push('%');
                        output.push(spec);
                        printed_chars += 2;
                    }
                }
            }
        } else {
            output.push(ch);
            printed_chars += 1;
        }
    }

    arch::console::console_print(format_args!("{}", output));

    Ok(printed_chars)
}
