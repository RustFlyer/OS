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

/// Size of kernel stack
pub const KERNEL_STACK_SIZE: usize = 64 * 1024;
/// Size of kernel heap
pub const KERNEL_HEAP_SIZE: usize = 32 * 1024 * 1024;

pub const APP_BASE_ADDRESS: usize = 0x1000_0000;
pub const APP_SIZE_LIMIT: usize = 1024 * 1024 * 1024;
