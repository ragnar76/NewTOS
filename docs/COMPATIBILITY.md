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


**`wind_get(WF_SCREEN)` answers FALSE — NewTOS has no menu/alert buffer** —
sanctioned 2026-09-12 with the A54 repair, and listed here because the absence
is real and has to be *visible* rather than dressed up. EmuTOS' field 17 hands
out the AES' menu/alert screen-save buffer: `gsx_malloc` takes it once at AES
start (`references/emutos/aes/gemgsxif.c:108-114`, from `sh_main`,
`aes/gemshlib.c:236`), `gsx_mret` reports its address in `poutwds[0..1]` and its
length in `poutwds[2..3]` (`:125-129`), and `wm_get` returns TRUE
(`aes/gemwmlib.c:1563-1565`). Measured on the reference TT running the 1.4 ROM:
address `0x01000218`, length `40960` (`measurement-2026-09-12-aes-r4/`, cases
331, 332 and 439 of `tosdump/cases/aes-r4-subfunctions.tsv`).

NewTOS has no such region, and this is a statement about the code, not an
intention: the modal alert allocates a fresh buffer per alert and drops it on
close (`tos-api/src/aes/form.rs`, `BbSave::capture`), the menu keeps **two**
independent drop-down backings that are cleared and regrown to whatever box is
opening and start at capacity zero (`tos-api/src/aes/menu.rs`, `MBACK` /
`SMBACK` / `save_under`), and nothing anywhere is the `gsx_malloc` counterpart.
There is also no address NewTOS *could* hand out: those buffers live in the
kernel heap, which is mapped `AP_RW_EL1` — R/W at EL1, **no EL0 access**
(`kernel/src/arch/aarch64/mmu.rs:82,178-179,283-284`) — so an application
writing through a WF_SCREEN pointer, which is the whole point of the field,
would fault on the first store.

What NewTOS did until A54 was worse than the gap: four zero words and `ret = 1`.
A caller cannot tell that apart from "here is a buffer, it starts at 0" — it
reads the return code, believes it, and scribbles on the null page. It now
answers **FALSE and writes nothing**, which is `wm_get`'s own idiom for a field
this AES does not implement (`default: return FALSE`, `aes/gemwmlib.c:1590-1591`)
and the same repair AUD-P5-014 got for `WF_KIND`. A conforming caller tests the
return code before using the address, so FALSE is the one answer that cannot be
misread.

This entry is a **gap that is answered honestly, not a permanent decision**: it
goes away the day the AES gets a real persistent scratch buffer in
EL0-reachable memory, used by the alert save-under *and* the menu save-under —
a memory-pool and mapping question, not a `wind_get` one. Covered by
`tos-api/tests/aes_window_screen_buffer.rs`; round-1 reference case 166 is held
against it by name in `tos-api/tests/aes_reference/harness.rs` (`SANCTIONED`),
so NewTOS drifting back to `ret = 1` turns that case red again.


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
