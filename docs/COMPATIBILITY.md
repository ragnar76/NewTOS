# COMPATIBILITY — the contract NewTOS keeps, and what it is free to change

> The measuring stick for every spec, plan and implementation in NewTOS.
> Read alongside [`REQUIREMENTS.md`](REQUIREMENTS.md) (FR-1/FR-3),
> [`ARCHITECTURE.md`](ARCHITECTURE.md) (ABI Rules) and `CLAUDE.md` (Rule 1).
> When a doc or a piece of code is unsure "must this match EmuTOS exactly,
> or may we do it better?", this file decides.

## The goal, in one sentence

NewTOS is a **syscall- and variable-compatible reimplementation** of the
TOS API — **not a 1:1 port** of EmuTOS. EmuTOS is the *behaviour reference*;
the implementation is a clean, modern AArch64 design.

## Why this distinction matters

TOS and EmuTOS were written under 1980s constraints — kilobytes of RAM, a
slow 8 MHz 68000, ROM size limits. Many of their internal choices are
*workarounds for scarcity*, not part of what a program observes. On a
Raspberry Pi 4 those constraints are gone, so NewTOS can implement the same
*observable contract* with a cleaner, faster, safer design. Copying the
1980s internals would import their limitations for no benefit.

## Principle: clean first, optimise later

1. **First**: a correct, clean implementation of the compatibility contract
   (below). Verified against the EmuTOS behaviour and the documented ABI.
2. **Then** (and only then): optimisations and modern extensions, kept
   clearly separate so they never compromise the contract. See
   [Deferred optimisations](#deferred-optimisations-phase-2).

A program built for TOS must not be able to tell — through any documented
interface — that it is running on NewTOS rather than TOS.

## The compatibility contract — the invariant surface

These **must be exact**. They are what a program can observe, so they are
the contract:

| Surface | What is fixed |
|---|---|
| **Trap entry** | trap numbers / `svc` immediates (1 GEMDOS, 13 BIOS, 14 XBIOS; GEM `svc #2` with `x0` selector **115 = VDI**, **200 = AES**, parameter-block pointer in `x1`), function IDs |
| **Parameter ABI** | argument count, order, sizes, and register/stack placement per the two ABIs (m68k stack; AArch64 PCS, ID in `x8`) |
| **Return / error codes** | return values and the negative TOS error codes (`E_OK`, `EFILNF=-33`, …), bit-for-bit |
| **Data structures programs pass** | field layout + semantics of every struct crossing the boundary — DTA, FCB, BPB, MD, PD/basepage, OBJECT, TEDINFO, ICONBLK, BITBLK, AESPB, MFDB, … (offsets a program reads/writes are fixed; see [pointer width](#native-vs-m68k-struct-variants)) |
| **System variables** | the documented globals' *values and semantics* (`_hz_200`, `_bootdev`, `_v_bas_ad`, drive map, cookie jar, …) — see [System variables](#system-variables--compatible-how) |
| **Observable behaviour** | what a documented call *does*: side effects, ordering a program can detect, error conditions, defaults |

## Free implementation — what NewTOS may do differently / better

These are **not** the contract. NewTOS chooses them for cleanliness and
performance; a conforming program cannot observe them:

- Internal data structures, indexes, tables, caches.
- Algorithms (sorting, allocation strategy, FAT walking, scheduling policy).
- Memory layout, alignment, where things physically live (high-half kernel,
  per-process page tables, allocator design).
- Concurrency mechanics not exposed by the API.
- The EmuTOS-specific *scarcity workarounds* (packed bitfields to save ROM,
  re-using buffers, m68k-isms) — reimplemented straightforwardly.
- Anything behind a call whose *result* is unchanged.

> Rule of thumb: if a program can read it, time it, or depend on it through
> a documented interface, it is **contract**. Otherwise it is **free**.

### Sanctioned deviations, one by one

Not everything outside the contract is invisible. Where NewTOS deliberately
shows or does something EmuTOS does not, it is listed here — named, dated and
attributed, so that nobody later mistakes it for drift. The standing rule is
still *match EmuTOS*; an entry here means a human decided otherwise and said so.

**The boot screen reports the ST-RAM pool, not `phystop`** — user decision,
2026-09-04 (*"10Mb ist vollkommen OK"*). EmuTOS' `initinfo` prints
`stramsize = (long)phystop`, the machine's ST-RAM size
(`references/emutos/bios/initinfo.c:277`); on NewTOS that would be **14 MB**.
We print the **10 MiB pool** instead — what a program can actually be given.
Both are constants since `29a4581`; they differ by the 4 MiB the OS occupies
below `membot` and the `.dma` block above `memtop`. No program can observe
this: it is a screen line, not an interface — `Sysconf`, `Malloc(-1)` and the
memory-descriptor chain are untouched and stay EmuTOS-faithful.


**The AES's keyboard poll does not read through `v_string`** — sanctioned
2026-09-06, and it is the *remainder* of a divergence that was open here from
2026-09-04 to 2026-09-06. What was open: EmuTOS' AES reads a keystroke through
`gsx_char()`, which issues `vsin_mode(4, 2)` before every `v_string`
(`references/emutos/aes/gemgsxif.c:459-463`), and `chkkbd()` calls it from the
dispatcher's scheduler on every pass (`aes/gemdisp.c:157-177`), so on any
EmuTOS machine `vqin_mode(4)` reports **1** (SAMPLE) from the first AES tick
onward — measured twice on the reference TT
(`tosdump/reference/EMUTOS14-VDI-r2.BIN` record 16 / case 51,
`EMUTOS14-VDI-all.BIN` record 40 / case 77; devices 1, 2 and 3 read 0 in
records 6, 38, 39). NewTOS reported 0.

**That half is fixed.** `dsptch()` now calls `chkkbd()`, which performs the
same `vsin_mode(4, 2)` write into the same VDI-global state
(`tos-api/src/aes/mod.rs`, `tos-api/src/vdi/input.rs`); `v_opnwk` performs the
`vdimouse_init` reset that goes with it (`vdi/vdi_control.c:563`,
`vdi/vdi_mouse.c:575-578`). Checked against the dump itself by
`tos-api/tests/aes_reference.rs`'s
`aes_dispatch_leaves_the_string_device_in_sample_mode`.

**What stays deliberately different:** the *read*. EmuTOS' `chkkbd` pulls the
key out of the VDI with `v_string` and forks it into the mouse owner's
per-process AES key queue (`forkq(kchange, …)`, `aes/gemdisp.c:172-176`).
NewTOS' AES has no such queue — `evnt_keybd`, the form manager and the file
selector take the key straight from the IKBD ring — which is the same delivery
difference already recorded for the button side in
`tos-api/src/aes/input.rs`. Observable consequence: inside a running GEM
application the AES has *not* already consumed pending keystrokes, so a
`Bconin(2)` from that application still sees them. Closing it means giving the
AES a per-process key queue and is a scheduler change, not an input-mode one.
The mode half — the only part any measured case reaches — now matches.


**GEM trap selector −1 answers `0`, not a dispatcher address** — sanctioned
2026-09-12 with the V16 repair, and recorded here because it was previously
nowhere (audit finding F-2,
`docs/analysis/vdi-r2-improvement-traced-2026-09-12.md`). EmuTOS' `_vditrap`
treats `d0 = -1` (`cmp.w #0xffff,d0`) as *"give me the jsr-able address of the
VDI dispatcher"* and answers `move.l #_GSX_ENTRY,d0`
(`references/emutos/vdi/vdi_asm.S:40,51-52`); the extracted spec carries it
(`docs/api/vdi/_dispatcher.md`, "`d0` special values"). NewTOS has no such
entry point: the VDI is reachable **only** through `svc #2`, and the dispatcher
is EL1 kernel code that an EL0 caller can neither branch to nor read, so there
is no address that would answer the question truthfully. We return `0` — "no
jsr-able address" — which is the value a conforming caller tests for before
using the result, and the value NewTOS has always returned here
(`kernel/src/arch/aarch64/exceptions.rs`, arm `GEM_SEL_ADDR_QUERY`). It is
spelled out as its own trap arm so it cannot silently inherit the neighbouring
"leave `x0` alone" rule, which would hand back `-1` — a value that *looks* like
an address. Handing out a real kernel code address instead would be both
useless (EL0 cannot execute it) and a kernel-address leak. Covered by
`kernel/src/selftest_gemtrap.rs`.

**Not** a sanctioned deviation, and repaired in the same change: selector `-2`
(and every other unknown selector). There EmuTOS leaves `d0` untouched, and so
does NewTOS now — `vq_gdos()` reports *no* GDOS and `vq_vgdos()` returns `-2`,
as on Atari TOS.

**Latent width mismatch, no repair yet (A53, V16 follow-up):** EmuTOS compares
the selector as a **16-bit word** (`cmp.w #0x73,d0` / `cmp.w #0xffff,d0`,
`references/emutos/vdi/vdi_asm.S:33-52` — only the low-order WORD of `d0`
matters). NewTOS's arm (`kernel/src/arch/aarch64/exceptions.rs`, the
`TrapClass::Gem` `match args.regs[0]`) compares the **full 64-bit** `x0`.
Harmless today: no caller in the tree sets bits above the low word
(`grep -rn 'svc[[:space:]]*#2'` over `kernel tos-api sdk tosdump tools` turns
up none). It becomes relevant the day a real m68k binary is run through a
thunk that has to map its 32-bit `d0` into `x0` — if that mapping ever leaves
stale bits above bit 31 (e.g. a zero/sign-extension bug, or `x0` reused
across calls without being fully cleared), a selector NewTOS should recognise
(`115`, `200`, `-1`, `-2`) would instead fall through to the "leave `x0`
unchanged" arm here. No ticket — nothing to fix before the m68k thunk exists —
tracked as a comment at this entry per `TASK/A53.md` (V16 Nachtrag).


**`wind_get(WF_SCREEN)` answers FALSE when there is no menu/alert buffer;
EmuTOS answers TRUE with address 0** — sanctioned 2026-09-12 with the A54
repair, **narrowed 2026-09-20 with A57 to the buffer-less fallback**. EmuTOS'
field 17 hands out the AES' menu/alert screen-save buffer: `gsx_malloc` takes
it once when the AES goes graphic (`references/emutos/aes/gemgsxif.c:108-114`,
from `sh_tographic`, `aes/gemshlib.c:236`), `gsx_mret` reports its address in
`poutwds[0..1]` and its length in `poutwds[2..3]` (`:125-129`), and `wm_get`
returns TRUE (`aes/gemwmlib.c:1563-1565`). Measured on the reference TT
running the 1.4 ROM: address `0x01000218`, length `40960`
(`measurement-2026-09-12-aes-r4/`, cases 331, 332 and 439 of
`tosdump/cases/aes-r4-subfunctions.tsv`).

Since A57 NewTOS has that buffer (`tos-api/src/aes/gsxif.rs`: one
kernel-owned block of the BDOS pool, taken by the same `Mxalloc(…,
MX_PREFTTRAM)` from `sh_tographic`, used by the alert and the drop-down, handed
to an EL0 process read-write through its own address space) and answers
`WF_SCREEN` TRUE with the two LONGs, as EmuTOS does. What remains of this
entry is the one state in which `gsx_malloc` has nothing to report — the pool
could not supply the block at boot (`ENSMEM`). EmuTOS then leaves
`gl_tmp.fd_addr == NULL` and `gsx_mret` reports it: **TRUE, address 0**, a
pointer the program will scribble through. NewTOS answers **FALSE and writes
nothing** — `wm_get`'s own idiom for a field it cannot serve (`default: return
FALSE`, `aes/gemwmlib.c:1590-1591`), the same repair AUD-P5-014 got for
`WF_KIND`; a conforming caller tests the return code before using the
address, so FALSE is the one answer that cannot be misread as "here is a
buffer, at 0". The alert and the drop-down keep working in that state through
a per-save kernel-heap copy (the pre-A57 path), which no program can observe.
Covered by `tos-api/tests/aes_window_screen_buffer_absent.rs`; the round-1
reference case 166 compares directly again (the `SANCTIONED` list in
`tos-api/tests/aes_reference/harness.rs` is empty).


**The AES menu/alert screen buffer is one screen in device pixels, not
`MENU_BUFFER_SIZE` character cells** — sanctioned 2026-09-20 with A57
(design: `docs/analysis/a57-aes-screen-buffer-design-2026-09-20.md`). EmuTOS
sizes the buffer `gl_mlen = MENU_BUFFER_SIZE * gl_wchar * gl_hchar *
gl_nplanes / 8` with `MENU_BUFFER_SIZE = 640` cells ("somewhat arbitrary",
`aes/gemgsxif.c:44-73,98-104`) and, when a rectangle does not fit, clips the
saved **height** — "adjust height to fit buffer: this will leave droppings!"
(`:334-343`): the bottom of the box is never saved and never put back. At
NewTOS' profile (8×16 cell, 16 planes) the formula gives 163 840 bytes; the
largest `form_alert` the builder allows (three 20-character buttons,
`gemfmalt.c:173,231-239`) needs 172 800 and would overflow it. NewTOS takes
`stride16(W) * H * 2` bytes — `bb_save_words(W, H) * 4`, the whole screen:
614 400 at 640×480, 4 147 200 at 1920×1080 — from the same pool with the same
`Mxalloc`. Both users clamp their rectangle to the screen (`fm_alert`
`rc_intersect(&gl_rscreen, …)`, `gemfmalt.c:358`; the drop-down box lies on
it), so the overflow path is structurally unreachable; the guard stays as a
counted defence (`gsxif::clipped_saves`). **Observable:** the length
`wind_get(WF_SCREEN)` reports is larger than on a TOS/EmuTOS of the same
resolution (614 400 against 163 840 at 640×480/16 bit; 40 960 on the
four-plane TT). A program that takes the length as an upper bound for its own
saves (TeraDesk `resource.c:307-320`, which deletes thirteen menu entries when
its largest drop-down does not fit) gets more room, never less. Two more
things are internal and unobservable: the block is taken once at
`sh_tographic` and — unlike `gsx_mfree` in `sh_toalpha`
(`aes/gemshlib.c:258`) — never freed, so its address is stable across an alpha
interlude where EmuTOS' may move; and the EL0 view of the block is a
per-process window VA, not one address for all processes (ADR-023, the
bump-cursor window; a program only ever compares against the pointer it was
given). Pop-ups and nested submenus are unchanged: a fresh ST-RAM block per
opening, as `popup_blit` does (`aes/gemmnext.c:136-215`).


**An AES handler that writes fewer than `OUT_LEN` words delivers 0 in the
unwritten words; EmuTOS delivers the uninitialised dispatcher stack there** —
sanctioned 2026-09-19 with A27, after the A15 measurement made the difference
visible. EmuTOS' `xif` builds every result in a **local, uninitialised**
`WORD int_out[O_SIZE]` (`references/emutos/aes/gemsuper.c:432`) and copies
`OUT_LEN` words of it back **unconditionally** (`:444-445`), whatever the
handler filled in. A handler that writes fewer words — `wind_get(WF_COLOR)`
writes `int_out[2..3]` and never `[1]` or `[4]` (`aes/gemwmlib.c:1569-1570`),
the slider fields and `WF_TOP` write `[1]` only (`:1542-1554`), the end of a
rectangle list writes `[3..4]` only (`:1240`), a refused field writes nothing
but the return code (`:1521-1523`, `:1591-1592`), `objc_sysvar`'s set mode
writes nothing but the return code (`aes/gemoblib.c:1355-1390`), an unknown
opcode writes `-1` and nothing else (`gemsuper.c:406-417`) — therefore hands
the caller whatever the supervisor stack held under those words: in practice
the previous AES call's answer. Measured on the reference TT: `1, 31, 63, 0, 0`
and `1, 0, 19, 0, 0` for two end-of-list `WF_NEXTXYWH` calls in one run
(`TASK/A15-STATUS-UNFINISHED.MD`, cases 127/133/204 of
`tosdump/cases/aes-r2b-rlist.tsv`; erratum in `tosdump/cases/ERRATA.md`), and
`graf_handle` with `OUT_LEN 7` answering `…, 0, 1` in `int_out[5..6]`
(case 283 of `aes-r4-subfunctions.tsv`). NewTOS zeroes the same local array
(`tos-api/src/aes/mod.rs`, `dispatch`) and copies back the same
`min(OUT_LEN, O_SIZE)` words, so the unwritten words read **0**. The full
per-opcode inventory — which handlers under-write, on which path, against
which binding — is `docs/analysis/aes-underwriting-handlers-2026-09-19.md`.

Three reasons, and they point the same way:

1. **Applications: no effect.** On real TOS the value is indeterminate; a
   program that reads an unwritten `int_out` word is already broken there and
   depends on its own call history. Conforming programs do not read them
   (QED gates `kreturn` on `event & MU_KEYBD`, `sdk/apps/qed/src/aktion.c:148`).
   And 0 is what the client binding itself *intends* for such a word: gemlib
   pre-zeroes `aes_intout[3..4]` before `wind_get` and `aes_intout[5]` before
   `graf_xhandle` "for AES that doesn't support this extension"
   (`sdk/gemlib/gemlib-0.44.0/a_wind_get.c:355-358`, `a_graf_xhandle.c:48-49`)
   — a pre-zeroing that EmuTOS' unconditional copy-back defeats and NewTOS
   honours.
2. **Measurement: real effect, and it must be read as noise.** Every
   under-writing call whose case declares more `OUT_LEN` than the handler
   writes diffs against the TT reference in exactly those words — the class
   the r4 assessment files as "S3-a, no soll"
   (`docs/analysis/aes-r4-subfunktionen-einordnung-2026-09-12.md` §4, 23
   cases). A comparison harness must state those words itself rather than
   hold them against the recorded stack, as `tos-api/tests/aes_reference/harness.rs`
   already does for `WF_OWNER`. Whoever does not know this reports a defect
   where there is none — which is why the deviation is named here.
3. **Protection boundary: an argument for 0, not merely a tolerance of it.**
   Between the handler and the application NewTOS has an EL0/EL1 boundary
   classic TOS did not have. Copying an uninitialised kernel-stack word to an
   application was harmless on a machine without protection; here it would be
   a kernel-stack leak into EL0 on every under-writing call. Zeroing the local
   array is the boundary being kept, and the same rule the dispatcher applies
   to the inputs (`int_in`/`addr_in` past `IN_LEN`/`AIN_LEN`, A19).

What is **not** changed by this: the copy-back boundary itself. Exactly
`OUT_LEN` words are written, none past it, none at all for `OUT_LEN 0` — the
caller's poison stays outside the declaration on both machines (measured on
every case of every table). Within `OUT_LEN` the caller **never** keeps its
own word, on either machine; a spec sentence claiming "unchanged" for such a
word was wrong for both and has been corrected (`window-graphics.md`, D-095
and the end-of-list note). Covered by
`tos-api/tests/aes_intout_underwrite.rs` (`wind_get(0, WF_COLOR)`,
`graf_handle` at `OUT_LEN 7`, `wind_get(0, WF_HSLIDE)` at `OUT_LEN 5`,
`objc_sysvar` set mode at `OUT_LEN 3`, with a poisoned caller array).


**`rsrc_gaddr`/`rsrc_saddr` refuse an index past the table; EmuTOS hands out
a pointer past it** — sanctioned 2026-09-20 with A50 (user decision, option
(a): match the return rule, keep the bound). EmuTOS' `get_addr`
(`references/emutos/aes/gemrslib.c:112-176`) checks the **type** only:
every defined `R_*` arm computes `rs_hdr + offset + size * index` with no
look at the header's counts (`get_sub`, `:102-106`), and `rs_gaddr` answers
`ret = (*rsaddr != (void *)-1L)` (`:720`) — TRUE for anything that is not
the `default:` arm's `-1L` (`:170-172`). `rs_saddr` stores through the same
address (`:733-738`). NewTOS resolves in a descriptor that knows each
table's count and its target/kernel-alias pair, and answers **not found**
where the computed address would leave the table. Written out, the three
cases where the two machines differ:

1. **Index at or past the table, or a table the resource does not carry**
   (`R_OBJECT` index 1000 on a 3-object resource, r4 case 626; `R_BITBLK`
   on a resource without BITBLKs). EmuTOS: `int_out[0] = 1`,
   `addr_out[0]` = a pointer past the array, and `rsrc_saddr` writes there
   and answers TRUE. NewTOS: `addr_out[0] = 0`, `int_out[0] = 0`;
   `rsrc_saddr` answers FALSE and writes nothing.
2. **Nothing loaded** — no blob the caller's `AESGLOBAL` names (the A39
   contract, `docs/api/aes/resource-fileselector.md` §1 callout item 4).
   EmuTOS re-binds to `ap_rscmem` (`rs_sglobe`, `:673-679`) and parses
   whatever header stands there — with `ap_rscmem == NULL` that is the m68k
   vector table, and every defined type still answers `1` with an address
   computed from it (r4 cases 598–614, 616–621, 626, 627: all 25 measured
   with `rs_hdr == NULL` on both machines, `int_out[0] = 1` on the TT).
   NewTOS: `0`/`0` for every defined type, FALSE from `rsrc_saddr`, no header
   parsed. The one answer that does **not** depend on the header — the
   unknown type's `-1L` — is given on both machines whether or not anything
   is loaded (cases 623–625 are word-for-word since A50).
3. **`rsrc_saddr` through a NULL slot** (`R_STRING`/`R_IMAGEDATA` on a NULL
   free-string / free-image entry). `get_addr` returns the slot's *content*
   (`:159-162`), i.e. 0; `rsrc_gaddr` reports it as found (`0 != -1`) — that
   half NewTOS matches since A50 — but `rs_saddr` then stores `rsaddr` **at
   address 0** and answers TRUE (`:735-738`). NewTOS answers FALSE and
   stores nothing.

Why the bound stays: between the AES and the application NewTOS has the
EL0/EL1 boundary classic TOS did not have. The resource lives in the
process's window at a user VA and the AES reaches it through a kernel alias
(`tos-api/src/aes/resource.rs`, `Slot`); an address past the table names the
next section of the blob or, past its end, memory the blob does not own on
either side, and address 0 is mapped on neither, so `rsrc_saddr`'s store
would corrupt another table, fault at EL1, or scribble across the boundary —
and `rsrc_gaddr`'s pointer would be one the application may not be able to
dereference either. A conforming program never reaches any of the three: the
resource's own counts bound its indices, it loads before it asks, and it
re-points a free string through `R_FRSTR`, not `R_STRING`. The r4
measurement's 25 D-4 cases are all case 2 and stay a recorded difference by
this entry; they must be read as such, not as a defect.

What is **not** deviated, repaired by A50 in the same change: the return
rule. `int_out[0]` follows `ret = (addr != -1)`, so a NULL free-string /
free-image slot answers `1` with `addr_out[0] = 0` (it used to answer `0`),
and an unknown type answers `addr_out[0] = -1` at the caller's pointer width
(`0xFFFFFFFF` ILP32 — the TT's value — all ones LP64) with `0` (it used to
answer `0`/`0`; D-5 of `docs/analysis/aes-r4-subfunktionen-einordnung-2026-09-12.md`).
Covered by `tos-api/tests/aes_rsrc_gaddr_edges.rs` (unknown type with and
without a load, NULL slot, index past the table, absent table, nothing
loaded, `rsrc_saddr` on each).


## EmuTOS is the reference, not the template

CLAUDE.md Rule 1 says EmuTOS source *is* the spec. Through this charter that
means: EmuTOS defines **what a call must do** (the behaviour to match), not
**how NewTOS must build it**. We extract the *contract* from EmuTOS into
`docs/api/`; we then implement it cleanly. We do **not** port EmuTOS C, copy
its internal structures, or mirror its control flow where the result is
unobservable. ("Port `desk/`" wording in older plans means "reimplement the
`desk/` *behaviour*", not "translate the C".)

## System variables — compatible how

TOS exposes global variables at fixed low-memory addresses (`$420`, `$44E`,
`$4BA`, the cookie-jar pointer at `$5A0`, …). NewTOS keeps them
**compatible in value and semantics**; how they are *reached* depends on the
kind of caller:

- **Freshly-written native AArch64 source** reads them through the header
  accessors / BIOS calls (e.g. `Getbpb`, `Drvmap`, cookie-jar lookup,
  `Ssystem`). Same names, same values, same semantics.
- **1:1-compiled TOS ports** (ADR-019) contain the *classic* raw idiom
  (`*(LONG*)0x4F2`, the `$5A0` cookie-scan, `Setexc($5A0/4,-1)`). **B1
  (2026-07-24, commit `3d1c1fe`) populates the classic low-memory sysvars +
  OSHEADER + `$5A0` per-process at their real addresses in each app's EL0 VA**,
  so these raw accesses work — see `docs/analysis/bios-audit/b1-lowmem-feasibility.md`.
- **m68k binaries** (Phase 10 emulator) get the variables at their real
  addresses too: the emulator maps the low-memory region, so direct pokes work.

**Corrected 2026-07-24 (B1):** an earlier version of this section claimed
native code never uses raw low-memory addresses ("not via a raw `*(LONG*)0x4BA`")
and framed a strict two-world split (native → API only, m68k → low memory).
ADR-019 (1:1-compiled ports) + the TeraDesk `$4F2` boot crash refuted that:
native EL0 ports *do* hit the classic addresses, so NewTOS now provides them
per-process (B1). The kernel-struct-behind-BIOS-calls path (Phase-03 R3) remains
the source of truth and the idiom for freshly-written source.

## Native vs m68k struct variants

A struct's *fields and semantics* are contract; its *pointer width* follows
the ABI of the caller. NewTOS structs therefore exist in two faithful
layouts:

- **Native AArch64**: 64-bit pointers (e.g. `OBJECT` is 32 bytes with a
  64-bit `ob_spec`; `TEDINFO`/`ICONBLK`/`BITBLK` use 64-bit pointers). C
  source recompiled for NewTOS sees these via the NewTOS headers.
- **m68k**: the original 24/32-bit layout, seen by emulated binaries; the
  marshaling shim (Phase 10) translates between the two.

Both expose the **same fields with the same meaning**. The width difference
is an ABI detail the header set hides from source.

## The audit rubric — how to cross-check a spec or plan

For every `docs/api/*.md` entry, phase plan, or piece of code, check:

1. **Contract captured?** Are the fixed items (IDs, ABI, codes, struct
   layout, observable behaviour, variable values) stated correctly and
   completely?
2. **No internals smuggled in as contract?** Does it present an EmuTOS
   *implementation detail* (internal struct, algorithm, buffer trick) as if
   a program depended on it? → mark it **free**, don't bind ourselves to it.
3. **Observable vs internal separated?** Is it clear which parts must match
   and which are NewTOS's choice?
4. **Port-drift?** Does the wording say "port/translate EmuTOS" where it
   should say "reimplement the behaviour / contract"?
5. **Both worlds?** Where relevant, does it hold for native *and* m68k
   callers without the API branching on caller type (ARCHITECTURE ABI rule)?

Findings feed back into the spec (`docs/api/`) and the phase plans; ADRs go
in [`MEMORY.md`](MEMORY.md).

## Sanctioned FreeMiNT/TOS4 extensions

NewTOS is a **superset** of strict TOS 1-3: where a later reference (TOS 4 /
FreeMiNT) lifted a restriction and real apps depend on it, NewTOS adopts the
superset behaviour — additively, never breaking a conforming TOS 1-3 program.
Each such divergence is recorded here with its reason and reference.

- **`Fattrib` (GEMDOS 0x43) on directories.** GET (`wrt == 0`) on a folder
  returns the folder's attribute byte with `FA_SUBDIR` (0x10) set, instead of
  TOS 1-3's `EFILNF`. Reason: cflib `path_exists` (used by QED's clipboard init
  and every FreeMiNT/cflib app) probes a directory with
  `Fattrib(path, 0, 0) >= 0 && (attr & FA_DIR)`. Matches FreeMiNT `sys_f_attrib`
  (`references/freemint/sys/dosdir.c:998-1002`). SET on a folder stays rejected
  (`EACCDN`) — stricter than FreeMiNT — so a directory entry is never mutated
  (the #38 corruption guard). Spec: `docs/api/gemdos/file/43-fattrib.md`.

  **Both spellings, with and without a trailing separator** (2026-09-12, Q07).
  cflib's `make_normalpath` appends a separator to every path before it probes
  (`sdk/cflib/cflib-21/fsnorm.c:85-92`), so the caller this extension exists for
  always asks `Fattrib("C:\CLIPBRD\", 0, 0)`. Until 2026-09-12 that spelling
  still answered `EFILNF` and the stated purpose of the deviation was not met.
  FreeMiNT answers it with the folder's attributes (`relpath2cookie` peels the
  empty component, `sys/filesys.c:986-1010`; `fatfs_lookup` resolves the empty
  name as `"."`, `sys/fatfs.c:5753-5764`). The lift is confined to `Fattrib`
  GET and to paths that name a **directory**: a trailing separator on a file
  keeps `EPTHNF` (FreeMiNT's ENOTDIR is the same `-34`), on a missing name keeps
  `EPTHNF`, and the bare drive root `"C:\"` keeps `EFILNF` — FreeMiNT would
  synthesize `FA_DIR` there, NewTOS deliberately does not, and that remains an
  open question rather than a decided one. `Fsfirst`, `Fopen` and `Dcreate` on a
  path ending in a separator are **unchanged** and stay EmuTOS-faithful
  (measured identical on EmuTOS 1.4 and Atari TOS 3.06).

## Deferred optimisations (Phase 2)

Optimisations and modern extensions are **not built until the contract is
clean and verified**, and are kept here so they never blur the contract.
This section is a holding area for ideas — nothing in it changes observable
behaviour for a conforming TOS program.

- _(to be filled in — the user's optimisation ideas go here, each tagged
  with which free-implementation area it touches and confirmed not to alter
  the contract.)_
