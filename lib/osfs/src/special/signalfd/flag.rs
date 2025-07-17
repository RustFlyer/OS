use bitflags::bitflags;

bitflags! {
    pub struct SignalFdFlags: u32 {
        const CLOEXEC   = 0o2000000; // 02000000, Linux SFD_CLOEXEC
        const NONBLOCK  = 0o0004000; // 00004000, Linux SFD_NONBLOCK
    }
}
