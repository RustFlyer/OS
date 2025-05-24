.macro LOAD_CR n
    ld.d $s\n, $sp, (\n+36)*8
.endm

.macro LOAD_GP n
    ld x\n, \n*8(sp)
.endm
    .section .text   //originally k_eentry
    .globl __trap_from_user
    .globl __return_to_user
    .globl __trap_from_kernel
    .globl __user_rw_trap_vector
    .globl __user_rw_exception_entry
    .globl __try_read_user
    .globl __try_write_user


    .align 12
__trap_from_user:
    # swap sp and kernel stack pointer in KSAVE_CTX, as sstratch in RV
    csrwr   $sp, 0x31
    # Save general-purpose registers
    # Note: sp is r3 in LoongArch, which is different from RV's sp
    st.d    $ra, $sp,  1*8
    st.d    $tp, $sp,  2*8
    st.d    $a0, $sp,  4*8
    st.d    $a1, $sp,  5*8
    st.d    $a2, $sp,  6*8
    st.d    $a3, $sp,  7*8
    st.d    $a4, $sp,  8*8
    st.d    $a5, $sp,  9*8
    st.d    $a6, $sp, 10*8
    st.d    $a7, $sp, 11*8
    st.d    $t0, $sp, 12*8
    st.d    $t1, $sp, 13*8
    st.d    $t2, $sp, 14*8
    st.d    $t3, $sp, 15*8
    st.d    $t4, $sp, 16*8
    st.d    $t5, $sp, 17*8
    st.d    $t6, $sp, 18*8
    st.d    $t7, $sp, 19*8
    st.d    $t8, $sp, 20*8
    st.d    $r21,$sp, 21*8
    st.d    $fp, $sp, 22*8
    st.d    $s0, $sp, 23*8
    st.d    $s1, $sp, 24*8
    st.d    $s2, $sp, 25*8
    st.d    $s3, $sp, 26*8
    st.d    $s4, $sp, 27*8
    st.d    $s5, $sp, 28*8
    st.d    $s6, $sp, 29*8
    st.d    $s7, $sp, 30*8
    st.d    $s8, $sp, 31*8

    csrrd   $t0, 0x1        #prmd, as sstatus in RV
    csrrd   $t1, 0x6        #era, as sepc in RV
    st.d    $t0, $sp, 32*8
    st.d    $t1, $sp, 33*8

    csrrd   $t2, 0x31       #read user stack pointer in KSAVE_CTX into t2
    st.d    $t2, $sp, 3*8   #and save it in TrapContext

    ld.d    $ra, $sp, 35*8   #load kernel fn trap_return() address
    // load callee-saved registers, note that tp and r21 can't be used
    // (https://loongson.github.io/LoongArch-Documentation/LoongArch-ELF-ABI-CN.html)
    ld.d    $tp,  $sp, 36*8
    ld.d    $r21, $sp, 37*8
    ld.d    $s9,  $sp, 38*8
    ld.d    $s0,  $sp, 39*8
    ld.d    $s1,  $sp, 40*8
    ld.d    $s2,  $sp, 41*8
    ld.d    $s3,  $sp, 42*8
    ld.d    $s4,  $sp, 43*8
    ld.d    $s5,  $sp, 44*8
    ld.d    $s6,  $sp, 45*8
    ld.d    $s7,  $sp, 46*8
    ld.d    $s8,  $sp, 47*8

    ld.d $fp, $sp, 48*8
    ld.d $tp, $sp, 49*8

    ld.d $sp, $sp, 34*8

    // jump to ra(fn trap_return) without offset, drop current pc+4 to r0
    // Sugar: jr $ra
    jirl $r0, $ra, 0

__return_to_user:
    // Save kernel callee-saved registers
    st.d $sp, $a0, 34*8
    st.d $ra, $a0, 35*8
    st.d $tp, $a0, 36*8
    st.d $r21, $a0, 37*8
    st.d $s9, $a0, 38*8
    st.d $s0, $a0, 39*8
    st.d $s1, $a0, 40*8
    st.d $s2, $a0, 41*8
    st.d $s3, $a0, 42*8
    st.d $s4, $a0, 43*8
    st.d $s5, $a0, 44*8
    st.d $s6, $a0, 45*8
    st.d $s7, $a0, 46*8
    st.d $s8, $a0, 47*8
    st.d $fp, $a0, 48*8
    st.d $tp, $a0, 49*8

    move $sp, $a0
    csrwr $a0, 0x31

    ld.d $t0, $sp, 32*8
    ld.d $t1, $sp, 33*8
    csrwr $t0, 0x1
    csrwr $t1, 0x6

    ld.d    $ra, $sp, 1*8
    ld.d    $tp, $sp, 2*8
    ld.d    $a0, $sp, 4*8
    ld.d    $a1, $sp, 5*8
    ld.d    $a2, $sp, 6*8
    ld.d    $a3, $sp, 7*8
    ld.d    $a4, $sp, 8*8
    ld.d    $a5, $sp, 9*8
    ld.d    $a6, $sp, 10*8
    ld.d    $a7, $sp, 11*8
    ld.d    $t0, $sp, 12*8
    ld.d    $t1, $sp, 13*8
    ld.d    $t2, $sp, 14*8
    ld.d    $t3, $sp, 15*8
    ld.d    $t4, $sp, 16*8
    ld.d    $t5, $sp, 17*8
    ld.d    $t6, $sp, 18*8
    ld.d    $t7, $sp, 19*8
    ld.d    $t8, $sp, 20*8
    ld.d    $r21,$sp, 21*8
    ld.d    $fp, $sp, 22*8
    ld.d    $s0, $sp, 23*8
    ld.d    $s1, $sp, 24*8
    ld.d    $s2, $sp, 25*8
    ld.d    $s3, $sp, 26*8
    ld.d    $s4, $sp, 27*8
    ld.d    $s5, $sp, 28*8
    ld.d    $s6, $sp, 29*8
    ld.d    $s7, $sp, 30*8
    ld.d    $s8, $sp, 31*8

    ld.d    $sp, $sp, 3*8

    ertn

# kernel -> kernel
    .align 12
__trap_from_kernel:
    # only need to save caller-saved regs
    addi.d $sp, $sp, -19*8
    st.d  $ra, $sp, 1*8
    st.d  $t0, $sp, 2*8
    st.d  $t1, $sp, 3*8
    st.d  $t2, $sp, 4*8
    st.d  $t3, $sp, 5*8
    st.d  $t4, $sp, 6*8
    st.d  $t5, $sp, 7*8
    st.d  $t6, $sp, 8*8
    st.d  $t7, $sp, 9*8
    st.d  $t8, $sp, 10*8
    st.d  $a0, $sp, 11*8
    st.d  $a1, $sp, 12*8
    st.d  $a2, $sp, 13*8
    st.d  $a3, $sp, 14*8
    st.d  $a4, $sp, 15*8
    st.d  $a5, $sp, 16*8
    st.d  $a6, $sp, 17*8
    st.d  $a7, $sp, 18*8

    la.abs  $t0, kernel_trap_handler
    jirl $r0, $t0, 0

    ld.d  $ra, $sp, 1*8
    ld.d  $t0, $sp, 2*8
    ld.d  $t1, $sp, 3*8
    ld.d  $t2, $sp, 4*8
    ld.d  $t3, $sp, 5*8
    ld.d  $t4, $sp, 6*8
    ld.d  $t5, $sp, 7*8
    ld.d  $t6, $sp, 8*8
    ld.d  $t7, $sp, 9*8
    ld.d  $t8, $sp, 10*8
    ld.d  $a0, $sp, 11*8
    ld.d  $a1, $sp, 12*8
    ld.d  $a2, $sp, 13*8
    ld.d  $a3, $sp, 14*8
    ld.d  $a4, $sp, 15*8
    ld.d  $a5, $sp, 16*8
    ld.d  $a6, $sp, 17*8
    ld.d  $a7, $sp, 18*8
    addi.d $sp, $sp, 19*8
    ertn

__try_read_user:
    move $a1, $a0
    move $a0, $r0

    ld.b $a1, $a1, 0
    jirl $r0, $ra, 0 

__try_write_user:
    move $a2, $a0
    move $a0, $r0
    ld.b $a1, $a2, 0
    st.b $a1, $a2, 0
    jirl $r0, $ra, 0

__user_rw_exception_entry:
    csrrd $a0, 0x6
    addi.d $a0, $a0, 4
    csrwr $a0, 0x6
    addi.d $a0, $a0, 1
    csrrd $a1, 0x5
    ertn

    .align 12
__user_rw_trap_vector:
    .rept 64
    .align 3
    la.abs  $t0, __user_rw_exception_entry
    jirl $r0, $t0, 0
    .endr
    .rept 13
    .align 3
    la.abs  $t0, __trap_from_kernel
    jirl $r0, $t0, 0
    .endr
