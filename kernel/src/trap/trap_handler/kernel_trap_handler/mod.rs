//! Kernel trap handlers for different architectures

use polyhal_macro::define_arch_mods;

define_arch_mods!();

#[cfg(target_arch = "loongarch64")]
pub mod unaligned_la;
