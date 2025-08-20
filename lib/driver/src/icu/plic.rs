//! RISC-V Platform Level Interrupt Controller
//!
//! Adapted from PhoenixOS
//!
//! Controller setup helper

use config::mm::KERNEL_MAP_OFFSET;

use super::{ICU, icu_lavirt::TriggerType};
pub struct PLIC {
    /// MMIO base address.
    pub mmio_base: usize,
    /// MMIO region size.
    pub mmio_size: usize,
}

// const PLIC_ADDR: usize = 0xc00_0000 + VIRT_RAM_OFFSET;

impl PLIC {
    pub fn new(mmio_base: usize, mmio_size: usize) -> PLIC {
        PLIC {
            mmio_base,
            mmio_size,
        }
    }

    pub(crate) fn _enable_irq(&self, irq: usize, ctx_id: usize) {
        log::error!("enable irq {irq}, ctx_id: {ctx_id}");
        let plic = (self.mmio_base + KERNEL_MAP_OFFSET) as *mut plic::Plic;

        // Setup PLIC
        let src = PLICSrcWrapper::new(irq);
        let ctx = PLICCtxWrapper::new(ctx_id);

        unsafe { (*plic).set_threshold(ctx, 0) };
        unsafe { (*plic).enable(src, ctx) };
        unsafe { (*plic).set_priority(src, 6) };
    }

    /// Return the IRQ number of the highest priority pending interrupt
    pub(crate) fn _claim_irq(&self, ctx_id: usize) -> Option<usize> {
        let plic = (self.mmio_base + KERNEL_MAP_OFFSET) as *mut plic::Plic;
        let ctx = PLICCtxWrapper::new(ctx_id);

        let irq = unsafe { (*plic).claim(ctx) };
        irq.map(|irq| irq.get() as usize)
    }

    pub(crate) fn _complete_irq(&self, irq: usize, ctx_id: usize) {
        let plic = (self.mmio_base + KERNEL_MAP_OFFSET) as *mut plic::Plic;
        let src = PLICSrcWrapper::new(irq);
        let ctx = PLICCtxWrapper::new(ctx_id);
        unsafe { (*plic).complete(ctx, src) };
    }
}

#[derive(Debug, Clone, Copy)]
struct PLICSrcWrapper {
    irq: usize,
}

impl PLICSrcWrapper {
    fn new(irq: usize) -> Self {
        Self { irq }
    }
}

impl plic::InterruptSource for PLICSrcWrapper {
    fn id(self) -> core::num::NonZeroU32 {
        core::num::NonZeroU32::try_from(self.irq as u32).unwrap()
    }
}

#[derive(Debug, Clone, Copy)]
struct PLICCtxWrapper {
    ctx: usize,
}

impl PLICCtxWrapper {
    fn new(ctx: usize) -> Self {
        Self { ctx }
    }
}

impl plic::HartContext for PLICCtxWrapper {
    fn index(self) -> usize {
        self.ctx
    }
}

impl ICU for PLIC {
    fn enable_irq(&self, irq: usize, ctx_id: usize) {
        self._enable_irq(irq, ctx_id)
    }

    fn disable_irq(&self, irq: usize) {}

    fn claim_irq(&self, ctx_id: usize) -> Option<usize> {
        self._claim_irq(ctx_id)
    }

    fn complete_irq(&self, irq: usize, _cpu_id: usize) {
        self._complete_irq(irq, _cpu_id);
    }

    fn set_trigger_type(&self, irq: usize, trigger: TriggerType) {
        todo!()
    }
}
