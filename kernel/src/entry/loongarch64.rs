use core::arch::naked_asm;

use crate::rust_main;

use super::BOOT_STACK;

#[naked]
#[unsafe(no_mangle)]
#[unsafe(link_section = ".text.entry")]
unsafe extern "C" fn _start() -> ! {
    unsafe {
        naked_asm!("
            ori         $t0, $zero, 0x1     # Set CSR.DMW0.PLV0 = 1
            lu52i.d     $t0, $t0, -2048     # Set CSR.DMW0.VSEC = 8
            csrwr       $t0, 0x180          # Write CSR.DMW0 = 0x8000_0000_0000_0001
            ori         $t0, $zero, 0x11    # Set CSR.DMW1.MAT = 1, CSR.DMW1.PLV0 = 1
            lu52i.d     $t0, $t0, -1792     # Set CSR.DMW1.VSEC = 9
            csrwr       $t0, 0x181          # Write CSR.DMW1 = 0x9000_0000_0000_0001

            # Enable mapped address translation mode
            li.w        $t0, 0xb0           # Set CRMD.PLV = 0, CRMD.IE = 0, CRMD.PG = 1
            csrwr       $t0, 0x0            # Write CSR.CRMD
            li.w        $t0, 0x0            # Clear PRMD.PPLV (seems not necessary)
            csrwr       $t0, 0x1            # Write CSR.PRMD
            li.w        $t0, 0x00           # Set FPE = 0, SXE = 0, ASXE = 0, BTE = 0
            csrwr       $t0, 0x2            # Write CSR.EUEN

            # Set up the stack pointer
            la.global   $sp, {boot_stack}
            addi.d      $t0, $t0, 1
            slli.d      $t0, $t0, 16        # t0 = (hart_id + 1) * KERNEL_STACK_SIZE
            add.d       $sp, $sp, $t0
            csrrd       $a0, 0x20           # Pass the hart id to rust_main as the first argument
            la.global   $t0, {entry}
            jirl        $zero,$t0,0
            ",
            boot_stack = sym BOOT_STACK,
            entry = sym rust_main,
        )
    }
}
