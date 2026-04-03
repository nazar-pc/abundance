OUTPUT_ARCH(riscv)
ENTRY(rvtest_entry_point)

SECTIONS
{
    . = 0x80000000;

    .text : {
        *(.text.init)
        *(.text .text.*)
    }

    . = ALIGN(0x1000);

    /* HTIF tohost/fromhost words used by Sail for halt detection. */
    .tohost : {
        PROVIDE(tohost = .);
        . += 4;
        PROVIDE(fromhost = .);
        . += 4;
    }

    . = ALIGN(0x1000);

    .data : {
        *(.data .data.*)
        *(.sdata .sdata.*)
        *(.rodata .rodata.*)
    }

    . = ALIGN(0x1000);

    .bss : {
        *(.bss .bss.*)
        *(.sbss .sbss.*)
        *(COMMON)
    }

    . = ALIGN(16);
    _end = .;
}
