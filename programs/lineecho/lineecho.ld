/* Linker script for NewTOS user programs (Phase-06 D3 loader contract).
 *
 * The loader maps one 4 KiB page at user VA 0: basepage at 0x000..0x100,
 * the program from 0x100 up. Everything loadable must fit in
 * [0x100, 0x1000). `.text._start` is placed first so `e_entry = 0x100`
 * is the program's entry. No .bss is emitted (state lives on the stack),
 * so file size == memory size for the one segment.
 */

ENTRY(_start)

SECTIONS {
  /DISCARD/ : {
    *(.comment)
    *(.note .note.*)
    *(.eh_frame .eh_frame_hdr)
    *(.debug_*)
    *(.ARM.*)
  }

  . = 0x100;

  .text : {
    *(.text._start)
    *(.text .text.*)
    *(.rodata .rodata.*)
    *(.data .data.*)
  }

  .bss (NOLOAD) : {
    *(.bss .bss.*)
    *(COMMON)
  }
}
