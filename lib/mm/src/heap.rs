//! Module for kernel heap allocator.
//!
//! Currently, we use the buddy system allocator for the kernel heap and use it to
//! allocate memory for all kernel objects that require dynamic memory allocation.

use core::{
    alloc::{GlobalAlloc, Layout},
    ptr::{self, NonNull},
};

use buddy_system_allocator as buddy;

use config::mm::KERNEL_HEAP_SIZE;
use mutex::SpinNoIrqLock;

use crate::address::VirtAddr;

/// A heap allocator protected by a spin lock.
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
        let buf = self
            .0
            .lock()
            .alloc(layout)
            .ok()
            .map_or(ptr::null_mut(), |allocation| allocation.as_ptr());

        let sz = layout.size();
        let zbuf = [0u8; 512];
        for i in 0..((sz + 511) / 512) {
            let p = (buf as usize + 512 * i) as *mut u8;
            let m = 512.min(sz - i * 512);
            unsafe {
                p.copy_from_nonoverlapping(zbuf.as_ptr(), m);
            }
        }

        // let mut rbuf = [0u8; 128];
        // let min = layout.size().min(128);
        // unsafe {
        //     core::ptr::copy_nonoverlapping(buf, rbuf.as_mut_ptr(), min);
        // }
        // let tbuf = &rbuf[0..min];
        // log::warn!("[HEAP] alloc addr: {:#x}, buf: {:?}", buf as usize, tbuf);

        buf
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { self.0.lock().dealloc(NonNull::new_unchecked(ptr), layout) }
    }
}

#[repr(align(4096))]
struct HeapMemory([u8; KERNEL_HEAP_SIZE]);

/// Memory space for the kernel heap.
static mut HEAP_MEMORY: HeapMemory = HeapMemory([0; KERNEL_HEAP_SIZE]);

/// The global heap allocator for the kernel.
#[global_allocator]
static HEAP_ALLOCATOR: NoIrqLockedHeap<32> = NoIrqLockedHeap::new();

#[alloc_error_handler]
fn alloc_error_handler(layout: Layout) -> ! {
    panic!("heap allocation error, layout = {:?}", layout)
}

/// Initializes the kernel heap allocator.
///
/// # Safety
/// This function should be called only once before making any heap allocations.
pub unsafe fn init_heap_allocator() {
    let start_addr = unsafe {
        // SAFETY: we are the only one using the heap
        #[allow(static_mut_refs)]
        VirtAddr::new(HEAP_MEMORY.0.as_ptr() as usize).to_usize()
    };

    unsafe {
        HEAP_ALLOCATOR.init(start_addr, KERNEL_HEAP_SIZE);
    }

    log::info!(
        "heap memory: {:#x} - {:#x}",
        start_addr,
        start_addr + KERNEL_HEAP_SIZE
    );
}

pub fn allocate_align_memory(size: usize, align: usize) -> *mut u8 {
    unsafe {
        let layout = Layout::from_size_align_unchecked(size, align);
        HEAP_ALLOCATOR.alloc(layout)
    }
}
