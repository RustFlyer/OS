extern crate alloc;

use alloc::ffi::CString;
use core::mem::MaybeUninit;
use lwext4_rust::bindings::{ext4_dir_close, ext4_dir_open};

pub struct ExtDir(ext4_dir);

impl Drop for ExtDir {
    fn drop(&mut self) {
        unsafe {
            ext4_dir_close(&mut self.0);
        }
    }
}

impl ExtDir {
    pub fn open(path: &str) -> Result<Self, i32> {
        let c_path = CString::new(path).expect("CString::new failed");
        let mut dir = MaybeUninit::uninit();
        let r = unsafe { ext4_dir_open(dir.as_mut_ptr(), c_path.as_ptr()) };
        match r {
            0 => unsafe { Ok(Self(dir.assume_init())) },
            e => {
                error!("ext4_dir_open: {}, rc = {}", path, r);
                Err(e)
            }
        }
    }
}
