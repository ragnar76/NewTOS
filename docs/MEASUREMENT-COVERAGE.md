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

The AES row is the *measured* state. Round 2 (`cases/aes-r2-wind.tsv`, 78
cases) is written and built for both machines but **not yet run**; when it is,
it moves seven window opcodes — `wind_create`, `wind_open`, `wind_close`,
`wind_delete`, `wind_set`, `wind_update`, `wind_new` — out of the never-column,
and re-measures `wind_get`/`wind_find` against windows that really exist.

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
tree, a menu, a resource or a window needs a state reset the harness did not
have. For **windows** it now has one — `wind_new`, declared by the table and
issued before every case (`cases/aes-r2-wind.tsv`, 78 cases, written
2026-09-06, **not yet run**). For **trees, menus and resources** it is still
blocked, and not on the reset: they take an `addr_in[0]` the case-table format
has no column for.

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
| Window | `wind_create`(100) `wind_open`(101) `wind_close`(102) `wind_delete`(103) `wind_set`(105) `wind_update`(107) `wind_new`(109) — **all seven are in round 2's table, awaiting the run** |
| Resource | `rsrc_load`(110) `rsrc_free`(111) `rsrc_gaddr`(112) `rsrc_saddr`(113) `rsrc_obfix`(114) |
| Shell | `shel_read`(120) `shel_write`(121) `shel_get`(122) `shel_put`(123) `shel_find`(124) `shel_envrn`(125) `shel_rdef`(126) `shel_wdef`(127) |

---

## The known real deviations

Everything else that differs between the TT and NewTOS is EmuTOS' own formula
producing a different value on different hardware — 640×480 with 4 planes
against 1920×1080 with 16. Matching the TT there would make us *wrong*.

1. ~~**`vqin_mode(4)` returns 0, EmuTOS returns 1** — case 77.~~ **Closed
   2026-09-06.** EmuTOS' AES issues `vsin_mode(4,2)` before every keyboard poll
   (`gsx_char`, `aes/gemgsxif.c:459-463`, called from `chkkbd`,
   `aes/gemdisp.c:157-177`); NewTOS' `dsptch()` now calls a `chkkbd()` that
   performs the same write (`tos-api/src/aes/mod.rs`). Verified against the
   dump, not asserted: `tests/aes_reference.rs`'s
   `aes_dispatch_leaves_the_string_device_in_sample_mode` decodes r2 record 16
   and all record 40 and compares. The `vdi_reference` case 51 keeps its
   exclusion — that binary runs no AES *and* opens the physical workstation
   itself, which is where EmuTOS' `vdimouse_init` resets all four devices
   (`vdi/vdi_control.c:563`), so 0 is the right answer for that sequence; the
   reset is now implemented too (`v_opnwk`, `tos-api/src/vdi/workstation.rs`).
2. **`v_get_pixel` after `v_clrwk` reports `0xFFDF` where a Falcon reports
   `0xFFFF`** (`kernel/src/vdi_backend.rs`, `device_to_rgb565`). The TT could
   not show it — it ran 4-plane.

   **Cause corrected 2026-09-06 (VDI-OVL-001).** This item previously read
   "green is quantised to 5 bits, RGB565 has 6". That diagnosis is **wrong**:
   EmuTOS' truecolor colour model *has* five green bits — `set_color16` packs
   `(r<<11)|(g<<6)|b` from 5-bit guns (`vdi_col.c:601-614`) and `vq_color16`
   reads green back as `(rgb>>6)&0x1f` (`vdi_col.c:802-838`). Bit 5 is
   `OVERLAY_BIT` (`vdi_fill.c:26`), not a green LSB, and our 5-bit
   quantisation is faithful. What is actually lost is the overlay bit itself,
   which `v_clrwk`'s `memset(…, 0xff, …)` sets and a 24-bit device pixel
   cannot hold. Measured on `board-qemu-virt`: a cleared screen and a pen-0
   fill are the *same* framebuffer word `0xFFFFFFFF`. It affects three
   observable paths (`v_get_pixel`, `vro_cpyfm` Screen→Memory, and
   `v_contourfill` seeded on the cleared background), **not** every drawn
   colour. Diagnosis, blast radius, fix options and the open Falcon/VIDEL
   spec question: `docs/STATUS.md`, Recent Decisions 2026-09-06.
   **PARKED 2026-09-06 — User-Entscheidung.** Alle drei Reparaturvarianten
   sind invasiv, und die Frage, die zwischen ihnen entscheidet — zeigt das
   Falcon-VIDEL Bit 5 an? — hat **keine Quelle im Baum und keine Maschine**,
   auf der sie sich messen liesse. Keine bekannte Anwendung beruehrt es.
   Wortlaut des Users: *„wir sollten uns an solchen dingen nicht festhalten,
   wenn etwas kaputt ist dann merken wir das dann bei echten anwendungen“*.
   Das ist ein **geparkter diagnostizierter Defekt**, ausdruecklich **keine**
   bewusste Abweichung — er gehoert deshalb nicht in `COMPATIBILITY.md` und
   kommt zurueck, sobald eine echte Anwendung ihn zeigt.

---

3. **`vro_cpyfm` anchors an oversized destination rectangle differently** —
   `D-VDI-25`, measured as raster case 157. EmuTOS takes `PTSIN[6..7]`
   literally and starts a backwards copy there; we anchor at the destination's
   upper left. **Decision 2026-09-06 (user): documented and deferred.** Worst
   case is a drawing fault, not a crash — the copy size always comes from the
   source and both machines stay inside their form, so a block lands in the
   wrong place. No well-formed caller is affected, which is why the other 25
   pixel cases match bit-exactly. The route, if taken up: the anchoring is
   reproducible, the address comparison is not, and the screen-MFDB paths must
   be measured first.

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
- [x] ~~**AES round 2 — the state reset.**~~ Designed and built 2026-09-06.
      The AES has no attribute set to restore the way the VDI does, but the
      **window manager has the AES' own reset**: `wind_new` → `wm_new`
      (`aes/gemwmlib.c:1875-1893`) closes and deletes every window with a
      handle ≥ 1 and drains the `wind_update` mouse-control and screen-sync
      locks, and EmuTOS issues it itself after every graphics-mode program
      the shell runs (`aes/gemshlib.c:676-677`) — so a run that issues it
      leaves nothing the shell would not have swept. The table declares
      which reset runs, on a `!reset` directive line
      (`tosdump/core/cases.h`), because the dumper is double-clicked with no
      arguments and the file's CRC is the only thing a dump carries;
      absent means round 1's `none`, so `aes-r1.tsv` stays reproducible.
      `tosdump/cases/aes-r2-wind.tsv`, **78 cases, 126 preludes, CRC
      `AFB11C9F`**, closing the seven never-measured window opcodes.
- [ ] **TT — AES round 2 measurement.** Not taken yet. Deployed to
      `tosdump/deploy-tt/` and `tosdump/deploy-pi4/`.
- [ ] **TT — AES round 3: trees, menus, resources.** Still blocked, and not
      on the reset. `menu_bar`, `objc_*`, `rsrc_*`, `form_*` all take an
      `addr_in[0]` the case table has no column for, and an OBJECT built by
      the dumper is 24 bytes on the m68k and wider under the LP64 NewTOS
      build. That needs a *fixture* mechanism — the table names a tree the
      way `cases/vdi-raster.tsv` names a buffer — not another reset. The
      menu manager additionally needs its own reset (`menu_bar(tree, 0)`
      clears `gl_mntree`, `aes/gemmnlib.c:724-728`), which `wm_new` does
      not do.
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

- [x] ~~**`vqin_mode` / `vsin_mode(4,2)` before keyboard polls.**~~ Done
      2026-09-06: `dsptch()` → `chkkbd()` → `vdi::set_input_mode(4, 2)`, plus
      the `vdimouse_init` reset in `v_opnwk` that `str_mode != 0` made
      observable. One new test, `aes_dispatch_leaves_the_string_device_in_
      sample_mode`.
- [ ] **6-bit green in the device round-trip.** Plan first: it changes every
      colour, the MFDB round-trip and possibly stored screendump baselines.

### Method

- [ ] One logged Pi-4 run per week into `docs/HW-TEST-LOG.md`. The inventory's
      conclusion stands: the largest risk before the October meeting is not a
      missing function but a missing measurement.
- [ ] Predict before measuring. The `54 → 15` prediction of 2026-09-05 held
      exactly and made the result unarguable; the `a3563cb` post-hoc reading did
      not and was wrong.
