#[repr(C)]
#[derive(Debug, Default, Clone, Copy)]
pub struct CapUserHeader {
    pub version: u32,
    pub pid: i32,
}

#[repr(C)]
#[derive(Debug, Default, Clone, Copy)]
pub struct CapUserData {
    pub effective: u32,
    pub permitted: u32,
    pub inheritable: u32,
}

pub const _LINUX_CAPABILITY_VERSION_1: u32 = 0x19980330;
pub const _LINUX_CAPABILITY_VERSION_2: u32 = 0x20071026;
pub const _LINUX_CAPABILITY_VERSION_3: u32 = 0x20080522;
pub const CAPABILITY_U32S_1: usize = 1;
pub const CAPABILITY_U32S_2: usize = 2;
pub const CAPABILITY_U32S_3: usize = 2; // Linux 3/4/5 use 2

#[derive(Debug, Clone, Default)]
pub struct Capabilities {
    pub effective: [u32; 2],
    pub permitted: [u32; 2],
    pub inheritable: [u32; 2],
}

impl Capabilities {
    pub fn new() -> Self {
        let all_caps = [u32::MAX, u32::MAX];

        Self {
            effective: all_caps,
            permitted: all_caps,
            inheritable: [0, 0],
        }
    }
}
