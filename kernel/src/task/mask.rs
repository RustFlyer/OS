use bitflags::bitflags;

bitflags! {
    #[derive(Clone, Copy)]
    #[repr(C)]
    pub struct CpuMask: usize {
        const CPU0 = 0b00000001;
        const CPU1 = 0b00000010;
        const CPU2 = 0b00000100;
        const CPU3 = 0b00001000;
        const CPU4 = 0b00010000;
        const CPU5 = 0b00100000;
        const CPU6 = 0b01000000;
        const CPU7 = 0b10000000;
        const CPU_ALL = 0b11111111;
    }
}
