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
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
