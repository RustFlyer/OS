/// TMS is a struct used for organizing time data. It's just
/// used in syscall `sys_times` to pass message from kernel
/// to user space.
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct TMS {
    pub tms_utime: usize,
    pub tms_stime: usize,
    pub tms_cutime: usize,
    pub tms_cstime: usize,
}

impl TMS {
    pub fn new(utime: usize, stime: usize, cutime: usize, cstime: usize) -> Self {
        Self {
            tms_utime: utime,
            tms_stime: stime,
            tms_cutime: cutime,
            tms_cstime: cstime,
        }
    }
}
