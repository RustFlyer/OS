use crate::device::OSDevice;

#[derive(Debug)]
pub struct CPU {
    pub id: usize,
    pub usable: bool, // is the CPU usable? we need MMU
    pub clock_freq: usize,
    pub timebase_freq: usize,
}

impl OSDevice for CPU {
    fn meta(&self) -> &crate::device::OSDeviceMeta {
        todo!()
    }

    fn init(&self) {
        todo!()
    }

    fn handle_irq(&self) {
        todo!()
    }
}
