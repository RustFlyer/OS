#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct MsgHdr {
    pub msg_name: usize,       // 目标地址指针
    pub msg_namelen: u32,      // 地址长度
    pub msg_iov: usize,        // iovec 数组指针
    pub msg_iovlen: usize,     // iovec 数组长度
    pub msg_control: usize,    // 辅助数据指针
    pub msg_controllen: usize, // 辅助数据长度
    pub msg_flags: i32,        // 标志
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct MmsgHdr {
    pub msg_hdr: MsgHdr, // 消息头
    pub msg_len: u32,    // 发送/接收的字节数
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct IoVec {
    pub iov_base: usize, // 数据缓冲区指针
    pub iov_len: usize,  // 缓冲区长度
}
