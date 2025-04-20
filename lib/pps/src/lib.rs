#![no_std]
#![no_main]

use riscv::register::{
    satp::{self, Satp},
    sepc,
    sstatus::{self, Sstatus},
};

/// `ProcessorPrivilegeState` records processor privilege state in `RISCV` of a task.
/// when a task is scheduled(poll in or poll out), the hart will switch pps and
/// set its processor privilege to a correct state for task running.
///
/// # Member
/// - sum_cnt: it counts for sum. When SumGuard is created, it increases
///   by one. When SumGuard is drop, it subtract one by itself. When
///   `auto_sum()` is called, the state of sum will changed really.
///   sum is a bit in sstatus, which decides whether kernel can access
///   user space.
/// - sstatus: it stores current state in S mode.
/// - sepc: when kernel finish handling trap or interrupt, application
///   can return `sepc` address in user space.
/// - satp: a pagetable for mapping virtual address.
#[derive(Debug, Clone, Copy)]
pub struct ProcessorPrivilegeState {
    sum_cnt: usize,

    sstatus: usize,
    sepc: usize,
    satp: usize,
}

impl ProcessorPrivilegeState {
    pub const fn new() -> Self {
        Self {
            sum_cnt: 0,
            sstatus: 0,
            sepc: 0,
            satp: 0,
        }
    }

    pub fn auto_sum(&self) {
        if self.sum_cnt == 0 {
            unsafe { sstatus::clear_sum() };
        } else {
            unsafe { sstatus::set_sum() };
        }
    }

    pub fn inc_sum_cnt(&mut self) {
        if self.sum_cnt == 0 {
            unsafe { sstatus::set_sum() };
        }
        self.sum_cnt += 1;
    }

    pub fn dec_sum_cnt(&mut self) {
        self.sum_cnt -= 1;
        if self.sum_cnt == 0 {
            unsafe { sstatus::clear_sum() };
        }
    }

    pub fn change_privilege(&self, npps: &mut Self) {
        npps.auto_sum();
    }

    pub fn record(&mut self) {
        self.sstatus = sstatus::read().bits();
        self.sepc = sepc::read();
        self.satp = satp::read().bits();
    }

    pub fn restore(&mut self) {
        unsafe {
            sstatus::write(Sstatus::from_bits(self.sstatus));
            sepc::write(self.sepc);
            satp::write(Satp::from_bits(self.satp));
        }
    }
}
