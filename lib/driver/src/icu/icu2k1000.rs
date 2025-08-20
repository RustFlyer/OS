//! LoongArch 2K1000 ICU (Interrupt Control Unit)
//!
//! Based on official documentation and hardware specifications

use config::mm::KERNEL_MAP_OFFSET;
use core::ptr::{read_volatile, write_volatile};

use super::{ICU, icu_lavirt};

/// 龙芯2K1000中断控制器，支持64个中断源
/// 低32位可以路由到CPU0的Mailbox0(IP2)，高32位路由到CPU0的Mailbox0(IP3)
pub struct LoongArch2K1000ICU {
    /// 主寄存器基地址 (0x1fe01400)
    pub mmio_base1: usize,
    pub mmio_size1: usize,
    /// 辅助寄存器基地址 (0x1fe01040)
    pub mmio_base2: usize,
    pub mmio_size2: usize,
}

// ICU寄存器偏移定义
mod icu_regs {
    pub const INT_STATUS_LO: usize = 0x00; // 低32位中断状态
    pub const INT_STATUS_HI: usize = 0x04; // 高32位中断状态

    pub const INT_ENABLE_LO: usize = 0x08; // 低32位中断使能
    pub const INT_ENABLE_HI: usize = 0x0C; // 高32位中断使能

    pub const INT_SET_LO: usize = 0x10; // 低32位中断设置
    pub const INT_SET_HI: usize = 0x14; // 高32位中断设置

    pub const INT_CLEAR_LO: usize = 0x18; // 低32位中断清除
    pub const INT_CLEAR_HI: usize = 0x1C; // 高32位中断清除

    pub const INT_POL_LO: usize = 0x20; // 低32位中断极性
    pub const INT_POL_HI: usize = 0x24; // 高32位中断极性

    pub const INT_EDGE_LO: usize = 0x28; // 低32位边沿配置
    pub const INT_EDGE_HI: usize = 0x2C; // 高32位边沿配置

    pub const INT_ROUTE_LO: usize = 0x30; // 低32位路由配置
    pub const INT_ROUTE_HI: usize = 0x34; // 高32位路由配置
}

#[derive(Debug, Clone, Copy)]
pub enum TriggerType {
    HighLevel,
    LowLevel,
    RisingEdge,
    FallingEdge,
}

impl LoongArch2K1000ICU {
    pub fn new(mmio_base1: usize, mmio_size1: usize, mmio_base2: usize, mmio_size2: usize) -> Self {
        Self {
            mmio_base1,
            mmio_size1,
            mmio_base2,
            mmio_size2,
        }
    }

    pub fn set_trigger_type(&self, irq: usize, trigger: TriggerType) {
        if irq >= 64 {
            log::error!("Invalid IRQ number: {}", irq);
            return;
        }

        let base = (self.mmio_base1 + KERNEL_MAP_OFFSET) as *mut u32;
        let is_high_32 = irq >= 32;
        let bit = irq % 32;

        unsafe {
            // select hi/lo 32 reg by irq id
            let (pol_reg, edge_reg) = if is_high_32 {
                (
                    base.add(icu_regs::INT_POL_HI / 4),
                    base.add(icu_regs::INT_EDGE_HI / 4),
                )
            } else {
                (
                    base.add(icu_regs::INT_POL_LO / 4),
                    base.add(icu_regs::INT_EDGE_LO / 4),
                )
            };

            let mut pol_val = pol_reg.read_volatile();
            let mut edge_val = edge_reg.read_volatile();

            match trigger {
                TriggerType::HighLevel => {
                    pol_val &= !(1 << bit);
                    edge_val &= !(1 << bit);
                }
                TriggerType::LowLevel => {
                    pol_val |= 1 << bit;
                    edge_val &= !(1 << bit);
                }
                TriggerType::RisingEdge => {
                    pol_val &= !(1 << bit);
                    edge_val |= 1 << bit;
                }
                TriggerType::FallingEdge => {
                    pol_val |= 1 << bit;
                    edge_val |= 1 << bit;
                }
            }

            pol_reg.write_volatile(pol_val);
            edge_reg.write_volatile(edge_val);
        }
    }

    pub(crate) fn _enable_irq(&self, irq: usize, cpu_id: usize) {
        if irq >= 64 {
            log::error!("Invalid IRQ number: {}", irq);
            return;
        }

        log::info!("enable irq {irq}, cpu_id: {cpu_id}");

        let base = (self.mmio_base1 + KERNEL_MAP_OFFSET) as *mut u32;
        let is_high_32 = irq >= 32;
        let bit = irq % 32;

        unsafe {
            // 1. enable irq
            let enable_reg = if is_high_32 {
                base.add(icu_regs::INT_ENABLE_HI / 4)
            } else {
                base.add(icu_regs::INT_ENABLE_LO / 4)
            };

            let val = enable_reg.read_volatile() | (1 << bit);
            enable_reg.write_volatile(val);

            // 2. set irq to specified cpu
            let route_reg = if is_high_32 {
                base.add(icu_regs::INT_ROUTE_HI / 4)
            } else {
                base.add(icu_regs::INT_ROUTE_LO / 4)
            };

            let mut route_val = route_reg.read_volatile();
            if cpu_id == 1 {
                route_val |= 1 << bit; // to cpu1
            } else {
                route_val &= !(1 << bit); // to cpu0
            }
            route_reg.write_volatile(route_val);
        }
    }

    pub fn _disable_irq(&self, irq: usize) {
        if irq >= 64 {
            log::error!("Invalid IRQ number: {}", irq);
            return;
        }

        let base = (self.mmio_base1 + KERNEL_MAP_OFFSET) as *mut u32;
        let is_high_32 = irq >= 32;
        let bit = irq % 32;

        unsafe {
            let enable_reg = if is_high_32 {
                base.add(icu_regs::INT_ENABLE_HI / 4)
            } else {
                base.add(icu_regs::INT_ENABLE_LO / 4)
            };

            let val = enable_reg.read_volatile() & !(1 << bit);
            enable_reg.write_volatile(val);
        }
    }

    pub fn _claim_irq(&self, _cpu_id: usize) -> Option<usize> {
        let base = (self.mmio_base1 + KERNEL_MAP_OFFSET) as *mut u32;

        unsafe {
            // check lo-32 state
            let status_lo_reg = base.add(icu_regs::INT_STATUS_LO / 4);
            let enable_lo_reg = base.add(icu_regs::INT_ENABLE_LO / 4);

            let status_lo = status_lo_reg.read_volatile();
            let enable_lo = enable_lo_reg.read_volatile();
            let pending_lo = status_lo & enable_lo;

            if pending_lo != 0 {
                let irq = pending_lo.trailing_zeros() as usize;
                return Some(irq);
            }

            // check hi-32 state
            let status_hi_reg = base.add(icu_regs::INT_STATUS_HI / 4);
            let enable_hi_reg = base.add(icu_regs::INT_ENABLE_HI / 4);

            let status_hi = status_hi_reg.read_volatile();
            let enable_hi = enable_hi_reg.read_volatile();
            let pending_hi = status_hi & enable_hi;

            if pending_hi != 0 {
                let irq = 32 + pending_hi.trailing_zeros() as usize;
                return Some(irq);
            }

            None
        }
    }

    pub fn _complete_irq(&self, irq: usize, _cpu_id: usize) {
        if irq >= 64 {
            log::error!("Invalid IRQ number: {}", irq);
            return;
        }

        let base = (self.mmio_base1 + KERNEL_MAP_OFFSET) as *mut u32;
        let is_high_32 = irq >= 32;
        let bit = irq % 32;

        unsafe {
            let clear_reg = if is_high_32 {
                base.add(icu_regs::INT_CLEAR_HI / 4)
            } else {
                base.add(icu_regs::INT_CLEAR_LO / 4)
            };

            clear_reg.write_volatile(1 << bit);
        }
    }

    /// for software interrupt
    pub fn set_irq(&self, irq: usize) {
        if irq >= 64 {
            log::error!("Invalid IRQ number: {}", irq);
            return;
        }

        let base = (self.mmio_base1 + KERNEL_MAP_OFFSET) as *mut u32;
        let is_high_32 = irq >= 32;
        let bit = irq % 32;

        unsafe {
            let set_reg = if is_high_32 {
                base.add(icu_regs::INT_SET_HI / 4)
            } else {
                base.add(icu_regs::INT_SET_LO / 4)
            };

            set_reg.write_volatile(1 << bit);
        }
    }

    pub fn get_irq_status(&self, irq: usize) -> bool {
        if irq >= 64 {
            log::error!("Invalid IRQ number: {}", irq);
            return false;
        }

        let base = (self.mmio_base1 + KERNEL_MAP_OFFSET) as *mut u32;
        let is_high_32 = irq >= 32;
        let bit = irq % 32;

        unsafe {
            let status_reg = if is_high_32 {
                base.add(icu_regs::INT_STATUS_HI / 4)
            } else {
                base.add(icu_regs::INT_STATUS_LO / 4)
            };

            let status = status_reg.read_volatile();
            (status & (1 << bit)) != 0
        }
    }
}

pub mod irq_numbers {
    pub const UART0_IRQ: usize = 2;
    pub const UART1_IRQ: usize = 3;
    pub const UART2_IRQ: usize = 4;
    pub const UART3_IRQ: usize = 5;
    pub const RTC_IRQ: usize = 6;
    pub const NAND_IRQ: usize = 52;
    pub const GPIO0_IRQ: usize = 68;
    pub const GPIO1_IRQ: usize = 69;
}

impl ICU for LoongArch2K1000ICU {
    fn enable_irq(&self, irq: usize, ctx_id: usize) {
        self._enable_irq(irq, ctx_id)
    }

    fn disable_irq(&self, irq: usize) {
        self._disable_irq(irq);
    }

    fn claim_irq(&self, ctx_id: usize) -> Option<usize> {
        self._claim_irq(ctx_id)
    }

    fn complete_irq(&self, irq: usize, _cpu_id: usize) {
        self._complete_irq(irq, _cpu_id);
    }

    fn set_trigger_type(&self, irq: usize, trigger: icu_lavirt::TriggerType) {
        todo!()
    }
}
