# What has never been measured against EmuTOS

The reference dumper runs the same case table on a real Atari TT with EmuTOS 1.4
and on NewTOS, then diffs the results (`docs/TOS-REFERENCE-DUMPER.md`). Reading
the EmuTOS source tells us what *should* happen; only this tells us what *does*.

This page is the honest boundary of that method: which entry points it has
touched, and which it never has. Counts come from the dispatch tables
(`tos-api/src/vdi/mod.rs`, `tos-api/src/aes/mod.rs`) against the case tables
(`tosdump/cases/`), computed 2026-09-06.

| | wired | measured | **never measured** |
|---|---:|---:|---:|
| VDI | 65 | 47 | **18** |
| AES | 79 | 6 | **73** |

A function being unmeasured does not mean it is broken. It means nobody knows.
Everything measured so far came back with two real deviations in total, so the
prior is good — but the two worst finds of 2026-09-05 (a return value the
dispatcher never returned, and a test suite blind to it) were both in code that
looked fine and had never been measured.

---

## VDI — the 18 never measured

| Opcode | Function | Why it matters |
|---|---|---|
| **109** | `vro_cpyfm` | **Raster group.** Bit-block transfer: window redraw, icons, scrolling. The most-exercised part of the VDI in any GEM application. |
| **121** | `vrt_cpyfm` | Raster group, monochrome→colour expansion — every icon and every text-as-bitmap path. |
| **110** | `vr_trnfm` | Raster group, MFDB format transform. |
| 124 | `vq_mouse` | Mouse position and buttons — polled by anything interactive. |
| 128 | `vq_key_s` | Shift-key state. |
| 28 | `v_locator` | Input, mouse/keyboard sampling. |
| 31 | `v_string` | Input, string entry. |
| 14 | `vs_color` | Palette write. Colour fidelity depends on it. |
| 112 | `vsf_udpat` | User-defined fill pattern. |
| 119 / 120 | `vst_load_fonts` / `vst_unload_fonts` | Font loading. |
| 2 / 101 | `v_clswk` / `v_clsvwk` | Workstation close — the teardown half of a pair whose open half *is* measured. |
| 118, 125, 126, 127, 134 | `vex_timv`, `vex_butv`, `vex_motv`, `vex_curv`, `vex_wheelv` | Vector-install calls. |

The raster group is the sharpest gap: three opcodes, zero measurements, and the
highest call volume of anything in the VDI.

## AES — the 73 never measured

Round 1 was deliberately narrow — only calls that take no pointer, block on
nothing and change no state another case reads. That left 6 functions measured
(`evnt_timer`, `graf_handle`, `graf_mkstate`, `wind_get`, `wind_find`,
`wind_calc`), 60 of the 95 cases being `wind_calc` alone. Everything with a
tree, a menu, a resource or a window needs a state reset the harness does not
have yet — that is round 2, and it has never happened.

| Manager | Never measured |
|---|---|
| Application | `appl_init`(10) `appl_read`(11) `appl_write`(12) `appl_find`(13) `appl_tplay`(14) `appl_trecord`(15) `appl_yield`(17) `appl_exit`(19) |
| Event | `evnt_keybd`(20) `evnt_button`(21) `evnt_mouse`(22) `evnt_mesag`(23) `evnt_multi`(25) `evnt_dclick`(26) |
| Menu | `menu_bar`(30) `menu_icheck`(31) `menu_ienable`(32) `menu_tnormal`(33) `menu_text`(34) `menu_register`(35) `menu_popup`(36) `menu_attach`(37) `menu_istart`(38) `menu_settings`(39) |
| Object | `objc_add`(40) `objc_delete`(41) `objc_draw`(42) `objc_find`(43) `objc_offset`(44) `objc_order`(45) `objc_edit`(46) `objc_change`(47) `objc_sysvar`(48) |
| Form | `form_do`(50) `form_dial`(51) `form_alert`(52) `form_error`(53) `form_center`(54) `form_keybd`(55) `form_button`(56) |
| Graphics | `graf_rubberbox`(70) `graf_dragbox`(71) `graf_movebox`(72) `graf_growbox`(73) `graf_shrinkbox`(74) `graf_watchbox`(75) `graf_slidebox`(76) `graf_mouse`(78) |
| Scrap | `scrp_read`(80) `scrp_write`(81) `scrp_clear`(82) |
| File selector | `fsel_input`(90) `fsel_exinput`(91) |
| Window | `wind_create`(100) `wind_open`(101) `wind_close`(102) `wind_delete`(103) `wind_set`(105) `wind_update`(107) `wind_new`(109) |
| Resource | `rsrc_load`(110) `rsrc_free`(111) `rsrc_gaddr`(112) `rsrc_saddr`(113) `rsrc_obfix`(114) |
| Shell | `shel_read`(120) `shel_write`(121) `shel_get`(122) `shel_put`(123) `shel_find`(124) `shel_envrn`(125) `shel_rdef`(126) `shel_wdef`(127) |

---

## The two known real deviations

Everything else that differs between the TT and NewTOS is EmuTOS' own formula
producing a different value on different hardware — 640×480 with 4 planes
against 1920×1080 with 16. Matching the TT there would make us *wrong*.

1. **`vqin_mode(4)` returns 0, EmuTOS returns 1** — case 77. EmuTOS' AES issues
   `vsin_mode(4,2)` before every keyboard poll (`aes/gemgsxif.c:459-463`, called
   from `chkkbd`, `aes/gemdisp.c:157-177`); ours never does. The VDI side is
   correct. Already recorded in `docs/COMPATIBILITY.md`. **Small.**
2. **Green is quantised to 5 bits in the device round-trip** — `v_get_pixel`
   after `v_clrwk` reports `0xFFDF` where a real Falcon reports `0xFFFF`
   (`kernel/src/vdi_backend.rs`, `device_to_rgb565`). RGB565 has 6 green bits.
   Affects **every drawn colour**, not just this readback. The TT could not show
   it — it ran 4-plane. **Touches the whole colour path; plan it.**

---

## To do

Ordered by what closes the most risk per unit of work. Items marked **TT**
need the physical Atari and a measurement session; the rest are desk work.

### Measurement

- [x] ~~**TT — raster group harness.**~~ Built 2026-09-06: `DUMPRAST`
      (`tosdump/core/dumprast.c`), a separate area 6 with a 3696-byte record
      carrying two MFDBs and two pixel buffers, a 15-word CONTRL (an LP64
      caller packs the second MFDB pointer at `contrl[11..14]`), and a
      geometry guard in the loader — EmuTOS does not bounds-check a memory
      MFDB, so a typo in the table would scribble over the TT.
      `tosdump/cases/vdi-raster.tsv`, 59 cases, CRC `702EEA7D`.
- [ ] **TT — raster group measurement.** Not taken yet. What the table can
      settle: `vr_trnfm` at 8 planes or fewer is fully comparable (same plane
      permutation on both machines, and standard form is machine-independent
      by definition); the no-op rules of `vro_cpyfm`/`vrt_cpyfm` are
      comparable whatever a plane means; `vro_cpyfm` pixels at 16 planes are
      pixel-exact iff the ROM has `CONF_WITH_VDI_16BIT`, which the table
      measures rather than assumes. What it cannot: device-form pixels below
      nine planes (TT word-interleaved planes vs NewTOS one 16-bit word per
      pixel), any `vrt_cpyfm` that actually expands (the result is a colour),
      and `vr_trnfm` above 8 planes (`TRUECOLOR_MODE = v_planes > 8` splits
      the two machines on video mode, not on the VDI).
- [ ] **TT — AES round 2.** Trees, menus, resources, windows. Blocked on a state
      reset the harness does not have; designing that reset is the real work and
      can be done before the TT session.
- [ ] **TT — the remaining 15 VDI opcodes** (mouse/input/vector group). Lower
      volume, cheaper once the harness is out anyway. Triaged 2026-09-06
      against the EmuTOS source, not yet turned into cases: **safe to add**
      `vsf_udpat` (112), `vst_load_fonts`/`vst_unload_fonts` (119/120),
      `vq_key_s` (128, deterministic only while nobody holds a modifier);
      **must not be added** `v_clswk` (2, tears the AES down under the
      running program), `v_clsvwk` (101, every later case would carry a dead
      handle and `td_gem_close` would close it twice), the five `vex_*`
      vector installs (118/125/126/127/134 — they hand a C function to an
      interrupt path and return a machine-dependent old vector);
      **undecided, needs a source check before it is written** `v_locator`
      (28) and `v_string` (31), which block in request mode, `vq_mouse`
      (124), whose position is not comparable, and `vs_color` (14), which
      changes the device palette for the rest of the session.
- [ ] Fold the AES round-1 dump comparison into CI-visible form the way
      `vdi_reference` and `aes_reference` already are, for whatever round 2 adds.

### Repair

- [ ] **`vqin_mode` / `vsin_mode(4,2)` before keyboard polls.** Small, isolated,
      EmuTOS-sourced. Do this one first.
- [ ] **6-bit green in the device round-trip.** Plan first: it changes every
      colour, the MFDB round-trip and possibly stored screendump baselines.

### Method

- [ ] One logged Pi-4 run per week into `docs/HW-TEST-LOG.md`. The inventory's
      conclusion stands: the largest risk before the October meeting is not a
      missing function but a missing measurement.
- [ ] Predict before measuring. The `54 → 15` prediction of 2026-09-05 held
      exactly and made the result unarguable; the `a3563cb` post-hoc reading did
      not and was wrong.
