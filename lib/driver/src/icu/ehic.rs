//! LoongArch EIOINTC - Extended I/O Interrupt Controller
//! 兼容 loongson,ls2k2000-eiointc
//! 用于 QEMU virt 机型

use config::mm::KERNEL_MAP_OFFSET;
use core::ptr::{read_volatile, write_volatile};

use super::{ICU, icu_lavirt::TriggerType, pch::LoongArchPCHPIC};

/// EIOINTC 中断控制器
pub struct LoongArchEIOINTC {
    pub mmio_base: usize,
    pub mmio_size: usize,
}

/// EIOINTC 寄存器定义（基于常见实现）
mod regs {
    // 节点映射寄存器（每个IRQ映射到哪个CPU节点）
    pub const NODEMAP_BASE: usize = 0x000; // 每个IRQ 1字节

    // 中断使能寄存器（每个节点一组）
    pub const ENABLE_BASE: usize = 0x100; // 每个节点 0x40 字节（256位）
    pub const ENABLE_PER_NODE: usize = 0x40;

    // 中断状态寄存器（每个节点一组）
    pub const STATUS_BASE: usize = 0x200; // 每个节点 0x40 字节
    pub const STATUS_PER_NODE: usize = 0x40;

    // 中断路由寄存器（路由到CPU核心）
    pub const ROUTE_BASE: usize = 0x300; // 每个IRQ 1字节

    // 中断清除寄存器（每个节点一组）
    pub const CLEAR_BASE: usize = 0x400; // 每个节点 0x40 字节
    pub const CLEAR_PER_NODE: usize = 0x40;

    // 边沿触发配置
    pub const EDGE_BASE: usize = 0x500; // 256位位图

    // 极性配置
    pub const POL_BASE: usize = 0x540; // 256位位图
}

const MAX_IRQS: usize = 256; // EIOINTC 支持 256 个中断
const MAX_NODES: usize = 4; // 最多 4 个节点

impl LoongArchEIOINTC {
    pub fn new(mmio_base: usize, mmio_size: usize) -> Self {
        Self {
            mmio_base,
            mmio_size,
        }
    }

    #[inline]
    fn base_ptr(&self) -> *mut u8 {
        (self.mmio_base + KERNEL_MAP_OFFSET) as *mut u8
    }

    /// 设置中断触发类型
    pub fn set_trigger_type(&self, irq: usize, trigger: TriggerType) {
        if irq >= MAX_IRQS {
            log::error!("Invalid IRQ number: {}", irq);
            return;
        }

        let base = self.base_ptr();
        let byte_idx = irq / 8;
        let bit_idx = irq % 8;

        unsafe {
            // 边沿配置
            let edge_ptr = base.add(regs::EDGE_BASE + byte_idx);
            let mut edge_val = read_volatile(edge_ptr);

            // 极性配置
            let pol_ptr = base.add(regs::POL_BASE + byte_idx);
            let mut pol_val = read_volatile(pol_ptr);

            match trigger {
                TriggerType::HighLevel => {
                    edge_val &= !(1 << bit_idx); // 电平触发
                    pol_val &= !(1 << bit_idx); // 高电平
                }
                TriggerType::LowLevel => {
                    edge_val &= !(1 << bit_idx); // 电平触发
                    pol_val |= 1 << bit_idx; // 低电平
                }
                TriggerType::RisingEdge => {
                    edge_val |= 1 << bit_idx; // 边沿触发
                    pol_val &= !(1 << bit_idx); // 上升沿
                }
                TriggerType::FallingEdge => {
                    edge_val |= 1 << bit_idx; // 边沿触发
                    pol_val |= 1 << bit_idx; // 下降沿
                }
            }

            write_volatile(edge_ptr, edge_val);
            write_volatile(pol_ptr, pol_val);
        }
    }

    pub(crate) fn _enable_irq(&self, irq: usize, cpu_id: usize) {
        if irq >= MAX_IRQS {
            log::error!("Invalid IRQ number: {}", irq);
            return;
        }

        log::info!("EIOINTC: enable irq {}, cpu_id: {}", irq, cpu_id);

        let base = self.base_ptr();
        let node = cpu_id / 4; // 假设每个节点 4 个核心
        let core = cpu_id % 4;

        unsafe {
            // 1. 设置节点映射
            let nodemap_ptr = base.add(regs::NODEMAP_BASE + irq);
            write_volatile(nodemap_ptr, node as u8);

            // 2. 设置核心路由
            let route_ptr = base.add(regs::ROUTE_BASE + irq);
            write_volatile(route_ptr, 1 << core); // 路由到指定核心

            // 3. 使能中断（在对应节点的使能寄存器中）
            let enable_base = base.add(regs::ENABLE_BASE + node * regs::ENABLE_PER_NODE);
            let byte_idx = irq / 8;
            let bit_idx = irq % 8;
            let enable_ptr = enable_base.add(byte_idx);

            let val = read_volatile(enable_ptr) | (1 << bit_idx);
            write_volatile(enable_ptr, val);
        }
    }

    pub(crate) fn _disable_irq(&self, irq: usize) {
        if irq >= MAX_IRQS {
            log::error!("Invalid IRQ number: {}", irq);
            return;
        }

        let base = self.base_ptr();

        unsafe {
            // 从所有节点禁用该中断
            for node in 0..MAX_NODES {
                let enable_base = base.add(regs::ENABLE_BASE + node * regs::ENABLE_PER_NODE);
                let byte_idx = irq / 8;
                let bit_idx = irq % 8;
                let enable_ptr = enable_base.add(byte_idx);

                let val = read_volatile(enable_ptr) & !(1 << bit_idx);
                write_volatile(enable_ptr, val);
            }
        }
    }

    pub(crate) fn _claim_irq(&self, cpu_id: usize) -> Option<usize> {
        let base = self.base_ptr();
        let node = cpu_id / 4;

        unsafe {
            // 读取该节点的中断状态
            let status_base = base.add(regs::STATUS_BASE + node * regs::STATUS_PER_NODE);
            let enable_base = base.add(regs::ENABLE_BASE + node * regs::ENABLE_PER_NODE);

            // 扫描所有字节找到 pending 中断
            for byte_idx in 0..32 {
                // 256位 = 32字节
                let status = read_volatile(status_base.add(byte_idx));
                let enable = read_volatile(enable_base.add(byte_idx));
                let pending = status & enable;

                if pending != 0 {
                    let bit = pending.trailing_zeros() as usize;
                    return Some(byte_idx * 8 + bit);
                }
            }

            None
        }
    }

    pub(crate) fn _complete_irq(&self, irq: usize, cpu_id: usize) {
        if irq >= MAX_IRQS {
            log::error!("Invalid IRQ number: {}", irq);
            return;
        }

        let base = self.base_ptr();
        let node = cpu_id / 4;

        unsafe {
            // 清除中断（写1清除）
            let clear_base = base.add(regs::CLEAR_BASE + node * regs::CLEAR_PER_NODE);
            let byte_idx = irq / 8;
            let bit_idx = irq % 8;
            let clear_ptr = clear_base.add(byte_idx);

            write_volatile(clear_ptr, 1 << bit_idx);
        }
    }
}

impl ICU for LoongArchEIOINTC {
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

    fn set_trigger_type(&self, irq: usize, trigger: super::icu_lavirt::TriggerType) {
        todo!()
    }
}
