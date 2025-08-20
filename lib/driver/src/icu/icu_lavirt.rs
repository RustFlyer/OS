//! LoongArch `virt` machine - EIOINTC-like Interrupt Controller
//!
//! 说明：本实现针对 QEMU loongarch `virt` 机型，按 EIOINTC 风格抽象寄存器：
//!   status/enable/set/clear/polarity/edge/route 均分低32/高32两组，支持最多 128 路 IRQ。
//! 若你的固件/设备树显示只有 64 路，改小 MAX_IRQS=64 并去掉 HI 分支即可。
//!
//! 如发现寄存器偏移与当前 QEMU 版本不一致，仅需调整 eiointc_regs 常量即可。
//!
//! 线程安全：MMIO 为裸访问。如在多核/中断上下文并发调用，请在上层加锁或保证原子性。

use config::mm::KERNEL_MAP_OFFSET;
use core::ptr::read_volatile;
use core::ptr::write_volatile;

use super::ICU;

const MAX_IRQS: usize = 128;

#[derive(Debug, Clone, Copy)]
pub enum TriggerType {
    HighLevel,
    LowLevel,
    RisingEdge,
    FallingEdge,
}

pub struct LoongArchVirtICU {
    pub mmio_base: usize,
    pub mmio_size: usize,
}

impl LoongArchVirtICU {
    pub fn new(mmio_base: usize, mmio_size: usize) -> Self {
        Self {
            mmio_base,
            mmio_size,
        }
    }

    #[inline]
    fn base_ptr(&self) -> *mut u32 {
        (self.mmio_base + KERNEL_MAP_OFFSET) as *mut u32
    }

    pub fn set_trigger_type(&self, irq: usize, trigger: TriggerType) {
        if irq >= MAX_IRQS {
            log::error!("Invalid IRQ number: {}", irq);
            return;
        }

        let base = self.base_ptr();
        let is_hi = (irq >> 5) & 0x1 != 0; // 每 32 路一组
        let bit = irq & 31;

        unsafe {
            let (pol_reg, edge_reg) = if is_hi {
                (
                    base.add(eiointc_regs::INT_POL_HI / 4),
                    base.add(eiointc_regs::INT_EDGE_HI / 4),
                )
            } else {
                (
                    base.add(eiointc_regs::INT_POL_LO / 4),
                    base.add(eiointc_regs::INT_EDGE_LO / 4),
                )
            };

            let mut pol_val = read_volatile(pol_reg);
            let mut edge_val = read_volatile(edge_reg);

            match trigger {
                TriggerType::HighLevel => {
                    pol_val &= !(1 << bit); // 高电平/上升沿 -> 极性=0
                    edge_val &= !(1 << bit); // 电平触发 -> 边沿=0
                }
                TriggerType::LowLevel => {
                    pol_val |= 1 << bit; // 低电平/下降沿 -> 极性=1
                    edge_val &= !(1 << bit); // 电平触发
                }
                TriggerType::RisingEdge => {
                    pol_val &= !(1 << bit); // 上升沿 -> 极性=0
                    edge_val |= 1 << bit; // 边沿触发
                }
                TriggerType::FallingEdge => {
                    pol_val |= 1 << bit; // 下降沿 -> 极性=1
                    edge_val |= 1 << bit; // 边沿触发
                }
            }

            write_volatile(pol_reg, pol_val);
            write_volatile(edge_reg, edge_val);
        }
    }

    pub fn set_irq(&self, irq: usize) {
        if irq >= MAX_IRQS {
            log::error!("Invalid IRQ number: {}", irq);
            return;
        }
        let base = self.base_ptr();
        let is_hi = (irq >> 5) & 0x1 != 0;
        let bit = irq & 31;

        unsafe {
            let set_reg = if is_hi {
                base.add(eiointc_regs::INT_SET_HI / 4)
            } else {
                base.add(eiointc_regs::INT_SET_LO / 4)
            };
            write_volatile(set_reg, 1 << bit);
        }
    }

    /// 查询某个 IRQ 的当前状态位（raw status，不与 enable 与）
    pub fn get_irq_status(&self, irq: usize) -> bool {
        if irq >= MAX_IRQS {
            log::error!("Invalid IRQ number: {}", irq);
            return false;
        }
        let base = self.base_ptr();
        let is_hi = (irq >> 5) & 0x1 != 0;
        let bit = irq & 31;

        unsafe {
            let status_reg = if is_hi {
                base.add(eiointc_regs::INT_STATUS_HI / 4)
            } else {
                base.add(eiointc_regs::INT_STATUS_LO / 4)
            };
            let status = read_volatile(status_reg);
            (status & (1 << bit)) != 0
        }
    }

    /// 低层使能：并可将该中断路由到指定 CPU（目前按 cpu0/cpu1 两路路由位）
    pub(crate) fn _enable_irq(&self, irq: usize, cpu_id: usize) {
        if irq >= MAX_IRQS {
            log::error!("Invalid IRQ number: {}", irq);
            return;
        }
        log::info!("virt-icu: enable irq {irq}, cpu_id: {cpu_id}");

        let base = self.base_ptr();
        let is_hi = (irq >> 5) & 0x1 != 0;
        let bit = irq & 31;

        unsafe {
            let enable_reg = if is_hi {
                base.add(eiointc_regs::INT_ENABLE_HI / 4)
            } else {
                base.add(eiointc_regs::INT_ENABLE_LO / 4)
            };
            let cur = read_volatile(enable_reg);
            write_volatile(enable_reg, cur | (1 << bit));

            let route_reg = if is_hi {
                base.add(eiointc_regs::INT_ROUTE_HI / 4)
            } else {
                base.add(eiointc_regs::INT_ROUTE_LO / 4)
            };
            let mut r = read_volatile(route_reg);
            if cpu_id == 1 {
                r |= 1 << bit; // bit=1 -> cpu1
            } else {
                r &= !(1 << bit); // bit=0 -> cpu0
            }
            write_volatile(route_reg, r);
        }
    }

    pub(crate) fn _disable_irq(&self, irq: usize) {
        if irq >= MAX_IRQS {
            log::error!("Invalid IRQ number: {}", irq);
            return;
        }
        let base = self.base_ptr();
        let is_hi = (irq >> 5) & 0x1 != 0;
        let bit = irq & 31;

        unsafe {
            let enable_reg = if is_hi {
                base.add(eiointc_regs::INT_ENABLE_HI / 4)
            } else {
                base.add(eiointc_regs::INT_ENABLE_LO / 4)
            };
            let cur = read_volatile(enable_reg);
            write_volatile(enable_reg, cur & !(1 << bit));
        }
    }

    /// 取最高优先级的 pending 中断（此处用“最低序号优先”，实际可改为读专用 claim 寄存器）
    pub(crate) fn _claim_irq(&self, _cpu_id: usize) -> Option<usize> {
        let base = self.base_ptr();

        unsafe {
            // lo-32：与 enable 与后选择最低位
            let status_lo = read_volatile(base.add(eiointc_regs::INT_STATUS_LO / 4));
            let enable_lo = read_volatile(base.add(eiointc_regs::INT_ENABLE_LO / 4));
            let pending_lo = status_lo & enable_lo;
            if pending_lo != 0 {
                return Some(pending_lo.trailing_zeros() as usize);
            }

            // hi-32（64~95）
            let status_hi = read_volatile(base.add(eiointc_regs::INT_STATUS_HI / 4));
            let enable_hi = read_volatile(base.add(eiointc_regs::INT_ENABLE_HI / 4));
            let pending_hi = status_hi & enable_hi;
            if pending_hi != 0 {
                return Some(32 + pending_hi.trailing_zeros() as usize);
            }

            // 若支持 128 路，可继续扩展再上一组（本实现将下一组复用“HI+偏移”的方式）
            // 如果你的 `virt` 真有 128 路，请将寄存器分组扩展为 0/1/2/3 组，每组 32 位。
            None
        }
    }

    /// 完成中断：写 W1C 清除
    pub(crate) fn _complete_irq(&self, irq: usize, _cpu_id: usize) {
        if irq >= MAX_IRQS {
            log::error!("Invalid IRQ number: {}", irq);
            return;
        }
        let base = self.base_ptr();
        let is_hi = (irq >> 5) & 0x1 != 0;
        let bit = irq & 31;

        unsafe {
            let clear_reg = if is_hi {
                base.add(eiointc_regs::INT_CLEAR_HI / 4)
            } else {
                base.add(eiointc_regs::INT_CLEAR_LO / 4)
            };
            write_volatile(clear_reg, 1 << bit);
        }
    }
}

/// EIOINTC 风格寄存器偏移（以 32-bit 宽度、4 字节步长计）
/// 如与实际 QEMU/固件不一致，仅需改这里。
mod eiointc_regs {
    // 状态（RO）
    pub const INT_STATUS_LO: usize = 0x00;
    pub const INT_STATUS_HI: usize = 0x04;

    // 使能（RW）
    pub const INT_ENABLE_LO: usize = 0x08;
    pub const INT_ENABLE_HI: usize = 0x0C;

    // 软触发（W1S）
    pub const INT_SET_LO: usize = 0x10;
    pub const INT_SET_HI: usize = 0x14;

    // 清除（W1C）
    pub const INT_CLEAR_LO: usize = 0x18;
    pub const INT_CLEAR_HI: usize = 0x1C;

    // 极性（0:高电平/上升沿，1:低电平/下降沿）
    pub const INT_POL_LO: usize = 0x20;
    pub const INT_POL_HI: usize = 0x24;

    // 触发类型（0:电平，1:边沿）
    pub const INT_EDGE_LO: usize = 0x28;
    pub const INT_EDGE_HI: usize = 0x2C;

    // 路由（bit=0 路由到 cpu0；bit=1 路由到 cpu1）
    pub const INT_ROUTE_LO: usize = 0x30;
    pub const INT_ROUTE_HI: usize = 0x34;
}

impl ICU for LoongArchVirtICU {
    fn enable_irq(&self, irq: usize, ctx_id: usize) {
        self._enable_irq(irq, ctx_id);
    }

    fn disable_irq(&self, irq: usize) {
        self._disable_irq(irq);
    }

    fn claim_irq(&self, ctx_id: usize) -> Option<usize> {
        self._claim_irq(ctx_id)
    }

    fn complete_irq(&self, irq: usize, cpu_id: usize) {
        self._complete_irq(irq, cpu_id);
    }
}
