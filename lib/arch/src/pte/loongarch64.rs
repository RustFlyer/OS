use bitflags::bitflags;

use systype::memory_flags::MappingFlags;

use super::PageTableEntry;

/// Offset of the physical page number in a page table entry. In LoongArch64,
/// the physical page number is located at bits 12-`PA_LEN - 1` in a page table entry.
pub(super) const PPN_OFFSET: usize = 12;

bitflags! {
    /// Flags for a page table entry.
    ///
    /// The flags are defined in the LoongArch64 specification as follows:
    ///
    /// - `V`: Valid. When set, the PTE is valid.
    /// - `D`: Dirty. If set, there are dirty data on the page pointed at by the PTE.
    /// - `PLV`: Privilege Level. The privilege level of the page pointed at by the PTE.
    /// - `MAT`: Memory Access Type. There are 3 kind of memory access types:
    ///   - 0: Strongly-ordered UnCached (SUC)
    ///   - 1: Coherent Cached (CC)
    ///   - 2: Weakly-ordered UnCached (WUC)
    /// - `G`: Global. If set, the address range pointed at by the PTE is global
    ///   mapped, which is in all address spaces.
    /// - `P`: Physical page existing. The meaning of this bit is ambiguous, so we will
    ///   set it to 0 for now.
    /// - `W`: Write. If set, the page pointed at by the PTE is writable.
    /// - `X`: Execute. If set, the page pointed at by the PTE is executable.
    /// - `NR`: Non-Readable. If set, the page pointed at by the PTE is not readable.
    /// - `NX`: Non-Executable. If set, the page pointed at by the PTE is not executable.
    /// - `RPLV`: Restricted Privilege Level. If set, the page pointed at by the PTE can
    ///   only be accessed from the privilege level specified in `PLV`.
    ///
    /// Do not set any unknown bits.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct PteFlags: u64 {
        const V = 1 << 0;
        const D = 1 << 1;
        const PLV = 0b11 << 2;
        const MAT = 0b11 << 4;
        const G = 1 << 6;
        const P = 1 << 7;
        const W = 1 << 8;
        const NR = 1 << 61;
        const NX = 1 << 62;
        const RPLV = 1 << 63;

        const MAT_SUC = 0b00 << 4;
        const MAT_CC = 0b01 << 4;
        const MAT_WUC = 0b10 << 4;
        const PLV_USER = 0b11 << 2;

        /// Mask for bits that are used for memory access types. This mask should exist
        /// on all architectures.
        const RWX_MASK = Self::NR.bits() | Self::W.bits() | Self::NX.bits();
    }
}

impl From<MappingFlags> for PteFlags {
    /// Creates a `PteFlags` from a set of `MappingFlags`. Bits unspecified in `perm` are
    /// set to some default values properly.
    ///
    /// For LoongArch64, `NR` and `NX` bits are set even if `R` and `X` bits are not set
    /// in `perm`.
    fn from(perm: MappingFlags) -> Self {
        let mut flags = Self::MAT_CC;
        if perm.contains(MappingFlags::V) {
            flags |= Self::V;
        }
        if !perm.contains(MappingFlags::R) {
            flags |= Self::NR;
        }
        if perm.contains(MappingFlags::W) {
            flags |= Self::W;
            // Note: LoongArch64 requires the `D` bit to be set in the TLB to allow
            // writing to the page. Since we don't track the dirty state of pages
            // currently, we always set the `D` bit if the page is writable, so the
            // TLB refill exception handler can automatically set the `D` bit in the
            // TLB entry.
            flags |= Self::D;
        }
        if !perm.contains(MappingFlags::X) {
            flags |= Self::NX;
        }
        if perm.contains(MappingFlags::U) {
            flags |= Self::PLV_USER;
        }
        if perm.contains(MappingFlags::G) {
            flags |= Self::G;
        }
        flags
    }
}

impl From<PteFlags> for MappingFlags {
    /// Creates a `MappingFlags` from `PteFlags`.
    ///
    /// This function sets the permission bits given by `flags`.
    fn from(flags: PteFlags) -> Self {
        let mut perm = MappingFlags::empty();
        if flags.contains(PteFlags::V) {
            perm |= MappingFlags::V;
        }
        if !flags.contains(PteFlags::NR) {
            perm |= MappingFlags::R;
        }
        if flags.contains(PteFlags::W) {
            perm |= MappingFlags::W;
        }
        if !flags.contains(PteFlags::NX) {
            perm |= MappingFlags::X;
        }
        if flags.contains(PteFlags::PLV_USER) {
            perm |= MappingFlags::U;
        }
        if flags.contains(PteFlags::G) {
            perm |= MappingFlags::G;
        }
        perm
    }
}

impl PageTableEntry {
    /// Returns whether the page is valid.
    pub fn is_valid(self) -> bool {
        self.flags().contains(PteFlags::V)
    }

    /// Returns whether the page is readable.
    pub fn is_readable(self) -> bool {
        !self.flags().contains(PteFlags::NR)
    }

    /// Returns whether the page is writable.
    pub fn is_writable(self) -> bool {
        self.flags().contains(PteFlags::W)
    }

    /// Returns whether the page is executable.
    pub fn is_executable(self) -> bool {
        !self.flags().contains(PteFlags::NX)
    }

    /// Returns whether the page is accessible in user mode.
    pub fn is_user(self) -> bool {
        self.flags().contains(PteFlags::PLV_USER)
    }

    /// Returns whether the page is globally mapped.
    pub fn is_global(self) -> bool {
        self.flags().contains(PteFlags::G)
    }

    /// Returns whether the page has been accessed.
    pub fn is_accessed(self) -> bool {
        // Note: LoongArch64 doesn't have an explicit accessed bit like RISC-V.
        // For consistency with the API, we return true.
        true
    }

    /// Returns whether the page has been written to.
    pub fn is_dirty(self) -> bool {
        self.flags().contains(PteFlags::D)
    }
}
