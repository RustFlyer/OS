use bitflags::bitflags;

bitflags! {
    /// Flags for `splice` system call.
    #[derive(Default, Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub struct SpliceFlags: i32 {
        /// Attempt to move pages instead of copying. This is only a hint to the kernel.
        const SPLICE_F_MOVE = 0x01;
        /// Do not block on I/O.
        const SPLICE_F_NONBLOCK = 0x02;
        /// More data will be coming in a subsequent splice. This is only a hint to the
        /// kernel.
        const SPLICE_F_MORE = 0x04;
        /// Unused for `splice`; see `vmsplice`.
        const SPLICE_F_GIFT = 0x08;
    }
}
