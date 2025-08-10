use alloc::{format, string::String, vec::Vec};

pub struct ProcFdInfo {
    pub flags: u32,  // file's flags
    pub pos: u64,    // file's pos
    pub mnt_id: u32, // mount point ID
    pub ino: u64,    // inode id
}

impl ProcFdInfo {
    pub fn new(flags: u32, pos: u64, mnt_id: u32, ino: u64) -> Self {
        ProcFdInfo {
            flags,
            pos,
            mnt_id,
            ino,
        }
    }

    pub fn as_text(&self) -> String {
        format!(
            "pos:\t{}\nflags:\t{:o}\nmnt_id:\t{}\nino:\t{}\n",
            self.pos, self.flags, self.mnt_id, self.ino
        )
    }

    pub fn as_bytes(&self) -> Vec<u8> {
        self.as_text().into_bytes()
    }
}
