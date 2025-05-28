use core::fmt::Debug;

use bitflags::bitflags;

use systype::memory_flags::MappingFlags;

use super::PageTableEntry;

/// Offset of the physical page number in a page table entry. In RISC-V Sv39,
/// the physical page number is located at bits 10-53 in a page table entry.
pub(super) const PPN_OFFSET: usize = 10;

bitflags! {
    /// Flags for a page table entry.
    ///
    /// The flags are defined in the RISC-V Sv39 specification as follows:
    ///
    /// - `V`: Valid. When set, the PTE is valid. If one of the R, W, or X bits
    ///   is set, the PTE points to a physical page. Otherwise, the PTE points
    ///   to a next-level page table.
    /// - `R`: Read. If set, the page pointed at by the PTE is readable.
    /// - `W`: Write. If set, the page pointed at by the PTE is writable.
    /// - `X`: Execute. If set, the page pointed at by the PTE is executable.
    /// - `U`: User. If set, the page pointed at by the PTE is accessible in
    ///   user mode.
    /// - `G`: Global. If set, the address range pointed at by the PTE is global
    ///   mapped, which is in all address spaces.
    /// - `A`: Accessed. If set, the page pointed at by the PTE has been
    ///   accessed.
    /// - `D`: Dirty. If set, the page pointed at by the PTE has been written to.
    ///
    /// Flag `RSW` is reserved for supervisor software, but we do not use it in
    /// the current implementation.
    ///
    /// Do not set any unknown bits.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct PteFlags: u64 {
        const V = 1 << 0;
        const R = 1 << 1;
        const W = 1 << 2;
        const X = 1 << 3;
        const U = 1 << 4;
        const G = 1 << 5;
        const A = 1 << 6;
        const D = 1 << 7;

        /// Mask for bits that are used for memory access types. This mask should exist
        /// on all architectures.
        const RWX_MASK = Self::R.bits() | Self::W.bits() | Self::X.bits();
    }
}

impl From<MappingFlags> for PteFlags {
    /// Creates a `PteFlags` from a set of `MappingFlags`. Bits unspecified in `perm` are
    /// set to some default values properly.
    ///
    /// For RISC-V, only bits set in `perm` are set in the resulting `PteFlags`.
    fn from(perm: MappingFlags) -> Self {
        Self::from_bits_truncate(perm.bits())
    }
}

impl From<PteFlags> for MappingFlags {
    /// Creates a `MappingFlags` from a set of `PteFlags`.
    ///
    /// This function sets the permission bits given by `flags`. Other bits are
    /// ignored.
    fn from(flags: PteFlags) -> Self {
        MappingFlags::from_bits_truncate(flags.bits())
    }
}

impl PageTableEntry {
    /// Returns whether the page is valid.
    pub fn is_valid(self) -> bool {
        self.flags().contains(PteFlags::V)
    }

    /// Returns whether the page is readable.
    pub fn is_readable(self) -> bool {
        self.flags().contains(PteFlags::R)
    }

    /// Returns whether the page is writable.
    pub fn is_writable(self) -> bool {
        self.flags().contains(PteFlags::W)
    }

    /// Returns whether the page is executable.
    pub fn is_executable(self) -> bool {
        self.flags().contains(PteFlags::X)
    }

    /// Returns whether the page is accessible in user mode.
    pub fn is_user(self) -> bool {
        self.flags().contains(PteFlags::U)
    }

    /// Returns whether the page is globally mapped.
    pub fn is_global(self) -> bool {
        self.flags().contains(PteFlags::G)
    }

    /// Returns whether the page has been accessed.
    pub fn is_accessed(self) -> bool {
        self.flags().contains(PteFlags::A)
    }

    /// Returns whether the page has been written to.
    pub fn is_dirty(self) -> bool {
        self.flags().contains(PteFlags::D)
    }
}
