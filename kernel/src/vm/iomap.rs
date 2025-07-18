use alloc::collections::btree_map::BTreeMap;

use config::mm::{KERNEL_MAP_OFFSET, PAGE_SIZE};
use driver::println;
use mm::address::VirtAddr;
use mutex::SpinNoIrqLock;
use systype::{error::SysResult, memory_flags::MappingFlags};

use super::{
    KERNEL_PAGE_TABLE,
    vm_area::{OffsetArea, VmArea},
};

/// A map of I/O memory mappings, where the key is the starting virtual address
/// of an I/O mapping area, and the value is the length of that area.
pub static IO_MAPPINGS: SpinNoIrqLock<BTreeMap<VirtAddr, usize>> =
    SpinNoIrqLock::new(BTreeMap::new());

/// Map the physical addresses of I/O memory resources to the kernel page
/// table.
pub fn ioremap(paddr: usize, length: usize) -> SysResult<()> {
    let start_va = VirtAddr::new(paddr + KERNEL_MAP_OFFSET).round_down();
    let end_va = VirtAddr::new(paddr + KERNEL_MAP_OFFSET + length);
    let prot = MappingFlags::R | MappingFlags::W;
    let area = VmArea::new_kernel(start_va, end_va, prot);

    println!("KERNEL_PAGE_TABLE map(by lock)");
    OffsetArea::map(&area, &mut KERNEL_PAGE_TABLE.lock());
    println!("KERNEL_PAGE_TABLE map(by lock) over");

    println!("IO_MAPPINGS map(by lock)");
    IO_MAPPINGS.lock().insert(start_va, length);
    println!("IO_MAPPINGS insert(by lock) over");

    log::debug!("I/O memory mapped at {:#x} with size {:#x}", paddr, length);

    Ok(())
}

/// Unmap the I/O memory mapping area specified by the starting physical
/// address from the kernel page table.
pub fn iounmap(paddr: usize) {
    let vaddr = VirtAddr::new(paddr + KERNEL_MAP_OFFSET).round_down();
    if let Some(length) = IO_MAPPINGS.lock().remove(&vaddr) {
        KERNEL_PAGE_TABLE
            .lock()
            .unmap_range(vaddr.page_number(), length / PAGE_SIZE);
    } else {
        log::warn!(
            "Attempted to unmap non-existent I/O mapping at address: {:#x}",
            paddr
        );
    }
}
