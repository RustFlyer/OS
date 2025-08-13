use arch::hart::hart_start;
use config::device::MAX_HARTS;
use config::mm::HART_START_ADDR;
use driver::println;
use mm::address::{PhysPageNum, VirtAddr};

use crate::vm::trace_page_table_lookup;

/// start mutil cores
#[allow(unused)]
pub fn start_harts(hart_id: usize) {
    for i in 0..MAX_HARTS {
        if i == hart_id {
            continue;
        }
        hart_start(i, HART_START_ADDR);
    }
}

/// Clear BSS segment at start up.
pub fn clear_bss() {
    unsafe extern "C" {
        fn _skernel();
        fn _ekernel();
        fn _sbss();
        fn _kbss();
        fn _ebss();
    }
    unsafe {
        let start = _kbss as usize as *mut u64;
        let end = _ebss as usize as *mut u64;

        println!("s-kbss: {:#x} -  {:#x}", _sbss as usize, _kbss as usize);
        println!("k-ebss: {:#x} -  {:#x}", _kbss as usize, _ebss as usize);

        let len = end.offset_from(start) as usize;
        core::slice::from_raw_parts_mut(start, len).fill(0);

        // Handle any remaining bytes if the length is not a multiple of u64
        let start_byte = start as *mut u8;
        let len_bytes = _ebss as usize - _kbss as usize;

        if len_bytes % 8 != 0 {
            let offset = len * 8;
            core::slice::from_raw_parts_mut(start_byte.add(offset), len_bytes - offset).fill(0);
        }

        println!("try to read");
        let r = core::ptr::read_volatile(0x8000_0000_0000_0000 as *const usize);
        println!("r: {:#x}", r);

        println!("try to write");
        let buf = [0u8; 4096];
        // core::ptr::copy_nonoverlapping(&buf, 0x8000_0000_0000_0000 as *mut [u8; 4096], 1);
        core::ptr::write_volatile(0x8000_0000_0000_0000 as *mut [u8; 4096], buf);
        println!("finish write");

        println!("try to read");
        let r = core::ptr::read_volatile(0x8000_0000_0000_0000 as *const usize);
        println!("r: {:#x}", r);
    }
}
