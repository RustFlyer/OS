use strum::FromRepr;

#[derive(FromRepr, Debug)]
#[repr(u32)]
pub enum LoopIoctlCmd {
    SETFD = 0x4c00,
    CLRFD = 0x4c01,
    SETSTATUS = 0x4c02,
    GETSTATUS = 0x4c03,
    SETSTATUS64 = 0x4c04,
    GETSTATUS64 = 0x4c05,
    CHANGEFE = 0x4C06,     // LOOP_CHANGE_FD
    SETCAPACITY = 0x4C07,  // LOOP_SET_CAPACITY
    SETDIRECT = 0x4C08,    // LOOP_SET_DIRECT_IO
    SETBLOCKSIZE = 0x4C09, // LOOP_SET_BLOCK_SIZE
    CONFIGURE = 0x4C0A,    // LOOP_CONFIGURE
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

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct LoopInfo {
    pub lo_number: u32,
    pub lo_device: u16,
    pub lo_inode: u32,
    pub lo_rdevice: u16,
    pub lo_offset: u32,
    pub lo_encrypt_type: u32,
    pub lo_encrypt_key_size: u32,
    pub lo_flags: u32,
    pub lo_name: [u8; 64],
    pub lo_encrypt_key: [u8; 32],
    pub lo_init: [u64; 2],
}

impl Default for LoopInfo {
    fn default() -> Self {
        Self {
            lo_number: 0,
            lo_device: 0,
            lo_inode: 0,
            lo_rdevice: 0,
            lo_offset: 0,
            lo_encrypt_type: 0,
            lo_encrypt_key_size: 0,
            lo_flags: 0,
            lo_name: [0; 64],
            lo_encrypt_key: [0; 32],
            lo_init: [0; 2],
        }
    }
}

pub const LO_FLAGS_READ_ONLY: u32 = 1;
pub const LO_FLAGS_AUTOCLEAR: u32 = 4;
pub const LO_FLAGS_PARTSCAN: u32 = 8;
pub const LO_FLAGS_DIRECT_IO: u32 = 16;
