use alloc::string::String;
use config::inode::InodeType;

/// Directory entry.
#[derive(Debug, Clone)]
#[repr(C)]
pub struct DirEntry {
    pub ino: u64,
    pub off: u64,
    pub itype: InodeType,
    pub name: String,
}
