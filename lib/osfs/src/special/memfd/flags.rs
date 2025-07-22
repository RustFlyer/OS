use bitflags::bitflags;
use common::atomic_bitflags;

bitflags! {
    pub struct MemfdFlags: u32 {
        /// Close-on-exec (set the fd as O_CLOEXEC)
        const CLOEXEC         = 0x0001; // MFD_CLOEXEC

        /// Allow sealing operations on this memfd
        const ALLOW_SEALING   = 0x0002; // MFD_ALLOW_SEALING

        /// Create memfd as a hugetlbfs file (Linux 4.14+)
        const HUGETLB         = 0x0004; // MFD_HUGETLB

        /// Prevent creation of executable mappings (Linux 5.1+)
        const NOEXEC_SEAL     = 0x0008; // MFD_NOEXEC_SEAL

        /// Create memfd with huge page size, encode as (shift << 26)
        /// e.g. (21 << 26) for 2MiB, (30 << 26) for 1GiB
        const HUGETLB_SHIFT_MASK = 0x3f << 26; // MFD_HUGE_SHIFT, matches kernel

        /// Huge page size: 64KiB (Linux 5.10+)
        const HUGE_64KB   = 16 << 26; // MFD_HUGE_64KB
        /// Huge page size: 512KiB
        const HUGE_512KB  = 19 << 26; // MFD_HUGE_512KB
        /// Huge page size: 1MiB
        const HUGE_1MB    = 20 << 26; // MFD_HUGE_1MB
        /// Huge page size: 2MiB
        const HUGE_2MB    = 21 << 26; // MFD_HUGE_2MB
        /// Huge page size: 8MiB
        const HUGE_8MB    = 23 << 26; // MFD_HUGE_8MB
        /// Huge page size: 16MiB
        const HUGE_16MB   = 24 << 26; // MFD_HUGE_16MB
        /// Huge page size: 32MiB
        const HUGE_32MB   = 25 << 26; // MFD_HUGE_32MB
        /// Huge page size: 256MiB
        const HUGE_256MB  = 28 << 26; // MFD_HUGE_256MB
        /// Huge page size: 512MiB
        const HUGE_512MB  = 29 << 26; // MFD_HUGE_512MB
        /// Huge page size: 1GiB
        const HUGE_1GB    = 30 << 26; // MFD_HUGE_1GB
        /// Huge page size: 2GiB
        const HUGE_2GB    = 31 << 26; // MFD_HUGE_2GB
        /// Huge page size: 16GiB
        const HUGE_16GB   = 34 << 26; // MFD_HUGE_16GB
    }
}

bitflags! {
    #[derive(Clone,Copy,Debug)]
    pub struct MemfdSeals: u32 {
        /// Prevent writes
        const SEAL    = 0x0001; // F_SEAL_WRITE
        /// Prevent shrinking file size
        const SHRINK  = 0x0002; // F_SEAL_SHRINK
        /// Prevent growing file size
        const GROW    = 0x0004; // F_SEAL_GROW
        /// Prevent any further seals from being set
        const WRITE    = 0x0008; // F_SEAL_SEAL
        // Prevent execs
        // const EXEC     = 0x0010;
    }
}

atomic_bitflags!(MemfdFlags, AtomicU32);
atomic_bitflags!(MemfdSeals, AtomicU32);
