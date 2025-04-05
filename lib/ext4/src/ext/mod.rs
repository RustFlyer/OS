use alloc::ffi::CString;
use log::error;
use lwext4_rust::{
    InodeTypes,
    bindings::{
        EOK, ext4_dir_mv, ext4_dir_rm, ext4_fremove, ext4_frename, ext4_inode_exist, ext4_readlink,
    },
};

pub mod dir;
pub mod file;
pub mod link;

pub fn lwext4_check_inode_exist(path: &str, types: InodeTypes) -> bool {
    let c_path = CString::new(path).expect("CString::new failed");
    let r = unsafe { ext4_inode_exist(c_path.as_ptr(), types as i32) }; // eg: types: EXT4_DE_REG_FILE
    r == EOK as i32
}

/// Rename directory
pub fn lwext4_mvdir(path: &str, new_path: &str) -> Result<(), i32> {
    let c_path = CString::new(path).expect("CString::new failed");
    let c_new_path = CString::new(new_path).expect("CString::new failed");
    let r = unsafe { ext4_dir_mv(c_path.as_ptr(), c_new_path.as_ptr()) };
    match r {
        0 => Ok(()),
        _ => {
            error!("ext4_dir_mv error: rc = {}", r);
            Err(r)
        }
    }
}

/// Rename file
pub fn lwext4_mvfile(path: &str, new_path: &str) -> Result<(), i32> {
    let c_path = CString::new(path).expect("CString::new failed");
    let c_new_path = CString::new(new_path).expect("CString::new failed");
    let r = unsafe { ext4_frename(c_path.as_ptr(), c_new_path.as_ptr()) };
    match r {
        0 => Ok(()),
        _ => {
            error!("ext4_frename error: rc = {}", r);
            Err(r)
        }
    }
}

/// Recursive directory remove
pub fn lwext4_rmdir(path: &str) -> Result<(), i32> {
    let c_path = CString::new(path).expect("CString::new failed");
    let r = unsafe { ext4_dir_rm(c_path.as_ptr()) };
    match r {
        0 => Ok(()),
        e => {
            error!("ext4_dir_rm: rc = {}", r);
            Err(e)
        }
    }
}

/// Remove file by path.
pub fn lwext4_rmfile(path: &str) -> Result<(), i32> {
    let c_path = CString::new(path).expect("CString::new failed");
    let r = unsafe { ext4_fremove(c_path.as_ptr()) };
    match r {
        0 => Ok(()),
        _ => {
            error!("ext4_fremove error: rc = {}", r);
            Err(r)
        }
    }
}

pub fn lwext4_readlink(path: &str, buf: &mut [u8]) -> Result<usize, i32> {
    let c_path = CString::new(path).expect("CString::new failed");
    let mut r_cnt = 0;
    let r = unsafe {
        ext4_readlink(
            c_path.as_ptr(),
            buf.as_mut_ptr() as _,
            buf.len(),
            &mut r_cnt,
        )
    };

    match r {
        0 => Ok(r_cnt),
        _ => {
            error!("ext4_readlink: rc = {}", r);
            Err(r)
        }
    }
}
