use core::arch::global_asm;

#[cfg(target_arch = "riscv64")]
global_asm!(include_str!("riscv64_link_app.asm"));
#[cfg(target_arch = "loongarch64")]
global_asm!(include_str!("loongarch64_link_app.asm"));
