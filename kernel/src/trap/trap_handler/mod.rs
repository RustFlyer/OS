//! Trap handlers for different architectures

#![allow(unused)]

use alloc::collections::btree_map::BTreeMap;

use mutex::SpinNoIrqLock;

mod kernel_trap_handler;
mod user_trap_handler;

pub use kernel_trap_handler::*;
pub use user_trap_handler::*;

/// Statistics for external interrupts and clock interrupts.
///
/// This is a mapping from the interrupt number (exception number) to the count of
/// occurrences of that interrupt.
#[derive(Debug)]
pub struct TrapStats(SpinNoIrqLock<BTreeMap<usize, usize>>);

pub static TRAP_STATS: TrapStats = TrapStats(SpinNoIrqLock::new(BTreeMap::new()));

impl TrapStats {
    /// Increments the count for a specific trap by its trap number.
    pub fn inc(&self, trap: usize) {
        let mut stats = self.0.lock();
        *stats.entry(trap).or_insert(0) += 1;
    }

    /// Gets the current statistics as a mapping from trap numbers to counts.
    pub fn get_all(&self) -> BTreeMap<usize, usize> {
        let stats = self.0.lock();
        stats.clone()
    }
}
