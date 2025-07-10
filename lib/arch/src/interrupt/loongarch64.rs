use core::arch::asm;

pub fn enable_external_interrupt() {
    const LIE_ALL_EXT: usize = 0x3f;
    let ecfg: usize;
    unsafe {
        asm!("csrrd {}, 0x4", out(reg) ecfg); // 0x4 is ECFG
        asm!(
            "csrwr {val}, 0x4",
            val = in(reg) (ecfg | LIE_ALL_EXT)
        );
    }
}
