// This Module is adapted from Phoenix OS with a few modifications.

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
//! Use [`FrameTracker::build_batch`] and [`FrameDropper`] to allocate and deallocate
//! frames in batch, respectively.

use alloc::vec::Vec;
use core::{cell::SyncUnsafeCell, mem::ManuallyDrop};

use bitmap_allocator::{BitAlloc, BitAlloc64K};

use config::mm::{PAGE_SIZE, RAM_END, kernel_end_phys};
use mutex::SpinNoIrqLock;
use systype::error::{SysError, SysResult};

use crate::address::{PhysAddr, PhysPageNum, VirtPageNum};

/// Global frame allocator. It allocates and deallocates allocatable frames.
///
/// It is protected by a lock to be used in a multi-threaded environment.
static FRAME_ALLOCATOR: FrameAllocator = FrameAllocator {
    allocator: SpinNoIrqLock::new(BitAlloc64K::DEFAULT),
    offset: SyncUnsafeCell::new(0),
};

/// Frame allocator type.
struct FrameAllocator {
    /// Bitmap allocator.
    allocator: SpinNoIrqLock<BitAlloc64K>,
    /// Offset between PPNs and bit indices.
    offset: SyncUnsafeCell<usize>,
}

impl FrameAllocator {
    /// Gets the offset between PPNs and bit indices.
    fn offset(&self) -> usize {
        // SAFETY: `offset` is never mutated after initialization.
        unsafe { *self.offset.get() }
    }
}

/// Initializes the frame allocator.
///
/// # Safety
/// This function must be called only once.
pub unsafe fn init_frame_allocator() {
    let frames_ppn_start = PhysAddr::new(kernel_end_phys()).page_number().to_usize();
    let frames_ppn_end = PhysAddr::new(RAM_END).page_number().to_usize();
    let frame_count = frames_ppn_end - frames_ppn_start;
    let offset = frames_ppn_start;
    // SAFETY: `offset` is mutate only once here when initialization.
    unsafe {
        *FRAME_ALLOCATOR.offset.get() = offset;
    }
    FRAME_ALLOCATOR.allocator.lock().insert(0..frame_count);
    log::debug!(
        "frame allocator: allocatable frames from {:#x} - {:#x}",
        frames_ppn_start * PAGE_SIZE,
        frames_ppn_end * PAGE_SIZE
    );
}

/// RAII guard for an allocatable frame.
///
/// Constructing a value of this type will allocate a frame from the frame allocator,
/// and the frame will be deallocated when this guard is dropped.
///
/// # Note
/// Constructing and dropping a [`FrameTracker`] will acquire and release the frame
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
    pub fn build() -> SysResult<Self> {
        FRAME_ALLOCATOR
            .allocator
            .lock()
            .alloc()
            .map(|i| FrameTracker {
                ppn: PhysPageNum::new(FRAME_ALLOCATOR.offset() + i),
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
    /// than calling [`FrameTracker::build`] multiple times.
    ///
    /// Allocating a contiguous physical frames via [`FrameTracker::build_contiguous`]
    /// may be more efficient than this function, but there may not be enough contiguous
    /// free frames.
    ///
    /// # Errors
    /// Returns `ENOMEM` if there are no free frames.
    pub fn build_batch(count: usize) -> SysResult<Vec<Self>> {
        let mut allocator_lock = FRAME_ALLOCATOR.allocator.lock();
        let mut frames = Vec::with_capacity(count);
        for _ in 0..count {
            if let Some(i) = allocator_lock.alloc() {
                frames.push(FrameTracker {
                    ppn: PhysPageNum::new(FRAME_ALLOCATOR.offset() + i),
                });
            } else {
                FrameDropper::drop(frames);
                return Err(SysError::ENOMEM);
            }
        }
        Ok(frames)
    }

    pub fn build_contiguous(count: usize) -> SysResult<Vec<Self>> {
        let base = FRAME_ALLOCATOR
            .allocator
            .lock()
            .alloc_contiguous(None, count, 0);
        if let Some(base) = base {
            let frames = (FRAME_ALLOCATOR.offset() + base..FRAME_ALLOCATOR.offset() + base + count)
                .map(|frame| FrameTracker {
                    ppn: PhysPageNum::new(frame),
                })
                .collect();
            Ok(frames)
        } else {
            Err(SysError::ENOMEM)
        }
    }

    /// Gets the physical page number of the frame.
    pub fn ppn(&self) -> PhysPageNum {
        self.ppn
    }

    /// Gets the virtual page number of the frame in the kernel space.
    pub fn vpn(&self) -> VirtPageNum {
        self.ppn.to_vpn_kernel()
    }

    /// Gets a slice pointing to the frame.
    pub fn as_slice(&self) -> &[u8; PAGE_SIZE] {
        // SAFETY: The frame is allocated, and the returned slice does not outlive
        // the `FrameTracker` which lives as long as the frame.
        unsafe { self.vpn().as_slice() }
    }

    /// Gets a mutable slice pointing to the frame.
    pub fn as_mut_slice(&mut self) -> &mut [u8; PAGE_SIZE] {
        // SAFETY: The frame is allocated, and the returned slice does not outlive
        // the `FrameTracker` which lives as long as the frame.
        unsafe { self.vpn().as_slice_mut() }
    }
}

impl Drop for FrameTracker {
    /// Deallocate the frame.
    fn drop(&mut self) {
        FRAME_ALLOCATOR
            .allocator
            .lock()
            .dealloc(self.ppn.to_usize() - FRAME_ALLOCATOR.offset());
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
        let mut frame_allocator_locked = FRAME_ALLOCATOR.allocator.lock();
        // Manually deallocate the frames.
        for frame in self.frames.iter() {
            frame_allocator_locked.dealloc(frame.ppn.to_usize() - FRAME_ALLOCATOR.offset());
        }
    }
}

pub fn frame_alloc_test() {
    log::debug!("frame_alloc_test: start");
    {
        let f1 = FrameTracker::build().expect("frame_alloc_test: failed to allocate frame");
        let f2 = FrameTracker::build().expect("frame_alloc_test: failed to allocate frame");
        log::debug!(
            "frame_alloc_test: frame 1: {:#x}",
            f1.ppn().address().to_usize()
        );
        log::debug!(
            "frame_alloc_test: frame 2: {:#x}",
            f2.ppn().address().to_usize()
        );
    }
    {
        log::debug!("frame_alloc_test: frames 1 and 2 are dropped");
        let f3 = FrameTracker::build().expect("frame_alloc_test: failed to allocate frame");
        log::debug!(
            "frame_alloc_test: frame 3: {:#x}",
            f3.ppn().address().to_usize()
        );
        log::debug!("frame_alloc_test: frame 3 is dropped");
    }
    {
        log::debug!("frame_alloc_test: allocate 5 frames in a batch");
        let frames =
            FrameTracker::build_batch(5).expect("frame_alloc_test: failed to allocate frames");
        for (i, f) in frames.iter().enumerate() {
            log::debug!(
                "frame_alloc_test: frame {}: {:#x}",
                i,
                f.ppn().address().to_usize()
            );
        }
        FrameDropper::drop(frames);
        log::debug!("frame_alloc_test: frames are dropped");
        log::debug!("frame_alloc_test: allocate 5 frames in a batch");
        let frames = FrameTracker::build_batch(5).expect("failed to allocate frames");
        for (i, f) in frames.iter().enumerate() {
            log::debug!("frame {}: {:#x}", i, f.ppn().address().to_usize());
        }
        log::debug!("frame_alloc_test: frames are dropped");
    }
    {
        log::debug!("frame_alloc_test: allocate 8 contiguous frames");
        let frames =
            FrameTracker::build_contiguous(8).expect("frame_alloc_test: failed to allocate frames");
        for (i, f) in frames.iter().enumerate() {
            log::debug!(
                "frame_alloc_test: frame {}: {:#x}",
                i,
                f.ppn().address().to_usize()
            );
        }
        FrameDropper::drop(frames);
        log::debug!("frame_alloc_test: frames are dropped");
    }
    log::debug!("frame_alloc_test: end");
}
