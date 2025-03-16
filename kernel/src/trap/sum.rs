// This module is adapted from Phoenix OS.

//! Module for enabling and disabling kernel memory accesses to user address space.
//!
//! This module provides the [`SumGuard`] type that can be used to temporarily enable
//! kernel memory accesses to user address space. This is useful when the kernel needs
//! to copying data to/from user space. When a [`SumGuard`] is constructed, the SUM bit
//! in the `sstatus` register is set to 1, allowing kernel memory accesses to user space.
//! When the guard is dropped, the SUM bit is reset to 0, disabling kernel memory accesses
//! to user space.
//!
//! Considering the case that there are multiple living [`SumGuard`]s, a `sum_count` field
//! in [`HART`] struct is used to keep track of how many `SumGuard`s are living. When the
//! count is zero, kernel memory accesses to user space are disabled, which is the default state.

use core::sync::atomic::Ordering;

use riscv::register::sstatus;

use crate::processor::hart::{self, Hart};

pub struct SumGuard;

impl SumGuard {
    pub fn new(hart: &mut Hart) -> Self {
        let old = hart.sum_count.fetch_add(1, Ordering::Relaxed);
        if old == 0 {
            unsafe {
                sstatus::set_sum();
            }
        }
        Self
    }
}

impl Drop for SumGuard {
    fn drop(&mut self) {
        let hart = hart::get_current_hart();
        let old = hart.sum_count.fetch_sub(1, Ordering::Relaxed);
        if old == 1 {
            unsafe {
                sstatus::clear_sum();
            }
        }
    }
}
