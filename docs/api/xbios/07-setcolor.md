# `Setcolor` (XBIOS 0x07)

> Phase 00 spec (full-contract template). Source: EmuTOS @ `1a7484ab`.
> Verified against `bios/screen.c:1192` (`setcolor`) and
> `bios/screen.c:1015` (`atari_setcolor`).

> **Contract vs. reference** — see [`docs/COMPATIBILITY.md`](../../COMPATIBILITY.md).
> Contract: trap ID/ABI, the `colorNum & 0x0F` range, the `color < 0` query,
> the return of the previous (masked) colour, the ST `0x0777` / STe `0x0FFF`
> mask. Reference: the `palette[]` shadow and the hardware register write.

## EmuTOS source

**Primary location:** `bios/screen.c:1192` —
`WORD setcolor(WORD colorNum, WORD color)`, slot 7 of `xbios_vecs[]`
(`bios/xbios.c:1293`); delegates to `atari_setcolor` (`:1015-1036`).

**Installed handler / vector:** (none) — direct table entry.

**Helpers called:**
- `bios/screen.c:1015` — `atari_setcolor()` — the masked read-modify-write.

**Headers / type definitions used:**
- `bios/screen.h` — `RGB_*` colour constants; the palette-register base.

**System variables / globals read:**
- `palette[colorNum]` (the 16-entry ST/STe palette shadow).
- the video type (to pick the `0x0777` vs `0x0FFF` mask).

**System variables / globals written:**
- `palette[colorNum]` when `color >= 0` (`bios/screen.c:1033`), and the
  hardware palette register.

**Build configuration affecting this function:**
- `CONF_WITH_ATARI_VIDEO` — the register write; without it the shadow-only
  path applies.

---

## Source extraction summary

`Setcolor` sets one hardware palette entry and returns its **previous** value.
`colorNum` is masked to `& 0x000F` (`bios/screen.c:1024`), so only the 16
ST/STe entries are addressable. It reads the old value masked with `0x0777`
on a plain ST or `0x0FFF` on STe/TT/Videl (`:1027-1031`) — i.e. 3 bits per
channel on ST, 4 bits on STe.

If `color >= 0` it writes the new value into `palette[colorNum]` (and the
hardware register); if `color < 0` it does **not** write — this is the query
form, returning the current colour without changing it (`:1032-1033`). Either
way it returns the old (masked) value.

The packed word format is bits 11-8 red, 7-4 green, 3-0 blue (4 bits per
channel, only 3 significant on plain ST). Real STe/TT hardware rotates each
channel's LSB to the top bit for ST backward compatibility; EmuTOS stores and
masks straight nibbles, so the *stored/returned* word is contract and the
hardware nibble rotation is a driver detail.

---

## Trap interface

- **Trap number:** 14 (XBIOS)
- **Function ID:** 0x07
- **Stack frame on entry (m68k):** `Setcolor(colorNum, color)` =
  `xbios_w_ww(7, a, b)` (`include/xbiosbind.h:26`):
  ```
  +4: color    (word)
  +2: colorNum (word)
   0: function_id (word) = 0x0007
  ```
- **Total parameter bytes:** 6 (W + W).
- **Native (AArch64) ABI:** ID `0x07` in `x8`; args in `x0..x1`; the WORD
  result **sign-extended** into `x0`.

  | m68k stack | AArch64 reg | Name       | Type / size |
  |------------|-------------|------------|-------------|
  | +2         | `x0`        | `colorNum` | `WORD` (16) — masked `& 0x0F` |
  | +4         | `x1`        | `color`    | `WORD` (16) — `< 0` = query |

---

## Inputs

| Position | Name       | Type   | Size | Source location      | Notes |
|----------|------------|--------|------|----------------------|-------|
| arg0     | `colorNum` | `WORD` | 2    | `bios/screen.c:1024` | palette index, masked to `0..15` |
| arg1     | `color`    | `WORD` | 2    | `bios/screen.c:1032` | new packed colour; `< 0` = query only |

### Packed colour word

| Bits  | Channel | ST (plain) | STe/TT/Videl |
|-------|---------|------------|--------------|
| 11-8  | Red     | 3 sig. bits | 4 bits |
| 7-4   | Green   | 3 sig. bits | 4 bits |
| 3-0   | Blue    | 3 sig. bits | 4 bits |

Mask applied to the *returned* value: `0x0777` (ST) / `0x0FFF` (STe/TT/Videl).

---

## Outputs

### Return value

- **Register:** D0 / `x0`
- **Type:** WORD
- **Encoding:** the **previous** value of `palette[colorNum]`, masked
  (`0x0777`/`0x0FFF`).
- **Source location:** `bios/screen.c:1031,1035`.

### Side effects on global state

- `palette[colorNum] = color` when `color >= 0` (`bios/screen.c:1033`).

### Side effects on hardware

- Writes the corresponding hardware palette register (Atari video). No write
  when `color < 0`.

---

## Error conditions

| Condition | Source location | Returned value |
|-----------|-----------------|----------------|
| (no error return) | — | — |

`Setcolor` has no error path; an out-of-range `colorNum` is silently masked,
not rejected.

---

## Verification criteria

- **Set + read old:** `Setcolor(1, 0x0700)` returns the previous entry-1
  colour and makes `Setcolor(1, -1)` subsequently return `0x0700 &
  mask`.
- **Query does not write:** `Setcolor(2, -1)` returns entry 2 unchanged and
  leaves the palette untouched.
- **Index masking:** `Setcolor(16, c)` writes entry `16 & 0x0F = 0`, not a
  17th entry.
- **ST mask:** on a plain-ST profile, the returned value has bits 0/4/8
  masked to the `0x0777` pattern.

---

## Edge cases discovered in source

1. **`color < 0` is the query form.** No write occurs; only the old colour is
   returned (`bios/screen.c:1032`). This is how a program reads a single
   palette entry.
2. **`colorNum & 0x0F` — silent wraparound.** Out-of-range indices wrap into
   0..15 rather than erroring (`:1024`).
3. **Return is masked, write is raw.** The old value is masked with
   `0x0777`/`0x0FFF` (`:1031`); the new value is stored unmasked (`:1033`),
   so a later query re-masks it. NewTOS must mask the returned value the same
   way.
4. **STe LSB rotation is a hardware detail.** EmuTOS stores straight nibbles;
   the ST-vs-STe intensity-bit order lives in the hardware, not the stored
   word. NewTOS's 4-bit→8-bit VideoCore mapping is free as long as the
   returned word matches.

---

## Dependencies

### Requires (must be initialized / functional)

- The `palette[]` shadow and (Atari) the palette registers.

### Used by

- Palette animation, single-colour tweaks, VDI `vs_color`, demos.

---

## Related data structures

- `docs/api/xbios/_state.md §4` — palette shadow + word format.
- `docs/api/xbios/06-setpalette.md` — full-block counterpart.

---

## Cross-references

### Atari Compendium

- Not present in `references/`. Cross-check deferred. (Documents `colorNum`
  0..15, `color = -1` query, return of the old colour.)

### Atari Profibuch

- Not present in `references/`. Cross-check deferred.

---

## Configuration / version notes

- ST returns `& 0x0777` (3-bit channels); STe/TT/Videl `& 0x0FFF` (4-bit).
  The mask depends on the detected video, not a build flag alone.

---

## NewTOS implementation notes

(Empty during Phase 00.)

PHASE-08: `Setcolor` writes one entry of the VideoCore palette (4-bit→8-bit
expansion) and returns the previous masked word; the `color < 0` query and
the `colorNum & 0x0F` masking are preserved.

---

## Pi / BCM2711 binding

> Re-points the *hardware reference* (the `palette[]` shadow + the STe/TT
> palette register write) at the emulated 16-entry CLUT over the 32 bpp
> VideoCore framebuffer. The **contract is unchanged** — `colorNum & 0x0F`,
> `color < 0` query, the previous masked word returned. Peripheral +
> driver: see `docs/HARDWARE-PROFILE.md` §2/§3.1.

- **What `setcolor` does today.** Index is masked `& 0x0F`; when `color >=
  0` the entry is stored `& 0x0FFF` (STe 4-bit mask), and the previous
  value is returned as a WORD (`tos-api/src/xbios/screen.rs:108-117`).
  `color < 0` is the query — no write. Matches the contract's masking and
  query rules.
- **STe 4-bit mask (`0x0FFF`) is used, not the ST `0x0777`.** NewTOS treats
  the profile as STe-class for the return mask (4 significant bits/gun),
  which is consistent with an emulated CLUT that has ≥ 4-bit precision.
- **Same disconnected-shadow caveat as `Setpalette` (Step-2).** The 16
  entries written here live in the XBIOS ST-format shadow
  (`screen.rs:160-168`), **not** the VDI CLUT
  (`vdi_backend::PALETTE`) that actually colours pixels. So `Setcolor`
  updates a value you can read back via `Setcolor(n, -1)` but does not
  change the displayed colour. Bridging XBIOS `Setcolor` to the
  `RasterBackend::set_color` (4→8-bit expansion into the VideoCore CLUT) is
  the Step-2 fix — not done in this pass.
- **4→8-bit expansion (when wired).** Each stored ST nibble maps to an
  8-bit channel for the truecolor framebuffer, exactly the VideoCore
  mapping the PHASE-08 note describes; the returned word stays the masked
  ST value (contract), the 8-bit expansion is a driver detail.
- **1080p later.** Resolution-independent; the 16-entry CLUT model is
  unchanged by geometry.
