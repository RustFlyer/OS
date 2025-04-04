extern crate alloc;
use alloc::ffi::CString;
use core::mem::MaybeUninit;
use log::{error, warn};
use lwext4_rust::bindings::{
    EOK, ext4_block, ext4_fclose, ext4_file, ext4_fopen2, ext4_fread, ext4_fs_get_inode_dblk_idx,
    ext4_fs_get_inode_ref, ext4_fs_put_inode_ref, ext4_fseek, ext4_fsize, ext4_ftell,
    ext4_ftruncate, ext4_fwrite, ext4_inode_ref, ext4_lblk_t,
};

pub struct ExtFile(ext4_file);

impl Drop for ExtFile {
    fn drop(&mut self) {
        unsafe {
            ext4_fclose(&mut self.0);
        }
    }
}

impl ExtFile {
    pub fn size(&mut self) -> u64 {
        unsafe { ext4_fsize(&mut self.0) }
    }

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

    pub fn tell(&mut self) -> u64 {
        let r = unsafe { ext4_ftell(&mut self.0) };
        r
    }

    pub fn truncate(&mut self, size: u64) -> Result<(), i32> {
        let r = unsafe { ext4_ftruncate(&mut self.0, size) };
        match r {
            0 => Ok(()),
            e => {
                error!("ext4_ftruncate: rc = {}", r);
                Err(e)
            }
        }
    }

    pub fn file_get_blk_idx(&mut self) -> Result<u64, i32> {
        let block_idx;
        unsafe {
            let mut inode_ref = ext4_inode_ref {
                block: ext4_block {
                    lb_id: 0,
                    buf: core::ptr::null_mut(),
                    data: core::ptr::null_mut(),
                },
                inode: core::ptr::null_mut(),
                fs: core::ptr::null_mut(),
                index: 0,
                dirty: false,
            };
            let r = ext4_fs_get_inode_ref(&mut (*self.0.mp).fs, self.0.inode, &mut inode_ref);
            if r != EOK as i32 {
                error!("ext4_fs_get_inode_ref: rc = {}", r);
                return Err(r);
            }
            let sb = (*self.0.mp).fs.sb;
            let block_size = 1024 << sb.log_block_size.to_le();
            let iblock_idx: ext4_lblk_t = ((self.0.fpos) / block_size).try_into().unwrap();
            let mut fblock = 0;
            let r = ext4_fs_get_inode_dblk_idx(&mut inode_ref, iblock_idx, &mut fblock, true);
            if r != EOK as i32 {
                error!("ext4_fs_get_inode_dblk_idx: rc = {}", r);
                return Err(r);
            }
            ext4_fs_put_inode_ref(&mut inode_ref);

            let unalg = (self.0.fpos) % block_size;
            let bdev = *(*self.0.mp).fs.bdev;
            let off = fblock * block_size + unalg;
            block_idx = (off + bdev.part_offset) / ((*(bdev.bdif)).ph_bsize as u64);
        }
        Ok(block_idx)
    }
}
