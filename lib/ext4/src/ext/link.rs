use alloc::ffi::CString;
use log::error;
use lwext4_rust::bindings::{ext4_flink, ext4_fsymlink};

pub fn lwext4_symlink(target: &str, path: &str) -> Result<(), i32> {
    let c_target = CString::new(target).expect("CString::new failed");
    let c_path = CString::new(path).expect("CString::new failed");
    let r = unsafe { ext4_fsymlink(c_target.as_ptr(), c_path.as_ptr()) };
    match r {
        0 => Ok(()),
        _ => {
            error!("ext4_fsymlink: rc = {r}, path = {path}");
            Err(r)
        }
    }
}

pub fn lwext4_link(path: &str, hardlink_path: &str) -> Result<(), i32> {
    let c_path = CString::new(path).expect("CString::new failed");
    let c_hardlink_path = CString::new(hardlink_path).expect("CString::new failed");
    let r = unsafe { ext4_flink(c_path.as_ptr(), c_hardlink_path.as_ptr()) };
    match r {
        0 => Ok(()),
        _ => {
            error!("ext4_flink: rc = {r}, path = {path}");
            Err(r)
        }
    }
}
