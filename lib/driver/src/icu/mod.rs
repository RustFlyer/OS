pub mod icu2k1000;
pub mod plic;

pub trait ICU {
    fn enable_irq(&self, irq: usize, ctx_id: usize);
    fn disable_irq(&self, irq: usize);
    fn claim_irq(&self, ctx_id: usize) -> Option<usize>;
    fn complete_irq(&self, irq: usize, _cpu_id: usize);
}
