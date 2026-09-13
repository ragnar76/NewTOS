# AES MENU-group ABI-marshalling trace

**Question under test.** TeraDesk's interactive menu dropdown opens
**transparent** (no opaque background) on real HW, and drive-open fails, while
our in-kernel desktop draws the same menus correctly. Leading hypothesis: a
**toolchain / ABI-marshalling** bug — gemlib (TeraDesk side) and our kernel
disagree on how the AES parameter block and the menu OBJECT tree are laid out
in memory, so a field (specifically the dropdown box's `ob_spec` fill/colour)
lands at a different offset and is misread → wrong (hollow → transparent) fill.

**Method.** For every AES call on the menu path, trace the bytes across three
stages — TeraDesk-side call → toolchain marshalling → kernel reception — and
compute `offsetof`/`sizeof` for every struct field in **both** ABIs (LP64 and
ILP32), side by side, flagging any divergence. A companion Python simulator
(`menu_abi_sim.py`) lays a concrete menu tree in memory the producer way and
reads it back the kernel way, byte for byte.

**One fact that frames everything.** TeraDesk is built **ELF64 / LP64** and
linked against the **LP64** `gemlib.a`
(`sdk/apps/build-teradesk.sh:39,103,176`). At launch the loader reads the ELF
class and sets the process ABI accordingly
(`tos-api/src/bdos/process/loader.rs:556-558`, `img.is_ilp32()==false` →
`set_abi(Abi::Lp64)`), so `current_abi()`
(`tos-api/src/bdos/process.rs:668-671`) returns **`Lp64`** during TeraDesk's
AES trap. Every kernel accessor keyed on `current_abi()` therefore uses the
LP64 layout — the *same* layout the LP64 gemlib and the LP64 `mkrsc` converter
emit.

---

## 0. Authoritative struct sizes/offsets (both ABIs)

LP64 numbers were obtained by compiling the exact gemlib structs on the host
(`cc`, LP64) and printing `offsetof`/`sizeof`; they are cross-confirmed by
`tools/mkrsc.py` (the NRSC producer) and the kernel readers. ILP32 numbers
follow the aarch64 `-mabi=ilp32` rules (pointer/`long` = 4 bytes, 4-byte align)
and are cross-confirmed by `mkrsc.py`'s ILP32 branch
(`NAT_OBJ=24, NAT_TED=28, NAT_IB=36, NAT_BB=16`, `tools/mkrsc.py:160-166`) and
`ObjTree`'s ILP32 arm.

### OBJECT (`mt_gem.h:2081-2094` / `object.rs:29-55`)

| field | type | LP64 off | ILP32 off | note |
|-------|------|---------:|----------:|------|
| ob_next | short | 0 | 0 | |
| ob_head | short | 2 | 2 | |
| ob_tail | short | 4 | 4 | |
| ob_type | u16 | 6 | 6 | |
| ob_flags | u16 | 8 | 8 | |
| ob_state | u16 | 10 | 10 | |
| **ob_spec** | union/ptr | **16** | **12** | **8-byte union forces 8-byte align → +4 pad at 12..16 (LP64)** |
| ob_x | short | 24 | 16 | shifted by the ob_spec width delta |
| ob_y | short | 26 | 18 | |
| ob_width | short | 28 | 20 | |
| ob_height | short | 30 | 22 | |
| **sizeof** | | **32** | **24** | |

The single alignment/padding effect the whole hypothesis rests on: the
`OBSPEC` union contains a `long`/pointer, so under LP64 it is 8-byte aligned;
after the six 2-byte link/type WORDs end at offset 12, the compiler inserts
**4 bytes of padding** so `ob_spec` starts at **16**. That pushes the four
coordinate WORDs from 16..22 (ILP32) to **24..30** (LP64) and the record from
24 to **32** bytes.

### AESPB (`mt_gem.h:2527-2535` / `mod.rs:35-49`)

| field | type | LP64 off | ILP32 off |
|-------|------|---------:|----------:|
| control | short* | 0 | 0 |
| global | short* | 8 | 4 |
| intin | short* | 16 | 8 |
| intout | short* | 24 | 12 |
| addrin | long* | 32 | 16 |
| addrout | long* | 40 | 20 |
| **sizeof** | | **48** | **24** |

### TEDINFO / ICONBLK / BITBLK (referenced by menu items only via `ob_spec`)

| struct | LP64 sizeof | ILP32 sizeof | leading pointers (LP64 → ILP32) |
|--------|-----------:|------------:|--------------------------------|
| TEDINFO (`mt_gem.h:1988-2001`) | 40 | 28 | te_ptext@0, te_ptmplt@8→4, te_pvalid@16→8 |
| ICONBLK (`mt_gem.h:2004-2020`) | 48 | 36 | ib_pmask@0, ib_pdata@8→4, ib_ptext@16→8 |
| BITBLK (`mt_gem.h:2023-2031`) | 24 | 16 | bi_pdata@0; bi_wb@8→4 |

A menu **item** (`G_STRING`/`G_TITLE`) carries its text as a **`char*` in
`ob_spec` itself** — it does **not** go through TEDINFO (that is
`G_TEXT`/`G_BOXTEXT`). A dropdown **box** (`G_BOX`) carries a **packed colour
word** in `ob_spec`, not a pointer. So on the menu path only OBJECT and AESPB
are actually marshalled; TEDINFO/ICONBLK/BITBLK are listed for completeness and
are the same-shape 8→4 leading-pointer story if they ever appear.

### MENU (`mt_gem.h:2177-2191` / `menu.rs:830-903`) — only `menu_popup`/`menu_attach`

| field | type | LP64 off | ILP32 off |
|-------|------|---------:|----------:|
| mn_tree | OBJECT* | 0 | 0 |
| mn_menu | short | 8 | 4 |
| mn_item | short | 10 | 6 |
| mn_scroll | short | 12 | 8 |
| mn_keystate | short | 14 | 10 |
| **sizeof** | | **16** | **12** |

The kernel already reads MENU at the caller's ABI width
(`menu.rs:848-903`, `Menu::read_abi`/`write_abi`, WORD offset = `menu_ptr_size`
= 8 LP64 / 4 ILP32) — matching the table.

---

## 1. The 3-column trace table (one row per menu AES call)

Legend: **IN** = `int_in[]`, **AI** = `addr_in[]`, **OUT** = `int_out[]`.

| # | TeraDesk-side call (gemlib fn + AES contract) | Toolchain marshalling (`sdk/gemlib`) | Kernel reception (`tos-api/src/aes`) | Verdict |
|---|-----------------------------------------------|--------------------------------------|--------------------------------------|---------|
| 1 | **`menu_bar(tree, mode)`** — opcode **30**. `mt_menu_bar(me_tree, me_mode)`; IN[0]=mode, AI[0]=tree; OUT[0]=status. (`a_menu_bar.c:48-60`) | `AES_PARAMS(30,1,1,1,0)`: `control[5]` WORDs `{30,1,1,1,0}`; `aes_intin[0]=me_mode` (WORD); `aes_addrin[0]=(long)me_tree` — **8-byte** LP64 `long`. `AESPB` = 6 native (8-byte) pointers. Trap: `aes()` → `svc #2`, x0=200, x1=&AESPB. (`gem_aesP.h:70-83`, `_gc_aes.c:30-42`) | `Aespb::from_raw` reads the 6 block pointers at 8-byte stride (`mod.rs:69-83`); `menu_bar` reads `pb.addrin(0)` (8-byte, `mod.rs:106-112`) as `*mut Object` and `pb.intin(0)` (WORD) as `show` (`menu.rs:137-172`). | **MATCH** — control/intin WORDs ABI-neutral; addrin[0] 8-byte on both sides. Sim §4. |
| 2 | **Interactive open path.** TeraDesk calls `evnt_multi(...)` (opcode 25) in its main loop; a click over the bar is intercepted **inside the AES** (it does not marshal a separate "open dropdown" call). (`a_evnt_multi.c`) | Same `AESPB`/WORD/pointer packing as row 1 (opcode 25); the click is returned in the `evnt_multi` OUT WORDs; no menu-tree pointer is re-marshalled here. | `event::evnt_multi` → `menu_intercept` (`event.rs:69-97`): if a menu is installed and the cursor is on the bar, it runs `menu::track(tree, backend)` **kernel-internally** — the *same* function the desktop uses. `tree` = the installed `MNTREE` (`menu.rs:49,165`). | **MATCH** — the dropdown display is not a marshalled call; it re-reads the *already-installed* LP64 tree. No new ABI surface. |
| 3 | **Dropdown display / tracking** (AES-internal `menu_sub`/`open_box`). Reads the menu tree's box objects: `THESCREEN.ob_tail`→`THEMENUS`, its `ob_head` box chain, each box's `ob_x/y/w/h` and **`ob_spec`** (the box fill/colour). | The tree bytes are produced by **`mkrsc.py convert`** (NRSC/LP64) from `desktop.rsc` (`build-teradesk.sh:202`), LP64 `OBJ_FMT="<hhhHHH4xQhhhh"` → ob_spec@16, coords@24, stride 32 (`tools/mkrsc.py:168-169`). A `G_BOX` ob_spec is passed through verbatim: `nspec = spec & 0xFFFFFFFF` (`tools/mkrsc.py:266-267`). | `ObjTree::new(tree)` at `current_abi()==Lp64` → stride 32, `spec_off`=16, `coord_off`=24 (`object.rs:176-195`); `open_box`→`menu_sub`→`draw_obj_clip` reads box `ob_spec` via `spec()` at @16 (`object.rs:310-351`, `menu.rs:598-611`); interior filled by `gr_crack(ob_spec)`→`gr_rect` (`object.rs:2218,1964-1990`). | **MATCH** — producer @16 == consumer @16; box colour word preserved verbatim. Sim §2,§3. |
| 4 | **`menu_tnormal(tree, title, normal)`** — opcode **33**. IN[0]=title, IN[1]=normal, AI[0]=tree; OUT[0]=status. (`a_menu_tnormal.c:22-35`) | `AES_PARAMS(33,2,1,1,0)`: `intin[0]=title`, `intin[1]=normal` (WORDs); `addrin[0]=(long)tree` (8-byte). | `menu_tnormal` (`menu.rs:690-708`) reads `pb.intin(0)`/`pb.intin(1)` (WORDs) and `pb.addrin(0)` (8-byte); `do_chg` flips `ob_state` @10 via `ObjTree` (`menu.rs:804-818`). | **MATCH** — WORDs + 8-byte tree pointer both sides. Sim §5. |
| 5 | **`menu_icheck(tree, item, check)`** — opcode **31**. IN[0]=item, IN[1]=check, AI[0]=tree. (`a_menu_icheck.c`) | `AES_PARAMS(31,2,1,1,0)`; two IN WORDs + 8-byte `addrin[0]`. | `menu_icheck`→`do_chg(pb.intin(0),CHECKED,pb.intin(1)!=0,…)` (`menu.rs:669-671`); tree via `pb.addrin(0)`; `ob_state` @10. | **MATCH** — same shape as row 4. |
| 6 | **`menu_ienable(tree, item, enable)`** — opcode **32**. IN[0]=item (bit15=redraw-now), IN[1]=enable, AI[0]=tree. (`a_menu_ienable.c`) | `AES_PARAMS(32,2,1,1,0)`; two IN WORDs + 8-byte `addrin[0]`. | `menu_ienable` (`menu.rs:678-683`): `item=intin(0)&0x7fff`, bit15→dodraw; `do_chg(DISABLED,…)`; tree via `addrin(0)`. | **MATCH** — WORD bit-packing is ABI-neutral; pointer 8-byte both sides. |
| 7 | **`menu_text(tree, item, text)`** — opcode **34**. IN[0]=item, AI[0]=tree, **AI[1]=text**. (`a_menu_text.c`) | `AES_PARAMS(34,1,1,2,0)`; `addrin[0]=tree`, `addrin[1]=text` — **two** 8-byte `long`s. | `menu_text` (`menu.rs:716-745`): `tree=addrin(0)`, `src=addrin(1)`; writes into the item's `ob_spec` text buffer read via `ObjTree::spec(item)` at @16. | **MATCH** — two 8-byte addrin elements read at 8-byte stride; ob_spec text ptr @16 both sides. |

No menu row diverges. The menu path marshals only OBJECT + AESPB, and both are
byte-identical between the LP64 producer and the LP64 kernel consumer.

---

## 2. Why the *selectivity* is explained without an ABI bug

The report notes menu_bar installs the bar and icons render (static) but the
dropdown is transparent — cited as evidence of a *per-function* mismatch.
The trace shows the opposite:

- **The static bar and the dropdown box are drawn by the *same* code reading
  the *same* tree at the *same* ABI.** `menu_bar` draws `THEBAR` via
  `object::draw_obj` (`menu.rs:162`); the dropdown draws its `G_BOX` via
  `object::draw_obj_clip` (`menu.rs:608`) — both through `ObjTree` at
  `current_abi()==Lp64`, both reading `ob_spec` at offset 16. If the ABI/offset
  were wrong, the **static bar would be corrupt too** (its `THEBAR` box colour
  word sits at the same @16). It renders correctly → the offset is correct →
  the transparency is not an offset misread.
- **The in-kernel desktop and TeraDesk share the identical dropdown code**
  (`event.rs:97` → `menu::track` for both; the desktop just calls `evnt_multi`
  kernel-side, TeraDesk via the trap). Once inside `track`/`open_box`, the only
  variables are the *tree data* and *runtime state*, not the ABI.

---

## 3. Byte-level simulation result

`menu_abi_sim.py` lays the menu tree in memory the producer way (gemlib/mkrsc
LP64) and reads every field the kernel way (`ObjTree` at Lp64), plus packs and
reads the `menu_bar`/`menu_tnormal` AESPBs. Output:

- OBJECT layout producer vs kernel: **MATCH** for LP64 (stride 32, spec@16,
  coords@24) *and* for ILP32 (stride 24, spec@12, coords@16).
- All 6 objects of the sample tree: every field byte-identical.
- `menu_bar(tree,1)` and `menu_tnormal(tree,2,1)` AESPB round-trip:
  opcode/WORDs/8-byte tree pointer **MATCH**.
- **The decisive dropdown test:** the box's `ob_spec` fill word read the
  **correct** LP64 way (@16) decodes to `pattern=7` (IP_SOLID) → **opaque**.
  The **counterfactual** ILP32 misread of the same LP64 tree (@12, stride 24)
  returns `ob_spec=0` → `pattern=0` (IP_HOLLOW) → **transparent**.

That counterfactual is important: it confirms an ABI mismatch *would* produce
exactly the reported "transparent dropdown" symptom — but the kernel does **not**
take that path, because TeraDesk is LP64 and the kernel reads at LP64.

---

## 4. VERDICT

**MENU-group ABI marshalling MATCHES end-to-end for LP64 TeraDesk. The
transparent dropdown is NOT a toolchain / ABI-marshalling bug.**

Every producer/consumer pair agrees at the byte level:

1. **AESPB** — gemlib emits six native 8-byte pointers (`mt_gem.h:2527-2535`,
   `_gc_aes.c`); the kernel reads six pointers at 8-byte stride
   (`mod.rs:69-83`). `control`/`int_in`/`int_out` are 16-bit WORD arrays on
   both sides (ABI-neutral); `addr_in`/`addr_out` are 8-byte on both sides.
2. **OBJECT** — the union `ob_spec` forces the *same* 8-byte alignment/padding
   on both sides: producer (gemlib compile **and** `mkrsc` NRSC LP64,
   `tools/mkrsc.py:168-169`) and consumer (`ObjTree` at `current_abi()==Lp64`,
   `object.rs:176-195`) both put `ob_spec` at **16**, coords at **24**, stride
   **32**. No field is shifted.
3. **The dropdown box's `ob_spec` fill/colour word** — laid at 16 by the
   producer, read at 16 by the kernel, and preserved verbatim through `mkrsc`
   (`:266-267`). The box fill is decoded from the correct bytes.

**Where to look next (non-ABI).** Since the bytes align, the transparent
interior must come from one of:

- **The actual `ob_spec` fill value in TeraDesk's `desktop.rsc`** dropdown
  boxes. If those boxes encode an `IP_HOLLOW` interior (`pattern==0`), the
  kernel *correctly* paints nothing (`object.rs:1979`) and the box is
  see-through by design — relying on a clean white background beneath. This is
  a **resource-data** property, not ABI. (Compare the kernel desktop's
  own menu tree, `kernel/src/desk/menu.rs`, whose box fill values differ.)
- **`save_under`/`restore_under` (GEM `bb_save`) on real HW** — the dropdown
  backing is snapshotted with `backend.read_pixel` and blitted back with
  `fill_rect` (`menu.rs:550-593`). On real HW a write-combining/uncached or
  double-buffered framebuffer can make `read_pixel` return stale/blank pixels,
  so the box "background" (the saved-then-restored region, or a hollow interior
  over it) reads as transparent. This is a **VDI / framebuffer-coherency**
  issue, not ABI.
- **The write-mode of the interior fill** (`gr_rect` uses `WriteMode::Replace`,
  `object.rs:1968-1990`) vs. any transparent path — a VDI concern, not ABI.

A strong corroborating signal: an ABI-marshalling bug is **deterministic and
platform-independent** — it would render the dropdown transparent **identically
on QEMU and real HW**. The symptom being **real-HW-specific** points away from
ABI and toward the VDI/framebuffer real-HW path above.

---

## Files

- Trace table + verdict: `docs/analysis/aes-abi-trace/menu-functions.md` (this file)
- Byte-level simulator: `docs/analysis/aes-abi-trace/menu_abi_sim.py`
  (run: `python3 docs/analysis/aes-abi-trace/menu_abi_sim.py`)

## Primary sources cited

Toolchain (producer): `sdk/gemlib/gemlib-0.44.0/gem_aesP.h:70-83`,
`a_menu_bar.c:48-60`, `a_menu_tnormal.c:22-35`, `mt_gem.h:2081-2094`
(OBJECT), `:2527-2535` (AESPB), `:1988-2031` (TEDINFO/ICONBLK/BITBLK),
`:2177-2191` (MENU), `_gc_aes.c:30-42`; `tools/mkrsc.py:160-169,266-267`;
`sdk/apps/build-teradesk.sh:39,103,176,202`.
Kernel (consumer): `tos-api/src/aes/mod.rs:35-112,173-275`;
`tos-api/src/aes/object.rs:29-55,176-195,310-351,1944-1990,2208-2279`;
`tos-api/src/aes/menu.rs:137-172,198-320,598-611,690-745,830-903`;
`tos-api/src/aes/event.rs:69-97`;
`tos-api/src/bdos/process.rs:128-183,668-671`;
`tos-api/src/bdos/process/loader.rs:556-558`.
