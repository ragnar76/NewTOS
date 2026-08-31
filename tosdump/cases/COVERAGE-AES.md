# AES case coverage ledger

What `aes-r1.tsv` covers, what is still to come, and what is excluded with the
reason. The counterpart of `COVERAGE.md`, which does the same for the VDI.

Opcodes are the complete `crysbind` switch (`aes/gemsuper.c:87-409`) and the
opcode map in `docs/api/aes/_dispatcher.md` §3; behaviour comes from each
manager and from `docs/api/aes/`.

**One table per round.** A case id is the physical line number and the file's
CRC binds every dump to it, so *any* edit — an appended case, a fixed typo in
a comment — invalidates the dumps already taken. Round 1 is frozen as
`aes-r1.tsv` (**95 cases, CRC `BC46EACA`**); round 2 gets its own file.

---

## The three rules that decide what may be in round 1

**1. Nothing that blocks.** An opcode the machine does not implement reaches
the dispatcher's `default` arm, which pops an alert box and waits for a click
(`fm_show(ALNOFUNC, &opcode, 1)`, `gemsuper.c:411-417`). On the TT and on the
Pi alike that is a hung run and a power cycle. **Opcode 0 is the only unknown
opcode that is safe to issue** — `gemsuper.c:414` skips the alert for it
("Ignore zero since some PRGs are this call") and returns `-1` like any other
unsupported opcode. Everything that waits on the operator is therefore out:
the whole `evnt_*` family except `evnt_timer`, `form_do`, `form_alert`,
`form_error`, `fsel_input`, `fsel_exinput`, `graf_rubbox`, `graf_dragbox`,
`graf_mbox`, `graf_watchbox`, `graf_slidebox`, `shel_write`, `appl_exit`.

**2. Nothing undefined.** `xif`'s `int_in` and `int_out` are *uninitialised
stack buffers* (`gemsuper.c:430-432`). So:

- **Never declare more output words than the call writes** — the surplus is
  copied back from stack garbage, and comparing garbage produces a difference
  that means nothing. gemlib pre-zeroes `aes_intout[3]` and `[4]` before
  `wind_get` for exactly this reason (`a_wind_get.c:358`).
- **Never declare fewer input words than the call reads** — the handler then
  reads stack garbage as a parameter.

Declaring *fewer* output words is fine and is a measurement: the surplus stays
`0xDEAD` in the caller's array, and that absence is what the case proves.
Declaring *more* input words than the case lists is also fine: the surplus is
copied from the caller's own poisoned array, so it is `0xDEAD` and
deterministic.

**3. Nothing that mutates state another case reads,** because there is no
reset (`core/runner_aes.c`). The VDI reset restores the attribute defaults
`init_wk` establishes, which `docs/api/vdi/_state.md` documents as a defined
set. The AES has no equivalent — `docs/api/aes/_state.md` describes process
descriptors, message queues and run lists, and there is no documented default
set to restore. Round 2 solves this with the prelude mechanism plus a real
reset, designed then.

A fourth line, which is the spec's rather than the AES's: **round 1 is
pointer-free** — `addr_in`/`addr_out` stay poisoned, which rules out every
opcode needing a tree, a string or a resource.

### What "differs legitimately" means here

Spec section 6 is strict for this area: *"comparison is exact for every byte …
no exception mechanism — if you find yourself wanting one, the case is wrong,
not the comparator."* Applied to the AES that draws a line:

- **Accepted:** answers that scale with the screen resolution or the font
  metrics. Almost every geometric answer does, `wind_calc` first among them,
  and that dependency *is* the measurement — `graf_handle` is in the table so
  that the reader has `gl_wchar`/`gl_hchar`/`gl_wbox`/`gl_hbox` in the same
  dump to read the geometry against.
- **Rejected:** within-machine nondeterminism (uninitialised stack), operator
  preferences (double-click rate), operator state (where the mouse is, which
  windows are open), and machine addresses.

---

## What round 1 covers

| Opcode | Name | Cases | What it measures |
|---|---|---|---|
| 108 | `wind_calc` | 60 | the border↔work geometry contract, and the dispatcher's marshalling |
| 104 | `wind_get` | 22 | the desktop handle's fields, the per-field output count, handle validation |
| 106 | `wind_find` | 5 | the window hit test, on points no open window can change |
| 77 | `graf_handle` | 2 | the VDI handle and the four box metrics `wm_calc` runs on |
| 79 | `graf_mkstate` | 1 | that a void handler leaves `ret = TRUE` |
| 24 | `evnt_timer` | 2 | the one event call that cannot wait on an operator |
| 0 | — | 3 | the unsupported-opcode sentinel, and the copy-back rule at its sharpest |

### `wind_calc` (108), 60 cases

`wm_calc` is a pure function of type, kind and the box metrics
(`gemwmlib.c:1807-1867`); the spec says so outright ("wind_calc never touches
the window list"). It is `void`, so `int_out[0]` is `crysbind`'s `ret = TRUE`
(`gemsuper.c:85`) and not something `wm_calc` computed.

- **28 single-gadget cases**, all fourteen defined `kind` bits (`NAME`,
  `CLOSER`, `FULLER`, `MOVER`, `INFO`, `SIZER`, `UPARROW`, `DNARROW`,
  `VSLIDE`, `LFARROW`, `RTARROW`, `HSLIDE`, `HOTCLOSE`, and none), each in
  both `WC_WORK` and `WC_BORDER` form.
- **14 combination cases**: `TGADGETS`, `VGADGETS`, `HGADGETS`, `NAME|INFO`,
  the `SIZER`-with-a-bar cases, `0x0FEF` (the classic full window), `0x0FFF`,
  and `0xFFFF` (do the undefined bits do anything).
- **7 rect cases**: degenerate, full screen, 1×1 (the work area goes
  negative), negative origin, and a `WC_BORDER` rect that grows but stays
  inside a `WORD`.
- **2 type cases**: `2` and `-1`. Only `WC_BORDER` negates
  (`gemwmlib.c:1856`), so everything else has to behave as `WC_WORK`.
- **9 marshalling cases** — same call, same values, only the declared counts
  differ: `OUT_LEN` 0 / 1 / 3, `IN_LEN` 8 / 16 / 20, `AIN_LEN` 3 with no
  pointer, `control[4]` set, and `IN_LEN 6` with only four values listed so
  that `w` and `h` reach `wm_calc` as `0xDEAD`.

Two cases are aimed at a known fork. `MOVER` alone is **D-097**: the non-3D
branch tests `kind & (NAME|CLOSER|FULLER)` (`gemwmlib.c:1842`) while the 3D
branch tests `kind & TGADGETS`, which includes `MOVER` (`:1818`) — so a
`MOVER`-only window gets a title band in one build and not in the other.
`SIZER` alone is the other: in the 3D build a SIZER counts toward both bars,
gets stripped from both, and then the vertical bar is forced back on
(`:1827-1834`).

### `wind_get` (104), 22 cases

Handle 0 is a real window: `wm_init` calls `w_setup(ppd, DESKWH, NONE)`, which
sets `VF_INUSE` (`gemwmlib.c:1290`, `205-213`), so `get_pwin(0)` succeeds. Its
rects are set at the same place — `CURR`/`PREV` to `gl_rscreen`, `FULL`/`WORK`
to `gl_rfull` (`:1291-1294`), i.e. the whole screen and the screen below the
menu bar (`gemgraf.c:423-424`).

`OUT_LEN` is per field and is **not** gemlib's flat 5: `wm_get` writes four
words for the rect fields, one word for the scalar fields, and nothing at all
for a field it does not handle (`gemwmlib.c:1505-1599`).

- rect fields `WF_WXYWH`/`WF_CXYWH`/`WF_PXYWH`/`WF_FXYWH`, `OUT_LEN` 5
- scalar fields `WF_HSLIDE`/`WF_VSLIDE`/`WF_HSLSIZ`/`WF_VSLSIZ`/`WF_TOP`,
  `OUT_LEN` 2
- return-code-only probes: `WF_KIND`, `WF_NEWDESK`, `WF_SCREEN`, `WF_OWNER`,
  and fields 0, 13, 21
- `WF_WXYWH` again with `OUT_LEN` 1 and 3, the marshalling pair
- handle validation: 1 (never created), −1, 99, 32767

### `wind_find` (106), 5 cases

`wm_find` is `ob_find(gl_wtree, 0, 2, x, y)` (`gemwmlib.c:1767-1770`), a pure
read. `W_TREE[ROOT]` carries the desktop's `CURR` rect (`:240`, `:1291`), so a
point on the screen belongs to the desktop and a point off it to nobody. Only
points whose answer *cannot* depend on which windows are open are in: the four
off-screen ones, and `(0,0)`, which is the menu bar strip that no window can
cover (a window's rect comes out of `gl_rfull`, which starts below it).

---

## Rejected, with the reason

| Opcode / field | Status | Why |
|---|---|---|
| `objc_sysvar` (48) | **excluded — safety** | `#if CONF_WITH_3D_OBJECTS`, and EmuTOS ties that flag to **AES 3.40** (`config.h:1345-1354`). The same table records **TOS 3.06 as AES 3.20**. An AES without the opcode reaches the `default` arm and pops the modal alert — a hung TT. It goes back in only once TOS 3.06's opcode table is established from a primary source. |
| `graf_mkstate` (79), full form | reduced to `OUT_LEN` 1 | its four words are `xrat`, `yrat`, `button`, `kstate` (`gemgrlib.c:458-464`) — where the operator last left the mouse, not an API behaviour. With `OUT_LEN` 5 an unwritten slot is indistinguishable from a written one. The return code is kept: it proves a void handler leaves `TRUE`. |
| `evnt_dclick` (26) | excluded | its only output is `gl_dcindex` (`gemevlib.c:34`, `:308`), a user preference the desktop restores from `DESKTOP.INF`. Comparing it reports the operator's settings. Round 2 measures it properly: a prelude that *sets* a known index, then a read-back — which is what preludes are for. |
| `wind_get` `WF_FIRSTXYWH` (11) / `WF_NEXTXYWH` (12) | excluded | `w_owns` **writes** `pwin->w_rnext` (`gemwmlib.c:1235`), so `WF_FIRSTXYWH` arms the iteration and `WF_NEXTXYWH` advances it. Order-dependent between cases and between runs — exactly what rule 3 excludes. |
| `wind_get` `WF_COLOR` (18) / `WF_DCOLOR` (19) | excluded | `gadget = pinwds[0]` is `int_in[2]` and is **not range-checked** before indexing `pwin->w_tcolor[]` (`gemwmlib.c:1567-1576`), so a wrong `IN_LEN` in the table is an out-of-bounds read on real hardware. `poutwds[0]` is also left unwritten, so `OUT_LEN ≥ 2` samples stack garbage. |
| `wind_get` `WF_NAME` (2) / `WF_INFO` (3) | excluded | the classic binding passes a **destination pointer** in `int_in[2..3]` and declares `IN_LEN 4` (`a_wind_get.c:344-347`). An AES that implements the field writes through it — with a pointer-free round 1 that is a write to `0xDEADDEAD`. Round 2, with a real buffer. |
| `wind_get` `WF_SCREEN` (17), full form | reduced to `OUT_LEN` 1 | `gsx_mret` returns the menu/alert save buffer **address** and length (`gemwmlib.c:1564`); an address cannot be compared between a TT and a Pi. The return code can. |
| `wind_create` (100) … `wind_new` (109) | later | all mutate the window list. They need the round-2 reset. |
| `wind_set` (105) | later | mutates by definition; needs a prelude and a reset. |
| `appl_init` (10) | excluded | the run is *inside* one already (`core/gem.c`); a second one would change the AESGLOBAL every other case is recorded against. Its result is in the run log instead. |
| `appl_exit` (19) | excluded | ends the application. Every case after it would measure nothing. |
| `appl_read`/`write`/`find`/`tplay`/`trecord` (11-15) | later | pointer arguments (message buffer, name, event record). |
| `appl_yield` (17) | later | `#if CONF_WITH_PCGEM`; safe in principle, but an AES without it pops the alert — same clearance as `objc_sysvar` is needed first. |
| `evnt_keybd`/`button`/`mouse`/`mesag`/`multi` (20-23, 25) | **excluded — blocking** | they wait on the operator. |
| `menu_*` (30-39) | later | every one takes a tree in `addr_in[0]`. |
| `objc_*` (40-47) | later | every one takes a tree in `addr_in[0]`; several draw. |
| `form_do`/`alert`/`error` (50, 52, 53) | **excluded — blocking** | modal. |
| `form_dial`/`center`/`keybd`/`button` (51, 54-56) | later | tree or screen-state arguments. |
| `graf_rubbox`/`dragbox`/`mbox`/`watchbox`/`slidebox` (70-72, 75, 76) | **excluded — blocking** | they track the mouse. |
| `graf_growbox`/`shrinkbox` (73, 74) | later | non-blocking but they **draw**, so they change the screen the VDI dump is taken against. |
| `graf_mouse` (78) | later | changes the cursor shape and, with a custom form, takes a pointer. |
| `scrp_read`/`write` (80, 81) | later | path pointer, and they touch the filesystem. |
| `scrp_clear` (82) | later | `#if CONF_WITH_PCGEM`; alert clearance needed. |
| `fsel_input`/`exinput` (90, 91) | **excluded — blocking** | modal file selector. |
| `rsrc_*` (110-114) | later | the whole point of them is `global[]` + a resource file; round 2, and they are the reason `global[]` is recorded after every call. |
| `shel_write` (121) | **excluded — blocking** | launches a program. |
| `shel_read`/`get`/`put`/`find`/`envrn` (120, 122-125) | later | pointer arguments. |
| `shel_rdef`/`wdef` (126, 127) | later | `#if CONF_WITH_PCGEM`; pointers, and alert clearance needed. |

---

## Known limitations of round 1

- **`d0` is expected to be 0 in every record.** `super()` returns a hard `0`
  for selector 200 (`gemsuper.c:454-466`); the AES result is `int_out[0]`. So
  unlike the VDI's `flip_y`, a non-zero `d0` here is a finding, not noise.
- **`addr_in`/`addr_out` are recorded as the low 32 bits of a pointer.** The
  record's address slots are four bytes by format (spec section 3) while the
  AArch64 build's pointers are eight. Round 1 never writes one, so only the
  poison passes through, and the poison is `0xDEADDEAD` in the low 32 bits on
  both targets by construction (`core/runner_aes.c`). It becomes a real
  question in round 2, when `rsrc_gaddr` returns an address.
- **`wind_find` and any window query assume no desktop window is open.** The
  five cases in the table are chosen so their answer cannot change if one is,
  but a round-2 window case will need the operator instruction.
- **A true arithmetic-overflow case is missing.** `wind_calc(WC_BORDER)` with
  `w = 32767` would overflow a `WORD`; a debug-profile NewTOS would panic on
  it rather than wrap, which would abort the run. The table stops at 32000
  and the overflow question is deferred.
- **`tosdiff`'s per-case header will name an AES case wrongly.**
  `host/casefile.c` takes the comment with the five-column VDI rule, so for a
  seven-column table it prints columns 5-7 rather than the comment. Case
  *identity* is unaffected — the case count, the line numbers and the CRC all
  come from the shared line iterator — so `--validate` and the byte comparison
  are correct; only the label is. `core/cases.c` already has
  `td_case_line_comment_n` for it; wiring the area through
  `td_casefile_read` is host-side work for the first AES comparison.
