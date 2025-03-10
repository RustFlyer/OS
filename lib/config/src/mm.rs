//! Memory layout

/// Start of physical memory
pub const RAM_START: usize = 0x8000_0000;
/// Size of physical memory
pub const RAM_SIZE: usize = 128 * 1024 * 1024;

/// Start of kernel address space
pub const VIRT_START: usize = 0xffff_ffc0_8000_0000;
/// Offset of kernel from `RAM_START`
pub const KERNEL_OFFSET: usize = 0x20_0000;
/// Start of kernel in physical memory
pub const KERNEL_START_PHYS: usize = RAM_START + KERNEL_OFFSET;
/// Start of kernel in virtual memory
pub const KERNEL_START: usize = VIRT_START + KERNEL_OFFSET;

/// Offset of kernel in virtual memory from physical memory
pub const KERNEL_VM_OFFSET: usize = KERNEL_START - KERNEL_START_PHYS;

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

pub const APP_BASE_ADDRESS: usize = 0x1000_0000;
pub const APP_SIZE_LIMIT: usize = 1024 * 1024 * 1024;

unsafe extern "C" {
    pub fn _ekernel();
}
