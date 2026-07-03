# VDI source-vs-doc discrepancies (Phase 00b)

> Mirrors `docs/api/gemdos/_discrepancies.md`. Logged when the EmuTOS
> source disagrees with the secondary references (Compendium /
> Profibuch), with an earlier revision of our own spec, or is internally
> surprising. **EmuTOS is authoritative** (CLAUDE.md Rule 1); the spec
> follows the source and the discrepancy is recorded here for
> traceability. EmuTOS pinned at `1a7484ab`.
>
> **Cross-references (Compendium / Profibuch) are deferred:** there are no
> PDFs in `references/` (only `emutos/`, `freemint/`, `circle/`,
> `datasheets/`). Every "cross-check against the documented ABI" below is
> therefore **pending** until those references are acquired.
>
> ID scheme: `D-VDI-N`. `V-1` is the original pre-existing entry, kept as
> `D-VDI-1`.

| # | Function / area | Source behaviour (EmuTOS `1a7484ab`) | Note / resolution |
|---|-----------------|--------------------------------------|-------------------|
| D-VDI-1 (was V-1) | `vqm_attributes` (36) | Returns `INTOUT[0] = vwk->mark_index` **0-based**, while `vql_attributes`/`vqf_attributes` return their index `+1` and `vsm_type` stores `type-1`. A round-trip `vsm_type(n)` → `vqm_attributes` reports `n-1`, not `n`. | Internal asymmetry in EmuTOS, preserved verbatim as the source behaviour. **TODO:** cross-check the intended value against the Compendium `vqm_attributes` entry before Phase-12. See [`inquiry/_attributes.md`](inquiry/_attributes.md). |
| D-VDI-2 | `v_escape` sub 16 `vq_tabstatus` | `INTOUT[0] = 1` unconditionally ("a tablet/mouse device is available"), matching Atari TOS (`vdi_esc.c:248-252`). | **Doc correction:** an earlier revision of `_escape.md` claimed `INTOUT[0] = 0`. Fixed to `1`. Not an EmuTOS-vs-reference conflict; a prior-doc error. |
| D-VDI-3 | workstation handles (`v_opnvwk` 100 / dispatcher) | Handle range is `1..NUM_VDI_HANDLES`; `NUM_VDI_HANDLES` = **64** (small/STe target) or **128** (default) (`vdi_control.c:25-27`; `config.h:391-393,1660-1662`). | **Doc correction:** earlier `100-v_opnvwk.md` / `vwk.md` said "16 handles" — wrong. Fixed to the config-dependent value. **Open question:** which target profile does NewTOS adopt? (Affects the handle cap and the `MAX_VERTICES` clamp.) |
| D-VDI-4 | `v_escape` sub 17 `v_hardcopy` | Calls XBIOS `Scrdmp()` — a **real** screen dump to printer (`vdi_esc.c:260-263`), not a no-op. | **NewTOS scope decision** (not an EmuTOS conflict): the bring-up profile has no printer, so `v_hardcopy` is a documented no-op (return cleanly), recorded in `docs/STATUS.md` "Blocked" if a program needs it — an honest deferral, not a faked success. |
| D-VDI-5 | `DEV_TAB[35]` / `DEV_TAB[39]` (`v_opnwk`/`v_opnvwk` output) | ROM `DEV_TAB_rom[35] = 0`; patched to `(v_planes==1)?0:1` by `update_rez_dependent`. `[39]` = `get_palette()`, not the literal ROM `2` (`vdi_control.c:164,168,206-208`). | **Doc clarification:** the contract value a program reads is the *patched* value, not the ROM template. Earlier `100-v_opnvwk.md` labelled `[35]` without noting the ROM value is 0. Fixed. |
| D-VDI-6 | Native GEM-trap selector (NewTOS ABI, not EmuTOS) | m68k VDI = `trap #2`, `d0 = 0x73`, `d1 = VDIPB*` (`vdi_asm.S:35`, `gsx2.c:26-28`). NewTOS maps this to `svc #2` with a selector in `x0`. | **Doc conflict, NewTOS side:** the task charter / `_dispatcher.md` use selector **115**, while `COMPATIBILITY.md` says "GEM `svc #2` sel 200". These disagree. **Open question:** fix the canonical native GEM-trap selector before Phase-12 wiring. Purely a NewTOS ABI number, no EmuTOS fact involved. |
| D-VDI-7 | `DEV_TAB`/`INQ_TAB` values for the NewTOS framebuffer | EmuTOS patches `DEV_TAB[0,1,3,4,13,35,39]` and `INQ_TAB[4,5]` from `V_REZ_HZ/VT`, `v_planes`, `get_pixel_size`, `get_palette`, `get_monitor_type` (`vdi_control.c:203-213`). | **Open question:** which exact values does NewTOS report for its 640×480×16-colour (4-plane) framebuffer — `[13]=16`, `[35]=1`, `[39]`=palette size, `INQ_TAB[4]=4`, `[5]=1`? And are `DEV_TAB[3,4]` (pixel size in microns) reported truthfully or left at the ROM `372`? Decide during Phase-12 backend bring-up. |
