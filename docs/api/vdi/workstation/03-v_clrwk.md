# `v_clrwk` — VDI opcode 3 (clear workstation)

> Phase 00b spec. Source: EmuTOS @ `1a7484ab` —
> `vdi/vdi_control.c:599` (`vdi_v_clrwk`). Calling convention:
> [`../_dispatcher.md`](../_dispatcher.md).
>
> **Contract vs. reference** — see [`docs/COMPATIBILITY.md`](../../../COMPATIBILITY.md).
> **Contract:** opcode 3; clears the *entire* screen to background; no
> inputs/outputs; ignores the clip rectangle. **Reference:** the raw
> `memset(v_bas_ad, …)` of the framebuffer and the `0x00`/`0xff` fill byte
> — NewTOS clears the `hal::framebuffer` instead.

Clears the entire screen surface to the background colour.

## EmuTOS source

**Primary location:** `vdi/vdi_control.c:599` — `vdi_v_clrwk(Vwk *vwk)`
(reached via `jmptb1[2]`, `vdi_main.c:34`).

**Globals read:** `v_lin_wr` (bytes per scanline), `V_REZ_VT` (vertical
resolution), `v_bas_ad` (screen base). **Build config:**
`CONF_WITH_VDI_16BIT` selects the fill byte.

## Source extraction summary

`v_clrwk` computes the framebuffer byte size as `v_lin_wr * V_REZ_VT` and
`memset`s the whole screen from `v_bas_ad`. The fill byte is **`0x00`**
for palette modes (colour index 0) and — **only when
`CONF_WITH_VDI_16BIT` is built and the mode is TrueColor** — `0xff` for
the 16-bit virtual palette. It clears the **full screen**, ignoring the
workstation's clipping rectangle.

## Trap interface

| | |
|--|--|
| Opcode | `CONTRL[0] = 3` |
| Handle | `CONTRL[6]` (resolved, but the whole screen is cleared) |
| `# INTIN / PTSIN / INTOUT / PTSOUT` | 0 / 0 / 0 / 0 |

Native ABI: GEM trap `svc #2`, selector 115 in `x0`, `VDIPB*` in `x1`.

## Inputs / Outputs / Errors

No inputs, no outputs, no return value, no error path — always succeeds.

## Verification criteria

- After `v_clrwk`, every pixel of the framebuffer holds the background
  value (colour index 0 in a palette mode).
- The clip rectangle has **no** effect: a workstation with a small clip
  still clears the whole screen.

## Edge cases discovered in source

1. **Clip is ignored** — EmuTOS clears the full screen, not the clip
   region (`vdi_control.c:604-618`).
2. **Fill byte is `0x00` for palette modes** (index 0), `0xff` only in a
   `CONF_WITH_VDI_16BIT` TrueColor build.

## Dependencies

- Requires an open physical workstation (framebuffer base + geometry).
- `v_updwk` (opcode 4, "update workstation") is a **no-op** (`vdi_v_nop`,
  `vdi_main.c:36`) on the immediate-mode screen driver — no separate spec.

## Cross-references

### Atari Compendium / Profibuch
- Not present in `references/` (no PDFs). Cross-check deferred.

## NewTOS implementation notes

- Clears the whole `hal::framebuffer` to the background colour via the
  `RasterBackend`; ignores the clipping rectangle (matches EmuTOS).
- The `0x00`/`0xff` fill-byte distinction is a **reference** detail tied
  to EmuTOS's framebuffer format; NewTOS writes background colour index 0
  through its own backend regardless.

---

## Pi / BCM2711 binding

> Re-points the *hardware reference* (the raw `memset(v_bas_ad, …)` of the
> Shifter framebuffer) at a `RasterBackend` full-screen fill over the
> BCM2711 VideoCore buffer. The **contract is unchanged** — clears the
> whole screen to background (PEL 0), ignores the clip rect, always
> succeeds. Peripheral + driver: see `docs/HARDWARE-PROFILE.md` §2/§3.1.

- **How it clears.** `v_clrwk` fills the full `width × height` device with
  PEL 0 (background) via `RasterBackend::fill_rect` over a full clip rect
  (`tos-api/src/vdi/workstation.rs:193-196`); geometry is the live
  framebuffer size (640×480), so the whole VideoCore buffer is cleared,
  clip ignored — matching the contract.
- **PEL 0 = white.** The bring-up palette maps index 0 to white
  (`kernel/src/vdi_backend.rs:26-31`), so the cleared screen is white
  (GEM's default desktop background), not black. The EmuTOS `0x00`/`0xff`
  fill-byte choice is irrelevant here — NewTOS writes the resolved RGB of
  PEL 0 into the 32 bpp buffer through the backend.
- **Cache clean for scan-out.** The backend cleans the filled rectangle to
  PoC after writing (`vdi_backend.rs:118-125`, `:201`) so the VideoCore
  display DMA sees the cleared pixels on real hardware (the framebuffer is
  mapped Non-Cacheable on rpi4, making this a no-op there;
  `framebuffer.rs:183-187`).
- **1080p later.** Clears whatever geometry the `RasterBackend` reports;
  no dependency on the fixed 640×480 mode.
