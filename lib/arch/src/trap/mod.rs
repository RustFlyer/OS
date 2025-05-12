use polyhal_macro::define_arch_mods;

define_arch_mods!();

/// An abstract representation of the trap mode.
pub enum TrapMode {
    /// Traps into a specific address.
    Direct,
    /// Traps into a vector table.
    Vectored,
}
