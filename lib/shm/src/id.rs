use arch::riscv64::time::get_time_s;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ShmIdDs {
    // Ownership and permissions
    pub shm_perm: IpcPerm,
    // Size of segment (bytes). In our system, this must be aligned
    pub shm_segsz: usize,
    // Last attach time
    pub shm_atime: usize,
    // Last detach time
    pub shm_dtime: usize,
    // Creation time/time of last modification via shmctl()
    pub shm_ctime: usize,
    // PID of creator
    pub shm_cpid: usize,
    // PID of last shmat(2)/shmdt(2)
    pub shm_lpid: usize,
    // No. of current attaches
    pub shm_nattch: usize,
}

impl ShmIdDs {
    pub fn new(sz: usize, cpid: usize) -> Self {
        Self {
            shm_perm: IpcPerm::default(),
            shm_segsz: sz,
            shm_atime: 0,
            shm_dtime: 0,
            shm_ctime: get_time_s(),
            shm_cpid: cpid,
            shm_lpid: 0,
            shm_nattch: 0,
        }
    }

    pub fn attach(&mut self, lpid: usize) {
        // shm_atime is set to the current time.
        self.shm_atime = get_time_s();
        // shm_lpid is set to the process-ID of the calling process.
        self.shm_lpid = lpid;
        // shm_nattch is incremented by one.
        self.shm_nattch += 1;
    }

    /// return whether the SHARED_MEMORY_MANAGER should remove the SharedMemory
    /// which self ShmIdDs belongs to;
    pub fn detach(&mut self, lpid: usize) -> bool {
        // shm_dtime is set to the current time.
        self.shm_dtime = get_time_s();
        // shm_lpid is set to the process-ID of the calling process.
        self.shm_lpid = lpid;
        // shm_nattch is decremented by one.
        self.shm_nattch -= 1;
        //debug_assert!(self.shm_nattch >= 0);
        if self.shm_nattch == 0 {
            return true;
        }
        false
    }
}

#[repr(C)]
#[derive(Default, Clone, Copy, Debug)]
pub struct IpcPerm {
    key: i32,
    uid: u32,
    gid: u32,
    cuid: u32,
    cgid: u32,
    mode: u16,
    seq: u16,
}


