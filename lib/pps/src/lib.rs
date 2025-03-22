#![no_std]
#![no_main]

use riscv::register::{
    satp::{self, Satp},
    sepc,
    sstatus::{self, Sstatus},
};

/// 处理器特权状态结构体
///
/// 表示一个处理器的特权状态，包含保护块计数、sstatus、sepc和satp
/// 每个CPU核心持有一个，也可额外存储，用于保存与恢复状态
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
