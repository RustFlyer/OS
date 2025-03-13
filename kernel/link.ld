OUTPUT_ARCH(riscv)
ENTRY(_start)

SECTIONS
{
    . = ALIGN(4K);
    _skernel = .;
    _stext = .;
    .text : {
        *(.text.entry)
        . = ALIGN(4K);
        *(.text .text.*)
    } > VIRT AT> RAM

    . = ALIGN(4K);
    _etext = .;
    _srodata = .;
    .rodata : {
        *(.rodata .rodata.*)
        *(.srodata .srodata.*)
    } > VIRT AT> RAM

    . = ALIGN(4K);
    _erodata = .;
    _sdata = .;
    .data : {
        *(.data .data.*)
        *(.sdata .sdata.*)
    } > VIRT AT> RAM

    . = ALIGN(4K);
    _edata = .;
    _sbss = .;
    .bss : {
        *(.bss.stack)
        *(.bss .bss.*)
        *(.sbss .sbss.*)
    } > VIRT AT> RAM

    . = ALIGN(4K);
    _ebss = .;
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
