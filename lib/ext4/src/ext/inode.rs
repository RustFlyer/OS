use core::ffi::CStr;

use alloc::ffi::CString;
use lwext4_rust::{
    InodeTypes,
    bindings::{
        ext4_atime_get, ext4_atime_set, ext4_ctime_get, ext4_ctime_set, ext4_fs_get_inode_ref,
        ext4_inode, ext4_inode_exist, ext4_mtime_get, ext4_mtime_set, ext4_sblock,
    },
};

use systype::error::{SysError, SysResult};

use super::dir::ExtDir;

#[allow(unused)]
unsafe extern "C" {
    // Get/set mode
    fn ext4_inode_get_mode(sb: *const ext4_sblock, inode: *mut ext4_inode) -> u32;
    fn ext4_inode_set_mode(sb: *const ext4_sblock, inode: *mut ext4_inode, mode: u32);

    // Get/set user ID
    fn ext4_inode_get_uid(inode: *mut ext4_inode) -> u32;
    fn ext4_inode_set_uid(inode: *mut ext4_inode, uid: u32);

    // Get/set inode size
    fn ext4_inode_get_size(sb: *const ext4_sblock, inode: *mut ext4_inode) -> u64;
    fn ext4_inode_set_size(inode: *mut ext4_inode, size: u64);

    // Get/set access time
    fn ext4_inode_get_access_time(inode: *mut ext4_inode) -> u32;
    fn ext4_inode_set_access_time(inode: *mut ext4_inode, time: u32);

    // Get/set inode change time
    fn ext4_inode_get_change_inode_time(inode: *mut ext4_inode) -> u32;
    fn ext4_inode_set_change_inode_time(inode: *mut ext4_inode, time: u32);

    // Get/set modification time
    fn ext4_inode_get_modif_time(inode: *mut ext4_inode) -> u32;
    fn ext4_inode_set_modif_time(inode: *mut ext4_inode, time: u32);

    // Get/set group ID
    fn ext4_inode_get_gid(inode: *mut ext4_inode) -> u32;
    fn ext4_inode_set_gid(inode: *mut ext4_inode, gid: u32);

    // Get/set links count
    fn ext4_inode_get_links_cnt(inode: *mut ext4_inode) -> u16;
    fn ext4_inode_set_links_cnt(inode: *mut ext4_inode, cnt: u16);

    // Get/set blocks count
    fn ext4_inode_get_blocks_count(sb: *const ext4_sblock, inode: *mut ext4_inode) -> u64;
    fn ext4_inode_set_blocks_count(sb: *const ext4_sblock, inode: *mut ext4_inode, cnt: u64)
    -> i32;

    // Get/set flags
    fn ext4_inode_get_flags(inode: *mut ext4_inode) -> u32;
    fn ext4_inode_set_flags(inode: *mut ext4_inode, flags: u32);

    // Get/set device number
    fn ext4_inode_get_dev(inode: *mut ext4_inode) -> u32;
    fn ext4_inode_set_dev(inode: *mut ext4_inode, dev: u32);

    // Inode type functions
    fn ext4_inode_type(sb: *const ext4_sblock, inode: *mut ext4_inode) -> u32;
    fn ext4_inode_is_type(sb: *const ext4_sblock, inode: *mut ext4_inode, type_: u32) -> bool;

    // Flag operations
    fn ext4_inode_has_flag(inode: *mut ext4_inode, f: u32) -> bool;
    fn ext4_inode_clear_flag(inode: *mut ext4_inode, f: u32);
    fn ext4_inode_set_flag(inode: *mut ext4_inode, f: u32);
}

/// This struct is currently empty; it only serves as a namespace for functions that
/// operate on inodes.
pub struct ExtInode;

#[allow(unused)]
impl ExtInode {
    /// Returns `true` if the inode at the given path exists.
    pub fn exists(path: &CStr, inode_type: InodeTypes) -> SysResult<bool> {
        let err = unsafe { ext4_inode_exist(path.as_ptr(), inode_type as i32) };
        match err {
            0 => Ok(true),
            _ if err == lwext4_rust::bindings::ENOENT as i32 => Ok(false),
            _ => {
                let err = SysError::from_i32(err);
                log::warn!(
                    "ext4_inode_exist failed: path = {}, error = {:?}",
                    path.to_str().unwrap(),
                    err
                );
                Err(err)
            }
        }
    }

    /// Sets the access time of the inode at the given path.
    pub fn set_atime(path: &CStr, atime: u32) -> SysResult<()> {
        let err = unsafe { ext4_atime_set(path.as_ptr(), atime) };
        match err {
            0 => Ok(()),
            _ => {
                let err = SysError::from_i32(err);
                log::warn!(
                    "ext4_atime_set failed: path = {}, error = {:?}",
                    path.to_str().unwrap(),
                    err
                );
                Err(err)
            }
        }
    }

    /// Gets the access time of the inode at the given path.
    pub fn get_atime(path: &CStr) -> SysResult<u32> {
        let mut atime = 0;
        let err = unsafe { ext4_atime_get(path.as_ptr(), &mut atime) };
        match err {
            0 => Ok(atime),
            _ => {
                let err = SysError::from_i32(err);
                log::warn!(
                    "ext4_atime_get failed: path = {}, error = {:?}",
                    path.to_str().unwrap(),
                    err
                );
                Err(err)
            }
        }
    }

    /// Sets the modified time of the inode at the given path.
    pub fn set_mtime(path: &CStr, mtime: u32) -> SysResult<()> {
        let err = unsafe { ext4_mtime_set(path.as_ptr(), mtime) };
        match err {
            0 => Ok(()),
            _ => {
                let err = SysError::from_i32(err);
                log::warn!(
                    "ext4_mtime_set failed: path = {}, error = {:?}",
                    path.to_str().unwrap(),
                    err
                );
                Err(err)
            }
        }
    }

    /// Gets the modified time of the inode at the given path.
    pub fn get_mtime(path: &CStr) -> SysResult<u32> {
        let mut mtime = 0;
        let err = unsafe { ext4_mtime_get(path.as_ptr(), &mut mtime) };
        match err {
            0 => Ok(mtime),
            _ => {
                let err = SysError::from_i32(err);
                log::warn!(
                    "ext4_mtime_get failed: path = {}, error = {:?}",
                    path.to_str().unwrap(),
                    err
                );
                Err(err)
            }
        }
    }

    /// Sets the changed time of the inode at the given path.
    pub fn set_ctime(path: &CStr, ctime: u32) -> SysResult<()> {
        let err = unsafe { ext4_ctime_set(path.as_ptr(), ctime) };
        match err {
            0 => Ok(()),
            _ => {
                let err = SysError::from_i32(err);
                log::warn!(
                    "ext4_ctime_set failed: path = {}, error = {:?}",
                    path.to_str().unwrap(),
                    err
                );
                Err(err)
            }
        }
    }

    /// Gets the changed time of the inode at the given path.
    pub fn get_ctime(path: &CStr) -> SysResult<u32> {
        let mut ctime = 0;
        let err = unsafe { ext4_ctime_get(path.as_ptr(), &mut ctime) };
        match err {
            0 => Ok(ctime),
            _ => {
                let err = SysError::from_i32(err);
                log::warn!(
                    "ext4_ctime_get failed: path = {}, error = {:?}",
                    path.to_str().unwrap(),
                    err
                );
                Err(err)
            }
        }
    }

    /// Gets inode by its id.
    ///
    /// You should ensure that "/" root dir exist in fs.
    pub fn get_inode_by_id(inodeid: u32) -> ext4_inode {
        let cstr = CString::new("/").unwrap();
        let d: ExtDir = ExtDir::open(cstr.as_c_str()).unwrap();
        let mp = d.0.f.mp;
        let mut fs = unsafe { (*mp).fs };
        let inode = unsafe { core::mem::zeroed() };
        let ok = unsafe { ext4_fs_get_inode_ref(&mut fs, inodeid, inode) };
        unsafe { *(*inode).inode }
    }
}
