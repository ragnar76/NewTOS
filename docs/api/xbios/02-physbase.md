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

---

## Pi / BCM2711 binding

> Re-points the *hardware reference* (Shifter video-base registers
> `0xFFFF8201`/`0xFFFF8203`/`0xFFFF820D`) at the BCM2711 VideoCore
> framebuffer. The **contract is unchanged** — a full-64-bit pointer, no
> error path, `Physbase() == Logbase()` on the no-page-flip profile.
> Peripheral + driver: see `docs/HARDWARE-PROFILE.md` §2 (Screen /
> framebuffer).

- **What the base *is*.** The single VideoCore-allocated framebuffer, one
  buffer for both physical and logical (no hardware page flip). The mailbox
  reply's bus address is masked to an ARM physical address and offset into
  the kernel high half (`kernel/src/drivers/bcm2711/framebuffer.rs`), and
  that region is what `screen()`/`Physbase` describe
  (`kernel/src/vdi_backend.rs`, `kernel/src/host_env.rs`).

### Address space and cacheability (NewTOS, 2026-09-05)

- **The returned pointer is in the caller's address space** (ADR-023). An
  **EL1** caller — the desktop, the VDI, a boot self-test — gets the kernel
  high-half VA, which is what it can reach. An **EL0** caller gets the
  framebuffer published into its own per-process window
  (`usermem::expose_attr`) and the **user VA** back; the kernel VA is
  EL1-only and a program that followed it would take a permission fault, so
  handing it out satisfied the letter of "a pointer to the screen" and not
  its substance. Measured on QEMU `raspi4b`: `Physbase() = 0x00400000` from
  an EL0 `.NCP`, previously `0xFFFF0000…`.
- **The mapping is `PROT_G`** (FreeMiNT "global — any access OK",
  `references/freemint/sys/mint/mem.h:130-134`), not `PROT_PR`: the point of
  `Physbase` is that the program *writes* pixels.
- **The mapping is Normal Non-Cacheable** (MAIR AttrIdx 2). The pixels are
  read out by the VideoCore display engine, a DMA master that scans DRAM
  from outside the ARM caches (`docs/hardware/mailbox-framebuffer.md` §6).
  The kernel's own alias is Write-Back and the VDI cleans each drawn
  rectangle to the point of coherency itself; an EL0 program has no such
  hook and the TOS API has no cache-maintenance call, so the pages it is
  handed are the ones where a plain store lands in DRAM unaided. While such
  a mapping is live the VDI backend cleans **and invalidates** its
  rectangles instead of merely cleaning them, so the two aliases with
  mismatched cacheability stay coherent (`kernel/src/vdi_backend.rs`,
  `tos-api/src/bdos/process/usermem.rs`).
- **The published span** is `stride_bytes × height` — the whole pixel store,
  1.17 MiB at 640×480 and 7.91 MiB at 1920×1080. It is taken from the
  process's 16-MiB EL0 window, so a 1080p program keeps ≈ 7.7 MiB of window
  for `Malloc`.
- **Format.** Linear XRGB8888 (`0xFF_RR_GG_BB`), *not* Atari bitplanes.
  Shifter-format direct-screen writers are a project non-goal (CLAUDE.md
  "Non-goals") and belong to the later m68k emulator; a program written for
  NewTOS gets the linear buffer.
- **Proof.** `sdk/examples/physbase.c` (`PHYSBASE.NCP`) calls `Physbase`,
  stores three colour bands through the returned pointer with no cache
  maintenance of any kind, and blocks; the QMP screendump shows the bands.
- **How it is read.** `physbase()` returns the `PHYSBASE` shadow, or the
  live framebuffer base when the shadow is `0`
  (`tos-api/src/xbios/screen.rs:41-46`); the live base comes from
  `Host::screen().base` (`tos-api/src/xbios/screen.rs:36-38`). Before the
  framebuffer is up, `screen()` returns `None` and `Physbase` yields `0`.
- **Geometry (current):** 640 × 480, 32 bpp XRGB8888, single buffer
  (`framebuffer.rs:23-25`; `virt:` `qemu_virt/ramfb.rs:75-76`). Buffer
  size ≈ `pitch × 480`; the VC-reported pitch is 640 × 4 = 2560 B/line for
  this mode.
- **Setscreen interaction — verified 2026-09-05, still a shadow only.**
  `Setscreen(-1, phys, -1)` writes the `PHYSBASE` shadow and nothing else:
  the shadows are read by `physbase()`/`logbase()` and by no other code in
  the tree — the raster backend always draws into, and the VideoCore always
  scans out, the live framebuffer (`kernel/src/vdi_backend.rs`,
  `active_fb()`). So `Physbase` reports the new value while the display and
  the drawing target are unchanged, and there is **no double buffering**.
  Full page-flipping (a double-height virtual buffer + a mailbox
  set-virtual-offset flip, plus a user-VA → physical translation for the
  caller-supplied base) is a separate work item.
- **Datasheet.** Mailbox property transport: BCM2711 peripherals datasheet
  §1.3 (`references/datasheets/RP-008248-DS-1-bcm2711-peripherals.pdf`);
  the framebuffer allocate/get-pitch tags are firmware-property tags, not
  peripheral registers.
- **1080p later.** The 640×480 geometry is hard-coded today; querying the
  live HDMI mode and returning a mode-sized base is the deferred phase.
