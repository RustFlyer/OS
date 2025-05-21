    .section .text.trampoline
    .global _sigreturn_trampoline
_sigreturn_trampoline:
    li.w   $a7, 139         # $a7 = 139 (sys_sigreturn)
    syscall 0
