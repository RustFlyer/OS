use core::cmp;

use config::mm::PAGE_SIZE;
use mm::{
    address::{PhysPageNum, VirtAddr},
    frame::FrameTracker,
};

pub struct Page {
    frame: FrameTracker,
}

impl core::fmt::Debug for Page {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Page").field("frame", &self.ppn()).finish()
    }
}

impl Page {
    pub fn new() -> Self {
        Self {
            frame: FrameTracker::build().unwrap(),
        }
    }

    pub fn copy_data_from_another(&self, another: &Page) {
        fn usize_array(ppn: &PhysPageNum) -> &'static mut [usize] {
            let va: VirtAddr = ppn.to_vpn_kernel().into();
            unsafe {
                core::slice::from_raw_parts_mut(
                    va.to_usize() as *mut usize,
                    PAGE_SIZE / size_of::<usize>(),
                )
            }
        }
        let dst = usize_array(&self.ppn());
        let src = usize_array(&another.ppn());
        dst.copy_from_slice(src);
    }

    pub fn copy_from_slice(&self, data: &[u8]) {
        let len = cmp::min(PAGE_SIZE, data.len());
        self.bytes_array_range(0..len).copy_from_slice(data)
    }

    pub fn ppn(&self) -> PhysPageNum {
        self.frame.as_ppn()
    }

    pub fn bytes_array(&self) -> &'static mut [u8] {
        let va: VirtAddr = self.ppn().to_vpn_kernel().into();
        unsafe { core::slice::from_raw_parts_mut(va.to_usize() as *mut u8, PAGE_SIZE) }
    }

    pub fn bytes_array_range(&self, range: core::ops::Range<usize>) -> &'static mut [u8] {
        let mut va: VirtAddr = self.ppn().to_vpn_kernel().into();
        va = VirtAddr::new(va.to_usize() + range.start);
        unsafe { core::slice::from_raw_parts_mut(va.to_usize() as *mut u8, range.len()) }
    }
}
