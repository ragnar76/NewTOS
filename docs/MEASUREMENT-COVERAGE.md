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

- [ ] **TT — raster group.** Add `vro_cpyfm`, `vrt_cpyfm`, `vr_trnfm` to the
      case table and measure. Three opcodes, highest call volume in the VDI,
      zero coverage. Needs MFDB fixtures, so the case format may need widening.
- [ ] **TT — AES round 2.** Trees, menus, resources, windows. Blocked on a state
      reset the harness does not have; designing that reset is the real work and
      can be done before the TT session.
- [ ] **TT — the remaining 15 VDI opcodes** (mouse/input/vector group). Lower
      volume, cheaper once the harness is out anyway.
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
