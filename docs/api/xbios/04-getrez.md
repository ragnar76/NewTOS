# `Getrez` (XBIOS 0x04)

> Phase 00 spec (full-contract template). Source: EmuTOS @ `1a7484ab`.
> Verified against `bios/screen.c:1076` (`getrez`).

> **Contract vs. reference** — see [`docs/COMPATIBILITY.md`](../../COMPATIBILITY.md).
> Contract: trap ID/ABI, the WORD result, the resolution-code values a
> program branches on. Reference: the Shifter/Videl register reads and
> `sshiftmod`.

## EmuTOS source

**Primary location:** `bios/screen.c:1076` — `WORD getrez(void)`, slot 4 of
`xbios_vecs[]` (`bios/xbios.c:1290`).

**Installed handler / vector:** (none) — direct table entry.

**Helpers called:**
- `bios/screen.c` — `atari_getrez()` — reads the resolution from the Videl
  mode (`vsetmode(-1)` mapped to a ST-compatible code), the TT shifter
  register (`0xFFFF8262`, masked), or the ST shifter register (`0xFFFF8260`,
  masked), depending on the compiled video.

**Headers / type definitions used:**
- `bios/screen.h` — resolution-code constants (`ST_LOW`=0, `ST_MEDIUM`=1,
  `ST_HIGH`=2, `FALCON_REZ`=3, `TT_MEDIUM`=4, `TT_HIGH`=6, `TT_LOW`=7).
- `include/tosvars.h` — `sshiftmod`.

**System variables / globals read:**
- Shifter/Videl mode register (Atari video), or
- `sshiftmod` (`0x44C`) — returned when there is no Atari video.

**System variables / globals written:** none.

**Build configuration affecting this function:**
- `CONF_WITH_ATARI_VIDEO`, `CONF_WITH_VIDEL`, `CONF_WITH_TT_SHIFTER` — which
  register is read. Without Atari video, returns `sshiftmod`.

---

## Source extraction summary

`Getrez` returns the current screen resolution as a small code. On Atari
video `atari_getrez` reads the active Shifter (ST/TT) or Videl mode and maps
it to the classic 0/1/2 (ST) plus the TT/Falcon extensions. Without Atari
video hardware it returns the shadow variable `sshiftmod`, which
`Setscreen`/`atari_setrez` keep in sync.

The codes are the contract programs branch on to pick a drawing strategy
(mono vs 4-plane vs 16-colour). NewTOS reports the code corresponding to the
framebuffer mode it presents.

---

## Trap interface

- **Trap number:** 14 (XBIOS)
- **Function ID:** 0x04
- **Stack frame on entry (m68k):**
  ```
   0: function_id (word) = 0x0004
  ```
  No parameters (`Getrez()` = `xbios_w_v(4)`, `include/xbiosbind.h:23`).
- **Total parameter bytes:** 2.
- **Native (AArch64) ABI:** ID `0x04` in `x8`; no argument registers; the
  WORD result **sign-extended** into `x0`.

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
- **Type:** WORD
- **Encoding:** resolution code:

  | Code | Name | Geometry (Atari) |
  |-----:|------|------------------|
  | 0 | `ST_LOW` | 320×200, 16 colours |
  | 1 | `ST_MEDIUM` | 640×200, 4 colours |
  | 2 | `ST_HIGH` | 640×400, mono |
  | 3 | `FALCON_REZ` | Videl mode (Falcon) |
  | 4 | `TT_MEDIUM` | 640×480, 16 colours |
  | 6 | `TT_HIGH` | 1280×960, mono |
  | 7 | `TT_LOW` | 320×480, 256 colours |

  (Code 5 is unused.)
- **Source location:** `bios/screen.c:1082` (`sshiftmod` fallback) or the
  `atari_getrez` register read.

### Side effects on global state / hardware

None.

---

## Error conditions

| Condition | Source location | Returned value |
|-----------|-----------------|----------------|
| (no error return) | — | — |

`Getrez` cannot fail.

---

## Verification criteria

- **Nominal (mono):** in a mono framebuffer mode, `Getrez()` returns `2`
  (`ST_HIGH`).
- **Consistency with `sshiftmod`:** on the no-Atari-video path, `Getrez()`
  equals the current `sshiftmod`.
- **Round-trip with `Setscreen`:** after `Setscreen(-1, -1, r)` for a valid
  `r`, `Getrez()` returns `r`.

---

## Edge cases discovered in source

1. **`sshiftmod` fallback.** With no Atari video, `Getrez` returns the
   `sshiftmod` shadow (`bios/screen.c:1082`), which `atari_setrez` writes on
   every `Setscreen` rez change — so `Getrez` and `Setscreen` stay coherent
   without real hardware.
2. **Videl maps to ST-compatible codes.** On Videl, `atari_getrez` collapses
   the bit-depth to `0`/`1`/`2` (or reports `FALCON_REZ`), so Videl programs
   can still read a familiar code.

---

## Dependencies

### Requires (must be initialized / functional)

- `sshiftmod` initialised by the display driver / `Setscreen`.

### Used by

- Every graphics program that adapts to resolution; VDI open; AES.

---

## Related data structures

- `docs/api/xbios/_state.md §4` — `sshiftmod` and the resolution codes.

---

## Cross-references

### Atari Compendium

- Not present in `references/`. Cross-check deferred. (Documents 0/1/2 for
  ST; TT/Falcon codes added later.)

### Atari Profibuch

- Not present in `references/`. Cross-check deferred.

---

## Configuration / version notes

- ST returns 0/1/2; TT adds 4/6/7; Falcon adds 3 (`FALCON_REZ`). The value
  set depends on `CONF_WITH_VIDEL`/`CONF_WITH_TT_SHIFTER`.

---

## NewTOS implementation notes

(Empty during Phase 00.)

PHASE-08: `Getrez` reports the code matching the active framebuffer mode; the
TT-shifter profile can return TT codes (4/6/7) for HD-friendly modes.

---

## Pi / BCM2711 binding

> Re-points the *hardware reference* (Shifter/Videl mode register, or the
> `sshiftmod` shadow at `0x44C`) at the fixed NewTOS framebuffer mode. The
> **contract is unchanged** — a WORD resolution code, no error path.
> Peripheral + driver: see `docs/HARDWARE-PROFILE.md` §2/§3.1.

- **Value NewTOS returns today: `2` (`ST_HIGH`).** `getrez()` returns the
  `REZ` shadow, defaulting to `ST_HIGH = 2` when unset
  (`tos-api/src/xbios/screen.rs:25`, `:57-62`). This is the
  no-Atari-video `sshiftmod`-fallback path: there is no Shifter to read, so
  a fixed shadow is reported.
- **This is a compatibility shim, not a truthful geometry code.** The
  framebuffer is 640×480 presenting a **16-colour / 4-plane** VDI raster
  (`kernel/src/vdi_backend.rs:21` `NUM_COLORS=16`), whereas code `2`
  (`ST_HIGH`) is 640×400 **monochrome**. The one Atari code that actually
  matches 640×480×16-colour is `4` (`TT_MEDIUM`). NewTOS returns `2`
  because the VDI reads geometry/colours from the `RasterBackend`
  (`DEV_TAB`/`INQ_TAB`, §3.1), not from `Getrez`, so the rez code is only a
  branch hint for legacy programs.
- **Decision (recommended).** Keep publishing **`2` (`ST_HIGH`)** as the
  NewTOS rez code for the bring-up 640×480 mode — it is what the code
  returns, it is the safe "high-res desktop" hint most GEM programs expect,
  and the true device capabilities come from the VDI tables. Do **not**
  switch to `4` (`TT_MEDIUM`) unless the TT-shifter profile is adopted:
  `MAX_REZ` is `2` (`tos-api/src/xbios/screen.rs:27`), so `Setscreen`
  would reject a rez of `4` today (see the mismatch note below). Revisit
  when the 1080p / mode-query phase lands, where `Getrez` should report a
  code chosen per the negotiated HDMI mode.
- **Round-trip.** `Setscreen(-1, -1, r)` for `r ∈ [0, 2]` stores the
  shadow and a later `Getrez` returns `r`
  (`tos-api/src/xbios/screen.rs:78-84`); the shadow does **not** change any
  real geometry.
- **Mismatch flagged (Step-2).** The `docs/api/xbios/05-setscreen.md`
  round-trip criterion and this spec's "reports the code matching the
  active mode" wording are in tension with returning a mono code (`2`) for
  a 16-colour framebuffer. Recorded as a Step-2 finding — no code change in
  this pass.
- **1080p later.** A mode-derived rez code is deferred to the HDMI-query
  phase.
