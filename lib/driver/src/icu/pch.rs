//! LoongArch PCH-PIC - Platform Controller Hub Programmable Interrupt Controller
//! 兼容 loongson,pch-pic-1.0
//! 用于 QEMU virt 机型

use config::mm::KERNEL_MAP_OFFSET;
use core::ptr::{read_volatile, write_volatile};

use super::{ICU, icu_lavirt::TriggerType};

/// PCH-PIC 中断控制器
/// 负责收集外设中断并转发给上级控制器（EIOINTC）
pub struct LoongArchPCHPIC {
    pub mmio_base: usize,
    pub mmio_size: usize,
    pub base_vec: u32,
}

/// PCH-PIC 寄存器定义
mod regs {
    // 中断屏蔽寄存器（RW）
    pub const INT_MASK: usize = 0x20; // 64位，每位对应一个中断

    // HT消息使能寄存器（RW）
    pub const HTMSG_EN: usize = 0x40; // 64位，使能向上级发送

    // 中断状态寄存器（RO）
    pub const INT_STATUS: usize = 0x3A0; // 64位，中断请求状态

    // 中断边沿寄存器（RW）
    pub const INT_EDGE: usize = 0x3E0; // 64位，0=电平 1=边沿

    // 中断清除寄存器（WO）
    pub const INT_CLEAR: usize = 0x3C0; // 64位，写1清除

    // 中断极性寄存器（RW）
    pub const INT_POL: usize = 0x3E8; // 64位，0=高/上升 1=低/下降

    // HT消息向量寄存器基址
    pub const HTMSG_VEC_BASE: usize = 0x200;
    pub const HTMSG_VEC_COUNT: usize = 64; // 64个中断

    // 自动轮询相关
    pub const AUTO_CTRL0: usize = 0x0C0;
    pub const AUTO_CTRL1: usize = 0x0E0;
}

const MAX_IRQS: usize = 64;

#[repr(C)]
struct HTMsgVector {
    raw: [u8; 8],
}

impl HTMsgVector {
    fn new(vector: u8, dest: u8) -> Self {
        let mut raw = [0u8; 8];

        raw[0] = vector;

        raw[1] = 0;

        raw[2] = dest;
        raw[3] = 0;

        raw[4] = 0x00;
        raw[5] = 0x00;
        raw[6] = 0x00;
        raw[7] = 0x01;

        Self { raw }
    }
}

impl LoongArchPCHPIC {
    pub fn new(mmio_base: usize, mmio_size: usize, base_vec: u32) -> Self {
        Self {
            mmio_base,
            mmio_size,
            base_vec,
        }
    }

    #[inline]
    fn base_ptr(&self) -> *mut u8 {
        (self.mmio_base + KERNEL_MAP_OFFSET) as *mut u8
    }

    /// 读64位寄存器
    unsafe fn read_reg64(&self, offset: usize) -> u64 {
        let ptr = self.base_ptr().add(offset) as *const u64;
        read_volatile(ptr)
    }

    /// 写64位寄存器
    unsafe fn write_reg64(&self, offset: usize, val: u64) {
        let ptr = self.base_ptr().add(offset) as *mut u64;
        write_volatile(ptr, val);
    }

    /// 设置中断触发类型
    pub fn set_trigger_type(&self, irq: usize, trigger: TriggerType) {
        if irq >= MAX_IRQS {
            log::error!("PCH-PIC: Invalid IRQ number: {}", irq);
            return;
        }

        let bit = 1u64 << irq;

        unsafe {
            let mut edge = self.read_reg64(regs::INT_EDGE);
            let mut pol = self.read_reg64(regs::INT_POL);

            match trigger {
                TriggerType::HighLevel => {
                    edge &= !bit; // 电平触发
                    pol &= !bit; // 高电平
                }
                TriggerType::LowLevel => {
                    edge &= !bit; // 电平触发
                    pol |= bit; // 低电平
                }
                TriggerType::RisingEdge => {
                    edge |= bit; // 边沿触发
                    pol &= !bit; // 上升沿
                }
                TriggerType::FallingEdge => {
                    edge |= bit; // 边沿触发
                    pol |= bit; // 下降沿
                }
            }

            self.write_reg64(regs::INT_EDGE, edge);
            self.write_reg64(regs::INT_POL, pol);
        }
    }

    fn configure_irq_vector(&self, irq: usize, eiointc_irq: u8) {
        if irq >= MAX_IRQS {
            return;
        }

        let base = self.base_ptr();
        let offset = regs::HTMSG_VEC_BASE + irq * 8;

        unsafe {
            let vec_ptr = base.add(offset);

            let vector = (self.base_vec as u8).wrapping_add(eiointc_irq);

            let vec = HTMsgVector::new(vector, 0);

            core::ptr::copy_nonoverlapping(vec.raw.as_ptr(), vec_ptr, 8);

            log::debug!(
                "PCH-PIC: IRQ {} -> EIOINTC IRQ {} (vector 0x{:02x})",
                irq,
                eiointc_irq,
                vector
            );
        }
    }

    pub(crate) fn _enable_irq(&self, irq: usize, cpu_id: usize) {
        if irq >= MAX_IRQS {
            log::error!("PCH-PIC: Invalid IRQ number: {}", irq);
            return;
        }

        log::info!("PCH-PIC: enable IRQ {}, CPU {}", irq, cpu_id);

        let bit = 1u64 << irq;

        unsafe {
            self.configure_irq_vector(irq, irq as u8);

            let htmsg_en = self.read_reg64(regs::HTMSG_EN);
            self.write_reg64(regs::HTMSG_EN, htmsg_en | bit);

            let mask = self.read_reg64(regs::INT_MASK);
            self.write_reg64(regs::INT_MASK, mask & !bit);
        }
    }

    pub(crate) fn _disable_irq(&self, irq: usize) {
        if irq >= MAX_IRQS {
            log::error!("PCH-PIC: Invalid IRQ number: {}", irq);
            return;
        }

        let bit = 1u64 << irq;

        unsafe {
            let mask = self.read_reg64(regs::INT_MASK);
            self.write_reg64(regs::INT_MASK, mask | bit);

            let htmsg_en = self.read_reg64(regs::HTMSG_EN);
            self.write_reg64(regs::HTMSG_EN, htmsg_en & !bit);
        }
    }

    pub(crate) fn _claim_irq(&self, _cpu_id: usize) -> Option<usize> {
        unsafe {
            let status = self.read_reg64(regs::INT_STATUS);
            let mask = self.read_reg64(regs::INT_MASK);
            let pending = status & !mask;

            if pending != 0 {
                let irq = pending.trailing_zeros() as usize;
                log::trace!("PCH-PIC: claimed IRQ {}", irq);
                return Some(irq);
            }
        }

        None
    }

    pub(crate) fn _complete_irq(&self, irq: usize, _cpu_id: usize) {
        if irq >= MAX_IRQS {
            log::error!("PCH-PIC: Invalid IRQ number: {}", irq);
            return;
        }

        let bit = 1u64 << irq;

        unsafe {
            self.write_reg64(regs::INT_CLEAR, bit);
        }
    }
}

impl ICU for LoongArchPCHPIC {
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
