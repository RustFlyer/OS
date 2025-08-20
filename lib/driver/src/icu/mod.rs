use icu_lavirt::TriggerType;

pub mod cascaded;
pub mod ehic;
pub mod extioi;
pub mod icu2k1000;
pub mod icu_lavirt;
pub mod pch;
pub mod plic;

pub trait ICU {
    fn enable_irq(&self, irq: usize, ctx_id: usize);
    fn disable_irq(&self, irq: usize);
    fn claim_irq(&self, ctx_id: usize) -> Option<usize>;
    fn complete_irq(&self, irq: usize, _cpu_id: usize);
    fn set_trigger_type(&self, irq: usize, trigger: TriggerType);
}
