//! Module for abstracting memory permissions.

use bitflags::bitflags;

bitflags! {
    /// An abstract of page table entry flags, which is independent of the architecture.
    ///
    /// This struct is used to represent page table entry flags, and the type of memory
    /// accesses or memory protection.
    ///
    /// For page table entry flags, these bits can be ORed together to form page table
    /// flags.
    /// - `V`: Valid
    /// - `R`: Readable
    /// - `W`: Writable
    /// - `X`: Executable
    /// - `U`: User-accessible
    /// - `G`: Global
    ///
    /// For types of memory accesses and memory protection, exactly one of these bits can
    /// be set at a time:
    /// - `R`: Read
    /// - `W`: Write
    /// - `X`: Execute
    ///
    /// Do not set any unknown bits.
    ///
    /// # Note for RISC-V
    /// The bits of `MappingFlags` are a subset of the bits of RISC-V's page table entry
    /// flags, and their positions are the same as those in the page table entry.
    ///
    /// A page table entry in RISC-V also has `A` and `D` bits, but other architectures
    /// may not have them. Specifically, the `A` bit does not exist in LoongArch, and
    /// the `D` bit, which indeed exists in LoongArch, has different semantics.
    ///
    /// # Note for LoongArch
    /// A page table entry in LoongArch does not have `R`, `X`, `U`, and `A` bits.
    /// Instead, it uses `NR` and `NX` bits to represent non-readable and non-executable.
    /// It uses `PLV` bits to represent the privilege level of the page, which is similar
    /// to the `U` bit in RISC-V page table entries.
    #[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct MappingFlags: u64 {
        const V = 1 << 0;
        const R = 1 << 1;
        const W = 1 << 2;
        const X = 1 << 3;
        const U = 1 << 4;
        const G = 1 << 5;

        // Mask for R, W, and X bits.
        const RWX = Self::R.bits() | Self::W.bits() | Self::X.bits();
    }
}

bitflags! {
    /// Memory protection/access flags for system calls like `mmap`.
    ///
    /// Note that `MappingFlags` is an interior representation of general memory access
    /// permissions, while `MmapProt` is used as an interface for Linux-compatible
    /// system calls.
    #[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct MmapProt: i32 {
        /// No access.
        const PROT_NONE = 0x0;
        /// Page can be read.
        const PROT_READ = 0x1;
        /// Page can be written.
        const PROT_WRITE = 0x2;
        /// Page can be executed.
        const PROT_EXEC = 0x4;
    }
}

bitflags! {
    /// Flags for `mmap` system call that specify how the memory should be mapped.
    #[derive(Default, Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub struct MmapFlags: i32 {
        // Sharing types (must choose one and only one of these).
        /// Share changes.
        const MAP_SHARED = 0x01;
        /// Changes are private.
        const MAP_PRIVATE = 0x02;
        /// Share changes and validate
        const MAP_SHARED_VALIDATE = 0x03;
        const MAP_TYPE_MASK = 0x03;

        // Other flags
        /// Interpret addr exactly.
        const MAP_FIXED = 0x10;
        /// Don't use a file.
        const MAP_ANONYMOUS = 0x20;
        /// Don't check for reservations.
        const MAP_NORESERVE = 0x04000;
    }
}

impl From<MmapProt> for MappingFlags {
    /// Creates a set of `MappingFlags` from a set of `MmapProt`. `RWX` bits are set
    /// according to the `MmapProt` bits.
    fn from(prot: MmapProt) -> Self {
        let mut ret = MappingFlags::empty();
        if prot.contains(MmapProt::PROT_READ) {
            ret |= Self::R;
        }
        if prot.contains(MmapProt::PROT_WRITE) {
            ret |= Self::W;
        }
        if prot.contains(MmapProt::PROT_EXEC) {
            ret |= Self::X;
        }
        ret
    }
}

impl From<MappingFlags> for MmapProt {
    /// Creates a set of `MmapProt` from a set of `MappingFlags`. Only `R`, `W`, and `X`
    /// bits in `flags` are considered, and the rest are ignored.
    fn from(flags: MappingFlags) -> Self {
        let mut ret = MmapProt::PROT_NONE;
        if flags.contains(MappingFlags::R) {
            ret |= MmapProt::PROT_READ;
        }
        if flags.contains(MappingFlags::W) {
            ret |= MmapProt::PROT_WRITE;
        }
        if flags.contains(MappingFlags::X) {
            ret |= MmapProt::PROT_EXEC;
        }
        ret
    }
}
