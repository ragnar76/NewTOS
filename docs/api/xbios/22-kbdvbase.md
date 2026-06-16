# `Kbdvbase` — XBIOS 0x22

> Phase 00 spec. Source: EmuTOS @ `1a7484ab` —
> `bios/xbios.c:716` (`kbdvbase`).

## EmuTOS source

```c
static LONG kbdvbase(void) { return (LONG)&kbdvecs; }
```

`kbdvecs` is a `struct kbdvecs` global (`bios/ikbd.c`) populated
at boot with default IKBD-event handlers.

## Trap interface

| | |
|--|--|
| Trap | 14 (XBIOS) |
| Function ID | `0x22` |
| WORD args | 0 |
| Stack frame | `+0: 0x0022` |
| Vector | `xbios_vecs[34]` |

## Inputs

None.

## Outputs

| Reg | D0 |
| --- | --- |
| Type | LONG (`struct kbdvecs *`) |

Pointer to the global `kbdvecs` struct.

## `struct kbdvecs` (`include/biosdefs.h:105`)

| Offset | Field        | Description                              |
|-------:|--------------|------------------------------------------|
| 0x00   | `midivec`    | MIDI input handler                        |
| 0x04   | `vkbderr`    | IKBD error handler                        |
| 0x08   | `vmiderr`    | MIDI error handler                        |
| 0x0C   | `statvec`    | IKBD status packet handler                |
| 0x10   | `mousevec`   | IKBD mouse-packet handler (`evnt_mouse`)  |
| 0x14   | `clockvec`   | IKBD clock-packet handler                 |
| 0x18   | `joyvec`     | IKBD joystick-packet handler              |
| 0x1C   | `midisys`    | top-level MIDI ISR                        |
| 0x20   | `ikbdsys`    | top-level IKBD ISR                        |

All fields are `PFVOID` (function pointers, 4 bytes on m68k).
Total size 36 bytes.

## Errors

None.

## Edge cases

### 1. Direct hook installation

Programs install their own handlers by overwriting the relevant
`kbdvecs` field. Example: AES installs `mousevec` to translate
IKBD mouse packets into AES mouse events. The pointer is **not**
returned previous-vector style; programs must read the field, save
it, and chain manually.

### 2. ISR context

The handlers run from interrupt context (IPL 6 for ACIA RX). They
must save/restore registers per Atari ISR convention; calls into
GEMDOS / line-A from these handlers are unsafe.

### 3. Pointer remains valid forever

`kbdvecs` lives in BIOS BSS — not subject to GEMDOS memory
management. The pointer returned is process-lifetime stable.

### 4. NULL handlers

Initially populated with default no-op handlers. Setting a field
to `NULL` causes the dispatcher to call address 0 → crash.
Programs uninstalling must restore the saved-original pointer.

### 5. NewTOS without IKBD chip

The `kbdvecs` struct is preserved as ABI; the entries point at
NewTOS-internal stubs that translate USB-HID events into the
expected IKBD packet format and call the user-installed handlers.

## Dependencies

- `kbdvecs` global (BIOS BSS).
- IKBD ACIA interrupt handlers in `bios/ikbd.c`.

Used by: AES (mouse vector hook), MIDI sequencer programs (midivec
hook), TSRs that intercept keypresses.

## Cross-references

- **Compendium:** "Kbdvbase" — agrees on struct layout.
- **Profibuch:** "Kbdvbase (0x22)" — agrees.
- `docs/api/structures/iorec.md` — IKBD ringbuffer.

## Configuration

None.

## NewTOS implementation notes

(Empty during Phase 00.)

PHASE-07: NewTOS keeps the exact 36-byte struct layout for ABI.
USB-HID events feed into the IKBD-format packet generator
(`bios/ikbd.c` analogues) which then calls through `kbdvecs`
fields. Programs that hook `mousevec` work unchanged.
