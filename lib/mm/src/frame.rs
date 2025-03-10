//! Module for managing frames (physical pages).
//!
//! This module provides functions for allocating and deallocating frames.

use lazy_static::lazy_static;

use config::mm::{_ekernel, PAGE_SIZE, RAM_SIZE, RAM_START};
use id_allocator::{IdAllocator, VecIdAllocator};
use mutex::SpinNoIrqLock;

use crate::address::{PhysAddr, VirtPageNum};

type FrameAllocator = VecIdAllocator;

lazy_static! {
    static ref FRAME_ALLOCATOR: SpinNoIrqLock<FrameAllocator> = {
        let frames_vpn_start = PhysAddr::new(_ekernel as usize)
            .to_va_kernel()
            .page_number()
            .to_usize();
        let frames_vpn_end = PhysAddr::new(RAM_START + RAM_SIZE)
            .to_va_kernel()
            .page_number()
            .to_usize();
        log::info!(
            "free frame memory: {:#x} - {:#x}",
            frames_vpn_start * PAGE_SIZE,
            frames_vpn_end * PAGE_SIZE
        );
        SpinNoIrqLock::new(FrameAllocator::new(frames_vpn_start, frames_vpn_end))
    };
}

/// RAII guard for a frame.
///
/// Constructing this type will allocate a frame from the frame allocator,
/// and the frame will be deallocated when this guard is dropped.
#[derive(Debug)]
pub struct FrameTracker {
    frame: VirtPageNum,
}

impl FrameTracker {
    /// Allocate a frame.
    ///
    /// Returns `Some(FrameTracker)` if a frame is successfully allocated,
    /// or `None` if there are no free frames.
    fn new() -> Option<Self> {
        FRAME_ALLOCATOR.lock().alloc().map(|frame| FrameTracker {
            frame: VirtPageNum::new(frame),
        })
    }
}

impl Drop for FrameTracker {
    /// Deallocate the frame.
    fn drop(&mut self) {
        // SAFETY: `self.frame` is an allocated ID because
        // - its constructor calls `alloc` on the frame allocator, and
        // - a `FrameTracker` cannot be cloned.
        unsafe {
            FRAME_ALLOCATOR.lock().dealloc(self.frame.to_usize());
        }
    }
}

/// Allocate a frame.
///
/// Returns `Some(FrameTracker)` if a frame is successfully allocated,
/// or `None` if there are no free frames.
pub fn alloc_frame() -> Option<FrameTracker> {
    FrameTracker::new()
}

/// Deallocate a frame.
pub fn dealloc_frame(_frame: FrameTracker) {}
