// NewTOS AUTO-folder test program.
//
// Prints a message, waits ~5 s (300 Vsync frames at ~60 Hz), then terminates
// so the boot sequence continues to the desktop. A minimal demonstration
// that \AUTO\*.PRG programs load, run at EL0, and Pterm back to the kernel.
//
// NewTOS syscall ABI: x8 = function number, x0.. = arguments.
//   GEMDOS = svc #1, XBIOS = svc #14. Only x0 (the result) is clobbered by a
//   syscall; x1..x30 are restored from the trap frame, so x19 survives.

    .text
    .global _start
_start:
    // Cconws(msg) — GEMDOS 0x09, print a NUL-terminated string.
    adr  x0, msg
    mov  x8, #0x09
    svc  #1

    // ~5 second pause: 300 Vsyncs at the ~60 Hz host frame clock.
    mov  x19, #300
.Lwait:
    mov  x8, #0x25          // Vsync — XBIOS, block until the next frame
    svc  #14
    subs x19, x19, #1
    b.ne .Lwait

    // Pterm(0) — GEMDOS 0x4C, return to the launcher with code 0.
    mov  x0, #0
    mov  x8, #0x4C
    svc  #1
.Lhang:
    b    .Lhang             // safety net if Pterm ever returns

    .balign 4
msg:
    .ascii  "AUTO: NewTOS AUTO-folder test program\r\n"
    .ascii  "Pausing ~5 seconds, then continuing to the desktop...\r\n"
    .byte   0
