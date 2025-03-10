//! Kernel heap allocator
//! 
//! Currently, we use the buddy system allocator for the kernel heap.

use core::alloc::Layout;

use buddy_system_allocator as buddy;

use config::mm::KERNEL_HEAP_SIZE;

extern crate alloc;

static mut KERNEL_HEAP: [u8; KERNEL_HEAP_SIZE] = [0; KERNEL_HEAP_SIZE];

#[global_allocator]
static HEAP_ALLOCATOR: buddy::LockedHeap<32> = buddy::LockedHeap::empty();

#[alloc_error_handler]
fn alloc_error_handler(layout: Layout) -> ! {
    panic!("heap allocation error, layout = {:?}", layout)
}

/// Initialize heap allocator
///
/// # Safety
///
/// - This function should be called only once
/// - The caller should ensure that the heap is not used and referenced
pub unsafe fn init_heap_allocator() {
    unsafe {
        // SAFETY: we are the only one using the heap
        #[allow(static_mut_refs)]
        let start_addr = KERNEL_HEAP.as_ptr() as usize;

        HEAP_ALLOCATOR.lock().init(start_addr, KERNEL_HEAP_SIZE);

        log::info!(
            "[kernel] heap initialized: {:#x} - {:#x}",
            start_addr,
            start_addr + KERNEL_HEAP_SIZE
        );
    }
}
