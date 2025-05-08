//! Module for abstracting memory permissions.

use bitflags::bitflags;
use riscv::interrupt::supervisor;

use super::pte::PteFlags;

bitflags! {
    /// Memory permission/access type corresponding to R, W, X, and U bits in a page
    /// table entry.
    ///
    /// The bits of `MemPerm` are a subset of the bits of `PteFlags`, and their bit
    /// positions are the same as those in `PteFlags` for easy conversion between them.
    ///
    /// Although the `bitflags` crate does allow the user to set unknown bits, do not
    /// do so.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct MemPerm: u8 {
        const R = 1 << 1;
        const W = 1 << 2;
        const X = 1 << 3;
        const U = 1 << 4;
    }
}

impl MemPerm {
    /// Create a new `MemPerm` from a set of `PteFlags`.
    pub fn from(flags: PteFlags) -> Self {
        Self::from_bits_truncate(flags.bits())
    }
}

bitflags! {
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub struct PageFaultAccessType: u8 {
        const READ = 1 << 0;
        const WRITE = 1 << 1;
        const EXECUTE = 1 << 2;
    }
}

impl MemPerm {
    pub const RW: Self = Self::R.union(Self::W);
    pub const RX: Self = Self::R.union(Self::X);

    pub fn from_exception(e: supervisor::Exception) -> Self {
        match e {
            supervisor::Exception::InstructionPageFault => Self::RX,
            supervisor::Exception::LoadPageFault => Self::R,
            supervisor::Exception::StorePageFault => Self::RW,
            _ => panic!("unexcepted exception type for PageFaultAccessType"),
        }
    }
}