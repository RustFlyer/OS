use strum::FromRepr;

#[derive(FromRepr, Debug)]
#[repr(usize)]
pub enum BlkIoctlCmd {
    BLKGETSIZE64 = 0x80081272, // 获取设备字节数 (u64*)
    BLKSSZGET = 0x1268,        // 获取扇区大小 (u32*)
    BLKGETSIZE = 0x1260,       // 获取512字节块数 (u32*)
    BLKFLSBUF = 0x1261,        // 刷新缓冲区
}
