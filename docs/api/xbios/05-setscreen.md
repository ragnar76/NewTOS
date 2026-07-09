# `Setscreen` (XBIOS 0x05)

> Phase 00 spec (full-contract template). Source: EmuTOS @ `1a7484ab`.
> Verified against `bios/screen.c:1102` (`setscreen`).

> **Contract vs. reference** — see [`docs/COMPATIBILITY.md`](../../COMPATIBILITY.md).
> Contract: trap ID/ABI, the negative-argument "ignore" convention, the WORD
> return (`-1` on error, previous Videl mode, else 0), the rez-change side
> effects a program can observe (screen cleared, cursor homed, VT52 reset).
> Reference: `vblsem`, `atari_setrez`, `Srealloc`, the Videl internals.

## EmuTOS source

**Primary location:** `bios/screen.c:1102` —
`WORD setscreen(UBYTE *logLoc, const UBYTE *physLoc, WORD rez, WORD videlmode)`,
slot 5 of `xbios_vecs[]` (`bios/xbios.c:1291`).

**Installed handler / vector:** (none) — direct table entry.

**Helpers called:**
- `bios/screen.c` — `setphys(physLoc)` — program the hardware physical base
  (`:1111`).
- `bios/videl.c` — `vfixmode()`, `vgetsize()`, `vsetmode()` — Videl mode
  fixup/size/set (`:1133,1135,1143`, `CONF_WITH_VIDEL`).
- `bios/biosmem.c` — `Srealloc()` — reallocate screen RAM for a new Videl
  size (`:1135`).
- `bios/screen.c` — `atari_setrez(rez, videlmode)` — apply the resolution
  (updates `sshiftmod`) (`:1154`).
- `bios/lineainit.c` — `linea_init()` — re-init Line-A vars (`:1160`).
- `bios/vt52.c` — `vt52_init()` — reset the VT52 emulator (`:1161`).
- `bios/screen.c:1205` — `vsync()` — wait for end-of-frame before switching
  (`:1149`).

**Headers / type definitions used:**
- `bios/screen.h` — `MIN_REZ`, `MAX_REZ`, `FALCON_REZ` (3), resolution codes.
- `include/tosvars.h` — `v_bas_ad`, `vblsem`.

**System variables / globals read:**
- `rez_was_hacked` (Line-A hack guard, `:1115`), `has_videl` (`:1130`).

**System variables / globals written:**
- `v_bas_ad` (`0x44E`, `:1107` and Videl realloc `:1139`).
- `vblsem` (`0x452`, cleared `:1158`, restored `:1163`).
- `sshiftmod` (`0x44C`, via `atari_setrez`).
- the physical base register (via `setphys`).

**Build configuration affecting this function:**
- `CONF_WITH_VIDEL` — enables the `rez == FALCON_REZ` reallocation/mode path.
- `CONF_WITH_ATARI_VIDEO` — `atari_setrez` / `vsync` mechanics.

---

## Source extraction summary

`Setscreen` reconfigures the display: it optionally moves the logical base
(`logLoc`), the physical base (`physLoc`), and the resolution (`rez`). A
**negative** value for any of the three means "leave that unchanged": the code
tests each as a signed LONG/WORD and only applies it when positive
(`logLoc > 0 → v_bas_ad = logLoc`, `:1106-1107`; `physLoc > 0 →
setphys(physLoc)`, `:1110-1111`; `rez == -1 → return 0` without a rez change,
`:1115-1116`).

The resolution is validated against `[MIN_REZ, MAX_REZ]`; an out-of-range
`rez` returns `-1` (`:1120-1121`). On a Videl machine, `rez == FALCON_REZ`
(3) enters a special path: if `videlmode != -1` it fixes the mode with
`vfixmode`; if *both* `logLoc` and `physLoc` are NULL it reallocates screen
RAM (`Srealloc(vgetsize(videlmode))`), returning `-1` on failure and setting
both bases to the new buffer (`:1130-1141`). It saves the previous Videl mode
(`oldmode = vsetmode(-1)`).

When the resolution actually changes it: `vsync()`s to a frame boundary,
clears `vblsem` to suspend the VBL, calls `atari_setrez` (which also updates
`sshiftmod`), re-inits Line-A (`linea_init`) and the VT52 (`vt52_init`) —
this is what clears the screen and homes the cursor — then restores
`vblsem`. The WORD return is `oldmode` for Falcon resolutions, else `0`.

Note the whole function is documented as `void` in Atari docs but EmuTOS
returns a WORD for TOS-4 compatibility.

---

## Trap interface

- **Trap number:** 14 (XBIOS)
- **Function ID:** 0x05
- **Stack frame on entry (m68k):** `Setscreen(logLoc, physLoc, rez,
  videlmode)` = `xbios_v_llww(5, …)` (`include/xbiosbind.h:24`):
  ```
  +10: videlmode (word)
   +8: rez       (word)
   +4: physLoc   (LONG, pointer)
   +2: logLoc    (LONG, pointer)
    0: function_id (word) = 0x0005
  ```
- **Total parameter bytes:** 12 (L + L + W + W).
- **Native (AArch64) ABI:** ID `0x05` in `x8`; args in `x0..x3`; the WORD
  result **sign-extended** into `x0`.

  | m68k stack | AArch64 reg | Name        | Type / size |
  |------------|-------------|-------------|-------------|
  | +2         | `x0`        | `logLoc`    | `UBYTE *` (ptr) |
  | +4         | `x1`        | `physLoc`   | `const UBYTE *` (ptr) |
  | +8         | `x2`        | `rez`       | `WORD` (16) |
  | +10        | `x3`        | `videlmode` | `WORD` (16) |

---

## Inputs

| Position | Name        | Type    | Size | Source location       | Notes |
|----------|-------------|---------|------|-----------------------|-------|
| arg0     | `logLoc`    | `UBYTE*`| 4    | `bios/screen.c:1106`  | new logical base; ≤ 0 ⇒ unchanged |
| arg1     | `physLoc`   | `UBYTE*`| 4    | `bios/screen.c:1110`  | new physical base; ≤ 0 ⇒ unchanged |
| arg2     | `rez`       | `WORD`  | 2    | `bios/screen.c:1115`  | resolution code; `-1` ⇒ unchanged; out of `[MIN_REZ,MAX_REZ]` ⇒ error |
| arg3     | `videlmode` | `WORD`  | 2    | `bios/screen.c:1132`  | Videl mode word (used only when `rez == FALCON_REZ`); `-1` ⇒ keep |

> "≤ 0 ⇒ unchanged" for the pointers: the source tests `if ((LONG)logLoc > 0)`
> / `if ((LONG)physLoc > 0)`, so `NULL` (0) and any negative value skip the
> base update. (Documentation says "negative"; the code treats `0` the same
> as negative for the base pointers.)

---

## Outputs

### Return value

- **Register:** D0 / `x0`
- **Type:** WORD
- **Encoding:**
  - `-1` if `rez` is invalid, or `Srealloc()` failed (`:1121,1137`).
  - the **previous Videl mode** for Falcon resolutions (`oldmode =
    vsetmode(-1)`, `:1143`).
  - `0` otherwise (including `rez == -1` no-op, `:1116`).
- **Source location:** `bios/screen.c:1116,1121,1137,1143,1165`.

### Side effects on global state

- `v_bas_ad = logLoc` when `logLoc > 0` (`:1107`); both bases on a Videl
  realloc (`:1139-1140`).
- `sshiftmod` updated by `atari_setrez` on a rez change.
- `vblsem` cleared then restored around the rez change (`:1158,1163`).

### Side effects on hardware

- `setphys` programs the physical video-base register.
- On a rez change: screen cleared + cursor homed + VT52 reset (via
  `linea_init`/`vt52_init`), one `vsync` wait.

---

## Error conditions

| Condition | Source location | Returned value |
|-----------|-----------------|----------------|
| `rez < MIN_REZ` or `rez > MAX_REZ` (and `rez != -1`) | `bios/screen.c:1120-1121` | `-1` |
| Videl `Srealloc()` failed (both bases NULL, `rez == FALCON_REZ`) | `bios/screen.c:1135-1137` | `-1` |

`rez == -1` is **not** an error — it returns `0` and changes only the bases.

---

## Verification criteria

- **Move logical base only:** `Setscreen(buf, -1, -1, -1)` sets
  `Logbase() == buf`, leaves `Physbase()` and `Getrez()` unchanged, returns
  `0`.
- **Invalid rez:** `Setscreen(-1, -1, 99, -1)` returns `-1` and changes
  nothing.
- **Rez change clears screen:** `Setscreen(-1, -1, 2, -1)` sets `Getrez()`
  to `2`, clears the screen, homes the VT52 cursor, returns `0`.
- **No-op rez:** `Setscreen(-1, -1, -1, -1)` returns `0` and leaves the
  display untouched.
- **Falcon return:** on a Videl machine, `Setscreen(0, 0, 3, mode)` returns
  the previous Videl mode (or `-1` on realloc failure).

---

## Edge cases discovered in source

1. **Base pointers use `> 0`, not `>= 0`.** `NULL` (0) skips the base update
   just like a negative value (`:1106,1110`); only a strictly positive
   pointer moves the base.
2. **`rez == -1` short-circuits before validation.** It returns `0`
   immediately (`:1115-1116`), so you can move the bases without touching the
   resolution and without risking the `-1` error.
3. **Videl NULL/NULL triggers screen-RAM reallocation.** `rez ==
   FALCON_REZ` with both bases NULL calls `Srealloc(vgetsize(videlmode))`
   and points both bases at the fresh buffer; a failed alloc returns `-1`
   (`:1134-1141`).
4. **The WORD return is an EmuTOS/TOS-4 extension.** Atari docs call
   `Setscreen` `void`; EmuTOS returns `-1`/oldmode/0. NewTOS follows EmuTOS.
5. **Line-A "hacked" resolutions bypass the change.** If `rez_was_hacked`,
   the function returns `0` early without touching rez (`:1115`).

---

## Dependencies

### Requires (must be initialized / functional)

- The display driver (`v_bas_ad`, `setphys`, `atari_setrez`), the VBL
  (`vblsem`, `vsync`), the VT52/Line-A init.
- (Videl only) `Srealloc`/`vgetsize`/`vsetmode`.

### Used by

- Double-buffering, mode switches, VDI `v_opnwk`, demos.

---

## Related data structures

- `docs/api/xbios/_state.md §4` — `v_bas_ad`, `sshiftmod`, `vblsem`.
- `docs/api/xbios/04-getrez.md` — the resolution codes.

---

## Cross-references

### Atari Compendium

- Not present in `references/`. Cross-check deferred. (Documents `Setscreen`
  as `void` with the negative-ignore convention; the WORD return is EmuTOS/
  TOS-4.)

### Atari Profibuch

- Not present in `references/`. Cross-check deferred.

---

## Configuration / version notes

- The `FALCON_REZ` realloc path is `CONF_WITH_VIDEL`-only; on the NewTOS
  TT-shifter profile only ST/TT rez codes apply.

---

## NewTOS implementation notes

(Empty during Phase 00.)

PHASE-08: `Setscreen` maps to framebuffer retarget (log base) + swap-chain
present (phys base) + mode reconfigure (rez) via the VideoCore mailbox; the
screen-clear/cursor-home/VT52-reset side effects on a rez change must be
reproduced. The WORD return follows EmuTOS (`-1`/0).

---

## Pi / BCM2711 binding

> Re-points the *hardware reference* (`setphys` video-base register,
> `atari_setrez`, the Videl realloc path) at the BCM2711 VideoCore
> framebuffer. The **contract is unchanged** — the negative-argument
> "ignore" convention, the `-1`-on-invalid-rez WORD return, `rez == -1` =
> no-op. Peripheral + driver: see `docs/HARDWARE-PROFILE.md` §2/§3.1.

> **Resolution policy (ADR-017, revised 2026-07-09, `docs/MEMORY.md`):**
> NewTOS targets a **native, geometry-independent** resolution — default
> **1920×1080**, switchable at runtime to **1280×800** — both at 16 VDI
> colours, applied via the same mailbox path `Setscreen` would need to
> reach. This **supersedes** a same-day, never-committed draft that fixed
> NewTOS permanently to 640×480×16/`TT_MEDIUM`; that draft's rez-range
> widening (D-077 in `docs/api/_discrepancies.md`) is **reopened**, not
> retracted — see D-078.

- **Base re-pointing is unaffected by this policy change.** `setscreen`
  stores `log`/`phys` into shadows when each is `> 0`
  (`tos-api/src/xbios/screen.rs:72-77`, the `> 0` / `NULL-or-negative-
  ignored` rule matching `screen.c:1106,1110`), but does **not** program
  the VideoCore scan-out or retarget the `RasterBackend`. `Physbase`/
  `Logbase` report the shadow values while the display keeps showing the
  live framebuffer. Real page-flipping (a second VC buffer +
  set-virtual-offset) remains deferred, orthogonal to this ADR.
- **A real `Setscreen`-driven mode reconfigure is now in scope** (it was
  filed as deferred "1080p later" work under the superseded draft): the
  1920×1080 ⇄ 1280×800 switch needs a mailbox re-request (`set physical/
  virtual size`, re-`allocate`, `get pitch` — `kernel/src/drivers/bcm2711/
  mailbox.rs`, `docs/hardware/mailbox-framebuffer.md` §3) plus the desktop
  re-init chain (ADR-017 Decision point 5). Whether that reconfigure is
  triggered *through* the `Setscreen` XBIOS call itself (giving `rez` a
  live-switching meaning again, unlike the superseded draft's "no code
  ever really changes anything" semantics) or only through a
  desktop-menu/NEWDESK.INF path outside the XBIOS surface is an **open
  Lead decision**, tied to the `Getrez`/`Setscreen` rez-value question in
  `docs/api/xbios/04-getrez.md`.
- **The `rez` validation range and match arms below are PROVISIONAL,
  pending the same Lead decision** — they described the superseded
  single-mode policy and are kept here only as the still-relevant EmuTOS
  source precedent for "an in-range-but-hardware-unsupported `rez` is a
  silent no-op, not an error" (`atari_setrez`/`videl_setrez`/`vsetmode`,
  `bios/screen.c:990-1013`, `bios/videl.c:619-659,976-993`) — a precedent
  that still constrains whatever the Lead decides, since it is sourced from
  EmuTOS itself, not invented:
  ```
  rez == -1              → no rez change; return 0.               (screen.c:1115-1116)
  rez < 0 || rez > 7      → return -1 (invalid code).               (screen.c:1120-1121)
  rez == <TBD>            → [OPEN] which code(s), if any, map to the
                            two live NewTOS resolutions is undecided
                            (see 04-getrez.md).
  every other in-range rez → [PROVISIONAL] accept the call (return 0)
                            but make no change, mirroring EmuTOS's
                            own "hardware can't do this rez" no-op
                            precedent — unless the Lead instead wires
                            rez to the real 1920×1080⇄1280×800 switch.
  ```
- **Falcon/Videl `videlmode` argument:** still unused by NewTOS (no Falcon
  hardware to target); whether it gains a NewTOS-specific meaning (e.g.
  selecting between the two live resolutions, echoing how Falcon overloads
  it for `vsetmode`) is part of the same open Lead decision.
- **`deskrez` (desktop resolution dialog) is back in scope** under this
  ADR (it was "moot" under the superseded single-mode draft) — see ADR-017
  point 7 and `docs/api/desktop/deskrez.md`, which needs its own EmuTOS
  extraction pass before implementation.
