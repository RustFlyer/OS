//! Module for managing allocatable frames (physical pages).
//!
//! An “allocatable frame” is defined as a frame that is not used by the kernel.
//! By contrast, a “kernel frame” is a frame that is used by the kernel—it is
//! in the kernel address space and is kernel's code, data, stack, etc.
//!
//! Allocatable frames need to be managed properly. Kernel frames only need to be
//! mapped to the kernel's page table, so they do not need to be managed.
//!
//! Allocatable frames are tracked by [`FrameTracker`]. A frame can be allocated
//! by constructing a [`FrameTracker`], and it will be deallocated automatically
//! when the [`FrameTracker`] is dropped.
//!
//! Every allocation and deallocation requires to acquire the frame allocator lock,
//! which is not efficient when allocating or deallocating multiple frames at once.
//! Use [`FrameTracker::new_batch`] and [`FrameDropper`] to allocate and deallocate
//! frames in batch, respectively.

use alloc::vec::Vec;
use core::mem::ManuallyDrop;
use simdebug::when_debug;
use systype::{SysError, SysResult};

use lazy_static::lazy_static;

use config::mm::{PAGE_SIZE, RAM_END, kernel_end_phys};
use id_allocator::{IdAllocator, VecIdAllocator};
use mutex::SpinNoIrqLock;

use crate::address::{PhysAddr, PhysPageNum, VirtPageNum};

/// The frame allocator type.
type FrameAllocator = VecIdAllocator;

lazy_static! {
    /// The frame allocator. It allocates and deallocates allocatable frames.
    ///
    /// It is protected by a lock to be used in a multi-threaded environment.
    static ref FRAME_ALLOCATOR: SpinNoIrqLock<FrameAllocator> = {
        let frames_ppn_start = PhysAddr::new(kernel_end_phys()).page_number().to_usize();
        let frames_ppn_end = PhysAddr::new(RAM_END).page_number().to_usize();
        when_debug!({
            log::info!(
                "frame allocator: free frame memory from {:#x} - {:#x}",
                frames_ppn_start * PAGE_SIZE,
                frames_ppn_end * PAGE_SIZE
            );
        });
        SpinNoIrqLock::new(FrameAllocator::new(frames_ppn_start, frames_ppn_end))
    };
}

/// RAII guard for a frame.
///
/// Constructing a value of this type will allocate a frame from the frame allocator,
/// and the frame will be deallocated when this guard is dropped.
///
/// # Note
/// Constructing and dropping a `FrameTrackerGuard` will acquire and release the frame
/// allocator lock, respectively. Therefore, it is recommended to use
/// [`FrameTracker::build_batch`] to allocate multiple frames in batch,
/// and to use [`FrameDropper`] to deallocate frames in batch.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct FrameTracker {
    /// Physical page number of the frame.
    ppn: PhysPageNum,
}

impl FrameTracker {
    /// Allocates a frame.
    ///
    /// Returns a `FrameTracker` if the frame is successfully allocated, or an `ENOMEM` error
    /// if there are no free frames.
    pub fn new() -> SysResult<Self> {
        FRAME_ALLOCATOR
            .lock()
            .alloc()
            .map(|frame| FrameTracker {
                ppn: PhysPageNum::new(frame),
            })
            .ok_or(SysError::ENOMEM)
    }

    /// Allocates a batch of frames.
    ///
    /// Returns a vector of `FrameTracker` if all frames are successfully allocated,
    /// or an `ENOMEM` error if there are no free frames. In the case of an error,
    /// no frames are allocated.
    ///
    /// This function acquires the frame allocator lock only once, so it is more efficient
    /// than calling [`FrameTracker::new`] multiple times.
    ///
    /// # Errors
    /// Returns [`ENOMEM`] if there are no free frames.
    pub fn build_batch(count: usize) -> SysResult<Vec<Self>> {
        let mut allocator_locked = FRAME_ALLOCATOR.lock();
        let mut frames = Vec::new();
        for _ in 0..count {
            if let Some(frame) = allocator_locked.alloc() {
                frames.push(FrameTracker {
                    ppn: PhysPageNum::new(frame),
                });
            } else {
                FrameDropper::drop(frames);
                return Err(SysError::ENOMEM);
            }
        }
        Ok(frames)
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
    pub fn as_slice(&self) -> &[u8; PAGE_SIZE] {
        // SAFETY: The frame is allocated, and the returned slice does not outlive
        // the `FrameTracker` which lives as long as the frame.
        unsafe { self.as_vpn().as_slice() }
    }

    /// Gets a mutable slice pointing to the frame.
    pub fn as_slice_mut(&self) -> &mut [u8; PAGE_SIZE] {
        // SAFETY: The frame is allocated, and the returned slice does not outlive
        // the `FrameTracker` which lives as long as the frame.
        unsafe { self.as_vpn().as_slice_mut() }
    }
}

impl Drop for FrameTracker {
    /// Deallocate the frame.
    fn drop(&mut self) {
        // SAFETY: `self.ppn` is an allocated ID because a `FrameTracker` can
        // only be constructed by `FrameTracker::new` which calls `IdAllocator::alloc`.
        unsafe {
            FRAME_ALLOCATOR.lock().dealloc(self.ppn.to_usize());
        }
    }
}

/// A helper struct for deallocating frames in batch.
pub struct FrameDropper {
    frames: ManuallyDrop<Vec<FrameTracker>>,
}

impl FrameDropper {
    /// Constructs a `FrameDropper` from a vector of `FrameTracker`.
    ///
    /// The frames will be deallocated when this `FrameDropper` is dropped.
    pub fn new(frames: Vec<FrameTracker>) -> Self {
        Self {
            frames: ManuallyDrop::new(frames),
        }
    }

    /// Drops a vector of `FrameTracker`.
    ///
    /// This function is equivalent to `drop(FrameDropper::new(frames))`.
    pub fn drop(frames: Vec<FrameTracker>) {
        Self::new(frames);
    }

    /// Adds a frame to the dropper.
    pub fn push(&mut self, frame: FrameTracker) {
        self.frames.push(frame);
    }

    /// Adds a batch of frames to the dropper.
    pub fn extend(&mut self, frames: Vec<FrameTracker>) {
        self.frames.extend(frames);
    }
}

impl Drop for FrameDropper {
    fn drop(&mut self) {
        let mut frame_allocator_locked = FRAME_ALLOCATOR.lock();
        // Manually deallocate the frames.
        for frame in self.frames.iter() {
            // SAFETY: `frame.ppn` is an allocated ID because a `FrameTracker` can
            // only be constructed by `FrameTracker::new` which calls `IdAllocator::alloc`.
            unsafe {
                frame_allocator_locked.dealloc(frame.ppn.to_usize());
            }
        }
    }
}

pub fn frame_alloc_test() {
    log::info!("frame_alloc_test: start");
    {
        let f1 = FrameTracker::new().expect("frame_alloc_test: failed to allocate frame");
        let f2 = FrameTracker::new().expect("frame_alloc_test: failed to allocate frame");
        log::info!(
            "frame_alloc_test: frame 1: {:#x}",
            f1.as_ppn().address().to_usize()
        );
        log::info!(
            "frame_alloc_test: frame 2: {:#x}",
            f2.as_ppn().address().to_usize()
        );
    }
    {
        log::info!("frame_alloc_test: frames 1 and 2 are dropped");
        let f3 = FrameTracker::new().expect("frame_alloc_test: failed to allocate frame");
        log::info!(
            "frame_alloc_test: frame 3: {:#x}",
            f3.as_ppn().address().to_usize()
        );
        log::info!("frame_alloc_test: frame 3 is dropped");
    }
    {
        log::info!("frame_alloc_test: allocate 5 frames in a batch");
        let frames =
            FrameTracker::build_batch(5).expect("frame_alloc_test: failed to allocate frames");
        for (i, f) in frames.iter().enumerate() {
            log::info!(
                "frame_alloc_test: frame {}: {:#x}",
                i,
                f.as_ppn().address().to_usize()
            );
        }
        FrameDropper::drop(frames);
        log::info!("frame_alloc_test: frames are dropped");
        log::info!("frame_alloc_test: allocate 5 frames in a batch");
        let frames = FrameTracker::build_batch(5).expect("failed to allocate frames");
        for (i, f) in frames.iter().enumerate() {
            log::info!("frame {}: {:#x}", i, f.as_ppn().address().to_usize());
        }
        log::info!("frame_alloc_test: frames are dropped");
    }
    log::info!("frame_alloc_test: end");
}
