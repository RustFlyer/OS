#![no_std]
#![no_main]

use riscv::register::{
    satp::{self, Satp},
    sepc,
    sstatus::{self, Sstatus},
};

#[derive(Debug, Clone, Copy)]
pub struct ProcessorPrivilegeState {
    sum_cnt: usize,

    sstatus: usize,
    sepc: usize,
    satp: usize,
}

impl ProcessorPrivilegeState {
    pub fn new() -> Self {
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
            unsafe { sstatus::clear_sum() };
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
        }
        unsafe {
            sepc::write(self.sepc);
        }
        unsafe {
            satp::write(Satp::from_bits(self.satp));
        }
    }
}
