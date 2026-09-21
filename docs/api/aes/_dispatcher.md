# AES Trap Dispatcher (GEM trap, selector 200)

> **PHASE-00b deliverable D8 (AES), Batch 0 (full-contract refinement).**
> Source: EmuTOS @ `1a7484ab` — `aes/gemsuper.c` (entry + marshalling +
> `crysbind` opcode switch), `aes/gemsuper.h` (`AESPB`), `include/aesdefs.h`
> (opcodes + slot/size macros), `aes/gemdosif.S` (the m68k trap glue).
>
> The AES shares the **GEM trap** (m68k `trap #2`) with the VDI. The two are
> distinguished by the **selector**: `200` → AES (`super`/`gemsuper.c`), `115`
> → VDI (`gsx2.c`). The selector is in `d0` on m68k (`x0` in the NewTOS ABI); a
> pointer to the parameter block is in `d1` (`x1`). This file documents the AES
> side; for the VDI parameter-block mechanics see
> [`../vdi/_dispatcher.md`](../vdi/_dispatcher.md). Shared AES globals:
> [`_state.md`](./_state.md).
>
> **Contract vs. reference** — see [`docs/COMPATIBILITY.md`](../../COMPATIBILITY.md).
> **Contract:** the `svc #2` + selector-200 + AESPB-pointer entry; the six-array
> AESPB shape; `control[0..3]` = opcode / #int_in / #int_out / #addr_in;
> `int_out[0]` = the return code; the opcode→function numbers (10–127); the
> `default`/opcode-0 handling. **Reference:** `xif`'s stack-copy sizes, the
> `crysbind` switch structure, and every `*_lib` internal — NewTOS reimplements
> the marshalling and the handlers freely.

---

## 1. The AES parameter block (`AESPB`)

`aes/gemsuper.h:13-21`. Unlike GEMDOS/BIOS/XBIOS (register ABI), the AES takes
a **parameter block of six array pointers**, exactly like the VDI:

| Field | Type | Contents |
|-------|------|----------|
| `control` | `WORD *` | opcode + array lengths |
| `global`  | `WORD *` | `AESGLOBAL` — per-application state (`appl_init` fills it) |
| `intin`   | `WORD *` | integer inputs |
| `intout`  | `WORD *` | integer outputs |
| `addrin`  | `LONG *` | address inputs (pointers) |
| `addrout` | `LONG *` | address outputs (pointers) |

### `control[]` layout (`include/aesdefs.h:131-134`)

| Index | Macro | Meaning |
|------:|-------|---------|
| 0 | `OP_CODE`  | AES function opcode (10..127) |
| 1 | `IN_LEN`   | number of `intin` words |
| 2 | `OUT_LEN`  | number of `intout` words |
| 3 | `AIN_LEN`  | number of `addrin` longs |

> There is **no `control[4]`** in this EmuTOS build — the address-output count
> is not read from `control`. The dispatcher copies exactly one `addrout` slot,
> and only for `rsrc_gaddr` (see §2). `AO_SIZE = 1` (`aesdefs.h:125`) is the
> array's declared size, not a `control`-driven count.

### Max array sizes (`include/aesdefs.h:121-125`)

| Array | Macro | Size |
|-------|-------|-----:|
| control | `C_SIZE`  | 4 |
| intin   | `I_SIZE`  | 16 |
| intout  | `O_SIZE`  | 7 |
| addrin  | `AI_SIZE` | 3 |
| addrout | `AO_SIZE` | 1 |

The **return value** is `RET_CODE = int_out[0]` (`aesdefs.h:140`).

### `global[]` / `AESGLOBAL`

`control`/`intin`/`intout`/`addr*` are per-call; `global` points at the calling
application's **`AESGLOBAL`** (`aes/gemrslib.h:15-27`): `ap_version`,
`ap_count`, `ap_id`, `ap_private`, `ap_ptree`, plane count, and reserved
fields. `appl_init` fills the public fields; full byte layout + the values
written are in the [`application.md`](./application.md) spec.

---

## 2. Entry, marshalling + dispatch

```
svc #2, x0=200 (d0), x1=AESPB* (d1)   (gemdosif.S trap glue)
        │
   super(cx=200, AESPB*)      aes/gemsuper.c:454   — AES supervisor entry
        │
   xif(AESPB*)                aes/gemsuper.c:428    — copy arrays in/out
        │
   crysbind(OP_CODE, global,  aes/gemsuper.c:77     — the opcode switch
            control, int_in, int_out, addr_in)
```

**`super` (`gemsuper.c:454-466`)** switches on the selector `cx`:
- **`case 200`** → `xif(pcrys_blk)`, then **falls through** to `dsptch()`.
- **`case 201`** → `dsptch()` only. This is an *undocumented TOS feature*
  (`gemsuper.c:461`): selector 201 performs a bare AES reschedule with no
  parameter-block call. `super` always returns `0` (the AES result is delivered
  in `int_out[0]`, not in the trap's return register).

**`xif` (`gemsuper.c:428-448`)** marshals the call:
1. Copy `control[C_SIZE]` (4 words) from the caller's block onto a local stack
   buffer (`:435`).
2. If `IN_LEN > 0`, copy `min(IN_LEN, I_SIZE)` `intin` words (`:436-437`).
3. If `AIN_LEN > 0`, copy `min(AIN_LEN, AI_SIZE)` `addrin` longs (`:438-439`).
4. Call `crysbind(OP_CODE, global, …)`; its `UWORD` result becomes
   `RET_CODE` = the caller's `int_out[0]` (`:441`).
5. If `OUT_LEN > 0`, copy `OUT_LEN` `intout` words back to the caller (`:444-445`).
   The source is the **local, uninitialised** `int_out[O_SIZE]` (`:432`) and
   the copy is unconditional — a handler that wrote fewer than `OUT_LEN` words
   hands the caller the dispatcher's stack in the rest, and the caller never
   keeps its own value inside `OUT_LEN`. Which handlers under-write, and on
   which path, is inventoried in
   `docs/analysis/aes-underwriting-handlers-2026-09-19.md`; NewTOS zeroes the
   local array (sanctioned deviation, [`docs/COMPATIBILITY.md`](../../COMPATIBILITY.md),
   A27).
6. **Only** for `OP_CODE == RSRC_GADDR` (112), copy the one `addrout[0]`
   result back (`:446-447`).

> **`addrout` marshalling is special-cased.** `rsrc_gaddr` is the *only* opcode
> whose `addrout[0]` the dispatcher copies back; every other function that
> "returns a pointer" does so through an `int_out`/output pointer the handler
> writes directly, or through the `global[]` array (`appl_init`). This is a
> contract detail for any AES that reproduces the exact array-copy behaviour.

**`crysbind` (`gemsuper.c:77-420`)** is a single `switch(opcode)` that unpacks
the `intin`/`addrin` arguments (via the `aesdefs.h` slot macros) for each
function and calls the matching library routine (`ap_*`, `ev_*`, `mn_*`,
`ob_*`, `fm_*`, `gr_*`, `sc_*`, `fs_*`, `wm_*`, `rs_*`, `sh_*`). Its `UWORD`
return becomes `RET_CODE`. It initialises `ret = TRUE` (`:85`), so a handler
that returns `void` leaves `int_out[0] = 1`.

### Unsupported-opcode path (correction)

Contrary to the previous version of this file, there **is** an
invalid-opcode path (`gemsuper.c:406-417`):

- Any opcode not matched by a `case` hits `default:` and sets
  `unsupported = TRUE` (`:406-408`).
- After the switch, if `unsupported`: `KDEBUG(...)`, and — **unless the opcode
  is 0** — pop up an alert `fm_show(ALNOFUNC, &opcode, 1)`; then set
  `ret = -1` (`:411-417`).
- **Opcode 0 is silently ignored** (returns `-1` without the alert) because
  "some PRGs make this call" (`:414`).

So an out-of-range or not-compiled-in opcode (e.g. `appl_yield`/17 when
`CONF_WITH_PCGEM == 0`, or the absent `appl_getinfo`/130) returns **`-1`**, not
an undefined value. (Contract: `-1` for unknown opcodes; the alert box is a
reference-level behaviour NewTOS need not reproduce, but the `-1` return is.)

---

## 3. The complete opcode map (`include/aesdefs.h:16-115`)

| Range | Manager | Opcodes | Spec |
|-------|---------|---------|------|
| 10–19 | **Application** (`appl_*`) | INIT 10, READ 11, WRITE 12, FIND 13, TPLAY 14, TRECORD 15, **YIELD 17¹**, EXIT 19 | [`application.md`](./application.md) |
| 20–26 | **Event** (`evnt_*`) | KEYBD 20, BUTTON 21, MOUSE 22, MESAG 23, TIMER 24, MULTI 25, DCLICK 26 | [`event.md`](./event.md) |
| 30–39 | **Menu** (`menu_*`) | BAR 30, ICHECK 31, IENABLE 32, TNORMAL 33, TEXT 34, REGISTER 35, POPUP 36², ATTACH 37², ISTART 38², SETTINGS 39² | [`menu.md`](./menu.md) |
| 40–48 | **Object** (`objc_*`) | ADD 40, DELETE 41, DRAW 42, FIND 43, OFFSET 44, ORDER 45, EDIT 46, CHANGE 47, SYSVAR 48³ | [`object-form.md`](./object-form.md) |
| 50–56 | **Form** (`form_*`) | DO 50, DIAL 51, ALERT 52, ERROR 53, CENTER 54, KEYBD 55, BUTTON 56 | [`object-form.md`](./object-form.md) |
| 70–79 | **Graphics** (`graf_*`) | RUBBOX 70, DRAGBOX 71, MBOX 72, GROWBOX 73, SHRINKBOX 74, WATCHBOX 75, SLIDEBOX 76, HANDLE 77, MOUSE 78, MKSTATE 79 | [`window-graphics.md`](./window-graphics.md) |
| 80–82 | **Scrap** (`scrp_*`) | READ 80, WRITE 81, CLEAR 82¹ | [`scrap-shell.md`](./scrap-shell.md) |
| 90–91 | **File selector** (`fsel_*`) | INPUT 90, EXINPUT 91 | [`resource-fileselector.md`](./resource-fileselector.md) |
| 100–109 | **Window** (`wind_*`) | CREATE 100, OPEN 101, CLOSE 102, DELETE 103, GET 104, SET 105, FIND 106, UPDATE 107, CALC 108, NEW 109 | [`window-graphics.md`](./window-graphics.md) |
| 110–114 | **Resource** (`rsrc_*`) | LOAD 110, FREE 111, GADDR 112, SADDR 113, OBFIX 114 | [`resource-fileselector.md`](./resource-fileselector.md) |
| 120–127 | **Shell** (`shel_*`) | READ 120, WRITE 121, GET 122, PUT 123, FIND 124, ENVRN 125, RDEF 126¹, WDEF 127¹ | [`scrap-shell.md`](./scrap-shell.md) |

¹ `#if CONF_WITH_PCGEM` (default **1**, `include/config.h:1319-1321`):
`appl_yield` (17), `scrp_clear` (82), `shel_rdef` (126), `shel_wdef` (127).
² `#if CONF_WITH_MENU_EXTENSION` (default 1): `menu_popup/attach/istart/settings`
(36–39). ³ `objc_sysvar` (48) is `#if CONF_WITH_3D_OBJECTS` (default 1).
When a flag is 0, the opcode falls to `default` → `-1`.

≈ **80 functions** across 11 managers. Opcodes with no `case` (e.g. 16, 18,
27–29, 49, 57–69, 83–89, 92–99, 115–119, ≥128) return **-1**.

> **Not implemented in this build:** `appl_getinfo` (130), `appl_search` (18),
> `appl_control` — no opcode macro, no `case`; they return `-1`.

---

## 3a. Re-validation 2026-07-10 — the `-1` convention confirmed, and its contrast with the other three TOS dispatch conventions

**Re-read against `1a7484ab` line-by-line** (`aes/gemsuper.c:77-466`,
`include/aesdefs.h:16-140`) as the first task of the AES leg of the bottom-up
sweep (GEMDOS, BIOS, XBIOS, VDI already done). This confirms D-047 (the `-1`
unknown-opcode path) without a factual correction, and adds the four-way
cross-trap contrast that D-061/D-079 already recorded for the other three
traps.

`crysbind`'s `default:` arm is the **only** path an unhandled `opcode` can
take (`gemsuper.c:406-417`): no case in the `switch(opcode)` falls through to
another one, and there is no secondary range check before the switch — any
`WORD` opcode not matched by a `case` (including negative values and values
`> 127`) lands on `default`. The arm always does the same three things:
`KDEBUG` trace, then (unless `opcode == 0`) `fm_show(ALNOFUNC, &opcode, 1)`
(an on-screen alert box), then unconditionally `ret = -1`. `crysbind`'s
return value is assigned to `xif`'s **local** `int_out[0]`
(`RET_CODE = crysbind(...)`, `gemsuper.c:441`), which is a stack buffer, not
a slot inside the caller's own AESPB array. That local buffer is copied back
to `pcrys_blk->intout` **only if `OUT_LEN` (`control[2]`) is non-zero**
(`gemsuper.c:444-445`). So a caller that declared `control[2] = 0` (no output
words requested) never sees the `-1` in its own `intout[0]` at all — that
array is left exactly as the caller set it up, and only the trap's own
return register is left to observe, which is unconditionally `0`
(`super()`, `gemsuper.c:465`, carries no information about success/failure).
**A caller must declare `OUT_LEN ≥ 1` to observe the `-1`.**

**This is a *fourth*, distinct convention, next to the three traps already
extracted in this sweep:**

| Trap | Unknown/unimplemented function ID | Source |
|------|-----------------------------------|--------|
| GEMDOS (trap #1) | Returns **`EINVFN` (-32)** — both for an out-of-range `fn` and for an in-range slot wired to `ni()` | [`docs/api/gemdos/_dispatcher.md`](../gemdos/_dispatcher.md) §4 |
| BIOS (trap #13) / XBIOS (trap #14) | Return the **function number itself** in `D0` — not an error code | [`docs/api/xbios/_dispatcher.md`](../xbios/_dispatcher.md) §3 (D-008/D-039), [`docs/api/bios/_dispatcher.md`](../bios/_dispatcher.md) §4 |
| VDI (trap #2, selector `0x73`) | **No signal at all** — a bare `return;`, no error code, no echo | [`docs/api/vdi/_dispatcher.md`](../vdi/_dispatcher.md) §"Re-validation 2026-07-09" (D-079) |
| AES (trap #2, selector `200`) | Returns the fixed literal **`-1`** in `int_out[0]` (only if the caller declared `OUT_LEN ≥ 1`) — plus a **synchronous on-screen alert** (`fm_show(ALNOFUNC,…)`) unless `opcode == 0`, which is silently ignored "since some PRGs make this call" (`gemsuper.c:414`) | `aes/gemsuper.c:77-420` (this file, §2) |

So the AES is the only one of the four TOS traps whose "unknown function"
path has a **user-visible side effect** (the alert box) in addition to its
return value, and the only one whose return value is a fixed sentinel
literal (`-1`) rather than an echoed function number or a named error
constant. Opcode `0` is a documented, deliberate exception to the alert
(not to the `-1` return) — EmuTOS's own comment attributes this to real
programs making a spurious opcode-0 call, so a conforming AES must special
-case it exactly as EmuTOS does (no alert, still `-1`).

Each of the four conventions is internally self-consistent within its own
trap; none should be "fixed" toward another by analogy — the same caution
D-061 and D-079 already recorded for the other three.

**Prior spec check:** D-047 already corrected the "no invalid-opcode path"
claim from an even earlier spec revision (2026-07-03) to the `-1`-with-alert
behaviour documented here; this re-read confirms that correction is accurate
and adds the four-way contrast table plus the `OUT_LEN ≥ 1` visibility
caveat (not previously stated). Logged as D-084.

---

## 4. NewTOS implementation notes

- The AESPB is preserved **bit-exactly** (six array pointers; `WORD` arrays +
  two `LONG`/pointer arrays), so recompiled GEM programs see the same ABI. The
  `control[0..3]` semantics and `int_out[0]` return are contract.
- Trap mapping: like the VDI, the AES is a parameter-block trap on `svc #2`,
  distinguished by **selector 200 in `x0`** (the m68k `d0 = 200` equivalent);
  the AESPB pointer is in `x1`. Consistent with the VDI selector choice
  (115) — see [`docs/COMPATIBILITY.md`](../../COMPATIBILITY.md) trap table.
- **Unsupported opcode → `-1`** (including opcode 0, silently) is contract;
  the reference `fm_show(ALNOFUNC)` alert is optional.
- **`int_out` words a handler did not write read 0**, not stack: `dispatch`
  zeroes its local `int_out[O_SIZE]` before `crysbind` and copies back
  `min(OUT_LEN, O_SIZE)` words exactly as `xif` does. Named deviation
  (kernel-stack leak across the EL0 boundary otherwise), A27 —
  [`docs/COMPATIBILITY.md`](../../COMPATIBILITY.md); regression test
  `tos-api/tests/aes_intout_underwrite.rs`.
- **Values wider than a WORD inside `int_in`/`int_out`** (T04, 2026-09-21):
  a `LONG` — EmuTOS's type at the access site, `evnt_timer`'s
  `MAKE_ULONG(T_HICOUNT, T_LOCOUNT)` with `T_LOCOUNT = int_in[0]`
  (`gemsuper.c:143`, `aesdefs.h:190-191`), `evnt_multi`'s `int_in[14..15]`
  (`:150`, `:220-221`), `wind_get(WF_SCREEN)`'s `*(LONG *)&int_out[1]` /
  `&int_out[3]` (`gemwmlib.c:1564`) — is **32 bits over two WORDs, low word
  first**, at every caller width; NewTOS reads `int_in[n] | int_in[n+1] <<
  16` (`tos-api/src/aes/event.rs`) and writes one unaligned 32-bit store per
  LONG (`tos-api/src/aes/mod.rs::set_intout_long`). A **pointer** packed into
  `int_in` (`wind_set`'s `WM_IX = int_in[2]` form, `gemsuper.c:346`,
  `aesdefs.h:504`) occupies the **caller's pointer width** — `IN_LEN` counts
  those WORDs (`Aespb::intin_ptr`/`ptr_words`, A81). An LP64 binding must
  therefore access a LONG slot with a 32-bit accessor (gemlib's pristine
  `*(long *)(int_in + n)` is an 8-byte access there) and must not read a
  LONG with a pointer-width read (`&int_out[1]` after `WF_SCREEN` gives
  `address | length << 32`). The contract entry with the full derivation is
  [`docs/COMPATIBILITY.md`](../../COMPATIBILITY.md) "A `LONG` in
  `int_in`/`int_out`/`ptsin`/`ptsout`"; the VDI side is the same rule
  ([`../vdi/_dispatcher.md`](../vdi/_dispatcher.md), "Values wider than a
  WORD"); regression test `tos-api/tests/aes_t04_intin_long_contract.rs`.
- **Selector 201** (bare reschedule, no block) is an undocumented feature; a
  faithful NewTOS may treat it as a cooperative yield or a no-op — flagged as an
  open question, low priority.
- Pointers in `addrin`/`addrout`/`global` are **user (application) pointers**;
  NewTOS must apply the same user/kernel boundary handling as GEMDOS string
  reads (cf. the `Cconws`/`Pexec` `ldtrb` path). Only `rsrc_gaddr` writes an
  `addrout`; every other handler returns via `int_out`/`global`.
- The AES is a GEM **application environment**: it runs on top of the VDI
  (Phase 12) for all drawing and on the process model (Phase 06) for
  `appl_*`/`shel_*`. Implementation is split per the
  [working list](./_working-list.md).
