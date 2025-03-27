extern crate alloc;
use alloc::ffi::CString;
use alloc::vec::Vec;
use core::mem::MaybeUninit;
use lwext4_rust::{
    InodeTypes,
    bindings::{ext4_dir, ext4_dir_close, ext4_dir_entry_next, ext4_dir_open},
};

pub struct ExtDirEntry<'a> {
    pub inode: u32,
    pub name: &'a str,
    pub type_: u8,
}

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

    pub fn next(&mut self) -> Option<ExtDirEntry> {
        unsafe {
            if ext4_dir_entry_next(&mut self.0).is_null() {
                return None;
            }
        };
        let name_buf = &self.0.de.name[..self.0.de.name_length as usize];
        Some(ExtDirEntry {
            inode: self.0.de.inode,
            name: core::str::from_utf8(&name_buf).unwrap(),
            type_: self.0.de.inode_type,
        })
    }
}
