# `Physbase` (XBIOS 0x02)

> Phase 00 spec (full-contract template). Source: EmuTOS @ `1a7484ab`.
> Verified against `bios/screen.c:1042` (`physbase`).

> **Contract vs. reference** — see [`docs/COMPATIBILITY.md`](../../COMPATIBILITY.md).
> Contract: trap ID/ABI, the full-64-bit pointer result, the
> physical-vs-logical distinction a program can observe. Reference: the
> Shifter video-base hardware registers and the platform fallback chain.

## EmuTOS source

**Primary location:** `bios/screen.c:1042` — `const UBYTE *physbase(void)`,
slot 2 of `xbios_vecs[]` (`bios/xbios.c:1288`).

**Installed handler / vector:** (none) — direct table entry.

**Helpers called:**
- `bios/screen.c` — `atari_physbase()` — reconstructs the 24-bit base from
  the Shifter registers `0xFFFF8201`/`0xFFFF8203` (+ `0xFFFF820D` on
  STe/TT/Videl for byte-granular base).
- `bios/screen.c:1071` — `logbase()` — fallback when there is no Atari video.

**Headers / type definitions used:**
- `include/tosvars.h` — `v_bas_ad` (only via the `logbase` fallback).

**System variables / globals read:**
- Shifter video-base MMIO registers (Atari video only).
- `v_bas_ad` (`0x44E`) — only on the `#else` fallback path.

**System variables / globals written:** none.

**Build configuration affecting this function:**
- `CONF_WITH_ATARI_VIDEO` — with it, reads the Shifter registers; without
  it, returns `logbase()`.
- `MACHINE_AMIGA` / `MACHINE_LISA` — platform-specific `*_physbase()`.

---

## Source extraction summary

`Physbase` returns the address the video hardware is *currently displaying*
— the physical screen base. On Atari video, `atari_physbase` reads the
Shifter's high/mid (and, on STe and later, low) video-base registers and
assembles the pointer. When no Atari video is compiled in, EmuTOS falls back
to `logbase()`, so `Physbase() == Logbase()` and there is no hardware page
flip.

This differs from `Logbase` (0x03), which returns where *future drawing*
lands (`v_bas_ad`). After `Setscreen(log, phys, …)` the two may differ until
the display catches up, which is how double-buffered programs flip pages.

The value is read straight back from hardware (or `v_bas_ad`); there is no
validation and the call cannot fail.

---

## Trap interface

- **Trap number:** 14 (XBIOS)
- **Function ID:** 0x02
- **Stack frame on entry (m68k):**
  ```
   0: function_id (word) = 0x0002
  ```
  No parameters (`Physbase()` = `xbios_l_v(2)`, `include/xbiosbind.h:21`).
- **Total parameter bytes:** 2 (function word only).
- **Native (AArch64) ABI:** ID `0x02` in `x8`; no argument registers; result
  is a **pointer** occupying the full 64-bit `x0` (not sign-extended from
  32 bits).

  | m68k stack | AArch64 reg | Name | Type / size |
  |------------|-------------|------|-------------|
  | (none)     | —           | —    | —           |

---

## Inputs

None.

| Position | Name | Type | Size | Source location | Notes |
|----------|------|------|------|-----------------|-------|
| —        | —    | —    | —    | —               | —     |

---

## Outputs

### Return value

- **Register:** D0 / `x0`
- **Type:** pointer (`UBYTE *`)
- **Encoding:** the physical screen base address. On a 24-bit ST this is a
  ≤ 0x00FFFFFF value; on NewTOS it is a full 64-bit framebuffer pointer.
- **Source location:** `bios/screen.c:1049` (`atari_physbase`) or `:1052`
  (`logbase` fallback).

### Side effects on global state

None.

### Side effects on hardware

None (a pure register read on Atari video).

---

## Error conditions

| Condition | Source location | Returned value |
|-----------|-----------------|----------------|
| (no error return) | — | — |

`Physbase` has no error path; it always returns a pointer.

---

## Verification criteria

- **Nominal:** on the NewTOS profile (no Atari video), `Physbase()` returns
  the same pointer as `Logbase()` until real page flipping exists.
- **Page-flip observability:** after `Setscreen(log, phys, -1)` with distinct
  `log != phys`, `Physbase()` returns `phys` and `Logbase()` returns `log`.
- **Alignment preserved:** `Physbase()` returns exactly the pointer last set
  by `Setscreen` (NewTOS must not silently re-align it away from the caller's
  value).

---

## Edge cases discovered in source

1. **Fallback to `logbase()` with no Atari video.** `bios/screen.c:1052` —
   `Physbase() == Logbase()`, so there is no hardware-level double buffer;
   NewTOS implements real flipping via the framebuffer swap chain.
2. **STe adds a low base byte.** On STe/TT/Videl `atari_physbase` folds in
   `0xFFFF820D`, giving byte-granular base; plain ST is 256-byte aligned.
   NewTOS preserves whatever value `Setscreen` recorded regardless of Atari
   alignment rules.

---

## Dependencies

### Requires (must be initialized / functional)

- The display driver / framebuffer (`v_bas_ad` set by boot / `Setscreen`).

### Used by

- Double-buffering, screen savers, hardware-cursor emulation, screenshot
  tools.

---

## Related data structures

- `docs/api/xbios/_state.md §4` — display state, `v_bas_ad`.

---

## Cross-references

### Atari Compendium

- Not present in `references/`. Cross-check deferred. (Historically documents
  `Physbase` as the displayed base, distinct from `Logbase`.)

### Atari Profibuch

- Not present in `references/`. Cross-check deferred.

---

## Configuration / version notes

- `CONF_WITH_ATARI_VIDEO` off → fallback to `logbase()`. Base granularity
  differs ST (256-byte) vs STe/TT/Videl (byte).

---

## NewTOS implementation notes

(Empty during Phase 00.)

PHASE-08: `Physbase` = the currently-displayed framebuffer in the VideoCore
swap chain; after `Setscreen(log, phys, …)` the next mailbox-driven flip
aligns the two on the next vblank.
