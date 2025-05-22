use alloc::vec::Vec;
use core::ptr::NonNull;
use mm::address::{PhysAddr, PhysPageNum, VirtAddr};
use mm::frame::FrameTracker;
use mutex::SpinNoIrqLock;
use virtio_drivers;

static FRAME_SPACE: SpinNoIrqLock<Vec<FrameTracker>> = SpinNoIrqLock::new(Vec::new());

pub struct VirtHalImpl;

unsafe impl virtio_drivers::Hal for VirtHalImpl {
    /// DMA Memory Alloc
    /// - pages: Numbers of Page Needed
    /// - direction: Direction of I/O
    ///
    /// Returns (Base PhyAddr of Alloc Frame, Virt Ptr to Frame)
    fn dma_alloc(
        pages: usize,
        _direction: virtio_drivers::BufferDirection,
    ) -> (virtio_drivers::PhysAddr, core::ptr::NonNull<u8>) {
        assert!(pages > 0);
        let mut base = PhysPageNum::new(0);
        let mut frame_space = FRAME_SPACE.lock();
        let mut frame_batch = FrameTracker::build_contiguous(pages).expect("virtio alloc no page!");
        for frame_id in 0..pages {
            let frame = frame_batch.pop().unwrap();
            if frame_id == pages - 1 {
                base = frame.ppn();
            }
            frame_space.push(frame);
        }

        let pa: PhysAddr = base.into();
        let va: VirtAddr = pa.to_va_kernel();
        let va_ptr = va.to_usize() as *mut u8;
        (pa.to_usize(), unsafe { NonNull::new_unchecked(va_ptr) })
    }

    /// DMA Memory Dealloc
    /// - paddr: PhyAddr of Page Base
    /// - vaddr: VirtAddr of Page Base
    /// - pages: Numbers of Page that Should be Dealloced
    ///
    /// Returns 0 as Success
    unsafe fn dma_dealloc(
        paddr: virtio_drivers::PhysAddr,
        _vaddr: core::ptr::NonNull<u8>,
        pages: usize,
    ) -> i32 {
        let pa = PhysAddr::new(paddr);
        let ppn_st = pa.page_number();
        let ppn_ed = PhysPageNum::new(ppn_st.to_usize() + pages);
        for _ppn in ppn_st.to_usize()..ppn_ed.to_usize() {
            // Here frame which owns the ppn should be dealloc
            todo!()
        }
        0
    }

    /// MMIO PhyAddr => VirtAddr
    ///
    /// Used to Access the mapped Mmio
    /// - paddr: PhyAddr
    /// - size: Mapped Size
    ///
    /// Returns Virt Ptr
    unsafe fn mmio_phys_to_virt(
        paddr: virtio_drivers::PhysAddr,
        _size: usize,
    ) -> core::ptr::NonNull<u8> {
        let pa = PhysAddr::new(paddr);
        let va = pa.to_va_kernel();
        let va_ptr = va.to_usize() as *mut u8;

        unsafe { NonNull::new_unchecked(va_ptr) }
    }

    /// Shares Memory Buffer
    ///
    /// VirtAddr => PhyAddr, Used to DMA operation
    /// - buffer: Buffer to Share
    /// - direction: Direction of DMA
    unsafe fn share(
        buffer: core::ptr::NonNull<[u8]>,
        _direction: virtio_drivers::BufferDirection,
    ) -> virtio_drivers::PhysAddr {
        let buffer = buffer.as_ptr() as *const usize as usize;
        let va = VirtAddr::new(buffer);
        // if va.to_usize() < KERNEL_MAP_OFFSET {
        //     return va.to_usize();
        // }
        let pa = va.to_pa_kernel();
        pa.to_usize()
    }

    /// Cancels Shared Memory Buffer
    ///
    /// Here is None.
    /// It's no need to implement now.
    unsafe fn unshare(
        _paddr: virtio_drivers::PhysAddr,
        _buffer: core::ptr::NonNull<[u8]>,
        _direction: virtio_drivers::BufferDirection,
    ) {
    }
}
