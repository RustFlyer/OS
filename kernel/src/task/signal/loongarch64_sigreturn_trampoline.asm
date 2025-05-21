    .section .text.trampoline
    .global _sigreturn_trampoline
_sigreturn_trampoline:
    li.d    $a7,139
    syscall 0
