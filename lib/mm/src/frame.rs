//! Module for managing frames (physical pages).
//!
//! This module provides functions for allocating and deallocating frames.

use lazy_static::lazy_static;

use config::mm::{_ekernel, PAGE_SIZE, RAM_SIZE, RAM_START};
use id_allocator::{IdAllocator, VecIdAllocator};
use mutex::SpinNoIrqLock;

use crate::address::{PhysAddr, PhysPageNum, VirtPageNum};

type FrameAllocator = VecIdAllocator;

lazy_static! {
    static ref FRAME_ALLOCATOR: SpinNoIrqLock<FrameAllocator> = {
        let frames_ppn_start = PhysAddr::new(_ekernel as usize).page_number().to_usize();
        let frames_ppn_end = PhysAddr::new(RAM_START + RAM_SIZE).page_number().to_usize();
        log::info!(
            "free frame memory: {:#x} - {:#x}",
            frames_ppn_start * PAGE_SIZE,
            frames_ppn_end * PAGE_SIZE
        );
        SpinNoIrqLock::new(FrameAllocator::new(frames_ppn_start, frames_ppn_end))
    };
}

/// RAII guard for a frame.
///
/// Constructing this type will allocate a frame from the frame allocator,
/// and the frame will be deallocated when this guard is dropped.
#[derive(Debug)]
pub struct FrameTracker {
    /// Physical page number of the frame.
    ppn: PhysPageNum,
}

impl FrameTracker {
    /// Allocates a frame.
    ///
    /// Returns `Some(FrameTracker)` if a frame is successfully allocated,
    /// or `None` if there are no free frames.
    pub fn new() -> Option<Self> {
        FRAME_ALLOCATOR.lock().alloc().map(|frame| FrameTracker {
            ppn: PhysPageNum::new(frame),
        })
    }

    /// Gets the physical page number of the frame.
    pub fn as_ppn(&self) -> PhysPageNum {
        self.ppn
    }

    /// Gets the virtual page number of the frame in the kernel space.
    pub fn as_vpn(&self) -> VirtPageNum {
        self.ppn.to_vpn_kernel()
    }

    /// Gets a slice pointing to the frame.
    pub fn as_slice(&self) -> &mut [u8; PAGE_SIZE] {
        // SAFETY: The frame is allocated, and the returned slice does not outlive
        // the `FrameTracker` which lives as long as the frame.
        unsafe { self.as_vpn().as_slice() }
    }
}

impl Drop for FrameTracker {
    /// Deallocate the frame.
    fn drop(&mut self) {
        // SAFETY: `self.frame` is an allocated ID because
        // - its constructor calls `alloc` on the frame allocator, and
        // - a `FrameTracker` cannot be cloned.
        unsafe {
            FRAME_ALLOCATOR.lock().dealloc(self.ppn.to_usize());
        }
    }
}

pub fn frame_alloc_test() {
    log::info!("test frame alloc: start");
    {
        let f1 = FrameTracker::new().expect("failed to allocate frame");
        let f2 = FrameTracker::new().expect("failed to allocate frame");
        log::info!("frame 1: 0x{:x}", f1.as_ppn().address().to_usize());
        log::info!("frame 2: 0x{:x}", f2.as_ppn().address().to_usize());
    }
    log::info!("frame 1 and 2 should have been dropped");
    let f3 = FrameTracker::new().expect("failed to allocate frame");
    log::info!("frame 3: 0x{:x}", f3.as_ppn().address().to_usize());
    log::info!("test frame alloc: end");
}
