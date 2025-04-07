use alloc::ffi::CString;
use lwext4_rust::bindings::{ext4_flink, ext4_fsymlink};

pub fn lwext4_symlink(target: &str, path: &str) -> Result<(), i32> {
    let c_target = CString::new(target).unwrap();
    let c_path = CString::new(path).unwrap();
    let err = unsafe { ext4_fsymlink(c_target.as_ptr(), c_path.as_ptr()) };
    match err {
        0 => Ok(()),
        _ => {
            log::warn!("ext4_fsymlink: rc = {err}, path = {path}");
            Err(err)
        }
    }
}

pub fn lwext4_link(path: &str, hardlink_path: &str) -> Result<(), i32> {
    let c_path = CString::new(path).unwrap();
    let c_hardlink_path = CString::new(hardlink_path).unwrap();
    let err = unsafe { ext4_flink(c_path.as_ptr(), c_hardlink_path.as_ptr()) };
    match err {
        0 => Ok(()),
        _ => {
            log::warn!("ext4_flink: rc = {err}, path = {path}");
            Err(err)
        }
    }
}
