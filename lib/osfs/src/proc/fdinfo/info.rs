use alloc::{format, string::String, vec::Vec};

pub struct ProcFdInfo {
    pub flags: u32,  // file's flag
    pub pos: u64,    // file's pos
    pub minflt: u64, // mini-file err cnt
    pub majflt: u64, // max-file err cnt
    pub nflock: u32, // file lock cnt
}

impl ProcFdInfo {
    pub fn new(flags: u32, pos: u64, minflt: u64, majflt: u64, nflock: u32) -> Self {
        ProcFdInfo {
            flags,
            pos,
            minflt,
            majflt,
            nflock,
        }
    }

    pub fn as_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&self.flags.to_le_bytes());
        bytes.extend_from_slice(&self.pos.to_le_bytes());
        bytes.extend_from_slice(&self.minflt.to_le_bytes());
        bytes.extend_from_slice(&self.majflt.to_le_bytes());
        bytes.extend_from_slice(&self.nflock.to_le_bytes());
        bytes
    }

    pub fn as_text(&self) -> String {
        format!(
            "flags: {:#x}\npos: {}\nminflt: {}\nmajflt: {}\nnflock: {}\n",
            self.flags, self.pos, self.minflt, self.majflt, self.nflock
        )
    }
}
