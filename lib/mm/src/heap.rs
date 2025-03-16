//! Module for kernel heap allocator
//!
//! Currently, we use the buddy system allocator for the kernel heap.

use core::alloc::Layout;

use buddy_system_allocator as buddy;

use config::mm::KERNEL_HEAP_SIZE;

use crate::address::PhysAddr;

static mut KERNEL_HEAP: [u8; KERNEL_HEAP_SIZE] = [0; KERNEL_HEAP_SIZE];

#[global_allocator]
static HEAP_ALLOCATOR: buddy::LockedHeap<32> = buddy::LockedHeap::empty();

#[alloc_error_handler]
fn alloc_error_handler(layout: Layout) -> ! {
    panic!("heap allocation error, layout = {:?}", layout)
}

/// Initializes heap allocator.
///
/// # Safety
///
/// - This function should be called only once
/// - The caller should ensure that the heap is not used and referenced
pub unsafe fn init_heap_allocator() {
    unsafe {
        // SAFETY: we are the only one using the heap
        #[allow(static_mut_refs)]
        let start_addr = PhysAddr::new(KERNEL_HEAP.as_ptr() as usize).to_usize();

        HEAP_ALLOCATOR.lock().init(start_addr, KERNEL_HEAP_SIZE);

        log::info!(
            "heap memory: {:#x} - {:#x}",
            start_addr,
            start_addr + KERNEL_HEAP_SIZE
        );
    }
}

pub fn heap_test() {
    use alloc::vec::Vec;
    log::info!("heap test: start");
    let mut vec = Vec::new();
    for i in 0..100 {
        vec.push(i);
    }
    let vec_start = vec.as_ptr() as usize;
    let vec_end = &vec[99] as *const _ as usize;
    log::info!("heap test: vec from {:#x} - {:#x}", vec_start, vec_end);
    log::info!("heap test: end");
}
