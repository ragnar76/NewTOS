# QED bring-up — verified findings log (lead)

Running log of root-cause findings that have been **verified against sources**
(EmuTOS + gemlib/cflib + our code), as we investigate the QED-on-desktop
rendering bugs. Separate from the two auditors' raw research docs
(`qed-desktop-rendering-rootcause.md`, `qed-syscall-inventory.md`) — this holds
conclusions the lead has confirmed. **No fixes here — research only.**

---

## F1 — `appl_getinfo(130)` / `appl_search(18)` return -1: NOT a bug (verified 2026-07-20)

**Question (user):** our AES dispatch has no opcode 18/130, so both hit
`_ => pb.set_ret(-1)` (`tos-api/src/aes/mod.rs:228`). An auditor said "EmuTOS
also returns -1, so it's contract-conformant." The user doubted it: `appl_getinfo`'s
documented return is **0 (fail) / non-zero (ok)** — so a -1 could be read as
*success*, making the caller read unfilled out-params (garbage).

**Verified resolution — it is safe, and here is the real reason (deeper than
"EmuTOS does it too"):**
- Both callers **version-gate** the `appl_getinfo` trap on the reported AES
  version `global[0] >= 0x0400` (AES 4.0):
  - gemlib `mt_appl_getinfo` — `sdk/gemlib/gemlib-0.44.0/a_appl_getinfo.c:365`
    (`has_agi = global_aes[0] >= 0x400 || mt_appl_find("?AGI…") >= 0`).
  - cflib `appl_xgetinfo` — `sdk/cflib/cflib-21/xgetinfo.c` (same gate).
- Our `appl_init` reports **`AES_VERSION = 0x0340`** (`tos-api/src/aes/appl.rs:19`),
  identical to EmuTOS (`references/emutos/include/config.h:1347`). `0x0340 < 0x0400`.
- ⇒ the gate is **false**, so gemlib/cflib **never issue** the trap; they return
  `0` (unavailable) locally and take the basic (pre-AES-4.0) path. Our -1 is
  never reached by them. That — not the -1 value — is why it's harmless.
- This is exactly why it's harmless in EmuTOS too: EmuTOS reports the same
  0x0340, so the trap is never called there either.

**Guard for future work:** do **NOT** raise `AES_VERSION` to `>= 0x0400` without
first implementing `appl_getinfo(130)` for the types apps query — bumping the
version flips the gate on, and then the -1 WOULD be read as "success" → garbage
out-params. If we ever want 4.0 features, implement 130 AND change the
unknown-opcode default so a valid-but-unimplemented AES call fails as `0`, not `-1`.

**Residual to confirm (low risk):** the 2nd gate `mt_appl_find("?AGI\0\0\0\0") >= 0`.
Our `appl_find` (opcode 13) must NOT match the pseudo-name `?AGI` (it searches
*registered* app names; `?AGI` is not registered → should return -1). Confirm
`tos-api/src/aes/appl.rs appl_find` has no accidental match. If it returned >= 0,
the gate would trigger appl_getinfo despite the 0x0340 version.

---

## F2 — CRITICAL: `vro_cpyfm`/`vrt_cpyfm` read MFDB pointers at LP64 offsets; QED (ILP32) packs them differently (verified 2026-07-20)

**Confirmed root cause (independently verified).** The raster blit ops read the
source/destination MFDB pointers out of the VDI CONTRL array at **LP64** offsets,
but QED links gemlib compiled **ILP32**, which packs them at half the stride:
- gemlib: `N_PTRINTS = sizeof(void*)/sizeof(short)` (`gem_vdiP.h:177`) = **2 for
  ILP32** (4 for LP64); pointer `n` is written at `CONTRL[7 + n*N_PTRINTS]`
  (`gem_vdiP.h:204`). ⇒ ILP32: **src MFDB @ CONTRL[7..8], dst MFDB @ CONTRL[9..10]**.
- ours: `tos-api/src/vdi/raster.rs:198` → `Mfdb::from_ptr(pb.contrl_u64(7))`,
  `pb.contrl_u64(11)` — reads **8-byte** pointers at CONTRL[7..10] (src) /
  CONTRL[11..14] (dst) = the **LP64** layout (the module doc lines 4-8 states this
  is the chosen "NewTOS native" convention).
- ⇒ under ILP32, `contrl_u64(7)` fuses QED's src-ptr (CONTRL[7..8]) and dst-ptr
  (CONTRL[9..10]) into one garbage 64-bit VA; `contrl_u64(11)` reads past the
  packed pointers (garbage). Both MFDB pointers are misread.
- **Impact (CONFIRMED static):** QED's window.c scroll uses `vro_cpyfm`
  screen→screen (`s.fd_addr=d.fd_addr=NULL`), called on every text scroll. With
  garbage (non-null) MFDB pointers our code treats them as memory-MFDBs and
  dereferences them → EL1 fault / corrupt blit. Not visible in the initial static
  screen (the HW photo), but hits as soon as QED scrolls.

**Fix direction (not implemented):** read the MFDB pointers at the **caller ABI
width** (as AESPB/VDIPB already do for pointer arrays) — ILP32: src = 32-bit @
CONTRL[7], dst = 32-bit @ CONTRL[9]; LP64: keep the current 64-bit @ [7]/[11].
This is the deferred "VDI CONTRL/MFDB ILP32 packing" item; also audit any other
`contrl_u64`/CONTRL pointer read in the VDI for the same LP64 assumption
(`vr_trnfm`, user-defined, etc.).

## F3 — ILP32 TEDINFO/BITBLK/ICONBLK render blank + `objc_edit` rejects ILP32 (= Part 2 / #106) (confirmed 2026-07-20)

**Confirmed (known deferred).** Stage-A `!is_ilp32()` guards mean that for an
ILP32 tree (QED), `object.rs` does NOT draw the ob_spec-pointed sub-structs:
G_TEXT/G_BOXTEXT/G_FTEXT/G_FBOXTEXT (TEDINFO), G_IMAGE (BITBLK), G_ICON (ICONBLK)
— `tos-api/src/aes/object.rs` ~1538/1597/1621 — and `objc_edit` rejects editing
under ILP32 (~830-832). ⇒ QED's dialog **edit fields render blank and are not
editable**, and dialog icons are blank. Menu/dialog **structure** is fine
(G_STRING/G_TITLE/G_BOX/G_BUTTON render correctly — ObjTree ILP32 offsets
verified). This is exactly task **#106** (Part 2). Not a new bug — a designed,
documented deferral, now confirmed as a real QED-dialog gap.

## Open (NOT a syscall stub) — the "thin-strip" window
The syscall audit could **not** attribute the thin-strip window to any stub: QED's
default window size (edit.c:2239) derives from cflib's `gl_desk`, set by cflib
ApplInit via `wind_get(0, WF_WORKXYWH)`; our `wind_get` returns the correct
`desktop_work` (screen minus `menu::desk_top()`, which is 19 or 0 — both normal
height). So the thin-strip is likely QED init-order / `gl_desk`-population, not a
missing/partial syscall. → being reproduced in QEMU by the rendering auditor (#109).

---
## #110 syscall-inventory completion (2026-07-20) — additions

**F2 is broader than the CONTRL packing alone.** Per the completed audit:
- Also affects **`vr_trnfm` (op 110)** and **`vrt_cpyfm` (121, color icons via
  cflib cicon)** — same LP64 MFDB assumption (`raster.rs:391`).
- Plus the **MFDB struct field widths**: `Mfdb::from_ptr` (`raster.rs:60-77`)
  expects an **8-byte `fd_addr`**, but QED's ILP32 MFDB has a **4-byte `fd_addr`**
  (and the following WORD fields shift accordingly). So the fix must ABI-width
  BOTH the CONTRL pointer read AND the MFDB struct decode.
- `vdi/mod.rs:101-116` explicitly admits "no ILP32 raster op is reachable through
  gemtest yet" — i.e. this was a known, flagged LP64 assumption.

**F4 — low-severity GDOS/printer gaps (PLAUSIBLE, off-screen, not QED-startup).**
`vq_devinfo (248)` MISSING → no-op; `vs_document_info`/escape sub-op>19 no-op
(`vdi/escape.rs:141`); `v_opnprn` opens the **screen** instead of a printer
(`v_opnwk` ignores the work_in device); `vqt_ext_name` sub-op ignored → system
font (`inquiry.rs:191`). All gated by `gl_gdos`/printing — not a rendering path.

**The 3 visible startup symptoms are NOT syscall stubs** (audit verified the
functions FULL): thin-strip (gl_desk/`wind_get` correct → QED init-order or kernel
desktop composition), dropdown redraw (menu save/restore present; only cross-tree
*submenu* selection deferred, irrelevant to QED's single-tree menu), no-handoff
(`v_opnvwk`/`v_clrwk` FULL → the kernel desktop-vs-QED **composition**: AES
`compose()` + `DESK_PAINTER`, `tos-api/src/aes/window.rs:236,270`). All three →
rendering-auditor #109 + real-Pi4 confirmation.

**Bug density:** ~100 QED-used AES/VDI functions audited → 2 confirmed
QED-reachable ILP32 defect classes (F2, F3) + 3 low GDOS/printer gaps (F4).
Everything else on QED's path is FULL. GEMDOS/BIOS not deep-audited (known ILP32
struct-width latency, [[lp64-tos-struct-trap]]).

---
## F5 — CRITICAL, root cause of symptom #1 (handoff) — STATICALLY CONFIRMED, no HW needed (2026-07-20)

The syscall auditor pointed at `compose()`/`DESK_PAINTER` for the "no handoff"
symptom but deferred it to "real-Pi4 test". Reading the code proves the cause
**statically** — no HW test needed. This is the lead's independent finding.

**Mechanism (`tos-api/src/aes/window.rs`):**
- Our AES is a **compositor** with a single **shared global window list**
  (`slots()`) and a registered **desktop-background painter** `DESK_PAINTER`
  (`window.rs:227`, set via `set_desktop_painter`).
- `compose()` (`:270`) = `paint_bg_clip()` (the DESK_PAINTER background) **+ every
  open window in the shared list**, back-to-front. `paint_bg_clip` (`:236`): if a
  painter is registered it paints it (the desktop's **green** bg + icons); if
  none, a plain **white** erase — literally EmuTOS's CLEAR_SCREEN.
- Every `wind_open`/redraw calls `compose()`.

**Why symptom #1 happens:** launching QED is a **nested `run_program`** from the
desktop's `event_loop` — the desktop never relinquishes. So when QED opens its
window, `compose()` still (a) paints `DESK_PAINTER` = the **desktop's green
background + icons** (never cleared/swapped), and (b) draws the **desktop's own
windows** (still `open` in the shared `slots()` list, never closed). QED's window
composites on top. ⇒ exactly the HW photo: green everywhere + leftover desktop
windows behind QED. The green (not white) is because DESK_PAINTER is still the
desktop's painter — a relinquished GEM screen would be the white CLEAR_SCREEN.

**This reconciles the auditor's "composition → real-HW" punt:** the cause is
plainly in `compose()`; it did NOT need HW. (Note to self: the auditor's static
"correct / not a stub → real-HW" conclusions for the visible symptoms are the same
class of glib dismissal as its initial "appl_getinfo -1 kein Bug" — verify them,
don't accept them. F5 is what that verification found for the handoff.)

**Fix direction (not implemented):** on launching a GEM app the desktop must
**relinquish** the shared AES state — clear/neutralise `DESK_PAINTER` (→ white
CLEAR_SCREEN) and close/hide its own windows out of `slots()` (save them), so QED
composites on a clean screen with only its own windows; on the app's `Pterm`,
restore DESK_PAINTER + the desktop windows and `redraw_all`. This is the
shel_write↔Pexec handoff (the EmuTOS relinquish→run→resume sequence #109 is
pinning). Still to verify at runtime: the **thin-strip** window geometry and the
**dropdown** stray column — those are NOT explained by F5.

---
## F6 — CRITICAL, root cause of symptom #3 (thin-strip window) — CONFIRMED (instrumented + code-verified, 2026-07-20)

The stopped rendering auditor completed anyway and drove this **empirically**
(real QED + AES instrumentation), and the lead code-verified it. Not a dismissal
— a confirmed bug. The "thin wide strip" is **QED's AKTION progress dialog**
(`aktion.c start_aktion`, opened at QED init — NOT a document window; QED opens 0
doc windows without a config).

**Instrumented (real QED):** `form_center abi_ilp32=1 wd=-1 hd=7192` → `wind_calc
w=1239 h=111` → `WIND_OPEN w=1241 h=131` (≈2× screen width = the strip).

**Root cause (code-verified):** `tos-api/src/aes/form.rs:713` casts the
app-provided tree to `*mut Object` — the **native 32-byte LP64** struct
(`object.rs:28` `ob_spec@16, ob_x@24, ob_width@28`) — and reads/writes
`root.ob_width/ob_x/ob_y/ob_spec` **directly** (`:725-736`), NOT through the
ABI-aware `ObjTree` accessor (24-byte for ILP32: `ob_width@20, ob_x@16`). For
QED (ILP32) it reads the wrong bytes (garbage w/h) AND writes `ob_x/ob_y` into
the wrong neighbouring object → **corrupts the tree** → the bloated AKTION
geometry. Proof our AES is otherwise fine: a cflib probe isolating start_aktion
(rsrc_load→248×80, fix_dial, set_string, form_center, wind_calc) yields the
correct 250×100 — the fault is the ILP32 tree access in `form_center`.

**Same bug class — Part-1's ObjTree conversion was INCOMPLETE.** Other sites
still cast the app tree to native `Object` instead of `ObjTree` (affects
ob_spec/ob_x/y/w/h; link WORDs @0-10 are ABI-identical so it "mostly works"):
auditor-listed `form.rs:514,876,918`, `graf.rs:281,329`, `menu.rs:713,1156`,
`object.rs:646` — the strict ABI-width enumeration (#111) will produce the
authoritative complete list. This is the [[lp64-tos-struct-trap]] class.

**Fix direction:** route these through `ObjTree`/`current_abi()`, not the raw
`*mut Object` cast.

## F1(handoff) note — exact EmuTOS relinquish/resume sequence (from the same auditor, confirms F5)
Child launch: `shel_write(SHW_EXEC)` records only → desktop event loop ends →
desktop epilog `menu_bar(NULL,0)` + `wind_set(DESKWH,WF_NEWDESK,NULL)` +
`appl_exit()` → desktop process Pterms → `sh_main`: `wm_init()` (tear all windows)
+ `sh_draw(CLEAR_SCREEN=white)` + `sh_ldapp`→`dos_exec(QED)`
(`gemshlib.c:157-196,592-731`, `deskmain.c:2085-2171`). Resume on QED Pterm:
`sh_main` loops → `wm_init` + white clear → reload desktop fresh. Ours
(`launch.rs:43-106`, `desk/main.rs:465-485`) does NONE of the teardown/white-clear
and `full_redraw` only runs after the child Pterms (QED never does). Confirms F5.

## Symptom 2 (dropdown stray column) — HYPOTHESIS, HW-confirm needed
Auditor excluded (with proof) box-mapping, sibling-leak, ILP32-coordinate causes;
`open_box`/save_under mirrors EmuTOS `menu_down` (`gemmnlib.c:229-245`).
Hypothesis: our menu uses a **hover** model (deviation from EmuTOS's click model)
with ONE save-under buffer; moving the cursor across adjacent overlapping title
boxes can leave the non-overlapping right (shortcut) column of the previously
open box on screen. Needs real-Pi4 mouse: hover Bearbeiten → move to Datei →
check if the column only appears after such a transition. Fix dir: restore the
full old box rect (or the union of overlapping boxes) before drawing the new one.

---
## F7 — CONSOLIDATED ILP32 ABI-width register (strict enum #111 + lead verification, 2026-07-20)

The strict, no-judgment enumeration (#111) gave the complete raw width table: **19
MATCH=no of 32 sites**. It found NEW width bugs the earlier audits missed, but it
ALSO wrongly cleared `form_center` (claimed "ObjTree → yes"; it is a native-Object
cast — F6). Lead re-grepped all native `.ob_*` field accesses to correct that. The
verified QED-reachable ILP32 width bugs, one systemic class = "kernel assumes LP64
width where the ILP32 app is narrower":

**A. Raster MFDB (F2) — CONFIRMED, QED scroll + cflib color-icons**
- `vdi/raster.rs:198` (vrt_cpyfm src+dst), `:247` (vro_cpyfm src+dst): CONTRL ptr
  read `contrl_u64(7)/(11)` = LP64 8B @ CONTRL[7]/[11]; ILP32 packs 4B @
  CONTRL[7]/[9] (`gem_vdiP.h:204`, N_PTRINTS=2). `:391/392` vr_trnfm (reach unclear).
- `vdi/raster.rs:60-77` MFDB struct: `fd_addr` 8B; ILP32 `fd_addr` 4B, later fields shift.

**B. OBJECT sub-structs (F3 = task #106) — CONFIRMED, QED dialogs (currently blank-guarded)**
- `aes/object.rs:838` (edit_field TedInfo), `:1541` (TedInfo), `:1600` (BitBlk),
  `:1624` (IconBlk): struct ptr fields 8B; ILP32 4B. `is_ilp32()` guards skip → blank.

**C. Native `Object` field access on app ILP32 tree (F6) — CONFIRMED, corrects #111**
- `aes/form.rs:713+726-736` **form_center** — thin-strip (instrumented wd=-1/hd=7192);
  also leaves the dialog un-centred.
- `aes/graf.rs:328-376` **graf_slidebox** (op 76) — reads/writes native `.ob_x/y/w/h`
  on the app tree (scrollbar-elevator drag).
- (form.rs:514/876/918, menu.rs, object.rs:646, graf.rs:281 go through ObjTree/
  object:: helpers → OK. fsel.rs/resource.rs native access is on kernel LP64 trees → OK.)

**D. NEW confirmed width bugs (#111) — QED-reachable**
- `aes/appl.rs:75-107` **AESGLOBAL write** — writes the native 56-byte AESGLOBAL
  (fields at LP64 offsets) into the app's `short aes_global[16]` = **32 bytes** →
  **OOB write of bytes 32..55** + fields at wrong offsets (ap_private@8 vs @6,
  rscfile, …). Happens on **every appl_init** (QED, first thing). Highest-suspicion
  latent corruption.
- `aes/window.rs:996/1003` **wind_set WF_NAME/WF_INFO** — reads `addrin(0)`, but
  ILP32 gemlib packs the title/info ptr in `intin[2..3]` (`a_wind_set_str.c:25`),
  `num_addrin=0` → reads the wrong place → garbage window title. + `window.rs:955`
  wind_get WF_NAME/WF_INFO MISSING. QED-reachable (cflib mdopen).
- `aes/menu.rs:758/772` **menu_popup MENU struct** — `mn_tree` 8B@0 etc.; ILP32 4B →
  wrong offsets, read AND write. cflib popups.
- `aes/shell.rs:346/354` **shel_envrn** — writes 8B into a 4B `char**` slot. cflib fontsel.

**E. STUB/MISSING (mostly not-QED / benign):** rsrc_saddr (not linked), appl_trecord/
tplay (unclear), appl_yield (single-app ok), rsrc_obfix (identity — verify vs QED),
AES default -1 (F1, safe via 0x0340 gate).

**Systemic conclusion:** the visible thin-strip (form_center) + the latent bugs
(garbage window titles, AESGLOBAL corruption, scroll blit, dialog blanks) are ONE
root class — **incomplete ILP32 ABI-width conversion**, ~15 QED-reachable sites.
Fix = one campaign: route every app-tree/CONTRL/sub-struct/TOS-struct read+write
through the caller-ABI width (ObjTree / ABI-aware readers). The **handoff** (F5) is a
separate architectural fix; **dropdown** (symptom 2) is a separate hover-model issue
(HW-confirm).

---
## F8 — AKTION ob_width corruptor: form.rs form-manager helpers use native 32-byte stride (OOB overrun) — CONFIRMED defect, PLAUSIBLE as the thin-strip cause (2026-07-20)

Instrumented trace (software watchpoint on the AKTION root ob_width word):
- **CONFIRMED:** AKTION loads correct (`ob_width=248`, tree 26, root obj 518, VA
  0x40313c) and stays 248 through the entire **headless** QED startup — no AES
  call flips it. RSC/reloc/mkrsc are clean.
- **Why the flip wasn't caught headless:** `start_aktion` (`aktion.c:46`) is only
  reached via an interactive **File>Open of a ≥100KB file** (`verbose && size>>10
  >= transfer_size=100`). The desk hook (`run_program`, no file) only runs QED to
  idle; `crt0.S:49` sets no argc/argv so no command-line file reaches QED's main;
  File>Open needs mouse (real-HW only). So the `form_center reads 1239`
  observation is from an interactive/HW run — **not reproducible headless**.
- Eliminated (static, from the parsed NRSC): `fix_dial` (cflib `userdef.c:493`,
  no-op for AKTION — no MagiC objects), `set_string` (writes te_ptext ~10KB away),
  `aktion[ABOX2].ob_width=1` / `set_flag` (hit objects 524/525, can't reach root+20).

**Strongest suspect (PLAUSIBLE, code-confirmed defect):** the form-manager
helpers in `tos-api/src/aes/form.rs` (`form_do`/`form_keybd`/`form_button`,
op 50/55/56) still walk the app tree with a raw `*mut Object` + `(*tree.add(i)).ob_*`
at **native 32-byte stride** instead of `ObjTree` — lines ~622/635/649/662 (tree_len,
next_editable, prev_editable, find_flag), ~802/816/818/819/824/839 (press, ob_fs,
select_radio). #2 converted only `form_center`, not these. Under ILP32 (24-byte
records) object `i` is read at `base + i*32` not `base + i*24`; `tree_len` scans
to LASTOB at 32-byte stride and **runs off the tree end into the next contiguous
tree** in the RSC blob (tree 25 @0x402fa4, AKTION @0x40313c). This is the OOB
pattern; it can corrupt AKTION (and any other tree) → the strongest candidate for
`ob_width=1239`. Not runtime-confirmed as THE corruptor (interactive path blocked),
but a confirmed defect regardless.

**Fix:** convert the form.rs form_do/keybd/button helper chain to `ObjTree`
(same F6 class as #2). **Verification caveat:** the thin-strip's full resolution
is only confirmable on **real Pi4** (File>Open path), or via a headless input
harness / crt0-argv shim. Also flagged: `crt0.S` no argc/argv (latent, separate).

---
## Note (2026-07-21, from #6) — menu_settings(39)/MnSet is ILP32-hardcoded (LP64-latent)
`menu_settings` (op 39) decodes `MN_SET` `{ long display, drag, delay, speed; short height; }`
(mt_gem.h:1257) with Rust `i32` for the four `long` fields — correct for ILP32
(long=4B) but WRONG for an LP64 caller (long=8B → height at the wrong offset,
longs truncated). No LP64 caller exists today (AES-4.1 feature), so it is latent —
not a QED bug. Fix direction if ever needed: decode MN_SET at the caller ABI
width. Separate from the MENU-struct fix (#6). Left untouched.

---
## Note (2026-07-21, from #7) — shel_envrn value points at a kernel static (ILP32-untranslatable)
Fixed the OOB slot write (#7: write_ptr at caller width). Separate, orthogonal:
when shel_envrn *finds* an env var, the returned value pointer is into the kernel
static `S.env` (high kernel VA); under ILP32 write_ptr truncates it to 32 bits →
not app-dereferenceable. QED has an empty env (p_env=0) → NULL path, unaffected.
If a future app has a non-empty env, the env must live in app-mapped memory
(see [[el0-app-memory-must-be-mapped]]). Latent, separate from the width fix.
