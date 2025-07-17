use signal::SigInfo;

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct SignalfdSiginfo {
    pub ssi_signo: u32,
    pub ssi_errno: i32,
    pub ssi_code: i32,
    pub ssi_pid: u32,
    pub ssi_uid: u32,
    pub ssi_fd: i32,
    pub ssi_tid: u32,
    pub ssi_band: u32,
    pub ssi_overrun: u32,
    pub ssi_trapno: u32,
    pub ssi_status: i32,
    pub ssi_int: i32,
    pub ssi_ptr: u64,
    pub ssi_utime: u64,
    pub ssi_stime: u64,
    pub ssi_addr: u64,
    pub _pad: [u8; 48],
}

impl Default for SignalfdSiginfo {
    fn default() -> Self {
        SignalfdSiginfo {
            ssi_signo: 0,
            ssi_errno: 0,
            ssi_code: 0,
            ssi_pid: 0,
            ssi_uid: 0,
            ssi_fd: 0,
            ssi_tid: 0,
            ssi_band: 0,
            ssi_overrun: 0,
            ssi_trapno: 0,
            ssi_status: 0,
            ssi_int: 0,
            ssi_ptr: 0,
            ssi_utime: 0,
            ssi_stime: 0,
            ssi_addr: 0,
            _pad: [0u8; 48],
        }
    }
}

impl From<&SigInfo> for SignalfdSiginfo {
    fn from(info: &SigInfo) -> Self {
        SignalfdSiginfo {
            ssi_signo: info.sig.raw() as u32,
            ssi_errno: 0,
            ssi_code: info.code,
            ssi_pid: info.details.get_sender_pid() as u32,
            ssi_uid: info.details.get_sender_pid() as u32,
            ssi_fd: 0,
            ssi_tid: 0,
            ssi_band: 0,
            ssi_overrun: 0,
            ssi_trapno: 0,
            ssi_status: 0,
            ssi_int: info.details.get_val() as i32,
            ssi_ptr: 0,
            ssi_utime: 0,
            ssi_stime: 0,
            ssi_addr: 0,
            _pad: [0u8; 48],
        }
    }
}
