use bitflags::bitflags;
use common::atomic_bitflags;

bitflags! {
    #[derive(Debug, Clone, Copy)]
    pub struct UserfaultfdFlags: u32 {
        /// Create file descriptor with close-on-exec flag set
        const UFFD_CLOEXEC = 0x80000;
        /// Create file descriptor with non-blocking flag set
        const UFFD_NONBLOCK = 0x800;
        /// Restrict to handle user faults only (no kernel faults)
        const UFFD_USER_MODE_ONLY = 0x1;
    }
}

bitflags! {
    #[derive(Debug, Clone, Copy)]
    pub struct UserfaultfdFeatures: u64 {
        /// Basic pagefault handling
        const UFFD_FEATURE_PAGEFAULT_FLAG_WP = 1 << 0;
        /// Support for events (fork, remap, remove, unmap)
        const UFFD_FEATURE_EVENT_FORK = 1 << 1;
        const UFFD_FEATURE_EVENT_REMAP = 1 << 2;
        const UFFD_FEATURE_EVENT_REMOVE = 1 << 3;
        const UFFD_FEATURE_EVENT_UNMAP = 1 << 4;
        /// Support for missing mode on hugetlbfs
        const UFFD_FEATURE_MISSING_HUGETLBFS = 1 << 5;
        /// Support for missing mode on shmem
        const UFFD_FEATURE_MISSING_SHMEM = 1 << 6;
        /// Support for minor mode on hugetlbfs
        const UFFD_FEATURE_MINOR_HUGETLBFS = 1 << 7;
        /// Support for minor mode on shmem
        const UFFD_FEATURE_MINOR_SHMEM = 1 << 8;
        /// Support for SIGBUS feature
        const UFFD_FEATURE_SIGBUS = 1 << 9;
        /// Support for thread ID in events
        const UFFD_FEATURE_THREAD_ID = 1 << 10;
    }
}

bitflags! {
    #[derive(Debug, Clone, Copy)]
    pub struct UserfaultfdRegisterMode: u64 {
        /// Register for missing page faults
        const UFFDIO_REGISTER_MODE_MISSING = 1 << 0;
        /// Register for write-protect faults
        const UFFDIO_REGISTER_MODE_WP = 1 << 1;
        /// Register for minor faults
        const UFFDIO_REGISTER_MODE_MINOR = 1 << 2;
    }
}

bitflags! {
    #[derive(Debug, Clone, Copy)]
    pub struct UserfaultfdIoctls: u64 {
        /// API handshake ioctl
        const UFFDIO_API = 1 << 0;
        /// Register memory range
        const UFFDIO_REGISTER = 1 << 1;
        /// Unregister memory range
        const UFFDIO_UNREGISTER = 1 << 2;
        /// Wake up blocked threads
        const UFFDIO_WAKE = 1 << 3;
        /// Copy pages to resolve faults
        const UFFDIO_COPY = 1 << 4;
        /// Zero pages to resolve faults
        const UFFDIO_ZEROPAGE = 1 << 5;
        /// Write protect pages
        const UFFDIO_WRITEPROTECT = 1 << 6;
        /// Continue execution after minor fault
        const UFFDIO_CONTINUE = 1 << 7;
        /// Poison pages
        const UFFDIO_POISON = 1 << 8;
    }
}

// API version constant
pub const UFFD_API: u64 = 0xAA;

atomic_bitflags!(UserfaultfdFlags, AtomicU32);
atomic_bitflags!(UserfaultfdFeatures, AtomicU64);
atomic_bitflags!(UserfaultfdRegisterMode, AtomicU64);
atomic_bitflags!(UserfaultfdIoctls, AtomicU64);
