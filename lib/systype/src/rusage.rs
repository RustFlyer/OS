use crate::time::TimeVal;

/// Rusage is resource usage statistics. It records various data of resource
/// used by a running process.
///```rust
///pub struct Rusage {
///    pub utime: TimeVal, // This is the total amount of time spent executing in user mode
///
///    pub stime: TimeVal, // This is the total amount of time spent
///                        // executing in kernel mode
///
///    pub maxrss: usize,  // maximum resident set size(maximum number
///                        // of physical pages in memory)
///
///    // pub ixrss: usize,    In modern systems, this field is usually no longer used
///    // pub idrss: usize,    In modern systems, this field is usually no longer used
///    // pub isrss: usize,    In modern systems, this field is usually no longer used
///
///    pub minflt: usize,  // page reclaims (soft page faults, just try to
///                        // remap in memory, not access hard disk)
///
///    pub majflt: usize,  // page faults (hard page faults, access hard disk)
///
///    pub nswap: usize,   // swaps (for modern system with page mechanism,
///                        // this rate is replaced by minjlt/maxjlt)
///
///    pub inblock: usize, // number of block input operations
///
///    pub oublock: usize, // number of block output operations
///
///    // pub msgsnd: usize,   In modern systems, this field is usually no longer used
///    // pub msgrcv: usize,   In modern systems, this field is usually no longer used
///    // pub nsignals: usize, In modern systems, this field is usually no longer used
///
///    pub nvcsw: usize,   // voluntary context switches
///
///    pub nivcsw: usize,  // involuntary context switches(now unuse)
///}
///```
#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
pub struct Rusage {
    pub utime: TimeVal,
    pub stime: TimeVal,
    pub maxrss: usize,
    pub ixrss: usize,
    pub idrss: usize,
    pub isrss: usize,
    pub minflt: usize,
    pub majflt: usize,
    pub nswap: usize,
    pub inblock: usize,
    pub oublock: usize,
    pub msgsnd: usize,
    pub msgrcv: usize,
    pub nsignals: usize,
    pub nvcsw: usize,
    pub nivcsw: usize,
}
