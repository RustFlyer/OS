use bitflags::bitflags;

bitflags! {
    pub struct TimerFdFlags: u32 {
        const CLOEXEC   = 0o2000000;
        const NONBLOCK  = 0o0004000;
    }
}
