use alloc::sync::Arc;
use core::ptr::{read_volatile, write_volatile};
use spin::Mutex;

use crate::println;

use super::{ICU, icu_lavirt::TriggerType};

// LoongArch 虚拟地址窗口
const LOONGARCH_UNCACHED_BASE: usize = 0x8000_0000_0000_0000; // 非缓存直接映射窗口
const LOONGARCH_CACHED_BASE: usize = 0x9000_0000_0000_0000; // 缓存直接映射窗口

// IOCSR 基地址（物理地址）
const IOCSR_BASE: usize = 0x1fe0_0000;

// EXTIOI 相关偏移
const IOCSR_EXTIOI_EN_BASE: usize = 0x1600;
const IOCSR_EXTIOI_ISR_BASE: usize = 0x1800;
const IOCSR_EXTIOI_ROUTE_BASE: usize = 0x1c00;
const IOCSR_EXTIOI_NODETYPE_BASE: usize = 0x14a0;
const IOCSR_EXTIOI_IPMAP_BASE: usize = 0x14c0;
const IOCSR_EXTIOI_BOUNCE_BASE: usize = 0x1680;
const IOCSR_EXTIOI_POL_BASE: usize = 0x1820; // 极性控制寄存器

const EXTIOI_IRQS: usize = 256;
const EXTIOI_IRQS_PER_GROUP: usize = 32;

// LoongArch 扩展 I/O 中断控制器
pub struct LoongArchEXTIOI {
    base: usize,
    inner: Arc<Mutex<EXTIOIInner>>,
}

struct EXTIOIInner {
    // 记录每个中断的使能状态
    enabled: [bool; EXTIOI_IRQS],
    // 记录每个中断的触发类型
    trigger_type: [TriggerType; EXTIOI_IRQS],
}

impl LoongArchEXTIOI {
    pub fn new() -> Self {
        // 使用非缓存窗口访问 IOCSR，确保读写立即生效
        let base = LOONGARCH_UNCACHED_BASE + IOCSR_BASE;

        Self {
            base,
            inner: Arc::new(Mutex::new(EXTIOIInner {
                enabled: [false; EXTIOI_IRQS],
                trigger_type: [TriggerType::HighLevel; EXTIOI_IRQS],
            })),
        }
    }

    // 通过 MMIO 方式读取
    #[inline]
    unsafe fn read_mmio(&self, offset: usize) -> u32 {
        let addr = self.base + offset;
        read_volatile(addr as *const u32)
    }

    // 通过 MMIO 方式写入
    #[inline]
    unsafe fn write_mmio(&self, offset: usize, value: u32) {
        let addr = self.base + offset;
        write_volatile(addr as *mut u32, value);
    }

    // 获取中断状态寄存器偏移
    fn get_isr_offset(&self, irq: usize) -> usize {
        IOCSR_EXTIOI_ISR_BASE + (irq / 32) * 4
    }

    // 获取中断使能寄存器偏移
    fn get_enable_offset(&self, irq: usize) -> usize {
        IOCSR_EXTIOI_EN_BASE + (irq / 32) * 4
    }

    // 获取中断路由寄存器偏移
    fn get_route_offset(&self, irq: usize) -> usize {
        IOCSR_EXTIOI_ROUTE_BASE + irq
    }

    // 获取节点类型寄存器偏移（边沿/电平触发）
    fn get_nodetype_offset(&self, irq: usize) -> usize {
        IOCSR_EXTIOI_NODETYPE_BASE + (irq / 64) * 4
    }

    // 获取极性寄存器偏移（高/低电平，上升/下降沿）
    fn get_polarity_offset(&self, irq: usize) -> usize {
        IOCSR_EXTIOI_POL_BASE + (irq / 32) * 4
    }

    // 检查是否为边沿触发
    fn is_edge_triggered(&self, trigger: TriggerType) -> bool {
        matches!(trigger, TriggerType::RisingEdge | TriggerType::FallingEdge)
    }

    // 检查是否为高电平/上升沿触发
    fn is_positive_triggered(&self, trigger: TriggerType) -> bool {
        matches!(trigger, TriggerType::HighLevel | TriggerType::RisingEdge)
    }
}

impl ICU for LoongArchEXTIOI {
    fn enable_irq(&self, irq: usize, ctx_id: usize) {
        if irq >= EXTIOI_IRQS {
            return;
        }

        let mut inner = self.inner.lock();
        inner.enabled[irq] = true;

        unsafe {
            // 设置中断路由到指定的 CPU
            let route_offset = self.get_route_offset(irq);
            self.write_mmio(route_offset & !3, ctx_id as u32);

            // 使能中断
            let bit = irq % 32;
            let enable_offset = self.get_enable_offset(irq);
            let old_val = self.read_mmio(enable_offset);
            self.write_mmio(enable_offset, old_val | (1 << bit));

            // 确保写入完成
            core::sync::atomic::fence(core::sync::atomic::Ordering::SeqCst);
        }
    }

    fn disable_irq(&self, irq: usize) {
        if irq >= EXTIOI_IRQS {
            return;
        }

        let mut inner = self.inner.lock();
        inner.enabled[irq] = false;

        unsafe {
            // 禁用中断
            let bit = irq % 32;
            let enable_offset = self.get_enable_offset(irq);
            let old_val = self.read_mmio(enable_offset);
            self.write_mmio(enable_offset, old_val & !(1 << bit));

            // 确保写入完成
            core::sync::atomic::fence(core::sync::atomic::Ordering::SeqCst);
        }
    }

    fn claim_irq(&self, _ctx_id: usize) -> Option<usize> {
        unsafe {
            // 遍历所有中断组，查找待处理的中断
            for group in 0..(EXTIOI_IRQS / EXTIOI_IRQS_PER_GROUP) {
                let isr_offset = IOCSR_EXTIOI_ISR_BASE + group * 4;
                let isr = self.read_mmio(isr_offset);

                if isr != 0 {
                    // 找到第一个置位的中断
                    let bit = isr.trailing_zeros() as usize;
                    let irq = group * EXTIOI_IRQS_PER_GROUP + bit;

                    // 对于边沿触发的中断，需要清除中断状态
                    let inner = self.inner.lock();
                    if self.is_edge_triggered(inner.trigger_type[irq]) {
                        drop(inner); // 释放锁以避免死锁
                        self.write_mmio(isr_offset, 1 << bit);
                    }

                    return Some(irq);
                }
            }
        }

        None
    }

    fn complete_irq(&self, irq: usize, _cpu_id: usize) {
        if irq >= EXTIOI_IRQS {
            return;
        }

        // 对于电平触发的中断，在这里清除中断状态
        let inner = self.inner.lock();
        if !self.is_edge_triggered(inner.trigger_type[irq]) {
            unsafe {
                let bit = irq % 32;
                let isr_offset = self.get_isr_offset(irq);
                self.write_mmio(isr_offset, 1 << bit);

                // 确保写入完成
                core::sync::atomic::fence(core::sync::atomic::Ordering::SeqCst);
            }
        }
    }

    fn set_trigger_type(&self, irq: usize, trigger: TriggerType) {
        if irq >= EXTIOI_IRQS {
            return;
        }

        let mut inner = self.inner.lock();
        inner.trigger_type[irq] = trigger;

        unsafe {
            // 设置边沿/电平触发模式
            let nodetype_offset = self.get_nodetype_offset(irq);
            let old_val = self.read_mmio(nodetype_offset);
            let bit_offset = (irq % 64) / 2;

            let new_val = if self.is_edge_triggered(trigger) {
                old_val | (1 << bit_offset) // 边沿触发
            } else {
                old_val & !(1 << bit_offset) // 电平触发
            };

            self.write_mmio(nodetype_offset, new_val);

            // 设置触发极性（高/低电平，上升/下降沿）
            let pol_offset = self.get_polarity_offset(irq);
            let pol_bit = irq % 32;
            let old_pol = self.read_mmio(pol_offset);

            let new_pol = if self.is_positive_triggered(trigger) {
                old_pol | (1 << pol_bit) // 高电平/上升沿
            } else {
                old_pol & !(1 << pol_bit) // 低电平/下降沿
            };

            self.write_mmio(pol_offset, new_pol);

            core::sync::atomic::fence(core::sync::atomic::Ordering::SeqCst);
        }
    }
}

lazy_static::lazy_static! {
    pub static ref LOONGARCH_ICU: LoongArchEXTIOI = {
        LoongArchEXTIOI::new()
    };
}

pub fn init_icu() {
    unsafe {
        for i in 0..4 {
            let offset = IOCSR_EXTIOI_IPMAP_BASE + i * 4;
            LOONGARCH_ICU.write_mmio(offset, 0x0);
        }
    }

    for irq in 0..EXTIOI_IRQS {
        LOONGARCH_ICU.set_trigger_type(irq, TriggerType::HighLevel);
        LOONGARCH_ICU.disable_irq(irq);
    }

    println!(
        "LoongArch EXTIOI initialized at 0x{:x}",
        LOONGARCH_UNCACHED_BASE + IOCSR_BASE
    );
}

pub fn dump_icu_status() {
    unsafe {
        println!("EXTIOI Status:");
        for group in 0..8 {
            let en_offset = IOCSR_EXTIOI_EN_BASE + group * 4;
            let isr_offset = IOCSR_EXTIOI_ISR_BASE + group * 4;
            let pol_offset = IOCSR_EXTIOI_POL_BASE + group * 4;

            let en = LOONGARCH_ICU.read_mmio(en_offset);
            let isr = LOONGARCH_ICU.read_mmio(isr_offset);
            let pol = LOONGARCH_ICU.read_mmio(pol_offset);

            if en != 0 || isr != 0 {
                println!(
                    "  Group {}: EN={:#010x}, ISR={:#010x}, POL={:#010x}",
                    group, en, isr, pol
                );
            }
        }
    }
}
