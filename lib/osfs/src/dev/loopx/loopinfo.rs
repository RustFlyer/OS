use strum::FromRepr;

#[derive(FromRepr, Debug)]
#[repr(usize)]
pub enum LoopIoctlCmd {
    SETFD = 0x4c00,
    CLRFD = 0x4c01,
    GETSTATUS = 0x4c02,
    SETSTATUS = 0x4c03,
    GETSTATUS64 = 0x4c05,
    SETSTATUS64 = 0x4c04,
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct LoopInfo64 {
    // refer to Linux include/uapi/linux/loop.h
    pub lo_device: u64,
    pub lo_inode: u64,
    pub lo_rdevice: u64,
    pub lo_offset: u64,
    pub lo_sizelimit: u64,
    pub lo_number: u32,
    pub lo_encrypt_type: u32,
    pub lo_encrypt_key_size: u32,
    pub lo_flags: u32,
    pub lo_file_name: [u8; 64],
    pub lo_crypt_name: [u8; 64],
    pub lo_encrypt_key: [u8; 32],
    pub lo_init: [u64; 2],
}

impl Default for LoopInfo64 {
    fn default() -> Self {
        Self {
            lo_device: 0,
            lo_inode: 0,
            lo_rdevice: 0,
            lo_offset: 0,
            lo_sizelimit: 0,
            lo_number: 0,
            lo_encrypt_type: 0,
            lo_encrypt_key_size: 0,
            lo_flags: 0,
            lo_file_name: [0; 64],
            lo_crypt_name: [0; 64],
            lo_encrypt_key: [0; 32],
            lo_init: [0; 2],
        }
    }
}
