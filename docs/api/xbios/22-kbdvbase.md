# `Kbdvbase` (XBIOS 0x22)

> Phase 00 spec (full-contract template). Source: EmuTOS @ `1a7484ab`.
> Verified against `bios/xbios.c:716` (`kbdvbase`).

> **Contract vs. reference** — see [`docs/COMPATIBILITY.md`](../../COMPATIBILITY.md).
> Contract: trap ID/ABI, the full-64-bit pointer result, the 36-byte
> `kbdvecs` layout programs hook. Reference: the ACIA ISRs the vectors
> point at.

## EmuTOS source

**Primary location:** `bios/xbios.c:716` — `static LONG kbdvbase(void)`; a
one-liner returning `(LONG)&kbdvecs`. Slot 0x22 of `xbios_vecs[]`
(`bios/xbios.c:1337`).

**Installed handler / vector:** (none) — direct table entry. Returns the
address of the vector table other calls (`Initmous`) install into.

**Helpers called:** none.

**Headers / type definitions used:**
- `include/biosdefs.h:105` — `struct kbdvecs` (36 bytes, 9 pointers).

**System variables / globals read:**
- `kbdvecs` (its address is returned).

**System variables / globals written:** none.

**Build configuration affecting this function:** none.

---

## Source extraction summary

`Kbdvbase` returns a pointer to the global `kbdvecs` structure, a table of 9
function pointers that dispatch IKBD/MIDI ACIA events (MIDI input, IKBD/MIDI
errors, IKBD status/mouse/clock/joystick packets, and the two top-level
ACIA ISRs). Applications install their own handlers by writing a field
directly — there is no previous-vector return, so a program that wants to
chain must read-save-restore the field itself.

The struct lives in BIOS BSS and is process-lifetime stable; the pointer
never changes. `Initmous` (0x00) writes `kbdvecs.mousevec`; AES hooks it to
turn IKBD mouse packets into GEM events; MIDI sequencers hook `midivec`.

---

## Trap interface

- **Trap number:** 14 (XBIOS)
- **Function ID:** 0x22
- **Stack frame on entry (m68k):**
  ```
   0: function_id (word) = 0x0022
  ```
  No parameters (`Kbdvbase()` = `xbios_l_v(34)`, `include/xbiosbind.h:52`).
- **Total parameter bytes:** 2.
- **Native (AArch64) ABI:** ID `0x22` in `x8`; no argument registers; result
  is a **pointer** in the full 64-bit `x0`.

  | m68k stack | AArch64 reg | Name | Type / size |
  |------------|-------------|------|-------------|
  | (none)     | —           | —    | —           |

---

## Inputs

None.

---

## Outputs

### Return value

- **Register:** D0 / `x0`
- **Type:** pointer (`struct kbdvecs *`)
- **Encoding:** `&kbdvecs`.
- **Source location:** `bios/xbios.c:718`.

### `struct kbdvecs` (`include/biosdefs.h:105`) — 36 bytes

| Offset | Field      | Purpose |
|-------:|------------|---------|
| 0x00   | `midivec`  | MIDI input byte |
| 0x04   | `vkbderr`  | IKBD ACIA overrun/error |
| 0x08   | `vmiderr`  | MIDI ACIA overrun/error |
| 0x0C   | `statvec`  | IKBD status packet (0xF6) |
| 0x10   | `mousevec` | IKBD mouse packet (0xF8..0xFB) |
| 0x14   | `clockvec` | IKBD clock packet (0xFC) |
| 0x18   | `joyvec`   | IKBD joystick packet (0xFD..0xFF) |
| 0x1C   | `midisys`  | top-level MIDI ACIA ISR |
| 0x20   | `ikbdsys`  | top-level IKBD ACIA ISR |

All fields `PFVOID` (4 bytes on m68k).

### Side effects on global state / hardware

None.

---

## Error conditions

| Condition | Source location | Returned value |
|-----------|-----------------|----------------|
| (no error return) | — | — |

---

## Verification criteria

- **Stable pointer:** two `Kbdvbase()` calls return the same address.
- **Hookable:** writing `((struct kbdvecs *)Kbdvbase())->mousevec = h` makes
  the mouse ISR dispatch through `h` (as `Initmous` also does).
- **36-byte layout:** offset `0x10` is `mousevec`, `0x1C`/`0x20` are the
  `midisys`/`ikbdsys` ISRs.

---

## Edge cases discovered in source

1. **No previous-vector return.** Installing a handler requires reading and
   saving the field first; the API does not hand back the old pointer.
2. **`NULL` field crashes the dispatcher.** The ISR calls the field
   unconditionally; a program uninstalling must restore the saved original,
   not write `NULL`.
3. **Private state adjacent to the table.** The IKBD packet state machine
   keeps `ikbdstate`/`kbdlength` near the table in `bios/aciavecs.S`; those
   are **not** part of the 36-byte contract returned to applications and need
   not be exposed by NewTOS.

---

## Dependencies

### Requires (must be initialized / functional)

- `kbdvecs` populated at boot with default handlers.

### Used by

- `Initmous` (writes `mousevec`), AES (mouse hook), MIDI apps (`midivec`),
  IKBD-packet TSRs.

---

## Related data structures

- `docs/api/xbios/_state.md §1` — `kbdvecs`.
- `docs/api/xbios/00-initmous.md` — writes `mousevec`.

---

## Cross-references

### Atari Compendium

- Not present in `references/`. Cross-check deferred. (Documents the 9-vector
  `kbdvecs` layout.)

### Atari Profibuch

- Not present in `references/`. Cross-check deferred.

---

## Configuration / version notes

- The struct is 36 bytes across EmuTOS builds; handler targets are ACIA ISRs
  on Atari, translator stubs on NewTOS.

---

## NewTOS implementation notes

(Empty during Phase 00.)

PHASE-07: NewTOS keeps the 36-byte layout; the USB-HID translator generates
IKBD-format packets and dispatches through these vectors, so programs that
hook `mousevec`/`midivec`/`joyvec` work unchanged.

## Pi / BCM2711 binding

> Re-points the *hardware reference* (the `kbdvecs` table whose slots point
> at the IKBD/MIDI **ACIA ISRs**) at the NewTOS profile, where the keyboard
> and mouse arrive as **USB-HID reports over xHCI/PCIe** and are synthesised
> into the same IKBD byte stream. The **contract is unchanged** — a stable
> full-64-bit pointer to a 9-slot vector table, no error path. Peripheral +
> driver: see `docs/HARDWARE-PROFILE.md` §2 (Keyboard / mouse → USB-HID) and
> §3.3.

- **Where the table lives.** `Kbdvbase` returns `&KBDVECS`, a `struct
  KbdVecs` of nine native-pointer slots in the tos-api layer
  (`tos-api/src/xbios/system.rs:63-78`). Native pointer width, not the m68k
  4-byte layout — the 36-byte offsets in the contract table above are the
  m68k reference; NewTOS keeps the field *order* (midivec, vkbderr, vmiderr,
  statvec, mousevec, clockvec, joyvec, midisys, ikbdsys) so a native caller
  hooks the right slot.
- **What feeds the vectors on the Pi.** There is **no ACIA ISR**. USB-HID
  boot reports are decoded by `kernel/src/drivers/usb/hid.rs` into IKBD
  bytes and pushed through `kernel/src/ikbd.rs::feed`, which drives the
  console key ring (keyboard) and `vdi::feed_relative` (mouse). The
  `kbdvecs` table is the *contract surface* a program reads/hooks; the
  actual delivery path is the USB→IKBD translator, not a dispatch through
  these slots.
- **Step-2 — the table is never populated.** `KBDVECS` is initialised to
  **all-zero** and no boot code installs default handlers or dispatches
  IKBD packets through `mousevec`/`statvec`/etc.
  (`tos-api/src/xbios/system.rs:63-73`). So today a program that reads
  `Kbdvbase()->mousevec` sees `0`, and hooking a slot has **no effect** —
  the IKBD decoder calls `vdi::feed_relative` directly, bypassing
  `mousevec`. The stable-pointer + 9-slot-layout contract holds; the
  dispatch-through-vectors contract does not yet. Recorded as K-series
  below. (`Initmous`, which would install `mousevec`, is likewise not
  wired — see `00-initmous.md`.)
- **Datasheet.** No peripheral register: the keyboard/mouse transport is
  the VL805 xHCI controller on the Pi-4 PCIe root complex; see
  `docs/HARDWARE-PROFILE.md` §2 and the USB specs under `docs/api/usb/`.
