//! Module defining constants related to memory management.

/// Start of physical memory
pub const RAM_START: usize = 0x8000_0000;
/// Size of physical memory
pub const RAM_SIZE: usize = 128 * 1024 * 1024;
/// End of physical memory
pub const RAM_END: usize = RAM_START + RAM_SIZE;

/// Start of kernel address space
pub const VIRT_START: usize = 0xffff_ffc0_8000_0000;
/// End of kernel address space
pub const VIRT_END: usize = VIRT_START + RAM_SIZE;
/// Offset of kernel from `RAM_START`
pub const KERNEL_RAM_OFFSET: usize = 0x20_0000;
/// Start of kernel in physical memory
pub const KERNEL_START_PHYS: usize = RAM_START + KERNEL_RAM_OFFSET;
/// End of kernel in physical memory
pub fn kernel_end_phys() -> usize {
    _ekernel as usize - KERNEL_MAP_OFFSET
}
/// Start of kernel in virtual memory
pub const KERNEL_START: usize = VIRT_START + KERNEL_RAM_OFFSET;

// Starting addresses of each section in the kernel are sure to be aligned to page size.
// Ending addresses of them are not necessarily aligned to page size.

/// Start of kernel in virtual memory. This function should be same as `KERNEL_START`.
pub fn kernel_start() -> usize {
    _skernel as usize
}
/// End of kernel in virtual memory. This value is aligned to page size.
pub fn kernel_end() -> usize {
    _ekernel as usize
}
/// Start of kernel text section in virtual memory
pub fn text_start() -> usize {
    _stext as usize
}
/// End of kernel text section in virtual memory
pub fn text_end() -> usize {
    _etext as usize
}
/// Start of kernel rodata section in virtual memory
pub fn rodata_start() -> usize {
    _srodata as usize
}
/// End of kernel rodata section in virtual memory
pub fn rodata_end() -> usize {
    _erodata as usize
}
/// Start of kernel data section in virtual memory
pub fn data_start() -> usize {
    _sdata as usize
}
/// End of kernel data section in virtual memory
pub fn data_end() -> usize {
    _edata as usize
}
/// Start of kernel bss section in virtual memory
pub fn bss_start() -> usize {
    _sbss as usize
}
/// End of kernel bss section in virtual memory
pub fn bss_end() -> usize {
    _ebss as usize
}

/// Offset of kernel in virtual memory from physical memory
pub const KERNEL_MAP_OFFSET: usize = KERNEL_START - KERNEL_START_PHYS;

/// Size of kernel stack
pub const KERNEL_STACK_SIZE: usize = 64 * 1024;
/// Size of kernel heap
pub const KERNEL_HEAP_SIZE: usize = 32 * 1024 * 1024;

/// Address width
pub const ADDRESS_WIDTH: usize = 64;
/// Page size
pub const PAGE_SIZE: usize = 4096;
/// Width of page offset
pub const PAGE_OFFSET_WIDTH: usize = 12;
/// Width of a physical address in Sv39
pub const PA_WIDTH_SV39: usize = 56;
/// Width of a virtual address in Sv39
pub const VA_WIDTH_SV39: usize = 39;
/// Width of a physical page number in Sv39
pub const PPN_WIDTH_SV39: usize = PA_WIDTH_SV39 - PAGE_OFFSET_WIDTH;
/// Width of a virtual page number in Sv39
pub const VPN_WIDTH_SV39: usize = VA_WIDTH_SV39 - PAGE_OFFSET_WIDTH;

/// Width of a page table entry in Sv39 (64-bit)
pub const PTE_WIDTH: usize = 8;
/// Number of page table entries in a page table
pub const PTE_PER_TABLE: usize = PAGE_SIZE / PTE_WIDTH;

/// Start of user space
pub const USER_START: usize = 0x0;
/// End of user space (avoid using the last page)
pub const USER_END: usize = 0x0000_003f_ffff_f000;
/// Start of program interpreter in user space
pub const USER_INTERP_BASE: usize = 0x0000_0020_0000_0000;

/// Start of mmap space in user space
pub const MMAP_START: usize = 0x0000_0010_0000_0000;
/// End of mmap space in user space
pub const MMAP_END: usize = 0x0000_0020_0000_0000;

/// Position of the stack of a user process in the virtual address space
pub const USER_STACK_UPPER: usize = 0x0000_003f_ffff_f000;
pub const USER_STACK_SIZE: usize = 8 * 1024 * 1024;
pub const USER_STACK_LOWER: usize = USER_STACK_UPPER - USER_STACK_SIZE;

/// boot hart start address
pub const HART_START_ADDR: usize = 0x80200000;

/// Start of MMIO space in physical memory
pub const MMIO_START_PHYS: usize = 0x0200_0000;
/// End of MMIO space in physical memory
pub const MMIO_END_PHYS: usize = 0x2000_0000;
/// Start of MMIO space in virtual memory
pub const MMIO_START: usize = MMIO_START_PHYS + KERNEL_MAP_OFFSET;
/// End of MMIO space in virtual memory
pub const MMIO_END: usize = MMIO_END_PHYS + KERNEL_MAP_OFFSET;

/// Detailed MMIO space ranges
pub const MMIO_PHYS_RANGES: &[(usize, usize)] = &[
    (0x0200_0000, 0x10000),  // CLINT
    (0x0c00_0000, 0x400000), // PLIC
    (0x1000_0000, 0x1000),   // UART
    (0x1000_1000, 0x1000),   // VIRTIO
];

/// Address of the device tree blob
pub static mut DTB_ADDR: usize = 0;
/// Maximum size of the device tree blob
pub const MAX_DTB_SIZE: usize = 0x100_0000;
/// Starting virtual address of the device tree blob
pub const DTB_START: usize = DTB_END - MAX_DTB_SIZE;
/// Ending virtual address of the device tree blob
pub const DTB_END: usize = 0xffff_ffff_f000_0000;

/* Symbols defined in the linker script */
unsafe extern "C" {
    fn _skernel();
    fn _ekernel();
    fn _stext();
    fn _etext();
    fn _srodata();
    fn _erodata();
    fn _sdata();
    fn _edata();
    fn _sbss();
    fn _ebss();
}
