OUTPUT_ARCH(riscv)
ENTRY(_start)

SECTIONS
{
    .text : ALIGN(4K) {
        _skernel = .;
        _stext = .;
        *(.text.entry)
        *(.text .text.*)
        _etext = .;
    } > VIRT AT> RAM

    .rodata : ALIGN(4K) {
        _srodata = .;
        *(.rodata .rodata.*)
        *(.srodata .srodata.*)
        _erodata = .;
    } > VIRT AT> RAM

    .data : ALIGN(4K) {
        _sdata = .;
        *(.data .data.*)
        *(.sdata .sdata.*)
        *(.got)
        _edata = .;
    } > VIRT AT> RAM

    .bss : ALIGN(4K) {
        _sbss = .;
        *(.bss.stack)
        *(.bss .bss.*)
        *(.sbss .sbss.*)
        _ebss = .;
    } > VIRT AT> RAM

    . = ALIGN(4K);
    _ekernel = .;

    /DISCARD/ : {
        *(.eh_frame)
    }
}

MEMORY
{
    VIRT : ORIGIN = %VIRT_START%, LENGTH = %RAM_SIZE%
    RAM : ORIGIN = %RAM_START%, LENGTH = %RAM_SIZE%
}
