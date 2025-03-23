.macro SAVE_GP n
    st.d $r\n, $sp, \n*8
.endm

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
    .align 4

__trap_from_user:
    # swap sp and kernel stack pointer in DSAVE, as sstratch in RV
    csrwr   $sp, 0x502
    # Save general-purpose registers
    st.d    $r1, $sp, 1*8
    st.d    $r2, $sp, 2*8
    # sp is r3 in loongarch
    .set n, 4
    .rept 28
        SAVE_GP %n
        .set n, n+1
    .endr

    csrrd   $t0, 0x1        #prmd, as sstatus in RV
    csrrd   $t1, 0x6        #era, as sepc in RV
    st.d    $t0, $sp, 32*8
    st.d    $t1, $sp, 33*8

    csrrd   $t2, 0x502       #read user stack pointer in DSAVE into t2
    st.d    $t2, $sp, 3*8   #and save it in TrapContext

    ld.d    $ra, $sp, 35*8   #load kernel fn trap_return() address
    // load callee-saved registers (s0-s11)
    .set m, 0
    .rept 12
        LOAD_CR %m
        .set m, m+1
    .endr

    ld.d $fp, $sp, 48*8
    ld.d $tp, $sp, 49*8

    ld.d $sp, $sp, 34*8

    // jump to ra(fn trap_return) without offset, drop current pc+4 to r0
    //Sugar: jr $ra
    jirl r0, ra, 0

__return_to_user:
    // Save kernel callee-saved registers
    st.d $sp, $a0, 34*8
    st.d $ra, $a0, 35*8
    st.d $s0, $a0, 36*8
    st.d $s1, $a0, 37*8
    st.d $s2, $a0, 38*8
    st.d $s3, $a0, 39*8
    st.d $s4, $a0, 40*8
    st.d $s5, $a0, 41*8
    st.d $s6, $a0, 42*8
    st.d $s7, $a0, 43*8
    st.d $s8, $a0, 44*8
    st.d $s9, $a0, 45*8
    st.d $s10, $a0, 46*8
    st.d $s11, $a0, 47*8
    st.d $fp, $a0, 48*8
    st.d $tp, $a0, 49*8
    
    //XXX: rCoreloongArch use the macro "move", which represents add.d,
    //but 0x502 is a CSR, so I got no idea but replaced it with csrwr
    csrwr $a0 0x502
    csrrd $sp 0x502

    ld.d $t0, $sp, 32*8
    ld.d $t1, $sp, 33*8
    csrwr $t0, 0x1
    csrwr $t1, 0x6

    ld.d $r1, $sp, 1*8
    .set n, 3
    .rept 29
        LOAD_GP %n
        .set n, n+1
    .endr

    ld.d $sp, $sp, 2*8

    // XXX: ertn makes pc jump to ERA only when it's not Debug, 
    // Error or TLB exception.
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

    .align 8
__user_rw_trap_vector:
    #TODO: what does align do?