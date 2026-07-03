# `Ikbdws` (XBIOS 0x19)

> Phase 00 spec (full-contract template). Source: EmuTOS @ `1a7484ab`.
> Verified against `bios/ikbd.c:948` (`ikbdws`).

> **Contract vs. reference** — see [`docs/COMPATIBILITY.md`](../../COMPATIBILITY.md).
> Contract: trap ID/ABI, `void` return, the **count = bytes − 1** (unsigned)
> convention. Reference: `ikbd_writeb` and the IKBD ACIA transport.

## EmuTOS source

**Primary location:** `bios/ikbd.c:948` —
`void ikbdws(WORD cnt, const UBYTE *ptr)`, slot 0x19 of `xbios_vecs[]`
(`bios/xbios.c:1315`).

**Installed handler / vector:** (none) — direct table entry.

**Helpers called:**
- `bios/ikbd.c` — `ikbd_writeb()` — poll the IKBD ACIA TX-ready flag, then
  write one byte.

**Headers / type definitions used:** none specific.

**System variables / globals read/written:** none (writes to the IKBD ACIA
data register only).

**Build configuration affecting this function:**
- `CONF_WITH_IKBD_ACIA` — the transport. On NewTOS the bytes are interpreted
  by the IKBD-command layer of the USB-HID translator.

---

## Source extraction summary

`Ikbdws` sends a raw command string to the intelligent keyboard controller.
`cnt` is the number of bytes to send **minus one**, treated as **unsigned**:
the loop is `do { ikbd_writeb(*ptr++); } while (cnt--);`, so to send N bytes
you pass `cnt = N-1`. Because `cnt` is unsigned, `cnt == 0xFFFF` sends 65536
bytes (matching Atari TOS). `ikbd_writeb` blocks on the ACIA TX-ready flag
before each byte, so the call is synchronous. There is no return value and no
error path.

This is the low-level escape hatch programs use to issue IKBD commands
directly (mouse/joystick modes, clock set, LED control, custom packets) that
`Initmous`/`Kbrate` don't cover.

---

## Trap interface

- **Trap number:** 14 (XBIOS)
- **Function ID:** 0x19
- **Stack frame on entry (m68k):** `Ikbdws(cnt, ptr)` = `xbios_v_wl(25, a, b)`
  (`include/xbiosbind.h:43`):
  ```
  +4: ptr (LONG, pointer)
  +2: cnt (word)
   0: function_id (word) = 0x0019
  ```
- **Total parameter bytes:** 6 (W + L).
- **Native (AArch64) ABI:** ID `0x19` in `x8`; args in `x0..x1`; **`void`** —
  no return value.

  | m68k stack | AArch64 reg | Name  | Type / size |
  |------------|-------------|-------|-------------|
  | +2         | `x0`        | `cnt` | `WORD` (16) — **bytes − 1, unsigned** |
  | +4         | `x1`        | `ptr` | `const UBYTE *` (ptr) |

---

## Inputs

| Position | Name  | Type   | Size | Source location   | Notes |
|----------|-------|--------|------|-------------------|-------|
| arg0     | `cnt` | `WORD` | 2    | `bios/ikbd.c:948` | count of bytes **minus one**, unsigned |
| arg1     | `ptr` | `const UBYTE *` | 4 | `bios/ikbd.c:951` | first byte of the command string |

---

## Outputs

### Return value

- **Register:** none (`void`).
- **Type:** `void`.
- **Source location:** `bios/ikbd.c:948` (declared `void`).

### Side effects on global state

None.

### Side effects on hardware

- Writes `cnt + 1` bytes to the IKBD ACIA, blocking on TX-ready per byte.

---

## Error conditions

| Condition | Source location | Returned value |
|-----------|-----------------|----------------|
| (no error return) | — | — |

No validation: a bogus `cnt` (e.g. `0xFFFF`) simply sends 65536 bytes.

---

## Verification criteria

- **Send N bytes:** `Ikbdws(N-1, buf)` transmits exactly `N` bytes from `buf`
  to the IKBD in order.
- **Single byte:** `Ikbdws(0, &b)` sends one byte.
- **Unsigned count:** `Ikbdws(0xFFFF, buf)` attempts 65536 bytes (do-while,
  unsigned) — a program must pass `bytes-1`, not `bytes`.

---

## Edge cases discovered in source

1. **`cnt` is bytes − 1, unsigned.** The `do { … } while (cnt--)` sends at
   least one byte even for `cnt == 0`, and wraps to 65536 for `cnt == 0xFFFF`
   (`bios/ikbd.c:951-953`). This is the classic off-by-one contract callers
   must honour.
2. **Synchronous, blocking.** `ikbd_writeb` polls TX-ready; a stuck ACIA
   would block. On NewTOS the "transport" is instantaneous (no ACIA).

---

## Dependencies

### Requires (must be initialized / functional)

- The IKBD ACIA transport (`ikbd_writeb`).

### Used by

- Custom mouse/joystick setup, IKBD clock set, keyboard LED/repeat control,
  programs issuing raw IKBD packets.

---

## Related data structures

- `docs/api/xbios/00-initmous.md` — the higher-level mouse configurator that
  uses the same transport.

---

## Cross-references

### Atari Compendium

- Not present in `references/`. Cross-check deferred. (Documents `cnt` as
  bytes − 1.)

### Atari Profibuch

- Not present in `references/`. Cross-check deferred.

---

## Configuration / version notes

- `CONF_WITH_IKBD_ACIA` transport on Atari.

---

## NewTOS implementation notes

(Empty during Phase 00.)

PHASE-07: NewTOS parses the IKBD command bytes and applies them to the
USB-HID translator (mouse mode, clock, etc.); the `bytes-1` unsigned count is
honoured.
