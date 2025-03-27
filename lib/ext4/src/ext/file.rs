extern crate alloc;
use alloc::ffi::CString;
use core::mem::MaybeUninit;
use log::error;
use lwext4_rust::bindings::{
    ext4_fclose, ext4_file, ext4_fopen2, ext4_fread, ext4_fseek, ext4_fwrite,
};

pub struct ExtFile(ext4_file);

impl Drop for ExtFile {
    fn drop(&mut self) {
        unsafe {
            ext4_fclose(&self.0);
        }
    }
}

impl ExtFile {
    pub fn open(path: &str, flags: i32) -> Result<Self, i32> {
        let c_path = CString::new(path).expect("CString::new failed");
        let mut file = MaybeUninit::uninit();
        let r = unsafe { ext4_fopen2(file.as_mut_ptr(), c_path.as_ptr(), flags) };
        match r {
            0 => unsafe { Ok(Self(file.assume_init())) },
            e => {
                error!("ext4_fopen: {}, rc = {}", path, r);
                Err(e)
            }
        }
    }

    pub fn seek(&mut self, offset: i64, seek_type: u32) -> Result<(), i32> {
        let mut offset = offset;
        let size = self.size() as i64;

        if offset > size {
            warn!("Seek beyond the end of the file");
            offset = size;
        }
        let r = unsafe { ext4_fseek(&mut self.0, offset, seek_type) };
        match r {
            0 => Ok(()),
            _ => {
                error!("ext4_fseek error: rc = {}", r);
                Err(r)
            }
        }
    }

    pub fn read(&mut self, buf: &mut [u8]) -> Result<usize, i32> {
        let mut r_cnt = 0;
        let r = unsafe { ext4_fread(&mut self.0, buf.as_mut_ptr() as _, buf.len(), &mut r_cnt) };

        match r {
            0 => Ok(r_cnt),
            e => {
                error!("ext4_fread: rc = {}", r);
                Err(e)
            }
        }
    }

    pub fn write(&mut self, buf: &[u8]) -> Result<usize, i32> {
        let mut w_cnt = 0;
        let r = unsafe { ext4_fwrite(&mut self.0, buf.as_ptr() as _, buf.len(), &mut w_cnt) };

        match r {
            0 => Ok(w_cnt),
            e => {
                error!("ext4_fwrite: rc = {}", r);
                Err(e)
            }
        }
    }
}
