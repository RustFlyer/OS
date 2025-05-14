//! Module for kernel heap allocator
//!
//! Currently, we use the buddy system allocator for the kernel heap.

use core::{
    alloc::{GlobalAlloc, Layout},
    ptr::NonNull,
};

use buddy_system_allocator as buddy;

use config::mm::KERNEL_HEAP_SIZE;
use mutex::SpinNoIrqLock;

use crate::address::PhysAddr;

struct NoIrqLockedHeap<const ORDER: usize>(SpinNoIrqLock<buddy::Heap<ORDER>>);

impl<const ORDER: usize> NoIrqLockedHeap<ORDER> {
    /// Creates a new heap allocator.
    const fn new() -> Self {
        Self(SpinNoIrqLock::new(buddy::Heap::empty()))
    }

    /// Initializes the heap allocator with the given start address and size.
    ///
    /// # Safety
    /// - This function should be called only once.
    /// - The caller should ensure that the heap is not being referenced by any other thread.
    unsafe fn init(&self, start_addr: usize, size: usize) {
        unsafe {
            self.0.lock().init(start_addr, size);
        }
    }
}

unsafe impl GlobalAlloc for NoIrqLockedHeap<32> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        self.0
            .lock()
            .alloc(layout)
            .ok()
            .map_or(core::ptr::null_mut(), |allocation| allocation.as_ptr())
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { self.0.lock().dealloc(NonNull::new_unchecked(ptr), layout) }
    }
}

static mut KERNEL_HEAP: [u8; KERNEL_HEAP_SIZE] = [0; KERNEL_HEAP_SIZE];

#[global_allocator]
static HEAP_ALLOCATOR: NoIrqLockedHeap<32> = NoIrqLockedHeap::new();

#[alloc_error_handler]
fn alloc_error_handler(layout: Layout) -> ! {
    panic!("heap allocation error, layout = {:?}", layout)
}

/// Initializes heap allocator.
///
/// # Safety
/// - This function should be called only once.
/// - The caller should ensure that the heap is not being referenced by any other thread.
pub unsafe fn init_heap_allocator() {
    unsafe {
        // SAFETY: we are the only one using the heap
        #[allow(static_mut_refs)]
        let start_addr = PhysAddr::new(KERNEL_HEAP.as_ptr() as usize).to_usize();

        HEAP_ALLOCATOR.init(start_addr, KERNEL_HEAP_SIZE);

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
