#![no_std]
#![no_main]

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
            unsafe { riscv::register::sstatus::clear_sum() };
        } else {
            unsafe { riscv::register::sstatus::set_sum() };
        }
    }

    pub fn inc_sum_cnt(&mut self) {
        if self.sum_cnt == 0 {
            unsafe { riscv::register::sstatus::clear_sum() };
        }
        self.sum_cnt += 1;
    }

    pub fn dec_sum_cnt(&mut self) {
        self.sum_cnt -= 1;
        if self.sum_cnt == 0 {
            unsafe { riscv::register::sstatus::clear_sum() };
        }
    }

    pub fn change_privilege(&self, npps: &mut Self) {
        npps.auto_sum();
    }

    pub fn record(&mut self) {
        self.sstatus = arch::riscv64::sstatus::read().bits();
        self.sepc = riscv::register::sepc::read();
        self.satp = riscv::register::satp::read().bits();
    }

    pub fn restore(&mut self) {
        arch::riscv64::sstatus::write(self.sstatus);
        riscv::register::sepc::write(self.sepc);
        riscv::register::satp::write(self.satp);
    }
}
