use bitflags::bitflags;
use common::atomic_bitflags;

/// Event types for perf_event_open
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PerfType {
    Hardware = 0,
    Software = 1,
    Tracepoint = 2,
    HwCache = 3,
    Raw = 4,
    Breakpoint = 5,
}

impl PerfType {
    pub fn try_from_u32(value: u32) -> Result<Self, ()> {
        match value {
            0 => Ok(PerfType::Hardware),
            1 => Ok(PerfType::Software),
            2 => Ok(PerfType::Tracepoint),
            3 => Ok(PerfType::HwCache),
            4 => Ok(PerfType::Raw),
            5 => Ok(PerfType::Breakpoint),
            _ => Err(()),
        }
    }
}

/// Hardware event types
#[repr(u64)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PerfHwId {
    CpuCycles = 0,
    Instructions = 1,
    CacheReferences = 2,
    CacheMisses = 3,
    BranchInstructions = 4,
    BranchMisses = 5,
    BusCycles = 6,
    StalledCyclesFrontend = 7,
    StalledCyclesBackend = 8,
    RefCpuCycles = 9,
}

/// Software event types
#[repr(u64)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PerfSwIds {
    CpuClock = 0,
    TaskClock = 1,
    PageFaults = 2,
    ContextSwitches = 3,
    CpuMigrations = 4,
    PageFaultsMin = 5,
    PageFaultsMaj = 6,
    AlignmentFaults = 7,
    EmulationFaults = 8,
    Dummy = 9,
    BpfOutput = 10,
    CgroupSwitches = 11,
}

/// Hardware cache events
#[repr(u64)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PerfHwCacheId {
    L1d = 0,
    L1i = 1,
    Ll = 2,
    Dtlb = 3,
    Itlb = 4,
    Bpu = 5,
    Node = 6,
}

#[repr(u64)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PerfHwCacheOpId {
    Read = 0,
    Write = 1,
    Prefetch = 2,
}

#[repr(u64)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PerfHwCacheOpResultId {
    Access = 0,
    Miss = 1,
}

bitflags! {
    #[derive(Debug, Clone, Copy)]
    pub struct PerfEventAttrFlags: u64 {
        /// off by default
        const DISABLED = 1 << 0;
        /// children inherit it
        const INHERIT = 1 << 1;
        /// must always be on PMU
        const PINNED = 1 << 2;
        /// only group on PMU
        const EXCLUSIVE = 1 << 3;
        /// don't count user
        const EXCLUDE_USER = 1 << 4;
        /// don't count kernel
        const EXCLUDE_KERNEL = 1 << 5;
        /// don't count hypervisor
        const EXCLUDE_HV = 1 << 6;
        /// don't count when idle
        const EXCLUDE_IDLE = 1 << 7;
        /// include mmap data
        const MMAP = 1 << 8;
        /// include comm data
        const COMM = 1 << 9;
        /// use freq, not period
        const FREQ = 1 << 10;
        /// per task counts
        const INHERIT_STAT = 1 << 11;
        /// next exec enables
        const ENABLE_ON_EXEC = 1 << 12;
        /// trace fork/exit
        const TASK = 1 << 13;
        /// wakeup_watermark
        const WATERMARK = 1 << 14;
        /// include precise IP
        const PRECISE_IP_1 = 1 << 15;
        const PRECISE_IP_2 = 1 << 16;
        /// non-exec mmap data
        const MMAP_DATA = 1 << 17;
        /// sample_type all events
        const SAMPLE_ID_ALL = 1 << 18;
        /// don't count in host
        const EXCLUDE_HOST = 1 << 19;
        /// don't count in guest
        const EXCLUDE_GUEST = 1 << 20;
        /// exclude kernel callchains
        const EXCLUDE_CALLCHAIN_KERNEL = 1 << 21;
        /// exclude user callchains
        const EXCLUDE_CALLCHAIN_USER = 1 << 22;
        /// include mmap with inode data
        const MMAP2 = 1 << 23;
        /// flag comm events that are due to an exec
        const COMM_EXEC = 1 << 24;
        /// use @clockid for time fields
        const USE_CLOCKID = 1 << 25;
        /// context switch data
        const CONTEXT_SWITCH = 1 << 26;
        /// Write ring buffer from end to beginning
        const WRITE_BACKWARD = 1 << 27;
        /// include namespaced data
        const NAMESPACES = 1 << 28;
        /// include ksymbol events
        const KSYMBOL = 1 << 29;
        /// include bpf events
        const BPF_EVENT = 1 << 30;
        /// use aux_output for this event
        const AUX_OUTPUT = 1_u64 << 31;
        /// generate cgroup events
        const CGROUP = 1_u64 << 32;
        /// include text poke events
        const TEXT_POKE = 1_u64 << 33;
        /// include build id events
        const BUILD_ID = 1_u64 << 34;
        /// use inherit_thread
        const INHERIT_THREAD = 1_u64 << 35;
        /// remove on exec
        const REMOVE_ON_EXEC = 1_u64 << 36;
        /// include PERF_RECORD_SIGTRAP
        const SIGTRAP = 1_u64 << 37;
    }
}

bitflags! {
    #[derive(Debug, Clone, Copy)]
    pub struct PerfReadFormat: u64 {
        const TOTAL_TIME_ENABLED = 1 << 0;
        const TOTAL_TIME_RUNNING = 1 << 1;
        const ID = 1 << 2;
        const GROUP = 1 << 3;
        const LOST = 1 << 4;
    }
}

bitflags! {
    #[derive(Debug, Clone, Copy)]
    pub struct PerfSampleType: u64 {
        const IP = 1 << 0;
        const TID = 1 << 1;
        const TIME = 1 << 2;
        const ADDR = 1 << 3;
        const READ = 1 << 4;
        const CALLCHAIN = 1 << 5;
        const ID = 1 << 6;
        const CPU = 1 << 7;
        const PERIOD = 1 << 8;
        const STREAM_ID = 1 << 9;
        const RAW = 1 << 10;
        const BRANCH_STACK = 1 << 11;
        const REGS_USER = 1 << 12;
        const STACK_USER = 1 << 13;
        const WEIGHT = 1 << 14;
        const DATA_SRC = 1 << 15;
        const IDENTIFIER = 1 << 16;
        const TRANSACTION = 1 << 17;
        const REGS_INTR = 1 << 18;
        const PHYS_ADDR = 1 << 19;
        const AUX = 1 << 20;
        const CGROUP = 1 << 21;
        const DATA_PAGE_SIZE = 1 << 22;
        const CODE_PAGE_SIZE = 1 << 23;
        const WEIGHT_STRUCT = 1 << 24;
    }
}

bitflags! {
    #[derive(Debug, Clone, Copy)]
    pub struct PerfBranchSampleType: u64 {
        const USER = 1 << 0;
        const KERNEL = 1 << 1;
        const HV = 1 << 2;
        const ANY = 1 << 3;
        const ANY_CALL = 1 << 4;
        const ANY_RETURN = 1 << 5;
        const IND_CALL = 1 << 6;
        const ABORT_TX = 1 << 7;
        const IN_TX = 1 << 8;
        const NO_TX = 1 << 9;
        const COND = 1 << 10;
        const CALL_STACK = 1 << 11;
        const IND_JUMP = 1 << 12;
        const CALL = 1 << 13;
        const NO_FLAGS = 1 << 14;
        const NO_CYCLES = 1 << 15;
        const TYPE_SAVE = 1 << 16;
        const HW_INDEX = 1 << 17;
        const PRIV_SAVE = 1 << 18;
    }
}

// Constants for perf_event_attr size versions
pub const PERF_ATTR_SIZE_VER0: u32 = 64;
pub const PERF_ATTR_SIZE_VER1: u32 = 72;
pub const PERF_ATTR_SIZE_VER2: u32 = 80;
pub const PERF_ATTR_SIZE_VER3: u32 = 96;
pub const PERF_ATTR_SIZE_VER4: u32 = 104;
pub const PERF_ATTR_SIZE_VER5: u32 = 112;
pub const PERF_ATTR_SIZE_VER6: u32 = 120;
pub const PERF_ATTR_SIZE_VER7: u32 = 128;
pub const PERF_ATTR_SIZE_VER8: u32 = 136;

atomic_bitflags!(PerfEventAttrFlags, AtomicU64);
atomic_bitflags!(PerfReadFormat, AtomicU64);
atomic_bitflags!(PerfSampleType, AtomicU64);
atomic_bitflags!(PerfBranchSampleType, AtomicU64);
