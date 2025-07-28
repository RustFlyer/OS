use bitflags::bitflags;
use common::atomic_bitflags;

bitflags! {
    #[derive(Debug, Clone, Copy)]
    pub struct BpfProgramType: u32 {
        /// Unspecified program type
        const BPF_PROG_TYPE_UNSPEC = 0;
        /// Socket filter program
        const BPF_PROG_TYPE_SOCKET_FILTER = 1;
        /// Kernel probe program
        const BPF_PROG_TYPE_KPROBE = 2;
        /// Scheduler classifier
        const BPF_PROG_TYPE_SCHED_CLS = 3;
        /// Scheduler action
        const BPF_PROG_TYPE_SCHED_ACT = 4;
        /// Tracepoint program
        const BPF_PROG_TYPE_TRACEPOINT = 5;
        /// XDP program
        const BPF_PROG_TYPE_XDP = 6;
        /// Perf event program
        const BPF_PROG_TYPE_PERF_EVENT = 7;
        /// cGroup socket program
        const BPF_PROG_TYPE_CGROUP_SOCKET = 8;
        /// cGroup skb program
        const BPF_PROG_TYPE_CGROUP_SKB = 9;
        /// Socket operations program
        const BPF_PROG_TYPE_SOCK_OPS = 10;
        /// SK SKB program
        const BPF_PROG_TYPE_SK_SKB = 11;
        /// cGroup device program
        const BPF_PROG_TYPE_CGROUP_DEVICE = 12;
        /// SK MSG program
        const BPF_PROG_TYPE_SK_MSG = 13;
        /// Raw tracepoint program
        const BPF_PROG_TYPE_RAW_TRACEPOINT = 14;
        /// cGroup sockaddr program
        const BPF_PROG_TYPE_CGROUP_SOCKADDR = 15;
        /// LWT IN program
        const BPF_PROG_TYPE_LWT_IN = 16;
        /// LWT OUT program
        const BPF_PROG_TYPE_LWT_OUT = 17;
        /// LWT XMIT program
        const BPF_PROG_TYPE_LWT_XMIT = 18;
        /// LWT SEG6LOCAL program
        const BPF_PROG_TYPE_LWT_SEG6LOCAL = 19;
        /// lirc mode2 program
        const BPF_PROG_TYPE_LIRC_MODE2 = 20;
        /// SK REUSEPORT program
        const BPF_PROG_TYPE_SK_REUSEPORT = 21;
        /// Flow dissector program
        const BPF_PROG_TYPE_FLOW_DISSECTOR = 22;
        /// cGroup syscall program
        const BPF_PROG_TYPE_CGROUP_SYSCALL = 23;
        /// Raw tracepoint writable program
        const BPF_PROG_TYPE_RAW_TRACEPOINT_WRITABLE = 24;
        /// cGroup sockopt program
        const BPF_PROG_TYPE_CGROUP_SOCKOPT = 25;
        /// Tracing program
        const BPF_PROG_TYPE_TRACING = 26;
        /// Struct ops program
        const BPF_PROG_TYPE_STRUCT_OPS = 27;
        /// Extension program
        const BPF_PROG_TYPE_EXT = 28;
        /// LSM program
        const BPF_PROG_TYPE_LSM = 29;
        /// SK lookup program
        const BPF_PROG_TYPE_SK_LOOKUP = 30;
        /// Syscall program
        const BPF_PROG_TYPE_SYSCALL = 31;
    }
}

bitflags! {
    #[derive(Debug, Clone, Copy)]
    pub struct BpfMapType: u32 {
        /// Unspecified map type
        const BPF_MAP_TYPE_UNSPEC = 0;
        /// Hash map
        const BPF_MAP_TYPE_HASH = 1;
        /// Array map
        const BPF_MAP_TYPE_ARRAY = 2;
        /// Program array
        const BPF_MAP_TYPE_PROG_ARRAY = 3;
        /// Perf event array
        const BPF_MAP_TYPE_PERF_EVENT_ARRAY = 4;
        /// Per-CPU hash map
        const BPF_MAP_TYPE_PERCPU_HASH = 5;
        /// Per-CPU array
        const BPF_MAP_TYPE_PERCPU_ARRAY = 6;
        /// Stack trace
        const BPF_MAP_TYPE_STACK_TRACE = 7;
        /// cGroup array
        const BPF_MAP_TYPE_CGROUP_ARRAY = 8;
        /// LRU hash
        const BPF_MAP_TYPE_LRU_HASH = 9;
        /// LRU per-CPU hash
        const BPF_MAP_TYPE_LRU_PERCPU_HASH = 10;
        /// LPM trie
        const BPF_MAP_TYPE_LPM_TRIE = 11;
        /// Array of maps
        const BPF_MAP_TYPE_ARRAY_OF_MAPS = 12;
        /// Hash of maps
        const BPF_MAP_TYPE_HASH_OF_MAPS = 13;
        /// Device map
        const BPF_MAP_TYPE_DEVMAP = 14;
        /// Socket map
        const BPF_MAP_TYPE_SOCKMAP = 15;
        /// CPU map
        const BPF_MAP_TYPE_CPUMAP = 16;
        /// XDP socket map
        const BPF_MAP_TYPE_XSKMAP = 17;
        /// Socket hash map
        const BPF_MAP_TYPE_SOCKHASH = 18;
        /// cGroup storage
        const BPF_MAP_TYPE_CGROUP_STORAGE = 19;
        /// REUSEPORT socket array
        const BPF_MAP_TYPE_REUSEPORT_SOCKARRAY = 20;
        /// Per-CPU cGroup storage
        const BPF_MAP_TYPE_PERCPU_CGROUP_STORAGE = 21;
        /// Queue
        const BPF_MAP_TYPE_QUEUE = 22;
        /// Stack
        const BPF_MAP_TYPE_STACK = 23;
        /// SK storage
        const BPF_MAP_TYPE_SK_STORAGE = 24;
        /// Device hash map
        const BPF_MAP_TYPE_DEVMAP_HASH = 25;
        /// Struct ops
        const BPF_MAP_TYPE_STRUCT_OPS = 26;
        /// Ring buffer
        const BPF_MAP_TYPE_RINGBUF = 27;
        /// Inode storage
        const BPF_MAP_TYPE_INODE_STORAGE = 28;
        /// Task storage
        const BPF_MAP_TYPE_TASK_STORAGE = 29;
        /// Bloom filter
        const BPF_MAP_TYPE_BLOOM_FILTER = 30;
    }
}

bitflags! {
    #[derive(Debug, Clone, Copy)]
    pub struct BpfProgramFlags: u32 {
        /// Strict alignment
        const BPF_F_STRICT_ALIGNMENT = 0x01;
        /// Any alignment
        const BPF_F_ANY_ALIGNMENT = 0x02;
        /// Test state changes
        const BPF_F_TEST_STATE_FREQ = 0x08;
        /// Sleepable program
        const BPF_F_SLEEPABLE = 0x10;
        /// XDP has frags
        const BPF_F_XDP_HAS_FRAGS = 0x20;
        /// XDP dev bound only
        const BPF_F_XDP_DEV_BOUND_ONLY = 0x40;
    }
}

bitflags! {
    #[derive(Debug, Clone, Copy)]
    pub struct BpfMapFlags: u32 {
        /// No prealloc
        const BPF_F_NO_PREALLOC = 0x01;
        /// No common LRU
        const BPF_F_NO_COMMON_LRU = 0x02;
        /// Numa node
        const BPF_F_NUMA_NODE = 0x04;
        /// Read only
        const BPF_F_RDONLY = 0x08;
        /// Write only
        const BPF_F_WRONLY = 0x10;
        /// Stack build id
        const BPF_F_STACK_BUILD_ID = 0x20;
        /// Zero seed
        const BPF_F_ZERO_SEED = 0x40;
        /// Read only prog
        const BPF_F_RDONLY_PROG = 0x80;
        /// Write only prog
        const BPF_F_WRONLY_PROG = 0x100;
        /// Clone
        const BPF_F_CLONE = 0x200;
        /// MMAPABLE
        const BPF_F_MMAPABLE = 0x400;
        /// Preserve elems
        const BPF_F_PRESERVE_ELEMS = 0x800;
        /// Inner map
        const BPF_F_INNER_MAP = 0x1000;
        /// Link
        const BPF_F_LINK = 0x2000;
        /// Path FD
        const BPF_F_PATH_FD = 0x4000;
        /// vmprot exec
        const BPF_F_VMPROT_EXEC = 0x8000;
        /// Token FD
        const BPF_F_TOKEN_FD = 0x10000;
        /// Segv on fault
        const BPF_F_SEGV_ON_FAULT = 0x20000;
        /// No user conv
        const BPF_F_NO_USER_CONV = 0x40000;
    }
}

/// BPF commands
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BpfCommand {
    /// Create a map
    BpfMapCreate = 0,
    /// Lookup element in map
    BpfMapLookupElem = 1,
    /// Update element in map
    BpfMapUpdateElem = 2,
    /// Delete element from map
    BpfMapDeleteElem = 3,
    /// Get next key in map
    BpfMapGetNextKey = 4,
    /// Load a program
    BpfProgLoad = 5,
    /// Pin object to filesystem
    BpfObjPin = 6,
    /// Get object from filesystem
    BpfObjGet = 7,
    /// Attach program
    BpfProgAttach = 8,
    /// Detach program
    BpfProgDetach = 9,
    /// Test run program
    BpfProgTestRun = 10,
    /// Query programs
    BpfProgQuery = 11,
    /// Get next map
    BpfMapGetNextId = 12,
    /// Get next program
    BpfProgGetNextId = 13,
    /// Get map by id
    BpfMapGetFdById = 14,
    /// Get program by id
    BpfProgGetFdById = 15,
    /// Get object info
    BpfObjGetInfoByFd = 16,
    /// Query programs
    BpfProgQuery2 = 17,
    /// Get raw tracepoint
    BpfRawTracepointOpen = 18,
    /// Get BTF info
    BpfBtfLoad = 19,
    /// Get BTF by id
    BpfBtfGetFdById = 20,
    /// Get next BTF
    BpfTaskFdQuery = 21,
    /// Freeze map
    BpfMapFreeze = 22,
    /// Get next BTF
    BpfBtfGetNextId = 23,
    /// Lookup batch
    BpfMapLookupBatch = 24,
    /// Lookup and delete batch
    BpfMapLookupAndDeleteBatch = 25,
    /// Update batch
    BpfMapUpdateBatch = 26,
    /// Delete batch
    BpfMapDeleteBatch = 27,
    /// Create link
    BpfLinkCreate = 28,
    /// Update link
    BpfLinkUpdate = 29,
    /// Get link by id
    BpfLinkGetFdById = 30,
    /// Get next link
    BpfLinkGetNextId = 31,
    /// Enable stats
    BpfEnableStats = 32,
    /// Create iter
    BpfIterCreate = 33,
    /// Detach link
    BpfLinkDetach = 34,
    /// Bind program to map
    BpfProgBindMap = 35,
}

atomic_bitflags!(BpfProgramType, AtomicU32);
atomic_bitflags!(BpfMapType, AtomicU32);
atomic_bitflags!(BpfProgramFlags, AtomicU32);
atomic_bitflags!(BpfMapFlags, AtomicU32);
