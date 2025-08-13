macro_rules! includes_trap_macros {
    () => {
        r#"
        .ifndef REGS_TRAP_MACROS_FLAG
        .equ REGS_TRAP_MACROS_FLAG, 1

        // 2, 4, 1
        .macro FIXUP_EX from, to, fix
        .if \fix
            .section .fixup, "ax"
        \to: 
            li.w	$a0, -1
            jr	$ra
            .previous
        .endif
            .section __ex_table, "a"
            .word	\from\()b, \to\()b
            .previous
        .endm

        .endif
        "#
    }
}
use core::arch::naked_asm;
use loongArch64::register::badv;

use crate::trap::trap_context::{KernelTrapContext, TrapContext};

// Load/Store opcodes - reg2i12_format
pub const LDB_OP: u32 = 0xa0;
pub const LDH_OP: u32 = 0xa1;
pub const LDW_OP: u32 = 0xa2;
pub const LDD_OP: u32 = 0xa3;
pub const STB_OP: u32 = 0xa4;
pub const STH_OP: u32 = 0xa5;
pub const STW_OP: u32 = 0xa6;
pub const STD_OP: u32 = 0xa7;
pub const LDBU_OP: u32 = 0xa8;
pub const LDHU_OP: u32 = 0xa9;
pub const LDWU_OP: u32 = 0xaa;

// Load/Store opcodes - reg2i14_format
pub const LDPTRW_OP: u32 = 0x24;
pub const STPTRW_OP: u32 = 0x25;
pub const LDPTRD_OP: u32 = 0x26;
pub const STPTRD_OP: u32 = 0x27;

// Load/Store opcodes - reg3_format
pub const LDXB_OP: u32 = 0x7000;
pub const LDXH_OP: u32 = 0x7008;
pub const LDXW_OP: u32 = 0x7010;
pub const LDXD_OP: u32 = 0x7018;
pub const STXB_OP: u32 = 0x7020;
pub const STXH_OP: u32 = 0x7028;
pub const STXW_OP: u32 = 0x7030;
pub const STXD_OP: u32 = 0x7038;
pub const LDXBU_OP: u32 = 0x7040;
pub const LDXHU_OP: u32 = 0x7048;
pub const LDXWU_OP: u32 = 0x7050;

// FPU Load/Store opcodes
pub const FLDS_OP: u32 = 0xac;
pub const FSTS_OP: u32 = 0xad;
pub const FLDD_OP: u32 = 0xae;
pub const FSTD_OP: u32 = 0xaf;
pub const FLDXS_OP: u32 = 0x7060;
pub const FLDXD_OP: u32 = 0x7068;
pub const FSTXS_OP: u32 = 0x7070;
pub const FSTXD_OP: u32 = 0x7078;

#[allow(binary_asm_labels)]
#[naked]
unsafe extern "C" fn unaligned_read(addr: u64, value: &mut u64, n: u64, sign: u32) -> i64 {
    unsafe {
        naked_asm!(
            includes_trap_macros!(),
            "
            beqz    $a2, 5f

            li.d    $t2, 0          // 初始化结果为0
            move    $t4, $a0        // 保存起始地址
            move    $t5, $a2        // 保存字节数

            // 对于小端系统，从低地址开始读取
            // 第一个字节放在最低位
            li.w    $t1, 0          // 当前位移量，从0开始

        2:  ld.bu   $t3, $t4, 0    // 总是使用无符号读取
            sll.d   $t3, $t3, $t1  // 左移当前位移量
            or      $t2, $t2, $t3  // OR到结果中
            
            addi.d  $t1, $t1, 8    // 下一个字节的位移量
            addi.d  $t4, $t4, 1    // 下一个地址
            addi.d  $a2, $a2, -1   // 减少计数
            bgt     $a2, $zero, 2b // 继续读取

            // 处理符号扩展
            beq     $a3, $zero, 4f // 如果是无符号，跳过符号扩展

            // 有符号扩展处理
            li.w    $t0, 8
            mul.d   $t0, $t5, $t0  // t0 = n * 8 (总位数)
            li.d    $t1, 64
            sub.d   $t1, $t1, $t0  // t1 = 64 - (n * 8)
            sll.d   $t2, $t2, $t1  // 左移到最高位
            sra.d   $t2, $t2, $t1  // 算术右移回来，实现符号扩展

        4:  st.d    $t2, $a1, 0    // 存储结果

            move    $a0, $zero     // 返回0表示成功
            jr      $ra

        5:  li.w    $a0, -1        // 返回-1表示失败
            jr      $ra

        6:  li.w    $a0, -1        // fixup返回-1
            jr      $ra

            FIXUP_EX 2, 6, 1
            FIXUP_EX 4, 6, 0
            ",
        )
    }
}

#[allow(binary_asm_labels)]
#[naked]
unsafe extern "C" fn unaligned_write(addr: u64, value: u64, n: u64) -> i64 {
    unsafe {
        naked_asm!(
            includes_trap_macros!(),
            "
            beqz    $a2, 3f

            move    $t4, $a0        // 保存起始地址
            move    $t5, $a1        // 保存要写入的值
            li.w    $t0, 0          // 当前位移量

        1:  andi    $t1, $t5, 0xFF // 获取最低字节
            st.b    $t1, $t4, 0    // 存储字节
            
            srli.d  $t5, $t5, 8    // 右移8位，准备下一个字节
            addi.d  $t4, $t4, 1    // 下一个地址
            addi.d  $a2, $a2, -1   // 减少计数
            bgt     $a2, $zero, 1b // 继续写入

            move    $a0, $zero     // 返回0表示成功
            jr      $ra

        3:  li.w    $a0, -1        // 返回-1表示失败
            jr      $ra

        4:  li.w    $a0, -1        // fixup返回-1
            jr      $ra

            FIXUP_EX 1, 4, 1
            ",
        )
    }
}

// Helper functions for FPU register access (simplified version)
#[inline(always)]
unsafe fn read_fpr(fd: usize) -> u64 {
    // This is a simplified version. In real implementation,
    // you need to use inline assembly to read FPU registers
    // For now, just return 0
    panic!("read fpr fail");
    0
}

#[inline(always)]
unsafe fn write_fpr(fd: usize, value: u64) {
    // This is a simplified version. In real implementation,
    // you need to use inline assembly to write FPU registers
    panic!("write fpr fail");
}

#[allow(unused_assignments)]
#[allow(unsafe_op_in_unsafe_fn)]
pub unsafe fn emulate_load_store_insn(pt_regs: &mut KernelTrapContext) {
    let mut insn: u32;
    let addr: u64;
    let rd: usize;
    let mut value: u64 = 0;
    let mut res: i64;

    // Fetch the instruction that caused the exception
    unsafe {
        core::arch::asm!(
            "ld.w {val}, {addr}, 0",
            addr = in(reg) pt_regs.sepc as u64,
            val = out(reg) insn,
        )
    }

    // Get the bad address that caused the unaligned access
    addr = badv::read().vaddr() as u64;

    // Extract destination register
    rd = (insn & 0x1f) as usize;
    let log_value = pt_regs.user_reg[rd];

    assert!(rd != 3, "sp cannot be handled!");

    // Decode instruction format and opcode
    let opcode_reg2i12 = (insn >> 22) & 0x3ff;
    let opcode_reg2i14 = (insn >> 24) & 0xff;
    let opcode_reg3 = (insn >> 15) & 0x7fff;
    log::warn!("-----------------------------------------");
    log::warn!(
        "Unaligned Access PC @ {:#x} bad addr: {:#x}",
        pt_regs.sepc,
        addr
    );

    log::warn!(
        "opcode_reg2i12: {:#x} opcode_reg2i14: {:#x} opcode_reg3: {:#x}",
        opcode_reg2i12,
        opcode_reg2i14,
        opcode_reg3
    );

    // Handle different instruction types
    match opcode_reg2i12 {
        LDD_OP => {
            res = unaligned_read(addr, &mut value, 8, 1);
            if res < 0 {
                panic!("Address Error @ {:#x}", addr);
            }
            pt_regs.user_reg[rd] = value as usize;
        }
        LDW_OP => {
            res = unaligned_read(addr, &mut value, 4, 1);
            if res < 0 {
                panic!("Address Error @ {:#x}", addr);
            }
            pt_regs.user_reg[rd] = (((value & 0xffffffff) as i32) as i64) as usize;
        }
        LDWU_OP => {
            res = unaligned_read(addr, &mut value, 4, 0);
            if res < 0 {
                panic!("Address Error @ {:#x}", addr);
            }
            pt_regs.user_reg[rd] = (value & 0xffffffff) as usize;
        }
        LDH_OP => {
            res = unaligned_read(addr, &mut value, 2, 1);
            if res < 0 {
                panic!("Address Error @ {:#x}", addr);
            }
            pt_regs.user_reg[rd] = (((value & 0xffff) as i16) as i64) as usize;
        }
        LDHU_OP => {
            res = unaligned_read(addr, &mut value, 2, 0);
            if res < 0 {
                panic!("Address Error @ {:#x}", addr);
            }
            pt_regs.user_reg[rd] = (value & 0xffff) as usize;
        }
        STD_OP => {
            value = pt_regs.user_reg[rd] as u64;

            res = unaligned_write(addr, value, 8);
            if res < 0 {
                panic!("Address Error @ {:#x}", addr);
            }
        }
        STW_OP => {
            value = pt_regs.user_reg[rd] as u64;

            res = unaligned_write(addr, value, 4);
            if res < 0 {
                panic!("Address Error @ {:#x}", addr);
            }
        }
        STH_OP => {
            value = pt_regs.user_reg[rd] as u64;

            res = unaligned_write(addr, value, 2);
            if res < 0 {
                panic!("Address Error @ {:#x}", addr);
            }
        }
        FLDD_OP => {
            res = unaligned_read(addr, &mut value, 8, 1);
            if res < 0 {
                panic!("Address Error @ {:#x}", addr);
            }
            write_fpr(rd, value);
        }
        FLDS_OP => {
            res = unaligned_read(addr, &mut value, 4, 1);
            if res < 0 {
                panic!("Address Error @ {:#x}", addr);
            }
            write_fpr(rd, value);
        }
        FSTD_OP => {
            value = read_fpr(rd);
            res = unaligned_write(addr, value, 8);
            if res < 0 {
                panic!("Address Error @ {:#x}", addr);
            }
        }
        FSTS_OP => {
            value = read_fpr(rd);
            res = unaligned_write(addr, value, 4);
            if res < 0 {
                panic!("Address Error @ {:#x}", addr);
            }
        }
        _ => {
            // Check reg2i14 format instructions
            match opcode_reg2i14 {
                LDPTRD_OP => {
                    res = unaligned_read(addr, &mut value, 8, 1);
                    if res < 0 {
                        panic!("Address Error @ {:#x}", addr);
                    }
                    pt_regs.user_reg[rd] = value as usize;
                }
                LDPTRW_OP => {
                    res = unaligned_read(addr, &mut value, 4, 1);
                    if res < 0 {
                        panic!("Address Error @ {:#x}", addr);
                    }
                    pt_regs.user_reg[rd] = ((value as i32) as i64) as usize;
                }
                STPTRD_OP => {
                    value = pt_regs.user_reg[rd] as u64;
                    res = unaligned_write(addr, value, 8);
                    if res < 0 {
                        panic!("Address Error @ {:#x}", addr);
                    }
                }
                STPTRW_OP => {
                    value = pt_regs.user_reg[rd] as u64;
                    res = unaligned_write(addr, value, 4);
                    if res < 0 {
                        panic!("Address Error @ {:#x}", addr);
                    }
                }
                _ => {
                    // Check reg3 format instructions
                    match opcode_reg3 {
                        LDXD_OP => {
                            res = unaligned_read(addr, &mut value, 8, 1);
                            if res < 0 {
                                panic!("Address Error @ {:#x}", addr);
                            }
                            pt_regs.user_reg[rd] = value as usize;
                        }
                        LDXW_OP => {
                            res = unaligned_read(addr, &mut value, 4, 1);
                            if res < 0 {
                                panic!("Address Error @ {:#x}", addr);
                            }
                            pt_regs.user_reg[rd] = (((value & 0xffffffff) as i32) as i64) as usize;
                        }
                        LDXWU_OP => {
                            res = unaligned_read(addr, &mut value, 4, 0);
                            if res < 0 {
                                panic!("Address Error @ {:#x}", addr);
                            }
                            pt_regs.user_reg[rd] = (value & 0xffffffff) as usize;
                        }
                        LDXH_OP => {
                            res = unaligned_read(addr, &mut value, 2, 1);
                            if res < 0 {
                                panic!("Address Error @ {:#x}", addr);
                            }
                            pt_regs.user_reg[rd] = (((value & 0xffff) as i16) as i64) as usize;
                        }
                        LDXHU_OP => {
                            res = unaligned_read(addr, &mut value, 2, 0);
                            if res < 0 {
                                panic!("Address Error @ {:#x}", addr);
                            }
                            pt_regs.user_reg[rd] = (value & 0xffff) as usize;
                        }
                        STXD_OP => {
                            value = pt_regs.user_reg[rd] as u64;
                            res = unaligned_write(addr, value, 8);
                            if res < 0 {
                                panic!("Address Error @ {:#x}", addr);
                            }
                        }
                        STXW_OP => {
                            value = pt_regs.user_reg[rd] as u64;
                            res = unaligned_write(addr, value, 4);
                            if res < 0 {
                                panic!("Address Error @ {:#x}", addr);
                            }
                        }
                        STXH_OP => {
                            value = pt_regs.user_reg[rd] as u64;
                            res = unaligned_write(addr, value, 2);
                            if res < 0 {
                                panic!("Address Error @ {:#x}", addr);
                            }
                        }
                        FLDXD_OP => {
                            res = unaligned_read(addr, &mut value, 8, 1);
                            if res < 0 {
                                panic!("Address Error @ {:#x}", addr);
                            }
                            write_fpr(rd, value);
                        }
                        FLDXS_OP => {
                            res = unaligned_read(addr, &mut value, 4, 1);
                            if res < 0 {
                                panic!("Address Error @ {:#x}", addr);
                            }
                            write_fpr(rd, value);
                        }
                        FSTXD_OP => {
                            value = read_fpr(rd);
                            res = unaligned_write(addr, value, 8);
                            if res < 0 {
                                panic!("Address Error @ {:#x}", addr);
                            }
                        }
                        FSTXS_OP => {
                            value = read_fpr(rd);
                            res = unaligned_write(addr, value, 4);
                            if res < 0 {
                                panic!("Address Error @ {:#x}", addr);
                            }
                        }
                        _ => {
                            panic!(
                                "unhandled unaligned instruction {:#x} at PC {:#x}",
                                insn, pt_regs.sepc
                            );
                        }
                    }
                }
            }
        }
    }
    log::warn!(
        "Unaligned rd: {} inst: {:#x}, reg[rd]: {:#x} -> {:#x}",
        rd,
        insn,
        log_value,
        pt_regs.user_reg[rd]
    );
    log::warn!("-----------------------------------------");

    // Memory barrier to ensure all memory operations complete
    arch::mm::fence();

    // Advance to next instruction
    pt_regs.sepc += 4;
}
