#![no_std]
#![no_main]

#[cfg(target_arch = "loongarch64")]
use core::arch::asm;

#[cfg(target_arch = "riscv64")]
use riscv::register::{
    satp::{self, Satp},
    sepc,
    sstatus::{self, Sstatus},
};

#[cfg(target_arch = "loongarch64")]
use loongArch64::register::{crmd, era, pgdl};

/// `ProcessorPrivilegeState` records processor privilege state of a task.
/// when a task is scheduled(poll in or poll out), the hart will switch pps and
/// set its processor privilege to a correct state for task running.
///
/// # Member
/// - sum_cnt(disabled in loongArch): it counts for sum. When SumGuard is created, it increases
///   by one. When SumGuard is drop, it subtract one by itself. When
///   `auto_sum()` is called, the state of sum will changed really.
///   sum is a bit in sstatus, which decides whether kernel can access
///   user space.
/// - sstatus(seen as CRMD in loongArch): it stores current state in S mode.
/// - sepc(seen as ERA in loongArch): when kernel finish handling trap or interrupt, application
///   can return `sepc` address in user space.
/// - satp(seen as PGDL in loongArch): a pagetable for mapping virtual address.
#[derive(Debug, Clone, Copy)]
pub struct ProcessorPrivilegeState {
    #[cfg(target_arch = "riscv64")]
    sum_cnt: usize,

    sstatus: usize,
    sepc: usize,
    satp: usize,
}

impl ProcessorPrivilegeState {
    pub const fn new() -> Self {
        Self {
            #[cfg(target_arch = "riscv64")]
            sum_cnt: 0,
            sstatus: 0,
            sepc: 0,
            satp: 0,
        }
    }

    #[cfg(target_arch = "riscv64")]
    pub fn auto_sum(&self) {
        if self.sum_cnt == 0 {
            unsafe { sstatus::clear_sum() };
        } else {
            unsafe { sstatus::set_sum() };
        }
    }

    #[cfg(target_arch = "loongarch64")]
    pub fn auto_sum(&self) {
        // LoongArch64 doesn't have SUM mechanism, kernel can always access user space
        // No operation needed
    }

    #[cfg(target_arch = "riscv64")]
    pub fn inc_sum_cnt(&mut self) {
        if self.sum_cnt == 0 {
            unsafe { sstatus::set_sum() };
        }
        self.sum_cnt += 1;
    }

    #[cfg(target_arch = "loongarch64")]
    pub fn inc_sum_cnt(&mut self) {
        // LoongArch64 doesn't have SUM mechanism, kernel can always access user space
        // No operation needed
    }

    #[cfg(target_arch = "riscv64")]
    pub fn dec_sum_cnt(&mut self) {
        self.sum_cnt -= 1;
        if self.sum_cnt == 0 {
            unsafe { sstatus::clear_sum() };
        }
    }

    #[cfg(target_arch = "loongarch64")]
    pub fn dec_sum_cnt(&mut self) {
        // LoongArch64 doesn't have SUM mechanism, kernel can always access user space
        // No operation needed
    }

    pub fn change_privilege(&self, npps: &mut Self) {
        npps.auto_sum();
    }

    #[cfg(target_arch = "riscv64")]
    pub fn record(&mut self) {
        self.sstatus = sstatus::read().bits();
        self.sepc = sepc::read();
        self.satp = satp::read().bits();
    }

    #[cfg(target_arch = "loongarch64")]
    pub fn record(&mut self) {
        self.sstatus = crmd::read().raw();
        self.sepc = era::read().raw();
        self.satp = pgdl::read().base();
    }

    #[cfg(target_arch = "riscv64")]
    pub fn restore(&mut self) {
        unsafe {
            sstatus::write(Sstatus::from_bits(self.sstatus));
            sepc::write(self.sepc);
            satp::write(Satp::from_bits(self.satp));
        }
    }

    #[cfg(target_arch = "loongarch64")]
    pub fn restore(&mut self) {
        let crmd = self.sstatus;
        let era = self.sepc;
        let pgdl = self.satp;
        unsafe {
            asm!("csrwr {}, 0x0", in(reg) crmd);
            asm!("csrwr {}, 0x6", in(reg) era);
            asm!("csrwr {}, 0x19", in(reg) pgdl);
        }
    }
}

impl Default for ProcessorPrivilegeState {
    fn default() -> Self {
        Self::new()
    }
}
