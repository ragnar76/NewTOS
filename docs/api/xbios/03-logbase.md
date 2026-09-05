# `Logbase` (XBIOS 0x03)

> Phase 00 spec (full-contract template). Source: EmuTOS @ `1a7484ab`.
> Verified against `bios/screen.c:1071` (`logbase`).

> **Contract vs. reference** — see [`docs/COMPATIBILITY.md`](../../COMPATIBILITY.md).
> Contract: trap ID/ABI, the full-64-bit pointer result (= `v_bas_ad`),
> the logical-vs-physical distinction. Reference: `v_bas_ad`'s physical
> low-memory address.

## EmuTOS source

**Primary location:** `bios/screen.c:1071` — `UBYTE *logbase(void)`; a
one-liner returning `v_bas_ad`. Slot 3 of `xbios_vecs[]`
(`bios/xbios.c:1289`).

**Installed handler / vector:** (none) — direct table entry.

**Helpers called:** none.

**Headers / type definitions used:**
- `include/tosvars.h` — `v_bas_ad` (`extern UBYTE *v_bas_ad`).

**System variables / globals read:**
- `v_bas_ad` (`0x44E`) — the logical screen base.

**System variables / globals written:** none.

**Build configuration affecting this function:** none (no `CONF_WITH_*`
gate — always returns `v_bas_ad`).

---

## Source extraction summary

`Logbase` returns `v_bas_ad`, the base of the buffer where the VT52
emulator, the Line-A primitives, and the VDI direct their drawing — the
*logical* screen. It is the destination of software rendering, which may
differ from the *physical* base (`Physbase`, 0x02) that the video hardware
displays; the difference is how double-buffered programs draw to an off-screen
page.

The function does no work beyond reading the global; `v_bas_ad` is written by
`Setscreen` (0x05) and at boot. It cannot fail.

---

## Trap interface

- **Trap number:** 14 (XBIOS)
- **Function ID:** 0x03
- **Stack frame on entry (m68k):**
  ```
   0: function_id (word) = 0x0003
  ```
  No parameters (`Logbase()` = `xbios_l_v(3)`, `include/xbiosbind.h:22`).
- **Total parameter bytes:** 2.
- **Native (AArch64) ABI:** ID `0x03` in `x8`; no argument registers; result
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
- **Type:** pointer (`UBYTE *`)
- **Encoding:** the logical screen base = `v_bas_ad`.
- **Source location:** `bios/screen.c:1073`.

### Side effects on global state

None.

### Side effects on hardware

None.

---

## Error conditions

| Condition | Source location | Returned value |
|-----------|-----------------|----------------|
| (no error return) | — | — |

---

## Verification criteria

- **Nominal:** `Logbase()` returns exactly the value of `v_bas_ad` (the base
  set at boot / by `Setscreen`).
- **Tracks `Setscreen`:** after `Setscreen(buf, -1, -1)`, `Logbase() == buf`
  while `Physbase()` is unchanged.
- **Immediate visibility:** the value returned is the one Line-A/VT52 will
  draw to on their next output (no deferral).

---

## Edge cases discovered in source

1. **No sync / no validation.** `v_bas_ad` is a plain global; EmuTOS is
   single-threaded so an unguarded read is fine. `Logbase` returns it raw,
   with no range check.
2. **Line-A reads `v_bas_ad` directly.** Changing it (via `Setscreen`) takes
   effect on the next Line-A draw, so `Logbase` and the drawing target stay
   coherent by construction.

---

## Dependencies

### Requires (must be initialized / functional)

- `v_bas_ad` initialised by the display driver at boot.

### Used by

- Line-A drawing, VDI, VT52 console, double-buffering setup.

---

## Related data structures

- `docs/api/xbios/_state.md §4` — display state.
- `docs/api/structures/osheader.md` — `v_bas_ad` exposure via the OS header.

---

## Cross-references

### Atari Compendium

- Not present in `references/`. Cross-check deferred.

### Atari Profibuch

- Not present in `references/`. Cross-check deferred.

---

## Configuration / version notes

- None. `v_bas_ad` is present on every configuration.

---

## NewTOS implementation notes

(Empty during Phase 00.)

PHASE-08: `Logbase` returns the current "draw" framebuffer in the swap chain;
`Setscreen(log, …)` retargets it.

---

## Pi / BCM2711 binding

> Re-points the *hardware reference* (`v_bas_ad` at m68k `0x44E`) at the
> BCM2711 VideoCore framebuffer. The **contract is unchanged** — a
> full-64-bit pointer equal to the logical draw base. Peripheral +
> driver: see `docs/HARDWARE-PROFILE.md` §2 (Screen / framebuffer).

- **`v_bas_ad` maps to the framebuffer base.** NewTOS keeps no low-memory
  `0x44E` cell; `Logbase` returns the `LOGBASE` shadow, or the live
  framebuffer base when the shadow is `0` (`tos-api/src/xbios/screen.rs`,
  `fb_base`). The live base is **expressed in the caller's address space**
  (see `02-physbase.md` §"Address space and cacheability"): the kernel
  high-half VA of the VideoCore pixel store for an EL1 caller, and a
  `PROT_G`, Normal Non-Cacheable mapping of the same physical pages in the
  process's own EL0 window for an EL0 caller — the same value `Physbase`
  returns on this single-buffer profile.
- **Single buffer.** With one VideoCore framebuffer and no page flip,
  `Logbase() == Physbase()` until double-buffering lands. The VDI, the VT52
  framebuffer console (`fbcon`), and the mouse sprite all draw through the
  `RasterBackend` into this one buffer.
- **Setscreen interaction.** `Setscreen(log, -1, -1)` writes the `LOGBASE`
  shadow (`tos-api/src/xbios/screen.rs:72-74`); a subsequent `Logbase`
  returns that pointer. But the NewTOS drawing path (the `RasterBackend`)
  always renders into the *live* framebuffer, not into a caller-supplied
  `log` buffer — so retargeting the logical base does not actually redirect
  VDI/console drawing yet (a Step-2 concern; see the profile note on
  deferred double-buffering).
- **1080p later.** Base is for the fixed 640×480 mode; a mode-sized base
  follows the HDMI-query phase.
