use bitflags::bitflags;
use common::atomic_bitflags;

bitflags! {
    #[derive(Debug, Clone, Copy)]
    pub struct IoUringSetupFlags: u32 {
        /// Use io_uring polled mode
        const IORING_SETUP_IOPOLL = 1 << 0;
        /// SQ thread will perform submission queue polling
        const IORING_SETUP_SQPOLL = 1 << 1;
        /// Set SQ thread CPU affinity
        const IORING_SETUP_SQ_AFF = 1 << 2;
        /// App and kernel share submission queue
        const IORING_SETUP_CQSIZE = 1 << 3;
        /// Clamp submission queue size
        const IORING_SETUP_CLAMP = 1 << 4;
        /// Attach to existing workqueue
        const IORING_SETUP_ATTACH_WQ = 1 << 5;
        /// Start the rings disabled
        const IORING_SETUP_R_DISABLED = 1 << 6;
        /// Continue submit on error
        const IORING_SETUP_SUBMIT_ALL = 1 << 7;
        /// Cooperative task running
        const IORING_SETUP_COOP_TASKRUN = 1 << 8;
        /// Task run flag
        const IORING_SETUP_TASKRUN_FLAG = 1 << 9;
        /// Single issuer
        const IORING_SETUP_SINGLE_ISSUER = 1 << 10;
        /// Defer task work
        const IORING_SETUP_DEFER_TASKRUN = 1 << 11;
        /// Skip CQE posting for successful operations
        const IORING_SETUP_CQE_32 = 1 << 12;
        /// Single mmap for SQ and CQ rings
        const IORING_SETUP_SINGLE_MMAP = 1 << 13;
        /// Register ring fd
        const IORING_SETUP_REGISTERED_FD_ONLY = 1 << 14;
    }
}

bitflags! {
    #[derive(Debug, Clone, Copy)]
    pub struct IoUringEnterFlags: u32 {
        /// Get events
        const IORING_ENTER_GETEVENTS = 1 << 0;
        /// SQ thread is polling
        const IORING_ENTER_SQ_WAKEUP = 1 << 1;
        /// SQ thread is sleeping, need wakeup
        const IORING_ENTER_SQ_WAIT = 1 << 2;
        /// Extend argument
        const IORING_ENTER_EXT_ARG = 1 << 3;
        /// Register ring FD
        const IORING_ENTER_REGISTERED_RING = 1 << 4;
    }
}

bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq)]
    pub struct IoUringRegisterOp: u32 {
        /// Register buffers
        const IORING_REGISTER_BUFFERS = 0;
        /// Unregister buffers
        const IORING_UNREGISTER_BUFFERS = 1;
        /// Register files
        const IORING_REGISTER_FILES = 2;
        /// Unregister files
        const IORING_UNREGISTER_FILES = 3;
        /// Register eventfd
        const IORING_REGISTER_EVENTFD = 4;
        /// Unregister eventfd
        const IORING_UNREGISTER_EVENTFD = 5;
        /// Register files update
        const IORING_REGISTER_FILES_UPDATE = 6;
        /// Register eventfd async
        const IORING_REGISTER_EVENTFD_ASYNC = 7;
        /// Register probe
        const IORING_REGISTER_PROBE = 8;
        /// Register personality
        const IORING_REGISTER_PERSONALITY = 9;
        /// Unregister personality
        const IORING_UNREGISTER_PERSONALITY = 10;
        /// Register restrictions
        const IORING_REGISTER_RESTRICTIONS = 11;
        /// Enable rings
        const IORING_REGISTER_ENABLE_RINGS = 12;
        /// Register files update
        const IORING_REGISTER_FILES2 = 13;
        /// Register files update
        const IORING_REGISTER_FILES_UPDATE2 = 14;
        /// Register buffers update
        const IORING_REGISTER_BUFFERS2 = 15;
        /// Register buffers update
        const IORING_REGISTER_BUFFERS_UPDATE = 16;
        /// Register io_uring file descriptors
        const IORING_REGISTER_IOWQ_AFF = 17;
        /// Unregister io_uring file descriptors
        const IORING_UNREGISTER_IOWQ_AFF = 18;
        /// Set io_uring max workers
        const IORING_REGISTER_IOWQ_MAX_WORKERS = 19;
        /// Register ring file descriptors
        const IORING_REGISTER_RING_FDS = 20;
        /// Unregister ring file descriptors
        const IORING_UNREGISTER_RING_FDS = 21;
        /// Register buffers sparse
        const IORING_REGISTER_PBUF_RING = 22;
        /// Unregister buffers sparse
        const IORING_UNREGISTER_PBUF_RING = 23;
        /// Sync file range
        const IORING_REGISTER_SYNC_CANCEL = 24;
        /// Register file allocation range
        const IORING_REGISTER_FILE_ALLOC_RANGE = 25;
    }
}

bitflags! {
    #[derive(Debug, Clone, Copy)]
    pub struct IoUringOpcode: u8 {
        const IORING_OP_NOP = 0;
        const IORING_OP_READV = 1;
        const IORING_OP_WRITEV = 2;
        const IORING_OP_FSYNC = 3;
        const IORING_OP_READ_FIXED = 4;
        const IORING_OP_WRITE_FIXED = 5;
        const IORING_OP_POLL_ADD = 6;
        const IORING_OP_POLL_REMOVE = 7;
        const IORING_OP_SYNC_FILE_RANGE = 8;
        const IORING_OP_SENDMSG = 9;
        const IORING_OP_RECVMSG = 10;
        const IORING_OP_TIMEOUT = 11;
        const IORING_OP_TIMEOUT_REMOVE = 12;
        const IORING_OP_ACCEPT = 13;
        const IORING_OP_ASYNC_CANCEL = 14;
        const IORING_OP_LINK_TIMEOUT = 15;
        const IORING_OP_CONNECT = 16;
        const IORING_OP_FALLOCATE = 17;
        const IORING_OP_OPENAT = 18;
        const IORING_OP_CLOSE = 19;
        const IORING_OP_FILES_UPDATE = 20;
        const IORING_OP_STATX = 21;
        const IORING_OP_READ = 22;
        const IORING_OP_WRITE = 23;
        const IORING_OP_FADVISE = 24;
        const IORING_OP_MADVISE = 25;
        const IORING_OP_SEND = 26;
        const IORING_OP_RECV = 27;
        const IORING_OP_OPENAT2 = 28;
        const IORING_OP_EPOLL_CTL = 29;
        const IORING_OP_SPLICE = 30;
        const IORING_OP_PROVIDE_BUFFERS = 31;
        const IORING_OP_REMOVE_BUFFERS = 32;
        const IORING_OP_TEE = 33;
        const IORING_OP_SHUTDOWN = 34;
        const IORING_OP_RENAMEAT = 35;
        const IORING_OP_UNLINKAT = 36;
        const IORING_OP_MKDIRAT = 37;
        const IORING_OP_SYMLINKAT = 38;
        const IORING_OP_LINKAT = 39;
        const IORING_OP_MSG_RING = 40;
        const IORING_OP_FSETXATTR = 41;
        const IORING_OP_SETXATTR = 42;
        const IORING_OP_FGETXATTR = 43;
        const IORING_OP_GETXATTR = 44;
        const IORING_OP_SOCKET = 45;
        const IORING_OP_URING_CMD = 46;
        const IORING_OP_SEND_ZC = 47;
        const IORING_OP_SENDMSG_ZC = 48;
    }
}

bitflags! {
    #[derive(Debug, Clone, Copy)]
    pub struct IoUringSqeFlags: u8 {
        /// Use fixed fileset
        const IOSQE_FIXED_FILE = 1 << 0;
        /// Issue after inflight IO
        const IOSQE_IO_DRAIN = 1 << 1;
        /// Link next sqe
        const IOSQE_IO_LINK = 1 << 2;
        /// Like LINK, but stronger
        const IOSQE_IO_HARDLINK = 1 << 3;
        /// Always go async
        const IOSQE_ASYNC = 1 << 4;
        /// Select buffer from buffer group
        const IOSQE_BUFFER_SELECT = 1 << 5;
        /// Don't post CQE if request succeeded
        const IOSQE_CQE_SKIP_SUCCESS = 1 << 6;
    }
}

atomic_bitflags!(IoUringSetupFlags, AtomicU32);
atomic_bitflags!(IoUringEnterFlags, AtomicU32);
atomic_bitflags!(IoUringRegisterOp, AtomicU32);
atomic_bitflags!(IoUringSqeFlags, AtomicU8);
