use arch::time::get_time_ms;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ShmStat {
    // Ownership and permissions
    pub perm: ShmPerm,
    // Size of segment (bytes). In our system, this must be aligned
    pub segsz: usize,
    // Last attach time
    pub atime: usize,
    // Last detach time
    pub dtime: usize,
    // Creation time/time of last modification via shmctl()
    pub ctime: usize,
    // PID of creator
    pub cpid: usize,
    // PID of last shmat(2)/shmdt(2)
    pub lpid: usize,
    // No. of current attaches
    pub nattch: usize,
}

impl ShmStat {
    pub fn new(sz: usize, cpid: usize) -> Self {
        Self {
            perm: ShmPerm::default(),
            segsz: sz,
            atime: 0,
            dtime: 0,
            ctime: get_time_ms() / 1000,
            cpid,
            lpid: 0,
            nattch: 0,
        }
    }

    pub fn attach(&mut self, lpid: usize) {
        // atime is set to the current time.
        self.atime = get_time_ms() / 1000;
        // lpid is set to the process-ID of the calling process.
        self.lpid = lpid;
        // nattch is incremented by one.
        self.nattch += 1;
    }

    /// return whether the SHARED_MEMORY_MANAGER should remove the SharedMemory
    /// which self ShmStat belongs to;
    pub fn detach(&mut self, lpid: usize) -> bool {
        // dtime is set to the current time.
        self.dtime = get_time_ms() / 1000;
        // lpid is set to the process-ID of the calling process.
        self.lpid = lpid;
        // nattch is decremented by one.
        self.nattch -= 1;
        //debug_assert!(self.nattch >= 0);
        if self.nattch == 0 {
            return true;
        }
        false
    }
}

#[repr(C)]
#[derive(Default, Clone, Copy, Debug)]
pub struct ShmPerm {
    key: i32,
    uid: u32,
    gid: u32,
    cuid: u32,
    cgid: u32,
    mode: u16,
    seq: u16,
}
