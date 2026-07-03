# Step 2 — code-vs-docs validation backlog

> **What this is.** Step 1 (the Pi/BCM2711 hardware binding, see
> `docs/HARDWARE-PROFILE.md`) re-points every spec's hardware reference at the
> real BCM2711 and pins the parametric values. While doing so, each binding
> agent recorded places where the **code does not (yet) match the contract**.
> This file is the resulting **Step-2 backlog**: the concrete
> code-vs-docs/contract fixes to make, subsystem by subsystem. No code is
> changed here — this is the to-do list Step 2 works through.
>
> Severity: **BUG** (wrong observable behaviour) · **GAP** (contract step
> unimplemented) · **TEXT** (spec text still cites EmuTOS figures) · **NOTE**
> (harmless divergence / bring-up limit).

---

## Screen / framebuffer

| # | Sev | Item | Where |
|---|-----|------|-------|
| S1 | BUG | XBIOS `Setpalette`/`Setcolor` write a separate ST-format shadow (`xbios/screen.rs`) that is **not** wired to the VDI CLUT (`vdi_backend::PALETTE`), so they change no displayed colour; the "apply at next VBL" timing is unmodelled. | `tos-api/src/xbios/screen.rs` |
| S2 | GAP | `Scrdmp` (XBIOS 0x14) is not wired in the XBIOS dispatcher (no 0x14 entry); the "reset `dumpflg`" contract step falls through. (No-op is correct for the printer-less Pi, but the call should exist.) | `tos-api/src/xbios/mod.rs` |
| S3 | NOTE | `Getrez` returns `2` (`ST_HIGH`, 640×400 mono) for a 640×480×16 mode; `MAX_REZ=2` blocks `Setscreen` from naming the real mode. Decision: keep `2` until the HDMI-mode-query (1080p) phase. | `tos-api/src/xbios/screen.rs:25,57` |
| S4 | NOTE | `Setscreen` has no rez-change side effects (no screen-clear / cursor-home / VT52-reset) and does not re-point the VideoCore scan-out (bounded no-op). Harmless while the mode is fixed. | `tos-api/src/xbios/screen.rs:72` |
| S5 | NOTE | `Vsync` is a synthesized 60 Hz from the generic timer (`CNTPCT*60/CNTFRQ`), not a real VideoCore vblank — terminates but not tear-free. | `kernel/src/host_env.rs:191` |
| S6 | TEXT | `v_opnvwk`/`vq_extnd` specs still cite EmuTOS `NUM_VDI_HANDLES` 64/128 and `MAX_VERTICES` 512/1024; NewTOS is **16 / 256**. Update the spec text. | `docs/api/vdi/workstation/*` |
| S7 | NOTE | Pixel size `DEV_TAB[3,4]` / `INQ_TAB[20..]` left at ROM `372` / 0 — not patched from the HDMI physical-display-size mailbox query. Revisit with 1080p. | `tos-api/src/vdi/workstation.rs:101` |
| S8 | NOTE | `Cursconf` internal flag bits differ from EmuTOS (`M_CFLASH=0x02`, `M_CVIS=0x10`) — write-only, not ABI-observable. | `tos-api/src/xbios/screen.rs` |
| S9 | NOTE | Off-screen colour MFDB↔MFDB copies are bring-up-limited (screen-form cases covered; full 16-op across arbitrary buffers is later work). | `tos-api/src/vdi/` backend |

<!-- Subsequent binding agents append their subsystem section below. -->
