# AES case coverage ledger

What `aes-r1.tsv` covers, what is still to come, and what is excluded with the
reason. The counterpart of `COVERAGE.md`, which does the same for the VDI.

Opcodes are the complete `crysbind` switch (`aes/gemsuper.c:87-409`) and the
opcode map in `docs/api/aes/_dispatcher.md` §3; behaviour comes from each
manager and from `docs/api/aes/`.

**One table per round.** A case id is the physical line number and the file's
CRC binds every dump to it, so *any* edit — an appended case, a fixed typo in
a comment — invalidates the dumps already taken. Round 1 is frozen as
`aes-r1.tsv` (**95 cases, CRC `33C13C8F`**); round 2 gets its own file.

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

### Rules 1 and 2 are enforced, not just written down

`core/cases.c` refuses to load a table — named reason, non-zero return, the
offending physical line — if any case *or prelude* names an opcode that stops
the run finishing, one TOS 3.06's AES does not have, or one that is not an AES
opcode at all. Opcode 0 is the single deliberate exception. The refusal table
carries a `file:line` citation per opcode. **There is no override**: taking an
opcode off the list is a source change with a citation, and that friction is
the point — a data file must not be able to hang a machine in a way that
cannot be told apart from a defect in the thing being measured.

Two entries were **dropped** from the working list after reading the source,
and three were **added** that were not on it:

| Opcode | | Why |
|---|---|---|
| `graf_mbox` (72) | dropped | `gr_movebox` is `gr_2box`/`gr_xor` only (`gemgrlib.c:287-307`) — a finite XOR animation, no wait. It stays out of *round 1* because it draws, which is this ledger's business, not the guard's. |
| `evnt_dclick` (26) | dropped | `ev_dclick` is a pure read (`gemevlib.c:300-309`). Out of round 1 as an operator preference, not as a hazard. |
| `form_button` (56) | **added** | `gr_watchbox` (`gemfmlib.c:251`) and `ev_button` (`:256`) both wait for the button to come up. |
| `appl_trecord` (15) | **added** | `while (gl_recd) ev_timer(100L);` (`gemaplib.c:216-217`) — loops until the operator has filled the event buffer. |
| `appl_read` (11) | **added** | `ap_rdwr(MU_MESAG, …)` → `ev_block` (`gemsuper.c:100-103`, `gemaplib.c:88`); `aqueue` blocks on an empty queue (`gemqueue.c:99`). |

**What the guard does NOT cover, and why.** Two residual hazards, both
outside the classes as stated, both real:

> **Lead decision, after the guard landed:** the four `CONF_WITH_PCGEM`
> opcodes — `appl_yield` (17), `scrp_clear` (82), `shel_rdef` (126),
> `shel_wdef` (127) (`gemsuper.c:115`, `:315`, `:398-403`) — **were added to
> the guard** as a fourth class after this section was first written. The
> argument for leaving them out was that the stated rule is "version-gated
> above AES 3.20" and `include/config.h` does not say that about them. True,
> and beside the point: the flag is absent from the `AES_VERSION` ladder
> entirely, so EmuTOS cannot say either way, and the cost of being wrong is a
> hang on the operator's TT that looks exactly like a real defect. Refused
> until a primary source settles it — Compendium or Profibuch, per `CLAUDE.md`
> rule 1's secondary cross-references. Removing one from the guard is a
> deliberate act with a citation, which is the friction we want.

- **`wind_update` (107)** — `wm_update(BEG_UPDATE)` blocks on
  `ev_block(MU_MUTEX, &wind_spb)` when the lock is already held
  (`gemwmlib.c:1786`). Two `BEG_UPDATE` cases in a row would self-deadlock,
  with no operator remedy at all. It is not in the guard because it does not
  block on the *operator* and because a balanced BEG/END pair is a legitimate
  round-2 measurement — but a round-2 table must pair them.
- **`appl_write` (12)** — `aqueue` blocks the event when the receiver's queue
  has no room (`gemqueue.c:120-124`). With a lone application and a 16-byte
  message it completes, so it is not a *reliable* hang; it is out of round 1
  for its pointer argument anyway.

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
| `appl_yield` (17) | **excluded — guarded** | `#if CONF_WITH_PCGEM`; safe in principle, but an AES without it pops the alert. Needs primary-source clearance. |
| `evnt_keybd`/`button`/`mouse`/`mesag`/`multi` (20-23, 25) | **excluded — blocking** | they wait on the operator. |
| `menu_*` (30-39) | later | every one takes a tree in `addr_in[0]`. |
| `objc_*` (40-47) | later | every one takes a tree in `addr_in[0]`; several draw. |
| `form_do`/`alert`/`error` (50, 52, 53) | **excluded — blocking** | modal. |
| `form_dial`/`center`/`keybd`/`button` (51, 54-56) | later | tree or screen-state arguments. |
| `graf_rubbox`/`dragbox`/`mbox`/`watchbox`/`slidebox` (70-72, 75, 76) | **excluded — blocking** | they track the mouse. |
| `graf_growbox`/`shrinkbox` (73, 74) | later | non-blocking but they **draw**, so they change the screen the VDI dump is taken against. |
| `graf_mouse` (78) | later | changes the cursor shape and, with a custom form, takes a pointer. |
| `scrp_read`/`write` (80, 81) | later | path pointer, and they touch the filesystem. |
| `scrp_clear` (82) | **excluded — guarded** | `#if CONF_WITH_PCGEM`; alert clearance needed. |
| `fsel_input`/`exinput` (90, 91) | **excluded — blocking** | modal file selector. |
| `rsrc_*` (110-114) | later | the whole point of them is `global[]` + a resource file; round 2, and they are the reason `global[]` is recorded after every call. |
| `shel_write` (121) | **excluded — blocking** | launches a program. |
| `shel_read`/`get`/`put`/`find`/`envrn` (120, 122-125) | later | pointer arguments. |
| `shel_rdef`/`wdef` (126, 127) | **excluded — guarded** | `#if CONF_WITH_PCGEM`; pointers, and alert clearance needed. |

---

## Predictions this run will test

Three divergences read out of `tos-api/src/aes/window.rs` while the table was
being written. They are recorded here rather than acted on: the point of the
dump is to let a *measurement* decide them, and a reading that goes straight
to a fix is the failure mode this whole exercise exists to break. **Nothing in
`tos-api/` was touched.** Each names the case in `aes-r1.tsv` that will
confirm or refute it.

| # | Prediction | Read from | Cases that decide it |
|---|---|---|---|
| P-AES-1 | NewTOS answers `wind_get` on **handle 0** for the four rect fields only. Its `win(handle)` returns `None` for handle 0 (`window.rs:905-911`, `handle < 1`), and the desktop is served by a separate `None if handle == 0` arm whose `match` has arms for `WF_WXYWH`/`WF_CXYWH`/`WF_PXYWH`/`WF_FXYWH` and `_ => ret 0`. EmuTOS answers far more, because there handle 0 *is* a window (`VF_INUSE`, `gemwmlib.c:1290`, `205-213`). | `tos-api/src/aes/window.rs:1633-1643` | the nine `wind_get 0` field cases, especially `WF_HSLIDE`/`WF_VSLIDE`/`WF_HSLSIZ`/`WF_VSLSIZ`/`WF_KIND` — a `0` return against EmuTOS' `1` |
| P-AES-2 | NewTOS's `wind_calc` uses the **non-3D** border arithmetic while the target profile is TOS4/3D. Its own comment says so. If it holds, `MOVER` alone gets no title band and the `SIZER`-only strip-and-force rule is absent. | `tos-api/src/aes/window.rs:915-917` (`"EmuTOS wm_calc, non-3D path"`) | `wind_calc WC_WORK/WC_BORDER MOVER alone` (D-097), `SIZER alone`, `VGADGETS\|SIZER`, `HGADGETS\|SIZER` |
| P-AES-3 | NewTOS **clamps** an out-of-range gadget index where EmuTOS does not range-check it at all. | `tos-api/src/aes/window.rs:317-319` (`g.clamp(0, NUM_ELEM-1)`) vs `gemwmlib.c:1568`, `:1573` | **none in round 1** — `WF_COLOR`/`WF_DCOLOR` are excluded precisely because measuring this needs a deliberate in-range `IN_LEN 3` case. Round 2. |

A prediction that the dump refutes is worth as much as one it confirms: it
would mean the reading was wrong, which is exactly what the measurement is for.

## Toolchain trap found while building this

**`uintptr_t` is four bytes in the NewTOS build, with eight-byte pointers.**
`build-dumpaes-newtos.sh` (and `build-dumpvdi-newtos.sh`, and every other
client built the same way) compiles with `-nostdinc -I sdk/libc/mintlib/include`
ahead of clang's own resource include, so mintlib's `stdint.h` — written for
the 32-bit m68k — wins. Measured with the exact build flags:

| type | size |
|---|---|
| `int16_t` | 2 |
| `int32_t` | 4 |
| `size_t` | 8 |
| `void *` | 8 |
| `unsigned long` | 8 |
| **`uintptr_t`** | **4** |

The fixed-width types and `size_t` are fine; `uintptr_t` is not, and it fails
*silently* — a pointer round-tripped through it loses its top half. This is a
live trap for anything else compiled through that path, not just the dumper.
`core/runner_aes.c` uses `unsigned long` plus a compile-time size assertion
instead, and says why at the point of use.

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
- **The comparator's per-case header is area-aware, but only because it is
  told.** `td_casefile_read` takes the area and picks the comment column from
  it (five for the VDI form, seven for the AES form); `tosdiff` passes the
  area the *dump header* carries, so a mislabelled dump would mislabel a case.
  Case *identity* is unaffected either way — the count, the line numbers and
  the CRC come from the shared line iterator, which is area-agnostic.

---

## Predictions for run 2 — written before the run, as round 1's were

Fixed in `16a78da`: the dispatcher now honours `OUT_LEN`, and `appl_init` writes
only the six fields `gemsuper.c:92-98` writes, within fifteen words.

No case can go fully to MATCH while `global[]` is compared in every record
(D-R1-5 and D-R1-6 are still open), so each prediction below is "this case loses
every `int_out[…]` line", not "this case turns green".

| Case | Op | `OUT_LEN` | today | predicted |
|---|---|---|---|---|
| 129 | 108 | 0 | `int_out[0..4] = 1,101,119,180,112` | all seven words `0xDEAD` |
| 130 | 108 | 1 | `[1..4] = 101,119,180,112` | `[0]=1`, `[1..6]` poisoned |
| 166 | 104 | 1 | `[1..4] = 0,0,0,0` | `[0]=1`, `[1..6]` poisoned |
| 172 | 104 | 1 | `[1..4] = 0,19,1920,1061` | `[0]=1`, `[1..6]` poisoned |
| 173 | 104 | 3 | `[3]=1920, [4]=1061` | `[0..2]` unchanged, `[3..6]` poisoned |
| 217 | 77 | 1 | `[1..4] = 8,16,19,19` | `[0]=1`, `[1..6]` poisoned |
| 218 | 79 | 1 | `[1..4] = 126,342,1,0` | `[0]=1`, `[1..6]` poisoned — the operator's mouse position leaves the record |
| 241 | 0 | 0 | `[0] = -1` | all poisoned; the -1 is computed and never delivered |

**Case 131 is a deliberate partial.** `int_out[3]`/`[4]` go poisoned, but
`int_out[2]` stays 119 against EmuTOS's 123 — that is D-R1-1 (`wind_calc` on the
non-3D branch), a different defect. **If `int_out[2]` also disappears, something
is wrong**, because this fix has no business touching geometry.

**Controls — these must NOT move.** Case 216 (`graf_handle`, `OUT_LEN 5`) stays
MATCH at `[1, 8, 16, 19, 19]`, which is what proves the full declared width is
still copied. The 53 `wind_calc` cases of D-R1-1 stay exactly as they are. The
21 cases already equal on `int_out` stay equal. `global[]` keeps showing its six
lines in all 95 records.

**Fix 2 changes nothing in this round**, and that is itself the prediction: the
log line `global after appl_init 832,1,1,112,0,0,0,0,0,0,16,-32592,17,0,0` must
come back byte for byte, because the dumper holds `aes_global` in `.bss` and the
four fields we no longer write were already zero there. Only `global[11]/[12]`
(`ap_3resv`, a kernel address) may legitimately move on a relink. Proving fix 2
from a dump needs a round-2 case that poisons `global[]` before `appl_init`;
until then its evidence is the selftest and the source review.

---

## Predictions for run 3, part 1 — `wind_calc` on the 3D branch (D-R1-1)

Written **before** the run, as rounds 1 and 2 were. Fix: `borders()` in
`tos-api/src/aes/window.rs` now computes `wm_calc`'s `#if CONF_WITH_3D_OBJECTS`
branch (`gemwmlib.c:1817-1840`) — `adj_hbox-1` title band, `TGADGETS` (so
`MOVER` alone gets one), a full `gl_hbox` for `INFO`, and the `SIZER`
strip-and-force rule — with `adj_wbox = adj_hbox = gl_?box + 2*ADJ3DSTD = 23`
on any screen ≥ 640 wide.

`global[]` still differs in every record (D-R1-5/6 open), so each line below is
again "this case loses every `int_out[…]` line", not "this case turns green".

The predicted value **is** the EmuTOS value already in
`reference/EMUTOS14-AES.BIN`; `old` is what `reference/NEWTOS-AES.BIN` holds.
All 53 cases plus the partial 131:

| Case | kind | word | old → predicted |
|---|---|---|---|
| 68, 69, 70 | NAME / CLOSER / FULLER | `[2]`,`[4]` | 119→**123**, 130→**126** |
| 71 | MOVER alone | `[2]`,`[4]` | 101→**123**, 148→**126** |
| 72 | INFO alone | `[2]`,`[4]` | 119→**120**, 130→**129** |
| 73 | SIZER alone | `[3]`,`[4]` | 180→**176**, 130→**148** |
| 74, 75, 76 | UPARROW / DNARROW / VSLIDE | `[3]` | 180→**176** |
| 77, 78, 79 | LFARROW / RTARROW / HSLIDE | `[4]` | 130→**126** |
| 83, 84, 85 | WC_BORDER NAME / CLOSER / FULLER | `[2]`,`[4]` | 81→**77**, 170→**174** |
| 86 | WC_BORDER MOVER | `[2]`,`[4]` | 99→**77**, 152→**174** |
| 87 | WC_BORDER INFO | `[2]`,`[4]` | 81→**80**, 170→**171** |
| 88 | WC_BORDER SIZER | `[3]`,`[4]` | 220→**224**, 170→**152** |
| 89, 90, 91 | WC_BORDER UP/DN/VSLIDE | `[3]` | 220→**224** |
| 92, 93, 94 | WC_BORDER LF/RT/HSLIDE | `[4]` | 170→**174** |
| 97, 98 | NAME\|CLOSER\|FULLER, TGADGETS | `[2]`,`[4]` | 119→**123**, 130→**126** |
| 99 | NAME\|INFO | `[2]`,`[4]` | 137→**142**, 112→**107** |
| 100 | VGADGETS | `[3]` | 180→**176** |
| 101 | HGADGETS | `[4]` | 130→**126** |
| 102 | VGADGETS\|SIZER | `[3]`,`[4]` | 180→**176**, 130→**148** |
| 103 | HGADGETS\|SIZER | `[3]`,`[4]` | 180→**198**, 130→**126** |
| 104 | VGADGETS\|HGADGETS\|SIZER | `[3]`,`[4]` | 180→**176**, 130→**126** |
| 105 | 0x0FEF | `[2]`,`[3]`,`[4]` | 119→**123**, 180→**176**, 112→**104** |
| 106, 107 | 0x0FFF, 0xFFFF | `[2]`,`[3]`,`[4]` | 137→**142**, 180→**176**, 94→**85** |
| 108 | WC_BORDER 0x0FEF | `[2]`,`[3]`,`[4]` | 81→**77**, 220→**224**, 188→**196** |
| 109 | WC_BORDER NAME\|INFO | `[2]`,`[4]` | 63→**58**, 188→**193** |
| 110 | WC_BORDER 0x0FE0 | `[3]`,`[4]` | 220→**224**, 170→**174** |
| 112 | 0x0FEF, rect 0,0,0,0 | `[2]`,`[3]`,`[4]` | 19→**23**, −20→**−24**, −38→**−46** |
| 113 | WC_BORDER, same rect | `[2]`,`[3]`,`[4]` | −19→**−23**, 20→**24**, 38→**46** |
| 114 | 640×400 | `[2]`,`[3]`,`[4]` | 19→**23**, 620→**616**, 362→**354** |
| 115 | WC_BORDER 640×400 | `[2]`,`[3]`,`[4]` | −19→**−23**, 660→**664**, 438→**446** |
| 116 | 1×1 at 5,5 | `[2]`,`[3]`,`[4]` | 24→**28**, −19→**−23**, −37→**−45** |
| 117 | negative origin | `[2]`,`[3]`,`[4]` | −81→**−77**, 30→**26**, 12→**4** |
| 118 | 32000×32000 border | `[2]`,`[3]`,`[4]` | −19→**−23**, 32020→**32024**, 32038→**32046** |
| 120, 121 | wtype 2 / −1 | `[2]`,`[3]`,`[4]` | 119→**123**, 180→**176**, 112→**104** |
| 132–136 | IN_LEN / AIN_LEN variants | `[2]`,`[3]`,`[4]` | 119→**123**, 180→**176**, 112→**104** |
| 137 | `w`/`h` arrive poisoned | `[2]`,`[3]`,`[4]` | 119→**123**, −8551→**−8555**, −8569→**−8577** |
| **131** | OUT_LEN 3 (partial) | `[2]` | 119→**123** — `[3]`/`[4]` stay poisoned from the OUT_LEN fix; **the whole case now matches** |

**Controls — these must NOT move.**

- Case **216** (`graf_handle`) stays MATCH at `[1, 8, 16, 19, 19]`. `graf_handle`
  reports the *unadjusted* `gl_wbox`/`gl_hbox` (`gemsuper.c:287-291`); if it
  reported 23 the 3D adjustment has leaked out of the window manager. Confirmed
  under QEMU: `[objc] graf_handle: … wbox=19 hbox=19`.
- Cases **67, 80, 82, 95** (kind 0, `HOTCLOSE`) stay MATCH: neither branch adds a
  border for them.
- The 21 cases already equal on `int_out` stay equal; `global[]` keeps its six
  lines in all 95 records.

**Not measurable from this table, and stated so it is not read as a miss:** the
*drawn* frame moved with the calculation (title band 19→23 px, scroll bars 19→23
px, gadgets `gl_wbox`×`gl_hbox` inset by `ADJ3DSTD`). Round 1 has no
`wind_create`/`wind_open`, so no case sees it. Its evidence is the QEMU
screendump and the kernel AES selftests (`[p5]` reports window A's work area as
`61,92,336,185` for `kind 0x0FFF` at `60,50,360,250` — exactly `tb=42, rb=23,
bb=23`).

---

## Predictions for run 3, part 2 — handle 0 is a real window (D-R1-3)

Written **before** the run. Fix: `tos-api/src/aes/window.rs` gives the desktop
window a table slot and builds it the way `wm_init` does
(`gemwmlib.c:1290-1294` + `w_setup`, `:205-218`) — `VF_INUSE`, `kind = NONE`,
`CURR = PREV = gl_rscreen`, `FULL = WORK = gl_rfull`, `hslide = vslide = 0`,
`hslsiz = vslsiz = -1`. `win()` is now `get_pwin` (`:1391-1404`): handle 0 is a
legal handle, so `wm_get` answers it exactly as it answers any window. The
work rectangle is *stored* (`w_move`, `:1073-1077`) instead of recomputed, which
is what lets the desktop's `WF_WXYWH` be `gl_rfull` rather than
`wm_calc(WC_WORK, NONE, screen)`. `wind_find` starts from `NIL` like `ob_find`
(`gemoblib.c:1044`) and answers 0 only inside the desktop window's `CURR`.

| Case | Field | today | predicted |
|---|---|---|---|
| 159 | `wind_get(0, WF_HSLIDE)` | `ret 0`, `[1]` poisoned | `ret 1`, `[1] = 0` → **MATCH** |
| 160 | `wind_get(0, WF_VSLIDE)` | `ret 0`, `[1]` poisoned | `ret 1`, `[1] = 0` → **MATCH** |
| 161 | `wind_get(0, WF_HSLSIZ)` | `ret 0`, `[1]` poisoned | `ret 1`, `[1] = −1` → **MATCH** |
| 162 | `wind_get(0, WF_VSLSIZ)` | `ret 0`, `[1]` poisoned | `ret 1`, `[1] = −1` → **MATCH** |
| 194 | `wind_find(−1,−1)` | `0` | **−1** → MATCH |
| 195 | `wind_find(0,−1)` | `0` | **−1** → MATCH |
| 196 | `wind_find(−1,0)` | `0` | **−1** → MATCH |
| 197 | `wind_find(32000,32000)` | `0` | **−1** → MATCH |
| 158 | `wind_get(0, WF_FXYWH)` | `0,0,1920,1080` | `0,**19**,1920,1061` — `int_out[2]` joins EmuTOS' 19 and its diff line disappears; `[3]`/`[4]` stay apart because the screens are 640×480 vs 1920×1080. **Partial by construction, not a miss.** |

**Two cases that must NOT be read as regressions.**

- **167** (`wind_get(0, WF_OWNER)`, `OUT_LEN 1`) **flips from MATCH to
  differing**: we answer `ret = 1`, the dump answers `0`. Today's match is a
  coincidence of two different causes (section 0.1 of
  `docs/analysis/aes-r1-findings.md`) — the TT runs the EmuTOS **1.4 release**,
  which has no `WF_OWNER` at all, while the pinned reference tree
  (`VERSION_1_4-71-g1a7484ab`, commit `acb7f3b4`) does. We follow the pinned
  tree. This is reference-version skew; it resolves by re-dumping on a post-1.4
  EmuTOS, not by changing our answer.
- **164** (`wind_get(0, WF_KIND)`, `OUT_LEN 1`) **stays MATCH at `ret = 0`** —
  but for a new reason. It used to match only because our `WF_KIND` arm was
  unreachable for handle 0; making handle 0 a window would have exposed it
  (`ret = 1`). `wm_get` does not handle `WF_KIND` (`gemwmlib.c:1591`,
  `default: return FALSE`), so the arm was removed rather than kept — the
  deliberate deviation **D-053 / AUD-P5-014 is retired**. If 164 comes back with
  `ret = 1`, the removal did not take.

**Unchanged, and each is a control.** 155/156/157 keep exactly their present
resolution-only differences (`WF_WXYWH`/`WF_CXYWH`/`WF_PXYWH` were already
right); 163 (`WF_TOP` = 0, EmuTOS `w_top` returns `DESKWH` when nothing is
topped, `gemwmlib.c:315-318`), 165 (`WF_NEWDESK` → FALSE), 166, 168, 169, 170,
172, 173 stay as they are; 175–178 (handles 1, −1, 99, 32767) keep `ret = 0` —
handle 0 became legal, nothing else did; 193 (`wind_find(0,0)` = 0) stays.

**Already measured here, on QEMU raspi4b at 640×480 — the TT's resolution — by
a temporary probe through the real `svc #2` trap (not committed):**

```
wind_get(0,WF_WXYWH) ret=1 [0,19,640,461]     wind_get(0,WF_HSLIDE) ret=1 [0]
wind_get(0,WF_CXYWH) ret=1 [0,0,640,480]      wind_get(0,WF_VSLIDE) ret=1 [0]
wind_get(0,WF_PXYWH) ret=1 [0,0,640,480]      wind_get(0,WF_HSLSIZ) ret=1 [-1]
wind_get(0,WF_FXYWH) ret=1 [0,19,640,461]     wind_get(0,WF_VSLSIZ) ret=1 [-1]
wind_get(0,WF_KIND)  ret=0 (nothing written)  wind_get(0,WF_OWNER)  ret=1
wind_find(0,0)=0  wind_find(-1,-1)=-1  wind_find(0,-1)=-1  wind_find(-1,0)=-1
wind_find(32000,32000)=-1
```

Those are the EmuTOS values of cases 155–158 **to the word**, at the same
resolution. The Pi run will differ from the TT only where the screen size does.

---

## Predictions for the next hardware trace — the three AES event defects (D-EV-1/2/3)

Written **before** anything was built or run, against
`tosdump/reference/TERADESK-AESTRACE-r1.BIN` (4734 records, 36.3 s, TeraDesk on
a real Pi 4) and its `--button` report. The fix is three changes in
`tos-api/src/aes/` — `event.rs` (`downorup`), the new `input.rs` (mouse
ownership + the button-delay window) and one call each from
`vdi/mouse.rs::feed_relative`. The tracer, the trace and `tools/aestrace.py`
are untouched, so **every number below is produced by the same decoder that
produced the r1 report** and the two are directly comparable.

### What the r1 trace measured (the baseline these predictions move)

All 153 button-asking calls are `evnt_multi` (25); there is no `evnt_button`
(21) in the trace. They fall into exactly two request classes:

| class | `bclk` | `bmask` | `bstate` | `flags` | calls | fired | calls that ever sampled a press (`mb_or != 0`) |
|---|---|---|---|---|---|---|---|
| **A** — TeraDesk's main loop | `0x0102` | `3` | `0` | `0x13` | 148 | **148** | **8** |
| **B** — its drag/release wait | `0x0002` | `1` | `0` | `0x22` | 5 | 2 | 5 |

Class A is `bflag = 1` (`(buparm >> 24) & 0xff`, `aes/geminput.c:350`): "tell me
when a button **is pressed**". Class B is `bflag = 0`, `mask = 1`, `state = 0`:
a genuine *level* wait for the left button to be **up**, and EmuTOS answers it
from the level too — class B is **correct today** and must not change.

### D-EV-1 — `downorup` honours the flag byte (`aes/geminput.c:348-357`)

1. **The hit rate collapses from 98.0 %.** With 8 real presses in 36 s the
   report should print single digits: `(8 + 2) / 153 ≈ 6.5 %` for an identical
   input sequence. A new session has a different press count, so the *rate* is
   only indicative — the invariant below is the real test.
2. **The falsifier, session-independent:** the report's line
   `of those, N sampled the button as UP (0x00) on EVERY evaluation` must go to
   **N = 0** — *provided the session contains no class-B call that fires with
   the button never pressed*. A class-A (`bflag = 1`) call that returns
   `MU_BUTTON` while `mb_or == 0` is the defect, unfixed. One such call
   falsifies the fix. (In r1 all 140 came from class A; all 5 class-B calls had
   `mb_or != 0`.)
3. `button-returning calls over N s` drops from **4.1 per second** to roughly
   the operator's real click rate — under 1/s for normal use.
4. **The report will still print `clicks=258` and still annotate that line
   `<-- 'button UP' request`.** `button_report` derives that note from
   `bmask`/`bstate` alone and never looks at the flag byte; it is not corrected
   by this fix and its persistence is **not** a falsification. Do not read it as
   one. (The tracer is the instrument; correcting its annotation is a separate
   change to `tools/aestrace.py`, not part of this work.)
5. The requested click **count** is now the low byte (`2`), not the raw `258`
   (`LOBYTE(HIWORD(parm))`, `aes/geminput.c:470,537`). Not directly visible in
   the report; visible as `bret` (EV_BRET) never exceeding 2 in the full
   `--events` listing.

### D-EV-2 — the `ev_multi` quick check is gated on mouse ownership (`aes/gemevlib.c:185`)

**Correction to the brief, established from the source before coding:** the gate
does *not* stop a level-satisfied button from firing. When the app does not own
the mouse, `ev_multi` falls through to `iasync(MU_BUTTON, buparm)` →
`abutton`, and `abutton` runs the very same `downorup(button, p)` test with **no
ownership condition** (`aes/geminput.c:525-528`) — it just answers `1` click
instead of `mclick`. The real filter is `post_button`, which walks only
`gl_mowner`'s sleeping list (`aes/geminput.c:344,465`): an app that went to
sleep unsatisfied is never woken while it does not own the mouse. So the
implemented rule is: **entry evaluation always tests the level; every later
evaluation only fires while we own the mouse.** Anyone reading the r1 report's
docstring ("never gets the level test") should read this paragraph instead.

Nothing in `--button` measures this directly. Its falsifiers are behavioural, in
the full `--events` listing:

6. A press that lands on a **window frame or a non-top window** produces the
   Control-Manager `POST` record (`WM_TOPPED` / `WM_MOVED` / `WM_CLOSED` / …)
   and **no** `evnt_multi` call returning `MU_BUTTON` with `mx`/`my` inside that
   frame for the duration of that press. A call that returns both is the
   unfixed double handling.
7. A press in the **top window's work area** or on the **desktop background**
   must still return `MU_BUTTON` exactly as before — `chk_ctrl` answers `1`
   resp. `0` there (`aes/geminput.c:86-104`), i.e. the app owns it. If drive
   icons or window contents stop responding to clicks, the gate is too wide and
   the fix is wrong.
8. `wind_update(BEG_MCTRL)` (mode 3) **grants** ownership to the app
   (`fm_own` → `ct_chgown(rlr, &gl_rscreen)` → `set_mown`, `aes/gemfmlib.c:93`,
   `aes/gemctrl.c:392-396`) — it does not take it away. So the 4 BEG/END_MCTRL
   brackets in r1 mark intervals where the quick check is *unconditionally on*.
   If a trace shows a button-wait inside such a bracket that does **not** fire
   on a press, the polarity has been implemented backwards.

### D-EV-3 — `mchange` cancels the pending click window (`aes/geminput.c:360-370`)

9. **`double-click windows opened` collapses from 150 to about the number of
   real presses** (≈ 10 for an r1-like session). This is D-EV-1's doing, not
   D-EV-3's: a window is only opened once `MU_BUTTON` has actually fired with
   more than one click requested. A next trace that still shows ~150 windows
   means D-EV-1 did not take.
10. **`... in which the pointer moved by more than EmuTOS's ±2` goes to 0**, and
    with it the `EmuTOS mchange would have cancelled …` verdict in the per-call
    listing. The cancel check sits directly after `pump_input()`, so the window
    ends *before* the poll that would have folded the offending step into
    `win_max_step`. A window that still reports `maxstep > 2` means the cancel
    did not fire — with one legitimate exception: a step assembled from several
    IKBD packets each ≤ 2 px between two polls. EmuTOS does not cancel those
    either (its test is per *packet*, `xrat-rx` vs the previous **reported**
    position), so such a window is EmuTOS-conformant, not a miss. At r1's poll
    density (55.8 M polls over 150 windows, ~370 k polls per window) this should
    be vanishingly rare.
11. `worst window: … polls=374055 travel=246px maxstep=75` has no successor: no
    surviving window should show three-digit travel. Long windows now end on the
    first move rather than on the full 220 ms `gl_dclick` expiry.
12. **The window is only armed for a press** (`if (gl_bpend && state)`,
    `aes/geminput.c:129-141`). A class-B (`state = 0`) call that fires with the
    button *up* opens an *unarmed* window that movement does **not** cancel —
    that is EmuTOS (`gl_bdely == 0` → `mchange` does nothing) and must not be
    read as a missing cancel.

### What cannot be shown here, and by what

Neither QEMU board has USB, so no mouse packet ever reaches `feed_relative` on
either board: `b_click`/`mchange` never run, the ownership latch never leaves
"the app owns", and the delay window is never cancelled. **Every one of the 12
points above is real-Pi-4-only.** What QEMU does prove is the negative: the
desktop still draws and still answers its own `bmask = 1, bstate = 1` wait
(screendump), and the kernel AES selftest's `evnt_button` case
(`bclk = 1, bmask = 1, bstate = 0`, `bflag = 0`) still returns 1 click with no
buttons down — the `bflag = 0` path is unchanged by construction.

## Predictions for the next hardware run — the AES desktop font size (D-FONT-1/2/3)

Written **before** the Pi 4 is booted with the change, against the user's report
("TeraDesk isn't using the small fonts, so all the icons and the text in the
windows too are cut off and hard to read") and against the two QEMU screendumps
taken before and after on `qemu-system-aarch64 -M raspi4b` (640×480, the SD
image from `tools/make-sd-image.sh`).

### What EmuTOS does, and what NewTOS did

EmuTOS keeps **two** font sizes open at once (`gsx_start`,
`aes/gemgraf.c:405-410`): the normal one `gl_wchar × gl_hchar` (8×16 on any
screen with ≥ 400 lines) and a small one `gl_wschar × gl_hschar`, obtained from
`vst_height(SIZ_TAB[1])` — the smallest `top` in the system face, which is the
6×6 font's 4, so the small cell is **6×6** (`vdi/vdi_text.c:635,706-713,666-679`).

There are exactly three places in all of EmuTOS that use the small pair
(`grep gl_wschar\|gl_hschar` over `aes/` + `desk/`):

| # | EmuTOS | what |
|---|--------|------|
| 1 | `aes/gemgraf.c:802` | an icon's overlay character (`gsx_tblt(SMALL, …)`) |
| 2 | `aes/gemgraf.c:806` | an icon's label (`gr_gtext(TE_CNTR, SMALL, …)`) |
| 3 | `desk/deskapp.c:519-520,810-811` | the icon label box + cell: `ib_wtext = 12*gl_wschar = 72`, `ib_htext = gl_hschar+2 = 8`, `g_wicon`, `g_hicon` |

plus any `TEDINFO` whose `te_font` says `SMALL` (5), which
`just_draw` passes straight to `gr_gtext` (`aes/gemoblib.c:757`).

NewTOS drew **all** of these in the 8×16 font. That is the whole defect: a GEM
icon resource sizes its label box for 6×6 — EmuTOS's own `desk/icon.rsc` and
TeraDesk's `sdk/apps/teradesk/icons.rsc` both carry
`ib_wtext = 72, ib_htext = 8, ib_xicon = 20` for every one of their 14 resp. 38
ICONBLKs — so an 8×16 label overflows **both** axes and the old per-object clip
cut it: vertically to 8 of 16 rows, horizontally to 72 of up to 96 px.

### D-FONT-1 — icon labels and overlay characters (`gr_gicon`)

1. **TeraDesk's directory windows in icon view show complete, readable file
   names.** A full 12-character 8.3 name (`LYNXBOOT.IMG`, `KEYBOARD.KBD`) is
   `12 × 6 = 72` px — exactly `ib_wtext`, so it fits with no slack and starts
   flush at the box's left edge. Names shorter than 12 characters are centred
   with the half-margin rounded **up** (`gr_just`, `gemgraf.c:578-586`).
2. **No label is clipped top or bottom.** The 6-row cell is centred in the
   8-row box at offset `(8-6+1)/2 = 1`, so one blank row above and one below.
   Any glyph still showing a sliced ascender/descender means the small font did
   not take.
3. **Names longer than 12 characters lose whole characters, never half a
   glyph** (`gsx_tcalc`, `gemgraf.c:494`). On a TOS 8.3 volume this is
   unobservable; it becomes visible with long names on a VFAT/MiNT volume.
4. **The drive letter on a drive icon sits inside the 32×32 bitmap.** It is
   drawn at `ib_xicon + ib_xchar` (`gemgraf.c:802`), i.e. `20 + 13 = 33` px
   from the object's left edge, `16` px down, in a 6×6 cell — so rows 16..21 of
   the bitmap. NewTOS previously drew it at `ib_xchar` alone in a 16-row cell
   (rows 16..31), which for an app-supplied ICONBLK put it 20 px too far left
   and let it run off the bitmap's bottom edge into the label box. **On
   TeraDesk this is the visible one**: its `icons.rsc` uses `ib_xicon = 20`
   (and 12/21/22/23/25 for some entries) throughout.

### D-FONT-2 — the desktop's own icon grid (`app_start`/`setup_iconblks`)

5. **The NewTOS desktop's icon cell shrinks from 100×50 to 76×40 px**
   (`g_wicon = 12*6 + 2*ib_xtext`, `g_hicon = 32 + 6 + 2`), so a
   directory window fits **six** icon columns at 640×480 where it fitted four,
   and the desktop drive/trash icons sit on a denser grid
   (`g_icw = g_desk.w / xcnt`). Confirmed on the two QEMU screendumps: the
   before shot shows 4 columns (`ACC APPS AUTO GEMSYS`), the after shot 6
   (`ACC APPS AUTO GEMSYS SUB BIG.NCP`).
6. **Clicking an icon still selects it, and clicking its label still selects
   it.** `desk::act` hit-tests the `ib_xtext..ib_htext` rectangle
   (`deskact.c:397`), which is now 72×8 rather than 96×16 — a click 10 px below
   an icon bitmap that used to hit the label now misses it. This is the one
   place where the change can *look* like a regression and is EmuTOS-correct:
   EmuTOS's label box is 8 px tall too.
7. **The drag outline is unaffected** — NewTOS has no `G.g_xyicon`
   (`deskapp.c:990-1005`) at all yet, so nothing there can move.

### D-FONT-3 — everything in the normal font must be unchanged

8. **The menu bar, the window title, the window info line, dialog buttons,
   alert text and the file selector are pixel-identical to before.** They are
   `G_STRING`/`G_TITLE`/`G_BUTTON`/`G_BOXTEXT` with `te_font = IBM`
   (`gemwmlib.c:156` for `W_NAME`/`W_INFO`; `gem_rsc.c:31-91` for the AES's own
   trees), i.e. still 8×16. The QEMU tile-diff of the before/after screendumps
   confirms this on `raspi4b`: **row 0..15 (the whole menu bar) is byte-identical**,
   as are the window frame, title, info line, scroll bars and the desktop
   pattern; only the two desktop icon cells and the window's icon area differ.
9. **A `G_BOXCHAR` gadget character (the window closer/fuller/arrows) stays
   8×16** — `just_draw` forces `edblk.te_font = IBM` for `G_BOXCHAR`
   (`gemoblib.c:729`).
10. **The `CHECKED` mark of a checked menu item does not move.** EmuTOS draws it
    at `t.g_x+2, t.g_y` with no vertical centring (`gemoblib.c:907`); NewTOS
    centred it. For a menu item, `ob_height == gl_hchar == 16`, so the two
    formulas agree exactly and the pixels are unchanged. On an object taller
    than 16 px the mark now sits at the top, as in EmuTOS.

### What QEMU cannot show, and why

TeraDesk is a GEM app and launches from the desktop, not from `C:\AUTO`
(`AUTO` is console-only, `.NCP`), and desktop interaction needs both a mouse
and C: storage, which no QEMU board gives at once. **Points 1, 3, 4 and 6 are
real-Pi-4-only**; they are the ones that involve TeraDesk's *own* resource
geometry. What QEMU does prove is points 5, 8 and — through the NewTOS
desktop's own icons — the *mechanism* of 1 and 2, plus the host test
`tos-api/tests/aes_object_font.rs`, which renders a stock 72×8 ICONBLK through
the real `objc_draw` body and compares every pixel against the 6×6 raster.

---

## Predictions for the next hardware run — the red area in TeraDesk's dialogs (D-OBSPEC-1)

Written before any hardware run, as the earlier prediction blocks were.

### What draws it, and how that was established

The red filled area in the upper right of every TeraDesk dialog is **not**
drawn by the AES or the VDI. It is drawn by TeraDesk's own `G_USERDEF`
callback for the dialog "dragbox" ear, `ub_drag`
(`sdk/apps/teradesk/library/xdialog/xddraw.c:508-568`), which every xdialog
dialog installs unconditionally (`xd_set_userobjects`, `xddraw.c:1749-1751`;
the original `ob_spec` is preserved in the XUSERBLK by `xd_xuserdef`,
`xdutil.c:138`). Its last block draws the fragment of dialog border that
would show behind the ear:

```c
b = abs((_WORD) obspec->obspec.framesize);          /* xddraw.c:540 */
if (b != 0) {
    udef_vsl_color(xd_vhandle, obspec->obspec.framecol);   /* xddraw.c:546 */
    for (i = 0; i < b; i++) { ... udef_v_pline(xd_vhandle, 3, pxy); ... }
}
```

`OBSPEC.obspec` is gemlib's `BFOBSPEC` bit-field view of `ob_spec`
(`mt_gem.h:2051-2061`), declared in m68k (big-endian) field order with no
endianness handling. On a little-endian target a C bit-field is allocated
from the **least** significant end of its storage unit (AAPCS64,
"Bit-fields"), so the whole word is mirrored: `character` lands in bits 0..7
and `interiorcol` in bits 28..31, instead of the GEM layout

```
31..24 character | 23..16 framesize | 15..12 framecol
11..8  textcol   | 7 textmode | 6..4 fillpattern | 3..0 interiorcol
```

which is what the resource file and the AES Object Manager both use
(EmuTOS `gr_crack`, `aes/gemgraf.c:522-535`; `ob_sst`, `aes/gemobjop.c:54-58`;
NewTOS `just_draw`, `tos-api/src/aes/object.rs:2283,2002-2010`).

The dragbox of every TeraDesk dialog carries `ob_spec = 0x00029180`
(measured in `sdk/apps/teradesk/build/dist/desktop.rsc`, tree `INFOBOX`
object 1 and tree `APPLIKAT` object 2): thickness 2, colour word `0x9180`.
Read through the mirrored bit-field that is

| field | GEM bits | GEM value | mirrored bits | value read |
|---|---|---|---|---|
| `framesize` | 23..16 | `2` | 15..8 (signed) | **-111** |
| `framecol`  | 15..12 | `9` (LBLACK) | 19..16 | **2 (RED)** |

so the loop runs **111** times in **colour 2** instead of twice in dark grey.
Each pass draws a horizontal and a vertical segment whose ends walk outwards
(`pxy[0]++, pxy[1]++, pxy[2]--, pxy[5]--`), so after the eighth pass the
rectangle inverts and the segments sweep left, right, up and down from the
16x16 ear.

This is measured, not inferred: `sdk/gemlib/tests/obspec_bits.c` reproduces
`framesize = -111` / `framecol = 2` against the pristine header, and the
compiled object `build/obj/library_xdialog_xddraw.o` that produced the
photographed screen contains

```
1074: ldr   w8, [x19]           ; ob_spec
1078: sbfx  w0, w8, #8, #8      ; framesize <- bits 8..15   (GEM: 16..23)
1088: ldrh  w8, [x19, #0x2]
1094: and   w1, w8, #0xf        ; framecol  <- bits 16..19  (GEM: 12..15)
```

### D-OBSPEC-1 — what must change on the photographed dialogs

1. **The red area is gone.** In its place, at the top right of the dialog,
   sits the 16x16 dragbox ear with a "\" diagonal and a **2-pixel dark grey
   (LBLACK, pen 9)** partial border along its top and right — the two
   iterations `framesize = 2` asks for. Nothing red is drawn anywhere by
   TeraDesk.
2. **Nothing spills past the dialog rectangle any more.** The overhang past
   the right and top edges was the outer end of the 111-pass sweep, clipped
   to the xdialog `drect` (`xd_clip_on(&pb->pb_xc)`, `xddraw.c:521`). With
   two passes every segment stays inside the ear.
3. **Objects drawn after the dragbox are unchanged.** All 24 dialog trees in
   `desktop.rsc` carry a dragbox (23 with `ob_spec = 0x00029180`, one with
   `0x00029181` — same `framesize`/`framecol`), always at the dialog's
   top-right corner and always near the front of the root's child list
   (child #0 in `INFOBOX`, child #1 in 22 of the others, child #8 in
   `QUITOPT`). That is why the red appeared in *every* dialog, in the same
   corner, with the rest of the dialog drawn on top of it. Those later
   objects never read the bit-field and are unaffected.
4. **The geometry that must match**, if anyone measures the old screens:
   for the 312x256 `INFOBOX` the sweep covers x `0.64..1.01` and y
   `-0.01..0.43` of the dialog rectangle — the right ~two fifths of the
   width, the top ~43 % of the height, one to two pixels past the right and
   top edges — with a one-column step at its top-left corner where the
   horizontal-segment triangle meets the vertical-segment one.

### Three more sites the same header fix changes

The bit-field is read or written in three other places in TeraDesk. None was
visible in the photographs; all three now address the documented bits:

5. **`resource.c:160`** `ob_spec.obspec.fillpattern = 7` on slider background
   objects wrote bits 25..27 (invisible to the AES, which reads bits 4..6);
   slider backgrounds therefore stayed hollow instead of solid dark grey.
   They should now come up **solid**.
6. **`icon.c:2481-2482`** sets `interiorcol`/`fillpattern` on TeraDesk's
   desktop background object. These wrote bits 25..31; the desktop colour and
   pattern chosen in TeraDesk's own preferences had **no effect** and the
   background stayed at the resource default. It should now follow the
   setting.
7. **`window.c:4545`** tests `obj[0].ob_spec.obspec.interiorcol == 0`, which
   read bits 28..31 and so was true only by accident.

### What is real-Pi-4-only, and why

TeraDesk is a GEM program launched by double-clicking `DESKTOP.NAP` from the
desktop, and opening a dialog needs a mouse *and* `C:`, which no QEMU board
offers at once (`raspi4b` has the SD, `virt` has the input). **Every point
above is real-Pi-4-only.** What QEMU does show is that the *kernel's* own
dialog path is red-free: a `virt` screendump of the AES `.RSC` dialog and of
`form_alert` contains **0 pixels** with `r >= 200, g < 80, b < 80`, and the
desktop behind them is the same green (0,255,0) as in the photographs.

The fix is entirely app-side — `sdk/gemlib/overlay/mt_gem.h.in` plus the
rebuilt `DESKTOP.NAP`. The kernel image is byte-identical, so no kernel
screendump can show a before/after of it.

---

## Predictions for the next hardware run — the window that empties on a click (D-WORK-1/2/3)

The reported defect, on a real Pi 4 with TeraDesk: **clicking a file inside an
open directory window empties the window** — the listing is gone and does not
come back. Written before any hardware run of the fix.

### What draws it, and how that was established

Three separate deviations feed the same symptom; all three sit on the window
**frame** path, which every window redraw goes through.

1. **The AES painted the window's work area.** `draw_frame` filled the client
   region white on every frame redraw (`tos-api/src/aes/window.rs`, the
   `fill_rect(work…, WHITE)` that is now gone). EmuTOS never touches it: the
   frame is the OBJECT tree `gl_awind`, whose `W_BOX`, `W_DATA` and `W_WORK`
   are `G_IBOX` (`aes/gemwmlib.c:62-69`) and whose `W_DATA`/`W_WORK` carry
   `ob_spec 0x00001101` — border thickness 0 (`:87-88`). `ob_draw` skips the
   border of a zero-thickness object and the interior fill of a `G_IBOX`
   (`aes/gemoblib.c:663-670`). The work area belongs to the application.
2. **The Control Manager topped the window itself.** `intercept` called
   `top_window` before sending `WM_TOPPED`. EmuTOS `hctl_window` sends
   `WM_UNTOPPED` + `WM_TOPPED` and **returns** (`aes/gemctrl.c:170-178`).
3. **`wind_set(WF_TOP)` ran on the already-top window.** EmuTOS guards it with
   `if (w_handle != gl_wtop)` (`aes/gemwmlib.c:1676-1680`). Because of (2) the
   owner's answering `WF_TOP` always found the window already topped, so the
   repaint it triggered — and with (1) the erase — happened with **no**
   `WM_REDRAW` behind it.

The reported gesture reaches (1) directly: TeraDesk's `dir_select` →
`dir_setnws(w, TRUE)` → `dir_info(w)` ends in
`wind_set_str(w->xw_handle, WF_INFO, w->info)` (`sdk/apps/teradesk/dir.c`,
`window.c`), i.e. **every** selection change rewrites the info line, and our
`wind_set(WF_INFO)` redrew the whole frame including the white work fill.
TeraDesk does not redraw after `dir_info`, so the window stayed blank. Our own
desktop calls `WF_INFO` only on folder navigation, where it follows with an
explicit `do_wredraw` — which is why the defect was TeraDesk-only.

Measured on the host rig (`tos-api/tests/aes_window_content.rs`, the real AES
into a pixel buffer): `wind_set(WF_INFO)` overwrote **65412 of 65412**
work-area pixels; `wind_set(WF_TOP)` on the already-top window changed
**71253** screen pixels and queued no message.

### D-WORK-1 — a click on a file keeps the listing

Click any file in an open TeraDesk directory window. The listing must stay on
screen, complete, with the clicked entry inverted; the info line above it must
update to the selected-item count/size. Nothing else on the screen changes.
Before the fix, the whole work area went white and stayed white.

### D-WORK-2 — a click on a window that is not on top

With two TeraDesk windows overlapping, click the lower one.

- Both windows keep the content that was **already visible**; nothing is
  repainted with the desktop background.
- The old top's frame redraws with its inactive title (no dither behind the
  name), the clicked window's with the active one.
- The clicked window is *not* topped by the AES: it comes to the front only
  when TeraDesk answers the `WM_TOPPED` with `wind_set(WF_TOP)`
  (`wd_type_topped`, `sdk/apps/teradesk/window.c:2375-2378`).
- The part that was covered repaints from the `WM_REDRAW` the AES posts for
  the whole work area — so no stale pixels of the other window are left behind.

### D-WORK-3 — two frame rows that are new, and correct

The window frame now has a black 1-pixel line **under the title bar** and
another **under the info line** that were not visible before. They are the
bottom border rows of EmuTOS's `W_TITLE` (a `G_BOX`, `ob_spec 0x00011101`,
thickness 1 — `aes/gemwmlib.c:63,84`) and `W_INFO` (a `G_BOXTEXT` whose
`gl_ainfo` TEDINFO has `te_thickness = 1` — `:153-157`), each on the last row
of its band, i.e. the row directly above the first row `wm_calc` gives the work
area (`tb = 1 + (adj_hbox-1) + gl_hbox`, `:1808-1812`). We drew them one row
too low, *inside* the work area, where the work fill immediately covered them
again — so they were invisible and the client region lost a row.

Both QEMU boards show exactly this and nothing else: a full-screen diff of the
desktop with the `C:\` window open is **1116 pixels on 2 rows** on `virt`
(1920x1080) and **1084 pixels on 2 rows** on `raspi4b` (640x480), at `y=65`
(was white) and `y=84` (was grey 189). The 20-item listing, icons, labels,
sliders, menu bar and background are byte-identical.

Still missing, and *not* changed here: EmuTOS's `W_INFO` box border is drawn on
all four sides, so it also has a black row at the **top** of the info band
(`y=66` in those dumps), where we still show grey. One pixel row, outside the
work area, pre-existing.

### What is real-Pi-4-only, and why

TeraDesk is launched by double-clicking `DESKTOP.NAP`, and clicking a file in a
directory window needs a mouse *and* `C:`, which no QEMU board offers at once
(`raspi4b` has the SD, `virt` has the input). **D-WORK-1 and D-WORK-2 are
real-Pi-4-only.** What QEMU does show is D-WORK-3 plus the no-regression
evidence above, and that a window drag on `virt` leaves the frame, the white
work area and the vacated background intact. The pixel-level proof of D-WORK-1
and D-WORK-2 is the host test, which drives the same AES entry points
(`wind_set(WF_INFO)`, `wind_set(WF_TOP)`, a `evnt_multi` click on a covered
window) into a framebuffer and counts the changed pixels.

---

## Round 4 predictions — the two QED undraw defects (2026-09-02)

Written **before** the hardware run, against the change described below. Two
symptoms reported on a real Pi 4 with QED launched from TeraDesk: QED's
background is white instead of the desktop background, and the file selector
stays on screen after a file has been picked. They do **not** share a cause.

### The two mechanisms

**D-BG-1 — `w_drawdesk`'s fallback root was white.** TeraDesk hands the screen
to a GEM application by un-registering its own background tree and asking the
AES to repaint the desktop rectangle: `wind_set(0, WF_NEWDESK, NULL, 0)`
(`sdk/apps/teradesk/startprg.c:264`) followed by `dsk_draw()`
(`startprg.c:294`), which is nothing but `form_dial(FMD_START)` +
`form_dial(FMD_FINISH)` over `xd_desk` (`sdk/apps/teradesk/icon.c:241-249`;
`xd_desk` = `wind_get(0, WF_WORKXYWH)`, `main.c:1145`). EmuTOS answers the
FMD_FINISH with `w_drawdesk(pt)` (`aes/gemfmlib.c:375-378`), and `w_drawdesk`
with `gl_newdesk == NULL` draws `gl_wtree[ROOT]` at depth 0
(`aes/gemwmlib.c:340-359`) — a `G_BOX` whose `ob_spec` `wm_init` copied from the
AES's own built-in DESKTOP tree, `4419L` = `0x1143`, pattern 4 in colour 3
(`aes/gemwmlib.c:1268-1271`, `aes/gem_rsc.c:551-556`), with its low byte
replaced by the `.INF` `#Q` background when there is one
(`set_aes_background`, `aes/geminit.c:635-641`, run before `wm_start` at
`:719-730`). We drew a solid **white** box there
(`tos-api/src/aes/window.rs`, `FALLBACK_ROOT_OBSPEC = 0x0070`). White belongs to
`sh_draw(cmd, TRUE)`, which swaps the DESKTOP root's `ob_spec` for
`CLEAR_SCREEN` = `0x0000`, draws once and puts the old spec straight back
(`aes/gemshlib.c:64,269-286`) — a one-shot, not a state.

**D-FSEL-1 — `fsel_input` never called `fm_dial`.** QED opens a file through
cflib's `select_file`, which calls `fsel_exinput`/`fsel_input`
(`sdk/cflib/cflib-21/filesel.c:255-257`). EmuTOS' `fs_input` brackets the whole
dialog with `fm_dial(FMD_START, &gl_rcenter, &gl_rfs)` before the first
`ob_draw` and `fm_dial(FMD_FINISH, &gl_rcenter, &gl_rfs)` after the loop
(`aes/gemfslib.c:707-709,918-919`); the FMD_FINISH is what repaints the desktop
under the dialog and re-asks every overlapped window for its content. Ours drew
the tree and returned — `git log -S form_dial -- tos-api/src/aes/fsel.rs` is
empty, so the call has been missing since the file selector landed (`b469b21`).
Both defects therefore predate `87e0362`/`fbe8d0e`/`065270b`/`00559c2`.

Measured on the host rig (`tos-api/tests/aes_dialog_undraw.rs`, the real AES
into a pixel buffer, 640x480): before the fix `form_dial(FMD_FINISH)` with no
registered desktop tree left **153600 of 307200** pixels wrong — exactly the
half of the screen the green dither would have covered; `fsel_input` left
**24735** pixels of the dialog on the screen after it returned. After the fix
both counts are 0.

### D-BG-1 — the background a launched GEM application sits on

Launch QED (or any `.NAP`) from TeraDesk on the Pi 4.

- **[E]** The area around QED's window shows the **desktop background**, not
  white: with the shipped placeholder `NEWDESK.INF` (no `#Q` line) that is the
  AES resource default `0x1143` — the 50 % green/white checkerboard, pixel
  `(x, y)` green iff `x + y` is even. If the card's `NEWDESK.INF` *does* carry a
  `#Q` line, the background is that pattern/colour instead, i.e. the same one
  the TeraDesk desktop itself shows.
- **[E]** The transition may still flash white: TeraDesk's `close_windows()`
  and our own desktop's launch path both clear the screen first
  (`sh_draw(cmd, TRUE)`), and only the following repaint is the pattern.
- **[E]** Quitting QED gives the TeraDesk desktop back unchanged.

### D-FSEL-1 — the file selector goes away

In QED, `File > Open…`, pick a file, `OK` (or double-click it, or press RETURN).

- **[E]** The selector box disappears the moment the call returns; the file is
  opened as before.
- **[E]** What is behind it comes back: the part of `gl_rfs` outside QED's
  window shows the desktop background, the part inside it is repainted by QED
  from the `WM_REDRAW` the AES posts (`w_update(DESKWH, gl_rfs, …)`).
- **[E]** `Cancel` behaves the same way — the selector goes away and nothing is
  opened.
- **[E]** The selector now sits **9 px lower** than before (measured on the host
  rig: its first drawn row moves from `y = 61` to `y = 70`, `x` unchanged at
  157): it is centred with
  `ob_center`, which centres **below** the menu bar
  (`yd = gl_hbox + (gl_height - gl_hbox - hd) / 2`, `aes/gemobed.c:76-80`),
  where our own `center()` had used the full screen height. Its 3 px `OUTLINED`
  ring is inside the rectangle that gets repainted, so no frame is left behind.
- **[E]** The desktop's own file selector (Install Application, etc.) undraws
  the same way, for the same reason.
- **Watch for:** a visible flash of the desktop background inside QED's window
  before QED answers the `WM_REDRAW`. Our compositor paints the background
  across the whole damage rectangle and lets the frames and the owner paint back
  over it, where EmuTOS' `do_walk` clips `w_drawdesk` to the desktop window's
  owner-rectangle list and never touches window interiors
  (`aes/gemwmlib.c:286-312`). That difference is pre-existing (it is how every
  `form_dial(FMD_FINISH)` and `wind_close` already repaints) and is **not**
  changed here; if the flash is objectionable it is its own work item.

### What is real-Pi-4-only, and why

Launching a GEM application needs a mouse *and* `C:`, which no QEMU board
offers at once (`raspi4b` has the SD, `virt` has the input), so **both** rows
above are real-Pi-4-only. What QEMU shows is the no-regression evidence: a
`raspi4b` boot screendump with the `C:\` window open is **byte-identical**
before and after the change (0 of 307200 pixels differ), because the desktop
registers its own background tree via `WF_NEWDESK` and so never reaches the
fallback root. The pixel-level proof of both fixes is the host test, which
drives the same AES entry points (`wind_set(0, WF_NEWDESK, NULL)` +
`form_dial(FMD_FINISH)`, and `fsel_input` ended with RETURN) into a framebuffer
and counts the changed pixels.

---

## Round 5 predictions — the desktop background over-paint (D-DESK-1…4, 2026-09-02)

Written **before** any run of the fixed build.

### What EmuTOS does, and how that was established

`w_drawdesk(pc)` — the AES's one and only desktop-background repaint — is a
single `do_walk(DESKWH, tree, root, depth, pc)` (`aes/gemwmlib.c:340-359`), and
`do_walk` (`:286-312`) is two clips, not one:

```c
if (pc) rc_intersect(&gl_rfull, pc); else pc = &gl_rfull;
for (po = D.w_win[wh].w_rlist; po; po = po->o_link) {
    rc_copy(&po->o_gr, &t);
    if (rc_intersect(pc, &t)) { gsx_sclip(&t); ob_draw(tree, obj, depth); }
}
```

1. `gl_rfull` = the screen **minus the menu row**, `r_set(&gl_rfull, 0, gl_hbox,
   gl_width, gl_height - gl_hbox)` (`aes/gemgraf.c:423-424`), set at `gsx_start`
   **unconditionally**, menu bar installed or not. So no `do_walk` ever touches
   the top `gl_hbox` (= 19) rows. Removing a menu bar does not repaint them
   either — `mn_bar`'s remove arm only drops `gl_mntree` and hands `gl_rmenu` to
   the control manager (`aes/gemmnlib.c:724-728`).
2. `D.w_win[DESKWH].w_rlist` = the desktop window's **owner rectangle list**:
   its own rectangle with every window above it broken out of it, rebuilt by
   `newrect`/`mkrect`/`brkrct` (`aes/gemwrect.c:64-205`) at the top of every
   `draw_change` (`aes/gemwmlib.c:1129`) — i.e. with the *new* geometry, before
   anything is drawn. An open window owns its rectangle, the desktop does not,
   so `w_drawdesk` cannot paint inside a window. What a window shows comes back
   from its owner in answer to the `WM_REDRAW` that `w_update`/`w_redraw` post
   (`:1004-1048,887-907`).

The same `do_walk` is `w_cpwalk`'s body (`:363-379`), so a window's **frame** is
likewise drawn once per piece of *its own* rectangle list — never over the
window covering it. That is the second half of the same contract.

Established by reading the EmuTOS sources above, and measured on the host rig
`tos-api/tests/aes_desk_background.rs`, which drives the real AES entry points
into a pixel buffer. Against the pre-fix build, all six of its assertions fail:

| entry point | pixels the AES painted where EmuTOS paints none |
|---|---|
| `wind_close` (uncovers a lower window) | 101 184 of 101 184 of the lower window's work area |
| `form_dial(FMD_FINISH)` over a window  | 101 184 of 101 184 |
| `wind_open` over an existing window    | 69 264 of 71 416 of the *new* window's work area |
| `wind_set(WF_CURRXYWH)`                | 10 266 of 69 360 |
| mover drag, one step                   | the whole vacated strip, incl. the half another window owns |
| any full-screen `form_dial(FMD_FINISH)`| 12 160 = 640 × 19, the menu row |

### D-DESK-1 — the boot desktop is unchanged

QEMU `raspi4b` + the SD image, boot to the desktop with the `C:\` window open.

- **[E]** The screendump is **byte-identical** before and after: 0 of 307 200
  pixels differ. The desktop paints its background tree over `gl_rfull` and its
  menu bar over `gl_rmenu`, and with a single window the owner rectangle list is
  the whole desktop rectangle minus that window — which is exactly the region
  the old unclipped paint covered *and then* overdrew with the window frame.
- **[E]** The menu bar is intact. It is drawn by `menu::install_menu_bar`, which
  `desk::redraw_desktop` runs after the AES composite
  (`kernel/src/desk/redraw.rs:206-213`) — the same order EmuTOS' desktop uses.

### D-DESK-2 — a window no longer flashes the desktop background (real Pi 4)

Two directory windows open, the lower one showing a listing. Close the top one;
or dismiss a dialog that covered both; or open a second window over the first.

- **[E]** The window underneath keeps its listing where it is *visible*. Before,
  the whole damage rectangle — including the parts of the lower window that no
  window covers — was painted with the desktop background (box **and icons**)
  and only came back when the owner answered its `WM_REDRAW`. That flash is
  gone; the AES now paints only the desktop's own pieces.
- **[E]** The part of the lower window that the *closed* window had covered is
  still stale until the owner repaints it — that is EmuTOS' behaviour too
  (`w_redraw` posts a message, it does not paint), and our desktop answers it in
  the same event turn.
- **[E]** Desktop icons never appear inside a window, at any moment.

### D-DESK-3 — dragging a window across another one (real Pi 4)

Drag one directory window by its title bar across a second one.

- **[E]** Over bare desktop: unchanged — the vacated L is repainted with the
  background exactly as before.
- **[E]** Over the other window: the background (and its icons) no longer
  appears inside that window during the drag.
- **[E] Known residue, reported, not fixed here:** NewTOS drags the window
  *solid* (a `copy_rect` per step) where EmuTOS drags an XOR outline
  (`gr_dragbox`, `aes/gemgrlib.c`). Each step's blit leaves the moved window's
  own left/top border column behind inside the covered window, and nothing
  repaints it until the drag settles — a thin comb trail for the length of the
  drag, cleared by the `WM_MOVED` repaint on release. EmuTOS has no such state
  because it moves the window once, at the end. Replacing the solid mover drag
  with `gr_dragbox` (as the sizer drag was already replaced with `gr_rubwind`)
  removes it; that is its own work item.

### D-DESK-4 — the menu row is never repainted by the AES

- **[E]** A full-screen `form_dial(FMD_FINISH)` — TeraDesk's `dsk_draw()` on a
  program launch, `sdk/apps/teradesk/icon.c:241-249` — leaves the top 19 rows
  exactly as they were, and the launched application's own `menu_bar()` paints
  them. Before, the AES painted the desktop dither across them first, so the
  menu bar blinked away and back on every launch and on every dialog dismissal
  that used the full screen.
- **[E]** Our desktop is unaffected in the steady state: it reinstalls its menu
  bar after every whole-screen composite.

### What is real-Pi-4-only, and why

D-DESK-2 and D-DESK-3 need a mouse *and* `C:` at once, which no QEMU board
offers (`raspi4b` has the SD, `virt` has the input). QEMU's contribution is
D-DESK-1, the byte-identical no-regression screendump, plus D-DESK-4 which the
host rig measures exactly. The pixel-level proof of the whole change is
`tos-api/tests/aes_desk_background.rs` (6 cases), which drives `wind_open`,
`wind_close`, `wind_set(WF_CURRXYWH)`, `form_dial(FMD_FINISH)` and a scripted
mover drag into a framebuffer and counts what changed outside the region EmuTOS
would repaint.
