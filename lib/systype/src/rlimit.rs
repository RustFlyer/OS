pub const RLIM_INFINITY: usize = usize::MAX;

/// Resource Limit
///
/// `rlim_cur` is the soft limit to current resource. User can
/// adjust the soft limit by relevant system call.
///
/// `rlim_max` is the hard limit to current resource. Supervisor
/// User can adjust the hard limit but User can not adjust it.
///
/// For convenience, `rlim_max` is set to [`RLIM_INFINITY`] now.
/// Therefore, it's no need to care for it except that Supervisor
/// User trys to adjust it.
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct RLimit {
    /// Soft limit: the kernel enforces for the corresponding resource
    pub rlim_cur: usize,
    /// Hard limit (ceiling for rlim_cur)
    pub rlim_max: usize,
}

impl RLimit {
    pub fn new(rlim_cur: usize) -> Self {
        Self {
            rlim_cur,
            rlim_max: RLIM_INFINITY,
        }
    }
}
