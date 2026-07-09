# TOS Discrepancy Log

> Phase 00 deliverable D11. Every behaviour difference found between
> EmuTOS source (the primary source) and Compendium / Profibuch
> documentation. Append-only.
>
> Format per entry: `D-NNN`, three-digit sequential.
>
> **NewTOS rule:** when EmuTOS and documentation disagree, NewTOS
> follows EmuTOS unless an ADR overrides this for a specific function.

---

## D-001: `Cconout` (GEMDOS 0x02) return value

**Date:** 2026-05-02
**Phase 00 spec:** `docs/api/gemdos/console/02-cconout.md`

**EmuTOS:** `bdos/console.c:xconout()` returns `1` (LONG)
unconditionally after calling `tabout()`.

**Compendium:** Variously documented as returning the function code (2)
or "undefined". Not consistent across editions.

**Profibuch:** "kein Rückgabewert" / value is undefined.

**Decision:** NewTOS follows EmuTOS — `Cconout` returns `1`.

---

## D-002: `Crawio` (GEMDOS 0x06) blocking semantics

**Date:** 2026-05-02
**Phase 00 spec:** `docs/api/gemdos/console/06-crawio.md`

**EmuTOS:** `bdos/console.c:xrawio()` with `parm == 0xFF` is
**non-blocking**: it calls `constat()` first; if no character is
available it returns `0L` immediately. This contradicts the most common
documentation, which describes `Crawio` as blocking when reading.

**Compendium:** Says input mode "returns the next character entered, or
0 if no character is available" — actually agrees with EmuTOS, but is
often misread as blocking.

**Profibuch:** Describes as non-blocking (consistent).

**Decision:** NewTOS follows EmuTOS — non-blocking. Distinct from
`Crawcin` (0x07), which **is** blocking.

---

## D-003: `Cauxin`/`Cauxout` typeahead behaviour vs. printer

**Date:** 2026-05-02
**Phase 00 spec:** `docs/api/gemdos/console/03-cauxin.md`,
`04-cauxout.md`, `05-cprnout.md`.

**EmuTOS:** Maintains a typeahead buffer for AUX (index 1, see
`bdos/console.c:46`) but **none** for PRN. `Cauxin` therefore goes
through `Bconin(1)` directly (no buffer integration) — only `Cconin`
(0x01) and `Cnecin` (0x08) consume the typeahead buffer via
`getch()`/`conbrk()`.

**Compendium / Profibuch:** Generally don't mention the typeahead
buffer. Treat `Cauxin` as a plain `Bconin(AUX)` — consistent with
EmuTOS.

**Decision:** NewTOS follows EmuTOS. AUX typeahead structure exists in
`bdos/console.c` but is allocated only and exercised by no current
GEMDOS function. The buffer is reserved for future use; we will keep
it inert in NewTOS.

---

## D-004: Standard-handle redirection (`p_uft[]`)

**Date:** 2026-05-02
**Affected:** all 0x01–0x0B + 0x10–0x13 console functions.

**EmuTOS:** All console functions consult the per-process
`run->p_uft[]` table (`bdos/bdosdefs.h:84`, `NUMSTD = 6`) **before**
falling through to the BIOS device. If `p_uft[stdh]` has been
redirected to a file handle (i.e., `> 0`), the BDOS dispatcher
(`osif()` in `bdos/bdosmain.c:498`) intercepts the call and routes I/O
through the filesystem (`xread`/`xwrite`) instead of calling the
xc/xa/... wrappers. The wrappers themselves only run for the
non-redirected (handle ≤ 0) case.

**Compendium / Profibuch:** Describe standard-handle redirection as a
property of `Fforce` (0x46) / GEMDOS handle 0–5. The interception by
the GEMDOS dispatcher is implementation-detail level and not visible
to applications, so no real discrepancy — just a note that the
"console function" specs need to mention this routing.

**Decision:** NewTOS replicates the dispatcher-level redirection
exactly (see `docs/api/gemdos/_dispatcher.md`).

---

## D-005: `Pterm`/`Pterm0` exit-code width

**Date:** 2026-05-02
**Phase 00 spec:** `docs/api/gemdos/process/4c-pterm.md`

**EmuTOS:** `xterm()` declares `rc` as `UWORD` and stores it directly
into the parent's `p_dreg[0]` (a `LONG` field) via
`run->p_dreg[0] = rc`. The store zero-extends to 32 bits.

**Compendium / Profibuch:** "Return code is a 16-bit value passed in
the low word of D0; the high word is undefined."

**Behavioural difference:** Code that interprets the parent's `Pexec`
return value as a full `LONG` sees a guaranteed-zero high word under
EmuTOS (and NewTOS), whereas under classic Atari TOS the high word
may contain garbage. Programs that mask with `& 0xFFFF` are
unaffected; programs that don't mask see deterministic behaviour
under EmuTOS — strictly an improvement, but technically a divergence
from "high word undefined".

**Decision:** NewTOS follows EmuTOS — zero-extend the exit code into
the LONG return.

---

## D-006: `Ptermres` ignores `xsetblk` return

**Date:** 2026-05-02
**Phase 00 spec:** `docs/api/gemdos/process/31-ptermres.md`

**EmuTOS:** `xtermres()` calls `xsetblk(0, run, blkln)` and
**discards the return value**, then proceeds with `reserve_blocks` +
`xterm` regardless. If `blkln` is invalid (< 256 or > original block
size), the basepage's MD is not actually shrunk; `reserve_blocks`
then orphans the *original* (full) MD, wasting any TPA the program
intended to release.

**Compendium / Profibuch:** Describe `Ptermres` as "shrink TPA to
`blkln` then keep resident", implying that a bad `blkln` returns a
GEMDOS error to the parent.

**Behavioural difference:** EmuTOS silently ignores invalid `blkln`
(no error code, function still does not return). Classic TOS
documentation suggests an error path that does not exist in code.

**Decision:** NewTOS follows EmuTOS — silently ignore. Document the
behaviour but do not introduce an artificial error return.

---

## D-007: `Tsetdate` accepts day 0 / month 0

**Date:** 2026-05-02
**Phase 00 spec:** `docs/api/gemdos/time/2b-tsetdate.md`

**EmuTOS:** `xsetdate()` validates only the upper bounds:
`year > 119`, `curmo > 12`, `day > nday[curmo]`. With `curmo == 0`
the lookup table entry `nday[0] == 0`, so `day == 0` passes the
test (`0 > 0` is false). Likewise `(curmo == 1, day == 0)` is
accepted. The function then tells the BIOS to commit the invalid
date.

**Compendium / Profibuch:** "Sets the system date. Returns ERR if
the date is invalid." Day 0 and month 0 are not valid Gregorian
dates.

**Behavioural difference:** EmuTOS silently accepts (year,0,0) and
(year,m,0) where it should reject. The accepted invalid date then
propagates into FAT directory entries via `Fcreate`, producing
on-disk timestamps that mainstream FAT tools may flag.

**Decision:** NewTOS adds explicit `day >= 1 && curmo >= 1` checks.
This is a strict superset of the documented Compendium behaviour;
no program that passes valid dates is affected.

---

## D-008: XBIOS unimplemented-function return value

**Date:** 2026-05-02
**Phase 00 spec:** `docs/api/xbios/_triage.md`

**EmuTOS:** `xbios_do_unimpl(WORD number) { return number; }` —
`bios/xbios.c`. Calling an unimplemented XBIOS function returns
the (positive) function number, not an error code.

**Compendium / Profibuch:** documented as returning `EINVFN` (-32).

**Atari TOS hardware:** matches EmuTOS — returns the function
number. Compendium documentation is incorrect.

**Impact:** Programs that test `if (Xbios(N, ...) < 0)` to detect
"unimplemented" will think the call succeeded. Defensive code uses
side-effect probing (write a known sentinel, see if it changed)
rather than return-value testing for this exact reason.

**Decision:** NewTOS follows EmuTOS — `xbios_unimpl` returns the
function number. Programs assuming the Compendium value would
already be broken on real Atari hardware.

---

## D-009: `Cconws` (GEMDOS 0x09) return value semantics

**Date:** 2026-05-07
**Phase 00 spec:** `docs/api/gemdos/console/09-cconws.md`

**EmuTOS:** `bdos/console.c:454` `xconws(char *p)` is declared `void`.
The dispatcher returns whatever the C compiler left in `D0` after
the void function — undefined ABI scratch. Spec section explicitly
states "the TOS API contract is that Cconws has an undefined return
value."

**Compendium:** documents the return as "the number of characters
output."

**Profibuch:** agrees with Compendium.

**NewTOS implementation:** returns the byte count actually written
(matching Compendium / Profibuch documentation, not the
EmuTOS-source undefined behaviour). Rationale: (a) the count is
genuinely useful to callers and matches what most TOS programs
expect from Compendium documentation; (b) the Phase-02 self-test
asserts the count, which is a cheap end-to-end-check of the trap
path; (c) replacing it with "undefined scratch" would force every
test to find another verification angle.

**Decision:** NewTOS deliberately diverges from EmuTOS source for
this one function. Programs that check the return value see the
count, just as Compendium claims they should.

---

## D-010: `Maddalt` (GEMDOS 0x14) ST-RAM overlap predicate

**Date:** 2026-05-07
**Phase 00 spec:** `docs/api/gemdos/memory/14-maddalt.md`

**EmuTOS:** `bdos/umem.c:455` `xmaddalt` rejects iff
`start < end_stram`, i.e. anything starting below the end of
ST-RAM is refused (`if (start < end_stram) return -1`). This
includes ranges entirely below ST-RAM start.

**NewTOS implementation:** uses the half-open intersection
predicate `[start, end) ∩ [stram_start, stram_end) ≠ ∅` — only
ranges that *actually overlap* ST-RAM get rejected. Ranges
entirely below ST-RAM are accepted.

**Behavioural difference:** on EmuTOS, calling
`Maddalt(0x100000, 0x10000)` when ST-RAM begins at `0x40414000`
returns `-1`. On NewTOS, the same call accepts the region (it
doesn't overlap ST-RAM).

**Practical impact:** none on supported boards. ST-RAM starts at
the kernel-image end (~0x40414000 on QEMU virt, ~0x80000+kernel
size on RPi4); the address space below is reserved for MMIO and
the kernel image itself, so no caller would legitimately pass
`Maddalt` such a region. The divergence is detectable only by
test code that deliberately targets the specific range.

**Decision:** keep the half-open intersection predicate. Strictly
better semantics (only reject genuinely-overlapping additions),
zero impact on real workloads.

---

## D-011: `Cconout` / `Crawio` write-side return values

**Date:** 2026-05-07
**Phase 00 specs:** `docs/api/gemdos/console/02-cconout.md`,
`06-crawio.md`

**EmuTOS:** `Cconout` returns `1` unconditionally (already
captured as D-001 vs Compendium). `Crawio` write-mode
(`parm != 0xFF`) calls `Bconout(1, parm)` and discards the BIOS
return — D0 holds whatever the dispatcher left, effectively `0`.

**Initial NewTOS implementation (PHASE-02 D3):** drifted from the
spec: `Cconout` returned `E_OK = 0` and `Crawio` write returned
the written byte (`parm as Long`). Caught during the PHASE-05
post-EC4 spec audit (2026-05-07) and corrected. **Cconout** now
returns `1` (matching D-001); **Crawio** write now returns `0`
(matching the `06-crawio.md` "Write: always 0" output table).

**Decision:** spec compliance. The drift never broke the
self-tests because neither is checked, but it would have
surfaced as soon as a Compendium-aware program checked
`Cconout`'s return for "1 means success".

---

## D-012: BIOS disk-vector low-memory addresses (spec-vs-source)

**Date:** 2026-07-03
**Phase 00 specs:** `docs/api/bios/04-rwabs.md`, `09-mediach.md`,
`07-getbpb.md`, `_state.md`

**EmuTOS:** `tosvars.ld` fixes the disk-vector sysvars at:
`hdv_bpb = 0x472`, `hdv_rw = 0x476`, `hdv_boot = 0x47a`,
`hdv_mediach = 0x47e`. Installed by `blkdev_init()` (`bios/blkdev.c:101-103`).

**Prior NewTOS docs (wrong):** `04-rwabs.md` claimed `hdv_rw = 0x47E`;
`09-mediach.md` claimed `hdv_mediach = 0x47A`. Both were swapped/wrong —
`0x47A` is `hdv_boot`, `0x47E` is `hdv_mediach`, and `hdv_rw` is `0x476`.

**Not a Compendium discrepancy** — an internal spec error. Corrected in the
refined docs and catalogued once in `_state.md §1`.

**Decision:** use the `tosvars.ld` addresses. (For native AArch64 these live
in a kernel struct; the physical address only matters to the Phase-10 m68k
emulator.)

---

## D-013: `Mediach` invalid-drive and error returns (spec-vs-source)

**Date:** 2026-07-03
**Phase 00 spec:** `docs/api/bios/09-mediach.md`

**EmuTOS:** `blkdev_mediach()` (`bios/blkdev.c:783`) returns **`EUNDEV` (-15)**
for an out-of-range / invalid drive (`:789-790`), and **passes through the
negative error code** from `flop_mediach`/`disk_mediach` when the probe fails
(`:810-811`). It does not normalise failures to `MEDIAMAYCHANGE`.

**Prior NewTOS doc (wrong):** said an out-of-range drive "returns 0 (pretends
no change)" and that "any underlying I/O failure is collapsed into 'may have
changed' (1)". Both contradict the source.

**Compendium:** documents only the 3-state 0/1/2 return; does not mention
`EUNDEV` or negative pass-through (so this is also a Compendium omission).

**Decision:** follow EmuTOS — return `EUNDEV` for invalid drive and propagate
negative driver errors. Corrected in `09-mediach.md`.

---

## D-014: `Rwabs` flag bits, `numb==0`, and undocumented features

**Date:** 2026-07-03
**Phase 00 spec:** `docs/api/bios/04-rwabs.md`

**EmuTOS:** `r_w` flags (`bios/disk.h:53-61`): bit0 `RW_WRITE`, bit1
`RW_NOMEDIACH`, bit2 `RW_NORETRIES`, bit3 `RW_NOTRANSLATE`, bit7
`RW_NOBYTESWAP`. `blkdev_rwabs()` has **no `EBADRQ` path for `numb==0`** (a
zero count returns `E_OK`). It has two behaviours not in any documentation:
(a) `adr==NULL` on a floppy **sets** `blkdev[dev].mediachange = numb` and
returns 0 (FastCopy-III feature, `bios/blkdev.c:419-428`); (b) a logical
**write to sector 0** arms `forcechange`, so the *next* access returns
`E_CHNG` until `Getbpb` clears it (`:452-453`).

**Prior NewTOS doc (wrong / incomplete):** named bit 3 as "don't retry" and
bit 4 as "don't translate" (off by one; missing `RW_NOBYTESWAP`); claimed
`numb==0` returns `EBADRQ` (no such check exists); omitted both undocumented
features.

**Compendium:** documents the standard flags and the `lfirst` extension; does
**not** document the `NULL`-buffer feature or `forcechange`.

**Decision:** follow EmuTOS — correct flag bits, no `EBADRQ` for zero count,
and reproduce both undocumented behaviours for m68k-binary compatibility
(may be NewTOS-native for source ports). Corrected in `04-rwabs.md`.

---

## D-015: `Getbpb` does not flush the GEMDOS FAT cache

**Date:** 2026-07-03
**Phase 00 spec:** `docs/api/bios/07-getbpb.md`

**EmuTOS:** `blkdev_getbpb()` (`bios/blkdev.c:569`) **resets the media-change
state** (`mediachange = MEDIANOCHANGE`, `forcechange = FALSE`, `:589-590`) and
re-reads/re-parses the boot sector on every call. It does **not** flush any
GEMDOS in-memory FAT cache — that is a separate GEMDOS concern.

**Prior NewTOS doc (wrong):** listed as an edge case that `Getbpb` "also
flushes any GEMDOS in-memory FAT cache for that drive when it detects media
change." No such action is in the BIOS-layer source.

**Decision:** follow EmuTOS — `Getbpb` resets the BIOS media-change flag and
rebuilds the BPB; the FAT-cache flush claim is removed. Corrected in
`07-getbpb.md`.

---

## D-016: `bconin0` (PRT input) is a stub, and MIDI/IKBD swap is bcostat-only

**Date:** 2026-07-03
**Phase 00 specs:** `docs/api/bios/02-bconin.md`, `08-bcostat.md`,
`_state.md`, `_dispatcher.md`

**EmuTOS:** `bconin0` (`bios/parport.c:123`) is `/* not implemented */` and
returns `0` unconditionally. The MIDI/IKBD slot swap exists **only** in the
`bcostat_vec` table (`bcostat_vec[3]=bcostat4`, `[4]=bcostat3`,
`bios/chardev.c:38-41`); the `bconin`/`bconout`/`bconstat` tables are **not**
swapped (handle 3 = MIDI, handle 4 = IKBD there).

**Prior NewTOS docs (wrong):** `02-bconin.md` claimed `bconin0` "reads the
parport status / data lines"; `_dispatcher.md §5` implied the MIDI/IKBD swap
applied to input as well as output.

**Decision:** follow EmuTOS — `bconin0` is a stub returning 0; the swap is
`bcostat`-only. Corrected in the refined docs.

**Note (not a discrepancy):** all four char-device default handlers are
Atari-hardware / `CONF_*` gated (MFP, ACIA, SCC, printer port). NewTOS supplies
equivalent handlers for its own devices; these EmuTOS internals are *behaviour
reference*, not contract.

---

## D-017: `Fopen` valid-mode mask is `0x07`, not `0x77` (spec-vs-source)

**Date:** 2026-07-03
**Phase 00 spec:** `docs/api/gemdos/file/3d-fopen.md`

**EmuTOS:** `xopen` masks the mode with `VALID_FOPEN_BITS = MODE_FAC = 0x07`
(`bdos/fs.h:169,173`; `bdos/fsopnclo.c:225`). Only the low three bits (the
access code `RO_MODE`/`WO_MODE`/`RW_MODE`) survive; the file-sharing bits 4-6
(`MODE_FSM`) and the inheritance bit 7 (`INH_MODE`) are **masked away** — this
EmuTOS build does not implement file sharing / record locking.

**Prior NewTOS doc (wrong):** `3d-fopen.md` claimed the mask was `0x77` and
documented a "sharing mode" in bits 4-6 that `Fopen` accepts. Neither is true
of the source.

**Compendium:** documents the sharing/compatibility modes as part of `Fopen`;
those bits are a real TOS/GEMDOS concept but are **not honoured** by EmuTOS
`1a7484ab` (they never reach the OFD via `Fopen`).

**Decision:** follow EmuTOS — `Fopen` honours only bits 0-2. Corrected in
`3d-fopen.md`. If NewTOS later implements sharing it must widen the mask
deliberately and document it.

---

## D-018: `Fopen`/`Fdelete`/`Fattrib` folder & path error codes (spec-vs-source)

**Date:** 2026-07-03
**Phase 00 specs:** `docs/api/gemdos/file/3d-fopen.md`, `41-fdelete.md`,
`43-fattrib.md`

**EmuTOS:**
- `xopen` returns **`EFILNF`** (-33) when `findit` fails — i.e. even when the
  *directory* part of the path is missing (`bdos/fsopnclo.c:248-249`). It never
  returns `EPTHNF` for `Fopen`.
- `xopen`/`xunlink` scan with `FA_NORM`, which excludes `FA_VOL`/`FA_SUBDIR`, so
  a **folder** of that name is "not found" → `EFILNF`, **not** `EACCDN`
  (`bdos/fsopnclo.c:256,604`).

**Prior NewTOS docs (wrong):** `3d-fopen.md` listed `EPTHNF` as a possible
`Fopen` return; `41-fdelete.md` claimed `Fdelete` on a folder returns `EACCDN`.
Both contradict the source (`Fdelete` on a folder → `EFILNF`; `Fopen` on a bad
path → `EFILNF`).

**Compendium:** documents `EPTHNF`/`EFILNF` generically; the exact collapse to
`EFILNF` is EmuTOS behaviour.

**Decision:** follow EmuTOS. Corrected in the three specs. (`Fattrib` and
`Frename` *do* return `EPTHNF` from their own `findit` failures — that part is
unchanged.)

---

## D-019: `Fsnext`/`Fsetdta` error and default facts (spec-vs-source)

**Date:** 2026-07-03
**Phase 00 specs:** `docs/api/gemdos/file/4f-fsnext.md`,
`docs/api/gemdos/dta/1a-fsetdta.md`, `docs/api/structures/dta.md`

**EmuTOS:** `xsnext` returns **`ENMFIL`** (-49) for *every* exhausted /
uninitialised case — no prior `Fsfirst`, failed search, or already-exhausted DTA
(`bdos/fsdir.c:662-663,669-670`). There is no `EINTRN` path in `Fsnext`. The
per-process DTA pointer field is **`p_xdta`** at basepage offset **0x20**
(`include/bdosdefs.h:79`), defaulting to `p_cmdlin` (offset 0x80,
`bdos/proc.c:375`).

**Prior NewTOS docs (wrong):** `dta.md` claimed "`Fsnext` without a prior
`Fsfirst` returns `EINTRN` (-65) on EmuTOS, `ENMFIL` on Atari TOS" — the source
returns `ENMFIL` in all cases. The old DTA docs also referred to the pointer as
`p_dta`/`p_xdta` inconsistently and to a nonexistent `p_dtabuf`.

**Compendium:** documents `ENMFIL` on exhaustion — consistent with EmuTOS; the
claimed `EINTRN` divergence never existed.

**Decision:** follow EmuTOS — `ENMFIL` for all exhausted/uninitialised `Fsnext`.
Corrected in `4f-fsnext.md`, `1a-fsetdta.md`, and the `dta.md` cross-reference
note.

---

## D-020: SFT/handle sizing — `OPNFILES = 75`, `NUMHANDLES = 81` (spec-vs-source)

**Date:** 2026-07-03
**Phase 00 specs:** `docs/api/gemdos/file/45-fdup.md`, `3d-fopen.md`,
`docs/api/gemdos/_state.md`

**EmuTOS:** `OPNFILES = 75`, `NUMSTD = 6`, `NUMHANDLES = NUMSTD + OPNFILES = 81`
(`bdos/fs.h:43-45`, `include/bdosdefs.h:61`). `PD.p_uft[NUMSTD]` is an array of
**signed bytes** holding either a negative BIOS handle or a non-standard handle
number; it is **not** an index into a `syshnd[]` array.

**Prior NewTOS docs (wrong):** `45-fdup.md` computed `OPNFILES = NUMHANDLES -
NUMSTD = 40 - 6 = 34`; `3d-fopen.md` and `_state.md` implied 32/40. All were
wrong. `_state.md` also mis-described `p_uft` as pointing "into syshnd".

**Not a Compendium discrepancy** — an internal spec error (Atari TOS 1.04-3.06
also use 75 system-wide open files; TOS 4.04 uses 19 per-process). Corrected in
the three docs; `docs/api/structures/ofd.md` already had the correct 75.

**Decision:** use `OPNFILES = 75`, `NUMHANDLES = 81`.

---

## D-021: `Fcreate` illegal-character set is narrow and create-specific

**Date:** 2026-07-03
**Phase 00 spec:** `docs/api/gemdos/file/3c-fcreate.md`

**EmuTOS:** `contains_illegal_characters` tests against
`ILLEGAL_FNAME_CHARACTERS = " *,:;<=>?[]|"` (`bdos/fsopnclo.c:64`) — space,
`*`, `,`, `:`, `;`, `<`, `=`, `>`, `?`, `[`, `]`, `|`. This is *mostly* the
MS-DOS set **but** Atari additionally allows `+`, and the set does **not**
include `/`, `\`, or `"`. The same test guards `Frename`'s new name.

**Prior NewTOS doc (wrong):** `3c-fcreate.md` listed the rejected set as
`< > : " / \ | ? *` — it wrongly included `"`, `/`, `\` and omitted space, `,`,
`;`, `=`, `[`, `]`.

**Compendium:** describes "invalid filename characters" loosely; the exact set
is EmuTOS's.

**Decision:** follow EmuTOS — reject exactly `" *,:;<=>?[]|"` on create/rename.
Corrected in `3c-fcreate.md` (and referenced by `56-frename.md`).

---

## D-022: `Fcreate` opens the new file `RO_MODE` when `FA_RO` is set (nuance)

**Date:** 2026-07-03
**Phase 00 spec:** `docs/api/gemdos/file/3c-fcreate.md`

**EmuTOS:** `ixcreat` calls `opnfil(..., (fcb->f_attrib & FA_RO) ? RO_MODE :
RW_MODE)` (`bdos/fsopnclo.c:201`) — so a file created with `FA_RO` gets an OFD
whose recorded access code is `RO_MODE`. **However**, EmuTOS does not enforce
the access code on `Fwrite` (`bdos/fsio.c:536-538`), so the returned handle is
still writable; the read-only attribute only blocks a *later* `Fopen` for write.

**Prior NewTOS doc (imprecise):** `3c-fcreate.md` said the handle is "opened
read-write, even if FA_RO is set". The observable result (writable handle) is
correct, but the recorded OFD mode is `RO_MODE`, not `RW_MODE` — the handle is
writable because the mode is *unenforced*, not because it is RW.

**Not a Compendium discrepancy** — a precision fix. Corrected in `3c-fcreate.md`
(edge case 2) and cross-referenced from `3d-fopen.md`/`3f-fread.md`/`40-fwrite.md`.

**Decision:** document the observable contract (writable handle) and the exact
mechanism (mode recorded but not enforced).

---

## D-023: `Frename` details — reserved word, folder `..` update, `EINTRN`, open-file corruption

**Date:** 2026-07-03
**Phase 00 spec:** `docs/api/gemdos/file/56-frename.md`

**EmuTOS:** `xrename(int n, char *p1, char *p2)` (`bdos/fsdir.c:895`):
- The `n` argument is a historical reserved word; the binding hard-codes 0
  (`include/bdosbind.h:53`) and the code ignores it.
- For a **cross-directory folder move** it **does** rewrite the moved folder's
  `..` entry to the new parent's start cluster and fixes the folder's attribute
  byte in the new parent (`bdos/fsdir.c:1043-1064`); failures there return
  **`EINTRN`** (-65).
- It does **not** check for open OFDs on the source; renaming an open file
  leaves the OFD pointing at the erased old entry, which corrupts the directory
  on the later `Fclose` (`bdos/fsdir.c` — no OFD check).

**Prior NewTOS doc (wrong/incomplete):** `56-frename.md` claimed the folder `.`
/`..` entries are **not** updated (the source updates `..`), and omitted the
`EINTRN` error path.

**NewTOS decision (deliberate divergence):** NewTOS will **not** reproduce the
open-file rename corruption — it will check the OFD chain and return `EACCDN`
if the source file has an open OFD. This is a strict safety improvement; a
program relying on the buggy behaviour would already be corrupting directories
on real TOS. Documented in `56-frename.md`; the `..`-update and `EINTRN` facts
are corrected to match the source.

---

## D-024: DTA public time/date/size are native (m68k big-endian) in the buffer

**Date:** 2026-07-03
**Phase 00 specs:** `docs/api/gemdos/file/4e-fsfirst.md`, `4f-fsnext.md`,
`docs/api/structures/dta.md`

**EmuTOS:** `makbuf` copies the found FCB's time/date/size into the DTA's public
fields using `swpw`/`swpl` (`bdos/fsdir.c:1851-1862`), converting from the
**little-endian** on-disk FCB to **native (m68k big-endian)** order. So a
program reading `DTA.d_time`/`d_date`/`d_length` sees big-endian words on m68k,
matching the packed FAT *bit* layout but in host byte order.

**Prior NewTOS doc (incomplete):** `dta.md` described the fields as "FAT packed"
without stating the byte order, which matters for the AArch64 ABI.

**Not a Compendium discrepancy** — a precision fix relevant to NewTOS's
big-endian-ABI marshaling. Corrected in the two `Fsfirst`/`Fsnext` specs and
`docs/api/gemdos/_state.md` §6.

**Decision:** the DTA's public time/date/size are native byte order (big-endian
for m68k callers) — contract for the marshaling shim.

---

## D-025: Drive count is 26 (A:..Z:), `LEN_ZPATH` is 114, and there is no global `curdrv` (spec-vs-source)

**Date:** 2026-07-03
**Phase 00 specs:** `docs/api/gemdos/drive/0e-dsetdrv.md`, `19-dgetdrv.md`,
`docs/api/gemdos/directory/39-dcreate.md`, `3b-dsetpath.md`, `47-dgetpath.md`,
`docs/api/gemdos/_state.md` §4, §5

**EmuTOS:**
- `BLKDEVNUM = 26` (`include/sysconf.h:43`, comment "A: ... Z:"). Drives are
  **A:..Z:**, and `Drvmap()`/`drvbits` use bits 0..25. `xsetdrv` bounds-checks
  `0 <= drv < 26` (`bdos/fsmain.c:422`).
- `LEN_ZPATH = 114` (`include/sysconf.h:51`), so the `Dcreate` total-path cap
  (`LEN_ZPATH-3`) is **111** (`bdos/fsdir.c:226`) and the `Dgetpath` write
  budget (`LEN_ZPATH-3`) is **111** (`bdos/fsdir.c:1245`); callers still
  allocate the full 114-byte buffer.
- The current drive is **per-process** in `PD.p_curdrv` (`UBYTE`, basepage
  offset 0x37, `include/bdosdefs.h:86`). There is **no** global `WORD curdrv`
  variable anywhere in EmuTOS. `PD.p_curdir[NUMCURDIR]` is `UBYTE[26]`
  (`NUMCURDIR = BLKDEVNUM = 26`), each entry an index into `dirtbl[NCURDIR]`
  with `NCURDIR = 40` (`bdos/fs.h:44`).

**Prior NewTOS docs (wrong):** the drive/directory specs and `_state.md` §4/§5
repeatedly used `BLKDEVNUM = 16` / "A:..P:", `LEN_ZPATH = 128` (deriving a
"124-byte cap"), a `p_curdir[16]` array, and a non-existent global
`WORD curdrv` "read by xgetdrv, written by xsetdrv". All contradict the source.

**Not a Compendium discrepancy** — internal spec errors. (The A:..P: figure and
`curdrv` come from original-TOS lore, not this EmuTOS build.) Corrected in all
five specs and `_state.md` §4/§5.

**Decision:** use `BLKDEVNUM = 26`, `LEN_ZPATH = 114` (cap 111), `NCURDIR = 40`,
and the per-process `PD.p_curdrv`/`PD.p_curdir[26]` model.

---

## D-026: `Dfree` returns generic `ERR` (-1) on bad drive, not `EDRIVE`

**Date:** 2026-07-03
**Phase 00 spec:** `docs/api/gemdos/directory/36-dfree.md`

**EmuTOS:** `xgetfree` (`bdos/fsfat.c:337`) calls `ckdrv(drv, TRUE)`; on any
negative return it does `return ERR` (`bdos/fsfat.c:345-346`) — the generic
`ERR = -1` (`include/gemerror.h:20`). `ckdrv` itself distinguishes `EDRIVE`
(-46), `EPTHNF` (-34), and `ENSMEM` (-39) (`bdos/fsdrive.c:89-102`), but
`xgetfree` **discards** the specific code and always returns `-1`.

**Compendium:** documents `EDRIVE` (-46) as the `Dfree` error return.

**Prior NewTOS doc:** already returned `ERR` (-1) in its output table (correct),
but flagged this only as a tentative "candidate D-010" that was never logged;
this entry records it properly. (`Dgetpath`, by contrast, *does* return `EDRIVE`
— `bdos/fsdir.c:1240` — so the two directory-group calls differ on bad-drive
codes.)

**Decision:** follow EmuTOS — `Dfree` returns `ERR` (-1) on any `ckdrv` failure.
Compendium's `EDRIVE` for `Dfree` is a documentation discrepancy. (Whether real
Atari TOS returns `-1` here is not verifiable from `references/` — no
disassembly is present — so that claim is left out; the EmuTOS behaviour is the
contract per Rule 1.)

---

## D-027: `conterm` bits, and the GEMDOS console never reads `conterm` (spec-vs-source)

**Date:** 2026-07-03
**Phase 00 specs:** `docs/api/gemdos/_state.md §7`, and all console specs
`docs/api/gemdos/console/*.md` (the "conterm" cross-references).

**EmuTOS:** `conterm` is a single `UBYTE` system variable at `0x484`
(`include/tosvars.h:38`, `tosvars.ld:70` `_conterm`), initialised to **`7`** by
`kbd_init` (`bios/ikbd.c:1100`). A full-tree grep finds it read in **only two
files**, both in the BIOS layer — never in `bdos/console.c`:
- bit 0 (`& 1`) → keyboard **key click** (`bios/ikbd.c:495,801,845,894`);
- bit 1 (`& 2`) → keyboard **key repeat** (`bios/ikbd.c:475`);
- bit 2 (`& 4`) → **system bell** on Ctrl-G / VT52 (`bios/vt52.c:215`);
- bit 3 (`& 8`) → return the **shift-state byte** in the `Bconin` high word,
  else mask it out (`bios/ikbd.c:279-280`).

In the GEMDOS console layer, echo (`xconin`→`conin`→`conout`) and Ctrl-C /
Ctrl-S / Ctrl-Q / Ctrl-X handling (`conbrk`, `bdos/console.c:206-244`) are
**unconditional** — no `conterm` bit gates any of them.

**Prior NewTOS docs (wrong):** `_state.md §7` gave the bit table as bit 0 =
"`Cconin` echoes input", bit 1 = "Ctrl-S/Ctrl-Q flow control", bit 2 = "Ctrl-C
abort", and stated `conterm` is "read by `xconin`, `xnecin`, `xconrs`". None of
that is true of this EmuTOS build; those meanings appear to have been invented
from original-TOS lore. The console specs that referenced a `conterm`-gated echo
were likewise corrected.

**Compendium:** documents the classic four-bit layout (key click / repeat /
bell / shift-status) — which **matches the EmuTOS code**, so the old NewTOS spec
was wrong against *both* sources.

**Decision:** follow EmuTOS — the four bits mean key click / key repeat /
system bell / shift-state packing, and their effects live in the BIOS keyboard
and screen layer, **not** in the GEMDOS console calls. The bit *values and
semantics* remain contract (a program can poke `0x484` and observe them); the
GEMDOS console echo and Ctrl-key handling are unconditional. Corrected in
`_state.md §7` and every console spec.

---

## D-028: `Crawio` (GEMDOS 0x06) is never redirected to a file (spec-vs-source)

**Date:** 2026-07-03
**Phase 00 spec:** `docs/api/gemdos/console/06-crawio.md`

**EmuTOS:** the `funcs[]` entry for `Crawio` has **`stdio_typ = 0`**
(`bdos/bdosmain.c:161`, `{ F(xrawio), 0, 1 }`). The dispatcher's standard-handle
interception is guarded by `if (typ && fn && ((fn<12) || …))`
(`bdos/bdosmain.c:498`); with `typ == 0` the whole block is skipped for function
6. Consequently the `case 6:` arm *inside* that switch (`:505-508`) is
**unreachable** for the normal dispatch of `Crawio`. `Crawio` therefore always
talks to the console BIOS device (`Bconstat`/`getch`/`Bconout`), even when
stdin/stdout have been `Fforce`d to files.

**Prior NewTOS doc (wrong):** `06-crawio.md` listed a "Standard-handle
redirection" edge case claiming the dispatcher intercepts `Crawio` and does a
one-byte file read (returning `EOF_INDICATOR`) or file write. That path is dead
code for function 6 and never runs.

**Compendium:** does not address dispatcher-level redirection; not a Compendium
discrepancy — an internal spec error.

**Decision:** follow EmuTOS — `Crawio` is never redirected; it is genuinely raw
console I/O. Corrected in `06-crawio.md`. (The other console read/write
functions, which *do* set a non-zero `stdio_typ`, are redirected as before.)

---

## D-029: `Cconin`/`Cnecin`/`Cconrs` echo to the stdin device, not stdout (spec-vs-source)

**Date:** 2026-07-03
**Phase 00 specs:** `docs/api/gemdos/console/01-cconin.md`, `0a-cconrs.md`

**EmuTOS:** `xconin` echoes via `conin(h)` where `h = HXFORM(run->p_uft[0])` —
the **stdin** device (`bdos/console.c:388,401-403`). `xconrs` passes
`HXFORM(run->p_uft[0])` to `cgets` for **both** the read and the echo
(`bdos/console.c:535`). Neither uses `p_uft[1]` (stdout) for the echo. So
redirecting *stdout* does not move the echo; only redirecting *stdin* does. Echo
is also unconditional (no `conterm` gate — see D-027).

**Prior NewTOS doc (wrong):** `01-cconin.md` stated the echoed character "is
also written to the device in `run->p_uft[1]`" and reasoned about stdout
redirection. The echo device is `p_uft[0]`, not `p_uft[1]`.

**Not a Compendium discrepancy** — an internal spec error.

**Decision:** follow EmuTOS — the console read functions echo on the stdin
device. Corrected in `01-cconin.md` and `0a-cconrs.md`.

---

## D-030: GEMDOS allocator lives in `bdos/iumem.c`, not `bdos/mem.c`; there is no `mdalloc` (spec-vs-source)

**Date:** 2026-07-03
**Phase 00 specs:** `docs/api/structures/md.md`, `docs/api/gemdos/memory/*.md`

**EmuTOS:** the user-memory trap wrappers (`xmalloc`/`xmfree`/`xsetblk`/
`xmxalloc`/`xmaddalt`/`srealloc`, `find_mpb`, `umem_init`, `set_owner`) are in
`bdos/umem.c`; the internal allocator (`ffit`/`freeit`/`shrinkit`) is in
`bdos/iumem.c`; the MD/OS-memory meta-allocator (`xmgetmd`/`xmfremd`/`xmgetblk`/
`xmfreblk`) is in `bdos/osmem.c`. There is **no** `bdos/mem.c` (only the header
`bdos/mem.h`) and **no** function named `mdalloc`.

**Prior NewTOS doc (wrong):** `md.md` repeatedly cited `bdos/mem.c:xmalloc`,
`bdos/mem.c:xmfree`, and `bdos/mem.c:mdalloc`; the memory specs said
`ffit`/`freeit` live in `bdos/mem.c`.

**Not a Compendium discrepancy** — internal spec / citation error.

**Decision:** cite `bdos/umem.c` (wrappers), `bdos/iumem.c` (allocator),
`bdos/osmem.c` (MD pool). Corrected across `md.md` and the five memory specs.

---

## D-031: `freeit` does NOT clear `m_own`; `m_own == NULL` is not a "free" flag (spec-vs-source)

**Date:** 2026-07-03
**Phase 00 specs:** `docs/api/structures/md.md`, `docs/api/gemdos/memory/49-mfree.md`,
`4a-mshrink.md`, `docs/api/gemdos/_state.md §8`

**EmuTOS:** `freeit` (`bdos/iumem.c:144-222`) only relinks the MD between
`mp_mal` and the address-sorted `mp_mfl` and coalesces neighbours — it **never**
writes `m_own`. `ffit` sets `m_own = run` on allocation (`bdos/iumem.c:134`),
but a freed MD keeps its stale owner pointer. Nothing in the alloc/free path
reads `m_own` for a free MD (membership is by list + `m_start`), so it is
internally harmless; but `m_own == NULL` is **not** an invariant of free MDs.
`shrinkit` likewise puts the freed tail MD on `mp_mal` without setting `m_own`
before `freeit`ing it (`bdos/iumem.c:242-260`). Only `Pterm`/`Ptermres`
(`free_all_owned`/`reserve_blocks`, `bdos/proc.c:77-101`) consult `m_own`, and
they walk `mp_mal` (allocated MDs), where it is meaningful.

**Prior NewTOS docs (wrong):** `md.md` asserted "Free flow … set `m_own =
NULL`" and the invariant "Every MD on `mp_mfl` has `m_own == NULL` (used as the
free flag)"; `4a-mshrink.md` claimed "the new freed-tail MD has `m_own = NULL`".

**Not a Compendium discrepancy** — internal spec error.

**Decision:** document that `freeit` leaves `m_own` stale; the free flag is list
membership, not `m_own`. NewTOS *may* cleanly clear `m_own` on free (a permitted
improvement — it does not rely on the stale value). Corrected in the four docs.

---

## D-032: `Maddalt` first add sets `pmdalt.mp_mal = NULL`, not `mp_rover` (spec-vs-source)

**Date:** 2026-07-03
**Phase 00 spec:** `docs/api/gemdos/memory/14-maddalt.md`

**EmuTOS:** on the first successful `xmaddalt`, the one-time init writes
`pmdalt.mp_mfl = md; pmdalt.mp_mal = NULL; has_alt_ram = 1`
(`bdos/umem.c:509-514`); `mp_rover` is never touched. The merge path extends an
adjacent free MD in place and consumes no MD (`bdos/umem.c:472-483`).

**Prior NewTOS doc (wrong):** `14-maddalt.md` carried a pasted C body showing
`pmdalt.mp_rover = md; has_alt_ram = TRUE;` on the first add — a fabricated
line; the source sets `mp_mal = NULL`, not `mp_rover`.

**Not a Compendium discrepancy** — the pasted "source extraction" C in the old
spec did not match the actual `bdos/umem.c`.

**Decision:** follow the actual source (`mp_mal = NULL`). Also removed the
pasted C bodies from all five memory specs per the sharpened cite-never-paste
rule (`_template.md`). Corrected in `14-maddalt.md`.

---

## D-033: `Malloc(-1)` largest-free scope depends on `PF_TTRAMMEM`; `Mxalloc(-1)` is per selected pool(s) (spec-vs-source)

**Date:** 2026-07-03
**Phase 00 specs:** `docs/api/gemdos/memory/48-malloc.md`, `44-mxalloc.md`

**EmuTOS:** `xmalloc` forwards `Malloc(-1)` to `xmxalloc(-1, mode)` where `mode`
is `MX_STRAM` (no `PF_TTRAMMEM`) or `MX_PREFTTRAM` (with it)
(`bdos/umem.c:120-126`). In `xmxalloc`, `amount == -1` returns `ffit(-1,&pmd)`
for `MX_STRAM`, `ffit(-1,&pmdalt)` for `MX_TTRAM`, and the **larger of both
pools'** biggest single free block for the two preferred modes
(`bdos/umem.c:247-278`). It is the largest *single* MD, never the sum of free
space. With `CONF_WITH_ALT_RAM == 0`, `MX_TTRAM` (mode 1) query falls to
`default → NULL`.

**Prior NewTOS docs (imprecise/wrong):** `48-malloc.md` said `Malloc(-1)`
returns "the largest free block across both pools" unconditionally — true only
for `PF_TTRAMMEM` programs; a plain program's `Malloc(-1)` reports ST-RAM only.

**Not a Compendium discrepancy** — a precision fix.

**Decision:** document the `PF_TTRAMMEM`-dependent scope for `Malloc(-1)` and
the per-selected-pool(s) scope for `Mxalloc(-1, mode)`. Corrected in both specs.

---

## D-034: `Ptermres`/`Pterm` process specs cited a nonexistent `bdos/mem.c` and `NUMCURDIR = 16` (spec-vs-source)

**Date:** 2026-07-03
**Phase 00 specs:** `docs/api/gemdos/process/31-ptermres.md`,
`4c-pterm.md`

**EmuTOS:**
- `Ptermres`'s TPA shrink is `xsetblk(0, run, blkln)` defined in **`bdos/umem.c`**
  (`bdos/umem.c:170`) — the same helper `Mshrink`/0x4A uses. There is **no**
  `bdos/mem.c` in the tree (only the header `bdos/mem.h`), consistent with D-030.
  `xsetblk` returns `EIMBA` (block not in a pool) / `EGSBF` (`blkln` >
  current block) / `E_OK`; `xtermres` **discards** this result (already D-006).
- `Pterm`'s current-directory teardown loops `NUMCURDIR = BLKDEVNUM = 26`
  (`include/bdosdefs.h:62`, `include/sysconf.h:43`), not 16.

**Prior NewTOS docs (wrong):** `31-ptermres.md` located `xsetblk` in
`bdos/mem.c`; `4c-pterm.md`'s configuration note said `NUMCURDIR = 16`. Both
contradict the source (the `NUMCURDIR = 16` figure is the same original-TOS
lore corrected for the drive group in D-025).

**Not a Compendium discrepancy** — internal spec / citation errors.

**Decision:** cite `bdos/umem.c` for `xsetblk`/`set_owner`; use `NUMCURDIR = 26`.
Both process specs were also migrated from the old "paste-the-C-body" format to
the sharpened full-contract template (cite-never-paste; native-AArch64 ABI table;
verification criteria) to match the other groups. Corrected in the two specs.

---

## D-035: `Pexec` never returns `EGSBF`; the load path collapses magic/reloc errors to `EPLFMT` (spec-vs-source)

**Date:** 2026-07-03
**Phase 00 spec:** `docs/api/gemdos/process/4b-pexec.md`

**EmuTOS:** the `xexec` load path (`bdos/proc.c:256-360`, `bdos/kpgmld.c`) yields
exactly: `EINVFN` (unknown mode), `EFILNF` (`ixsfirst` miss), a pass-through of
`xopen`'s error (`EPTHNF`/`ENHNDL`/…), `EPLFMT` (magic ≠ `0x601A`, short header,
or a malformed *present* relocation — `bdos/kpgmld.c:48-63,184-185,265-268`),
`ENSMEM` (no memory for env/TPA, or image too big — `bdos/proc.c:216-300`,
`bdos/kpgmld.c:140-141`), and a pass-through of `Rwabs` I/O errors during load.
It does **not** produce `EGSBF` (-67) — that code originates only in `xsetblk`
(`bdos/umem.c:211`), reached via `Mshrink`/`Ptermres`, never via `Pexec`. It is
also tolerant: a failed read of the relocation-start longword loads the program
**without** relocating rather than failing (`bdos/kpgmld.c:176-178`).

**Prior NewTOS doc (wrong):** `4b-pexec.md` listed `EGSBF` (-67, "bad relocation
table") as a `Pexec` error return. The source uses `EPLFMT` for bad relocation
and never returns `EGSBF` from the exec path.

**Compendium:** historically lists `EGSBF` among `Pexec` errors; that does not
match this EmuTOS load path (so also a Compendium discrepancy). Compendium's tie
of `EGSBF` to `Pexec` appears to come from the shared setblock machinery.

**Decision:** follow EmuTOS — `Pexec` returns `EPLFMT` for bad
magic/format/relocation and never `EGSBF`; relocation-read failure loads
unrelocated. Corrected in `4b-pexec.md`.

---

## D-036: `Pexec`/`init_pd_fields` copies the command tail verbatim + a NUL; it does not write a `\r` or synthesize the length byte (spec-vs-source)

**Date:** 2026-07-03
**Phase 00 specs:** `docs/api/gemdos/process/4b-pexec.md`,
`docs/api/structures/basepage.md` (flagged for correction — out of this pass's
write scope)

**EmuTOS:** `init_pd_fields` (`bdos/proc.c:379-384`) builds `p_cmdlin` by copying
the caller-supplied `tail` string byte-for-byte, stopping at the first NUL or at
`PDCLSIZE (0x80)` bytes, then appending a single **NUL (0x00)** terminator. It
does **not** append a carriage return, and it does **not** itself write or
compute the length byte — the length-prefixed `[len][text…]` framing is whatever
the caller passes in `tail`. A `NULL`/empty `tail` therefore yields
`p_cmdlin[0] = 0` followed by a NUL, **not** `p_cmdlin[1] = '\r'`.

**Prior NewTOS doc (wrong):** `docs/api/structures/basepage.md §p_cmdlin` states
the tail is "terminated by `\r` (0x0D), not NUL", that trailing bytes after the
CR are zeroed, and that a NULL tail gives `p_cmdlin[1] = '\r'`. None of that is
what `init_pd_fields` writes — those are the classic-TOS command-line
*convention* (what a shell typically supplies in `tail`), not what `Pexec`
synthesizes.

**Compendium:** documents the `[length byte][command tail]` convention; the CR
terminator is a DOS/TOS shell convention, not something the OS's basepage builder
adds.

**Decision:** follow EmuTOS — the OS copies `tail` verbatim and NUL-terminates;
the length byte and any CR are the caller's framing. Documented correctly in
`4b-pexec.md` (Inputs → "Command tail format" + Edge case 5). `basepage.md`
§`p_cmdlin` should be corrected in a structures pass to say the OS appends a NUL
(not `\r`) and does not synthesize the length byte; left flagged here since the
structures doc is outside this process-group pass.

---

## D-037: Time/date set goes through **XBIOS** `Settime` (22/0x16), and `Sversion` byte order / version table / OS-header claim (spec-vs-source)

**Date:** 2026-07-03
**Phase 00 specs:** `docs/api/gemdos/time/2a-tgetdate.md`, `2b-tsetdate.md`,
`2c-tgettime.md`, `2d-tsettime.md`, `docs/api/gemdos/version/30-sversion.md`,
`docs/api/gemdos/_state.md` §9, §12

**EmuTOS:**
- `xsetdate`/`xsettime` (`bdos/time.c:122,150`) commit via `Settime(...)`,
  which is **XBIOS function 22 (0x16)** — `#define Settime(a) xbios_v_l(22,a)`
  (`include/xbiosbind.h:40`) — reaching `settime()` (`bios/clock.c:1218`).
  `settime()` updates the GEMDOS globals `current_time`/`current_date` directly
  (`bios/clock.c:1221-1222`) **and** persists to the RTC. The boot seed comes
  from `Gettime()` = **XBIOS 23 (0x17)** (`time_init`, `bdos/time.c:160-165`).
- `Sversion`/`xgetver` (`bdos/bdosmain.c:298`) returns the compile-time
  constant `GEMDOS_VERSION` (`include/config.h:1590`, default `0x2000`), byte
  order **minor in the high byte, major in the low byte** (`config.h:1577-1578`).
  It does **not** read the OS header. The `config.h` enumeration of version
  values starts at `0x1300` (TOS 1.00/1.02); there is **no** `0x0F00` entry.

**Prior NewTOS docs (wrong):** the time specs said the setters call "BIOS
function 22" / "BIOS RTC" (it is **XBIOS** 22); several were still in the old
paste-the-C-body format without the native-AArch64 ABI table or verification
criteria. `30-sversion.md` carried a version table with a bogus `0x0F00` =
"TOS 1.00" row (not in `config.h`) and `_state.md §9` claimed the OS header is
"read by `Sversion`" — `Sversion` reads the `GEMDOS_VERSION` constant, not the
header.

**Not a Compendium discrepancy** — internal spec / citation errors (the BIOS-vs
-XBIOS mislabel, the invented version row, the OS-header conflation). The
day-0/month-0 acceptance remains tracked separately as **D-007** (unchanged).

**Decision:** cite `include/xbiosbind.h:40` (Settime = XBIOS 22) and
`bios/clock.c` (`settime`/`gettime`) for the RTC path; report `GEMDOS_VERSION =
0x2000` with the minor-high/major-low byte order; drop the `0x0F00` row; state
that `Sversion` is independent of the OS-header `os_version`. All five time/
version specs migrated to the sharpened full-contract template (cite-never
-paste; native-AArch64 ABI table; verification criteria) to match the other
groups. `_state.md` gained §12 (date/time globals + version constant).

---

## D-038: `Ssystem` cookie sub-ops — slot-index returns the tag (not `{tag,value}`), not-found is `EERROR`, and set/delete are root-gated (spec-vs-source)

**Date:** 2026-07-03
**Phase 00 spec:** `docs/api/gemdos/154-ssystem.md`
**Source:** FreeMiNT @ `f906416` (EmuTOS has no `Ssystem`; FreeMiNT is the
authority, per `SOURCE-ANALYSIS-PROTOCOL.md` Track A source 2b).

**FreeMiNT:**
- Trap `0x154` → `sys_s_system` (`sys/syscall_vectors.c:473`); signature
  `long s_system(int mode, ulong arg1, ulong arg2)` (`sys/ssystem.c:57`).
- `S_GETCOOKIE` (8) calls `get_cookie(NULL, arg1, (ulong*)arg2)`
  (`sys/ssystem.c:279`). Its by-**index** form (high word of `arg1` == 0)
  returns the **tag** of the Nth cookie, **not** a `{tag,value}` pair
  (`sys/cookie.c:472-482`), and an over-large index returns `EINVAL`
  (`sys/cookie.c:458-462`). A tag search that misses returns `EERROR` (-1)
  (`sys/cookie.c:508-509`). `arg2 == 0` returns the value/tag in d0; `arg2 != 0`
  writes it to `*arg2` and returns `E_OK`. **No privilege** required.
- `S_SETCOOKIE` (9) and `S_DELCOOKIE` (26) are **root only** — non-root returns
  `EPERM` before touching the jar (`sys/ssystem.c:285-289,295-299`).
  `set_cookie` rejects any tag with a zero byte (`EINVAL`,
  `sys/cookie.c:534-541`) and returns `ENOMEM` when the jar is full
  (`sys/cookie.c:574-575`). `del_cookie` returns `EINVAL` when the tag is not
  found (`sys/cookie.c:606-607`) — a *different* not-found code than
  `S_GETCOOKIE`'s `EERROR`.

**Prior NewTOS doc (imprecise):** `154-ssystem.md` said the by-index form
"writes its `{tag,value}` (FreeMiNT copies the entry)" (it returns the **tag**
only), gave the not-found error vaguely as "the not-found error" (it is
`EERROR`/`EINVAL` depending on the op), and omitted the **root gate** + `EPERM`
on set/delete, the `ENOMEM`-when-full path, and the `del_cookie` `EINVAL`.

**Not a Compendium discrepancy** — no Compendium/MiNT PDF is in `references/`
(the in-tree `sys/doc/.../ssystem.html` matches the source). This is a
spec-vs-FreeMiNT-source precision fix.

**Decision:** follow the FreeMiNT source. Corrected `154-ssystem.md`: by-index
returns the tag, not-found codes distinguished (`EERROR` for get, `EINVAL` for
delete), root gate + `EPERM` documented, `ENOMEM`/`EINVAL` set-side errors
tabulated. **Open question (not resolved):** the *numeric* error values NewTOS
returns — FreeMiNT's MiNT-errno set (`EERROR=-1`, `EINVAL=-25`, `EPERM=-38`,
`ENOMEM=-39`, `ENOSYS=-32`) collides with TOS codes (`ENOSYS -32 == EINVFN`;
`EBADARG -64 == ERANGE`; TOS has no `EINVAL`), so the mapping needs an ADR (see
`154-ssystem.md` Open questions).

---

## D-039: XBIOS out-of-range / unimplemented returns the **function number**, not `EINVFN` (spec-vs-source)

**Date:** 2026-07-03
**Phase 00 specs:** `docs/api/xbios/_dispatcher.md`, `_triage.md`,
`_tt-shifter.md`

**EmuTOS:** the shared `biosxbios` trampoline range-checks the function ID and,
when `d0 >= xbios_ent`, branches to `bx_ret_exc` (`bios/vectors.S:1325-1359`)
which `rte`s with **D0 unchanged** — D0 still holds the function number
(`moveq #0,d0; move.w (sp)+,d0`, `:1323-1324`). In-range unimplemented slots
use `xbios_unimpl` → `xbios_do_unimpl(number){return number;}`
(`bios/xbios.c:1251`). **Both** paths return the positive function number.
This is the same fact as D-008, restated for the *out-of-range* path.

**Prior NewTOS docs (wrong):** `_triage.md`'s "Build-profile boundary" and
`_tt-shifter.md`'s "Falcon overlap" said a trap with `ID > LAST_ENTRY (0x57)`
"returns `EINVFN` (-32)". It does not — it returns the function number.
`26-supexec.md` additionally claimed Supexec "returns `EINVFN` today (the
dispatcher catch-all)" — doubly wrong, since Supexec has its own table slot
(0x26) and never reaches the catch-all.

**Compendium:** documents `EINVFN` for unimplemented XBIOS — incorrect against
both the real hardware and EmuTOS (already noted in D-008).

**Decision:** follow EmuTOS — every unimplemented XBIOS call (out-of-range or
padded slot) returns its own function number. Corrected in `_dispatcher.md §3`,
`_triage.md`, `_tt-shifter.md`, `26-supexec.md`.

---

## D-040: `Initmous` type 0 / bad type forces `mousevec = just_rts` (spec-vs-source)

**Date:** 2026-07-03
**Phase 00 spec:** `docs/api/xbios/00-initmous.md`

**EmuTOS:** `Initmous` (`bios/mouse.c:40-122`) installs the mouse vector only
on the success path: `if (retval != 0 && type != 0) { … if (newvec != NULL)
kbdvecs.mousevec = newvec; } else { kbdvecs.mousevec = just_rts; }`
(`:105-121`). So **`type == 0` (disable) and any invalid `type` set
`mousevec = just_rts`** — the current handler is dropped — regardless of
`newvec`. The mode-config bytes for types 1/2/4 are sent only if `param !=
NULL`, and the button/Y-origin bytes are likewise `param`-gated. EmuTOS does
not implement Compendium's mouse `type == 3`.

**Prior NewTOS doc (incomplete):** `00-initmous.md` said `vec == NULL` "leaves
`kbdvecs.mousevec` untouched" and described type 0 only as "disable mouse",
without noting that type 0 / errors overwrite `mousevec` with `just_rts`.

**Compendium:** documents types 0/1/2 and a type 3 EmuTOS does not implement.

**Decision:** follow EmuTOS — document the `just_rts` install on
disable/error, the `param`-gating, and the unimplemented type 3. Corrected in
`00-initmous.md`.

---

## D-041: `Rsconf` query is `speed == -2`, and the MFP return omits `scr` (spec-vs-source)

**Date:** 2026-07-03
**Phase 00 spec:** `docs/api/xbios/0f-rsconf.md`

**EmuTOS:** the default handler `rsconf_mfp` (`bios/serport.c:217-249`)
treats **`baud == -2`** as the "return current baud code" query
(`rs232iorecptr->baudrate`, `:222-223`); `baud == -1` merely means "don't
change the baud". Each of `ucr`/`rsr`/`tsr`/`scr` is applied only when `>= 0`
(`:239-246`). The `ULONG` return packs the **old** registers as
`(ucr << 24) | (rsr << 16) | (tsr << 8)` (`:229`) — the low byte (`scr`) is
**not** included. The SCC handler packs a different subset.

**Prior NewTOS doc (wrong):** `0f-rsconf.md` said "`baud == -1` means query
current" (it is `-2`) and gave the return as `(ucr<<24)|(rsr<<16)|(tsr<<8)|scr`
(the `scr` byte is not in the MFP return).

**Compendium:** documents the baud table and flow modes; the `-2` query and the
exact return packing are EmuTOS specifics.

**Decision:** follow EmuTOS — `-2` is the baud query, `-1` is "keep", and the
MFP return is `(ucr<<24)|(rsr<<16)|(tsr<<8)`. Corrected in `0f-rsconf.md`.

---

## D-042: `Kbrate` defaults are 15/2 (not 25/5) and `struct keytbl` is 30 bytes (spec-vs-source)

**Date:** 2026-07-03
**Phase 00 specs:** `docs/api/xbios/23-kbrate.md`, `docs/api/xbios/10-keytbl.md`,
`docs/api/xbios/_state.md`

**EmuTOS:**
- `kbrate` (`bios/ikbd.c:439-449`) returns `MAKE_UWORD(kb_initial, kb_repeat)`
  (old initial in the high byte, old repeat in the low byte), stores each
  non-negative parameter masked `& 0x00ff`, and treats a negative parameter as
  a query. Power-up defaults are `KB_INITIAL = 15` and `KB_REPEAT = 2`
  (`include/sysconf.h:68-69`, set by `kbd_init`, `bios/ikbd.c:1092-1093`), in
  ~20 ms units.
- `struct keytbl` (`bios/ikbd.h:57-86`) is **30 bytes** with eight fields:
  `norm`, `shft`, `caps`, `altnorm`, `altshft`, `altcaps`, `dead` (a
  `const UBYTE *const *`), and `features` (WORD). `Keytbl` (`:122-134`) sets
  only `norm`/`shft`/`caps` (sentinel `(UBYTE *)-1` = keep) and returns
  `&current_keytbl`.

**Prior NewTOS docs (wrong):** `_state.md §2` gave defaults `kb_initial = 25`
/ `kb_repeat = 5` in 50 Hz units, and described `current_keytbl` as only
`{unshift, shift, caps}`. Both contradict the source (the 25/5 figure is
original-TOS lore).

**Compendium:** documents the packed `kbrate` return + `-1` query; classic-TOS
default figures are often quoted differently — EmuTOS uses 15/2.

**Decision:** follow EmuTOS — defaults 15/2 in 20 ms units, `MAKE_UWORD`
return, `(UBYTE *)-1` sentinel, 30-byte 8-field `struct keytbl`. Corrected in
`23-kbrate.md`, `10-keytbl.md`, `_state.md §2`.

---

## D-043: `IOREC` is 14 bytes input-only; IKBD buffer is 256 bytes; RS-232 uses `EXT_IOREC` (spec-vs-source)

**Date:** 2026-07-03
**Phase 00 specs:** `docs/api/xbios/0e-iorec.md`, `docs/api/xbios/_state.md`

**EmuTOS:** the bare `IOREC` (`bios/iorec.h:20-27`) is **14 bytes**:
`UBYTE *buf; WORD size; volatile WORD head; volatile WORD tail; WORD low;
WORD high` — **input only**. `Iorec(1)` (IKBD) and `Iorec(2)` (MIDI) return
bare `IOREC`s; the IKBD buffer is **256 bytes** (`low=64`, `high=192`), MIDI
128 bytes (`bios/aciavecs.S`). `Iorec(0)` returns `rs232iorecptr`, an
**`EXT_IOREC`** (`bios/serport.h:57-65`): an input `IOREC` **plus** an output
`IOREC` plus `baudrate`/`flowctrl`/`ucr`/`datamask`/`wr5`.

**Prior NewTOS docs (wrong):** `0e-iorec.md` and `_state.md §3` described a
single 16-byte `IOREC` "with input + output ringbuffer" and an IKBD buffer of
128 bytes, and treated all three devices as one shape.

**Compendium:** documents the `IOREC` fields; the input-only vs `EXT_IOREC`
split and the exact sizes are EmuTOS specifics.

**Decision:** follow EmuTOS — 14-byte input-only `IOREC` for IKBD/MIDI,
`EXT_IOREC` for RS-232, IKBD buffer 256 bytes. Corrected in `0e-iorec.md`,
`_state.md §3`.

---

## D-044: `BCONMAP` field order is `{maptab, maptabsize, mapped_device}` (spec-vs-source)

**Date:** 2026-07-03
**Phase 00 specs:** `docs/api/xbios/2c-bconmap.md`, `docs/api/xbios/_state.md`

**EmuTOS:** `BCONMAP` (`bios/serport.h:115-119`) is
`{ MAPTAB *maptab; WORD maptabsize; WORD mapped_device; }` — **`maptab` at
offset 0**. `MAPTAB` (`:106-113`) is 24 bytes: `Bconstat`, `Bconin`,
`Bcostat`, `Bconout`, `Rsconf`, `EXT_IOREC *Iorec`. `Bconmap(-2)` returns
`&bconmap_root`, so the field order is the layout a program walks.

**Prior NewTOS docs (wrong):** `2c-bconmap.md` and `_state.md §3` gave the
order as `{mapped_device, maptabsize, maptab}` — reversed.

**Not a Compendium discrepancy** — an internal spec / layout error.

**Decision:** use `{maptab, maptabsize, mapped_device}`. Corrected in
`2c-bconmap.md`, `_state.md §3`.

---

## D-045: `Bconmap` is unavailable (returns `0x2c`) on a single-serial build (spec-vs-source)

**Date:** 2026-07-03
**Phase 00 spec:** `docs/api/xbios/2c-bconmap.md`

**EmuTOS:** `BCONMAP_AVAILABLE = (CONF_WITH_SCC || CONF_WITH_TT_MFP)`
(`bios/serport.h:18`). When it is false, `bconmap` compiles to a single
`return 0x2c;` (`bios/serport.c:1124`) — the function-number "unimplemented"
signal (D-008). On a single-serial build there is no `MAPTAB` and `Bconmap`
reports "not available".

**Prior NewTOS doc (wrong):** `2c-bconmap.md` said NewTOS "enables
`BCONMAP_AVAILABLE = 1` with a single-entry `MAPTAB`" and that `Bconmap(6)` is
a no-op — presenting a design choice as if it were the source behaviour. The
source default for one serial port is the `0x2c` return.

**Not a Compendium discrepancy** — a spec-vs-source / plan-drift error.

**Decision:** on the single-PL011 NewTOS profile, `Bconmap` returns `0x2c`
(unavailable), matching EmuTOS. Enabling a `MAPTAB` would be a deliberate,
documented NewTOS extension. Corrected in `2c-bconmap.md`.

---

## D-046: `Scrdmp` default `dump_vec` is `just_rts` (no built-in printer dump) (spec-vs-source)

**Date:** 2026-07-03
**Phase 00 specs:** `docs/api/xbios/14-scrdmp.md`, `docs/api/xbios/_state.md`

**EmuTOS:** `scrdmp` (`bios/xbios.c:490-494`) calls `dump_vec` via `protect_v`
and resets `dumpflg = -1`. `dump_vec` (`0x502`) is initialised to `just_rts`
(a no-op) at `bios/chardev.c:84`; EmuTOS ships **no** built-in screen-dump
routine. Alt-Help does `dumpflg++` (`bios/ikbd.c:596`), which the VBL notices
to invoke `dump_vec`. `dumpflg` (`0x4EE`) defaults to `-1` (`bios/bios.c:357`).

**Prior NewTOS docs (imprecise):** `14-scrdmp.md` / `_state.md §5` said the
default `dump_vec` is `dmpfunc` that "writes the printer-byte stream" — there
is no such default; it is `just_rts`.

**Not a Compendium discrepancy** — a precision fix.

**Decision:** default `dump_vec = just_rts` (no-op); a NewTOS default dumper
(e.g. PNG) would be an install-time choice. Corrected in `14-scrdmp.md`,
`_state.md §5`.

---

## D-047: AES dispatcher DOES return -1 for an unsupported opcode (spec-vs-source)

**Date:** 2026-07-03
**Phase 00 specs:** `docs/api/aes/_dispatcher.md`, `docs/api/aes/application.md`

**EmuTOS:** `crysbind` (`aes/gemsuper.c:77-420`) has a `default:` arm that sets
`unsupported = TRUE` (`:406-408`); after the switch it runs
`fm_show(ALNOFUNC, &opcode, 1)` (unless `opcode == 0`, which is silently
ignored "since some PRGs make this call") and sets **`ret = -1`**
(`:411-417`). So an out-of-range or not-compiled-in opcode returns `-1`, a
defined value. Applies to `appl_yield` (17) when `CONF_WITH_PCGEM == 0`, the
absent `appl_getinfo` (130) / `appl_search` (18), and every gap in the opcode
table (16, 18, 27–29, 49, 57–69, 83–89, 92–99, 115–119, ≥128).

**Prior NewTOS doc (wrong):** `aes/_dispatcher.md §2` stated "There is **no**
'invalid opcode' error path — an out-of-range opcode falls through the switch
and returns an undefined value (the AES is trusted)." The source has an
explicit `default` → `-1` path with an alert.

**Not a Compendium discrepancy** — an internal spec-vs-source error.

**Decision:** follow EmuTOS — unknown/uncompiled opcodes return `-1` (opcode 0
silently, others with an alert). The `-1` return is contract; the `fm_show`
alert is a reference-level UI behaviour NewTOS need not reproduce. Corrected in
`aes/_dispatcher.md §2` and referenced from `aes/application.md`.

---

## D-048: `appl_init` fills `ap_private` (not `ap_ptree`); `appl_getinfo`/`appl_search` absent; AESGLOBAL layout (spec-vs-source)

**Date:** 2026-07-03
**Phase 00 spec:** `docs/api/aes/application.md`

**EmuTOS:** the `APPL_INIT` arm (`aes/gemsuper.c:92-99`) writes exactly:
`AP_VERSION`=`AES_VERSION` (`global[0]`), `AP_COUNT`=1 (`global[1]`),
`AP_ID`=`rlr->p_pid` (`global[2]`), **`AP_PRIVATE`** (`global[3]`) =
`rs_trees[DESKTOP][ROOT].ob_spec`, `ap_planes` (`global[10]`)=`gl_nplanes`, and
`ap_3resv` (`global[11]`) = `&D`. It does **not** write `AP_PTREE` (`global[5]`)
— that is filled later by `rsrc_load`. The `AESGLOBAL` field/offset table is
`aes/gemrslib.h:15-27` + `include/aesdefs.h:147-157`. `appl_getinfo` (130),
`appl_search` (18) and `appl_control` have **no** opcode macro and **no**
dispatch `case` in this build (`include/aesdefs.h:16-24`, `gemsuper.c:88-123`);
they return `-1` (per D-047). `AES_VERSION` is build-dependent
(`include/config.h:1345-1355`) and is **0x0340** in the default build (all four
of `CONF_WITH_3D_OBJECTS`/`MENU_EXTENSION`/`WINDOW_COLOURS`/
`GRAF_MOUSE_EXTENSION` = 1). `CONF_WITH_PCGEM` defaults to 1, so `appl_yield`
(17) is present.

**Prior NewTOS doc (wrong/imprecise):** `aes/application.md` said `appl_init`
sets `ap_private = DESKTOP tree's ROOT ob_spec` **and separately** labelled the
value against `ap_ptree` in the field description; it also did not state that
`ap_planes`/`ap_3resv` are written, nor the exact `AES_VERSION` default, and
listed `appl_getinfo`/`appl_search` as candidate functions rather than
confirming their absence.

**Not a Compendium discrepancy** — a spec precision / completeness fix. (The
`appl_getinfo`/`appl_search` functions are real AES 4.1+/MagiC calls, but they
are not in EmuTOS `1a7484ab`, so cross-check against Compendium is deferred.)

**Decision:** follow EmuTOS — `appl_init` writes `ap_private` (not `ap_ptree`),
plus `ap_planes`/`ap_3resv`; `ap_version` = 0x0340 in the default build;
`appl_getinfo`/`appl_search`/`appl_control` are unimplemented (→ `-1`). If
NewTOS later adds `appl_getinfo` it is a documented extension. Corrected in
`aes/application.md` (with the full `AESGLOBAL` layout table) and
`aes/_state.md`.

---

## D-049: `evnt_multi` slot map, `MU_TOSVALID`/`MU_M3` flag values, and the `int_out[1..6]` return block (spec-vs-source)

**Date:** 2026-07-03
**Phase 00 spec:** `docs/api/aes/event.md`

**EmuTOS:** the event-library slot macros (`include/aesdefs.h:178-224`) fix the
`evnt_multi` (25) parameter layout exactly:
`MU_FLAGS = int_in[0]`, button spec `MB_CLICKS = int_in[1]` / `MB_MASK =
int_in[2]` / `MB_STATE = int_in[3]`, rectangle 1 `MMO1_FLAGS..MMO1_HEIGHT =
int_in[4..8]`, rectangle 2 `MMO2_FLAGS..MMO2_HEIGHT = int_in[9..13]`, timer
`MT_LOCOUNT = int_in[14]` / `MT_HICOUNT = int_in[15]`, message buffer
`MME_PBUFF = addr_in[0]`. The shared **return block** is
`EV_MX = int_out[1]`, `EV_MY = int_out[2]`, `EV_MB = int_out[3]`,
`EV_KS = int_out[4]`, `EV_KRET = int_out[5]`, `EV_BRET = int_out[6]`, with
`int_out[0]` = `RET_CODE` = the fired-event `MU_*` mask. The single-event waits
`evnt_button`/`evnt_mouse` fill `int_out[1..4]` through the `&EV_MX` pointer
(`gemsuper.c:130,133`). The flag values are `MU_KEYBD=0x01`, `MU_BUTTON=0x02`,
`MU_M1=0x04`, `MU_M2=0x08`, `MU_MESAG=0x10`, `MU_TIMER=0x20`,
**`MU_TOSVALID=0x003F`**, **`MU_M3=0x0100` (internal only)**; `ev_multi` is
entered as `ev_multi(MU_FLAGS & MU_TOSVALID, …)`, so an application **cannot**
request `MU_M3` (`gemsuper.c:153`, `aesdefs.h:256-263`).

**Prior NewTOS doc (wrong/imprecise):** `aes/event.md`'s `evnt_multi` `int_in`
table was off for the rectangles/timer (it lumped `int_in[1..3]` as generic
"button clicks/mask/state" and mislaid the M2 rect / timer indices), described
`MU_TOSVALID = 0x3F` only as "all of the above" without noting it is the mask
applied to user flags, and **omitted `MU_M3 = 0x0100`**. It also cited
non-existent return-index names (`EV_MOUT`/`EV_BUTTON`/`EV_MMGPBUFF`); the real
names are `EV_MX/EV_MY/EV_MB/EV_KS/EV_KRET/EV_BRET` = `int_out[1..6]`, and the
last slot is the **button** click count (`EV_BRET`), not "mouse click count".

**Not a Compendium discrepancy** — a spec precision / completeness fix (no PDFs
in `references/`, so the Compendium slot cross-check is deferred).

**Decision:** follow EmuTOS — use the exact `int_in[0..15]` / `addr_in[0]` /
`int_out[0..6]` slot map, the `MU_*` values incl. `MU_TOSVALID = 0x3F` (the
user-flag mask) and internal `MU_M3 = 0x0100`. Corrected in `aes/event.md` with
the full slot tables.

---

## D-050: `evnt_keybd` returns the key in `int_out[0]`; `evnt_dclick` queries via `setit == 0` (not `rate == -1`); dclick default index 3 (spec-vs-source)

**Date:** 2026-07-03
**Phase 00 spec:** `docs/api/aes/event.md`

**EmuTOS:**
- **`evnt_keybd` (20)** returns the packed key code (scancode high byte, ASCII
  low byte) directly in **`int_out[0]` (`RET_CODE`)** via
  `ret = ev_block(MU_KEYBD, 0)` (`gemsuper.c:127`). This differs from
  `evnt_multi`, where the key is delivered in `EV_KRET = int_out[5]`
  (`gemevlib.c:179`). The two calls use different slots for the same datum.
- **`evnt_dclick` (26)** selects get-vs-set by `EV_DCSETIT = int_in[1]`:
  `setit == 0` **reads** the current speed (ignoring `rate`); `setit != 0`
  **sets** `gl_dcindex = rate` (0..4) (`gemevlib.c:300-309`). It **always**
  returns `gl_dcindex`. Compendium sometimes documents the query as
  `rate = -1`; this EmuTOS build keys the query off `setit`, not a `-1` rate.
- The **double-click rate table** is `gl_dcrates[5] = {450, 330, 275, 220, 165}`
  ms (indices 0..4), and the AES sets the **default to index 3** (220 ms) at
  start-up (`ev_dclick(3, TRUE)`, `geminit.c:708`). The returned value is the
  **index (0..4)**, never the ms figure.

**Prior NewTOS doc (incomplete):** `aes/event.md` summarised `evnt_dclick` as
"get (setit=0) or set" without the default-index-3 fact or the rate table
values, did not state that `evnt_keybd`'s key lands in `int_out[0]` (vs
`evnt_multi`'s `int_out[5]`), and did not flag the `rate == -1` vs `setit == 0`
query-selection difference against Compendium.

**Not a Compendium discrepancy for `evnt_keybd`** (an internal completeness
fix); **is a Compendium discrepancy for `evnt_dclick`'s query selector**
(`setit`-driven, not `rate == -1`), with the Compendium cross-check deferred (no
PDFs in `references/`).

**Decision:** follow EmuTOS — `evnt_keybd` key in `int_out[0]`;
`evnt_dclick` query via `setit == 0`; default speed index 3; rate table
`{450,330,275,220,165}` ms; return value is the index. Documented in
`aes/event.md`.

---

## D-051: `menu_*` slot map (`int_in[0]` index / `int_in[1]` state), `menu_ienable` item-bit-15 = redraw flag, and the full `MN_SELECTED` `msg[3..7]` layout (spec-vs-source)

**Date:** 2026-07-03
**Phase 00 spec:** `docs/api/aes/menu.md`

**EmuTOS:**
- **Menu slot map.** Every menu selector puts its **primary index in
  `int_in[0]`** and its **state/flag in `int_in[1]`**: `SHOW_IT`/`ITEM_NUM`/
  `TITLE_NUM`/`MM_PID` = `int_in[0]`; `CHECK_IT`/`ENABLE_IT`/`NORMAL_IT` =
  `int_in[1]` (`include/aesdefs.h:275-283`). The tree is `MM_ITREE = addr_in[0]`,
  the `menu_text` replacement string `MM_PTEXT = addr_in[1]`, the `menu_register`
  title `MM_PSTR = addr_in[0]`.
- **`menu_ienable` item bit 15.** The dispatch is
  `do_chg(tree, ITEM_NUM & 0x7fff, DISABLED, !ENABLE_IT, (ITEM_NUM & 0x8000) != 0,
  FALSE)` (`aes/gemsuper.c:174-176`). `do_chg`'s signature is
  `do_chg(tree, iitem, chgvalue, dochg, dodraw, chkdisabled)`
  (`aes/gemmnlib.c:145-146`), so **bit 15 of the item word is the `dodraw`
  (redraw-now) flag**, not a title/special-item marker; the object index is the
  low 15 bits.
- **Redraw / disabled-guard per call.** `menu_icheck` passes `dodraw=FALSE`
  (never repaints; `gemsuper.c:172`); `menu_tnormal` passes `dodraw=TRUE,
  chkdisabled=TRUE` (repaints, and returns `0` on a disabled title;
  `gemsuper.c:179`, `gemmnlib.c:158-159`); `menu_ienable` passes `chkdisabled=FALSE`.
- **`MN_SELECTED` message layout.** Built by `ct_msgup` →
  `ap_sendmsg(appl_msg, MN_SELECTED, owner, title, item, treehi, treelo, parent)`
  (`aes/gemctrl.c:353-368`, `aes/gemwmlib.c:851-863`): `msg[0]=MN_SELECTED (10)`,
  `msg[1]=sender pid`, `msg[2]=0`, `msg[3]=title obj`, `msg[4]=item obj`, and —
  **with `CONF_WITH_MENU_EXTENSION` (the default build)** — `msg[5]=HIWORD(tree)`,
  `msg[6]=LOWORD(tree)`, `msg[7]=submenu parent`; without the extension `msg[5..7]`
  are 0. A desk-accessory pick sends `AC_OPEN (40)` with the DA id in `msg[4]`
  instead (`gemctrl.c:337-349`).
- **`menu_register` return.** Returns the slot index `0..NUM_ACCS-1` (0–5,
  `NUM_ACCS=6`, `include/sysconf.h:37`), or `-1` on a negative pid / all slots
  used (`aes/gemmnlib.c:789-816`); it stores the caller's string **pointer**, not
  a copy (`:806`).

**Prior NewTOS doc (wrong/incomplete):** `aes/menu.md` placed `ITEM_NUM`/
`TITLE_NUM` at `int_in[1]` and `CHECK_IT`/`ENABLE_IT`/`NORMAL_IT` at `int_in[2]`
(off by one slot); described `menu_ienable`'s `item & 0x8000` as "a
title-bar/special item" flag (it is the redraw flag); did not note that
`menu_icheck` never redraws or that `menu_tnormal` fails on a disabled title;
and gave only `msg[3]=title, msg[4]=item` for `MN_SELECTED`, omitting the
`msg[5..7]` tree-pointer/parent words present in the default build.

**Not a Compendium discrepancy** — a spec precision/completeness fix (no PDFs in
`references/`, so the Compendium slot/`msg[5..7]` cross-check is deferred). The
`menu_ienable` high item bit and the AES-3.3 `msg[5..7]` tree pointer are
documented in Compendium and match this build; verification deferred.

**Decision:** follow EmuTOS — `int_in[0]` = index, `int_in[1]` = state;
`menu_ienable` item bit 15 = redraw flag over a 15-bit index; `menu_icheck`
deferred draw; `menu_tnormal` redraws + guards disabled titles; the full
`MN_SELECTED` `msg[0..7]` layout incl. `msg[5..7]` under
`CONF_WITH_MENU_EXTENSION`; `menu_register` slot `0..5` / `-1`, pointer stored
not copied. Corrected in `aes/menu.md`.

---

## D-052: `objc_*`/`form_*` — no `DRAW3D` state bit, colour-word bit split, `objc_change` slots `int_in[6/7]`, `objc_find` does not skip DISABLED, `form_alert` grammar/return (spec-vs-source)

**Date:** 2026-07-03
**Phase 00 spec:** `docs/api/aes/object-form.md`,
`docs/api/structures/{object,tedinfo,iconblk,bitblk}.md`

**EmuTOS:** `aes/gemoblib.c`, `aes/gemobed.c`, `aes/gemfmlib.c`,
`aes/gemfmalt.c`, `aes/gemobjop.c`, `aes/gemgraf.c:612-628`,
`include/obdefs.h`, `include/aesdefs.h:48-65,305-384`, `aes/gemsuper.c:203-263`.

The prior `aes/object-form.md` (a thin outline) was verified against the
source. The following are the substantive corrections (all map to
desktop-rendering / dialog behaviour):

- **No `ob_state` `DRAW3D = 0x80` bit.** `obdefs.h:100-107` defines state bits
  only through `WHITEBAK = 0x40`; 0x80 is not a standard state. 3D drawing is
  driven by the **`ob_flags` FL3D\* bits** (`FL3DIND=0x200`, `FL3DBAK=0x400`,
  `FL3DACT=0x600`, mask `FL3DMASK=0x600`, `obdefs.h:92-98`), plus `SUBMENU=0x800`.
  The task brief's "DRAW3D 0x80" was incorrect and is **not** documented as a
  state bit.
- **Colour-word bit split (`gr_crack`, `gemgraf.c:612-628`):** border colour
  bits 15–12, text colour 11–8, **writing mode = bit 7** (set → `MD_REPLACE`),
  inside pattern = 3 bits 6–4, inside colour bits 3–0. For box types the
  **border thickness is byte 1 of the LONG `ob_spec`** (`gemobjop.c:57`).
- **`objc_change` slots:** new state = `int_in[6]` (`OB_NEWSTATE`), redraw flag
  = `int_in[7]` (`OB_REDRAW`), clip rect = `int_in[2..5]`
  (`aesdefs.h:352-377`). (The old outline gave the argument order but not the
  slot numbers.)
- **`objc_find` does NOT skip `DISABLED`.** It returns the deepest object that
  is `inside()` and not `HIDETREE` (`gemoblib.c:1080`); disabled objects are
  ignored only at the *form* layer (`fm_button`, `gemfmlib.c:224`). The old
  wording "deepest visible **non-disabled** object" was wrong.
- **`form_alert` grammar/return:** `[icon][msg][buttons]`, icon `0`=none /
  `1`=NOTE / `2`=QUEST / `≥3`=STOP; up to 5 msg lines (max 40, TOS cap 32) and
  3 buttons (max 20, TOS cap 10); `||`/`]]` escape to literal `|`/`]`; return
  is the **1-based** pressed-button number (`gemfmalt.c:163,258-270,395`,
  `fm_strbrk`).
- **`objc_sysvar` (48) is present in the default build** (`CONF_WITH_3D_OBJECTS
  = 1`, `config.h:1259`), returning the 3D indicator/activator/colour globals
  with defaults from `init_3d` (`gemoblib.c:1327-1344`); `AD3DVALUE` is
  get-only = `ADJ3DSTD` (2).
- **`form_dial` rects** occupy `int_in[1..4]` (small) and `int_in[5..8]`
  (large), four WORDs each (`aesdefs.h:315-322`).
- **`OBJECT`/`TEDINFO`/`ICONBLK`/`BITBLK` byte layouts** documented byte-exact
  in `docs/api/structures/` (24/28/34/14 bytes m68k; native 64-bit-pointer
  variants noted).

**Not a Compendium discrepancy** — a spec precision/completeness fix. No PDFs
in `references/`, so the Compendium cross-check of the slot layouts, the
colour-word split, the alert grammar and the `te_pvalid` codes is **deferred**.

**Decision:** follow EmuTOS — no `DRAW3D` state bit (FL3D\* flags instead);
the `gr_crack` bit split above; `objc_change` new-state/redraw at
`int_in[6]`/`[7]`; `objc_find` returns DISABLED objects (form layer filters);
`form_alert` `[icon][msg][buttons]` grammar with `||`/`]]` escapes and 1-based
return; `objc_sysvar` present with `CONF_WITH_3D_OBJECTS=1`. Corrected in
`aes/object-form.md` and the four `structures/*.md` files.

---

## D-053: `wind_*`/`graf_*` — active-title cue direction, `WF_KIND` unsupported, `WM_NEWTOP`/`WM_ONTOP` never emitted, `WF_*` field numbers, `WM_*`/`WA_*` layouts, `graf_watchbox` return, `graf_growbox` slot (spec-vs-source)

**Date:** 2026-07-03
**Phase 00 spec:** `docs/api/aes/window-graphics.md`, `docs/api/aes/_state.md`

**EmuTOS:** `aes/gemwmlib.c`, `aes/gemwrect.c`, `aes/gemgrlib.c`,
`aes/gemctrl.c:63-315`, `aes/gemgraf.c:405-415`, `aes/gemlib.h:38-94`,
`include/aesdefs.h:68-77,89-98,226-253,390-562`, `include/obdefs.h:32-39,136`,
`aes/gemsuper.c:265-360`.

The prior `aes/window-graphics.md` (a compact outline) was verified against the
source. Substantive corrections (all map to real window-redraw behaviour):

- **Active/inactive title cue direction.** The **topped (active)** window's
  `W_NAME` carries the **`IP_2PATT` dither** fill (`TOPPED_COLOR = 0x11a1`); the
  **untopped (inactive)** one is **plain/hollow** (`UNTOPPED_COLOR = 0x1100`)
  (`gemwmlib.c:150-151`). In the non-3D build, CLOSER/FULLER/scroll gadgets are
  built **only** for the topped window. (Confirms — and pins the direction of —
  the task brief's "TOPPED = IP_2PATT dither vs UNTOPPED plain".)
- **`WF_KIND` (1) is NOT supported** by `wm_get`/`wm_set` in this build — both
  fall to `default: return FALSE` (`gemwmlib.c:1591,1736`). Compendium documents
  `WF_KIND`; EmuTOS omits it. (NewTOS open question — trivial to implement since
  `w_kind` is stored.)
- **`WM_NEWTOP` (29) and `WM_ONTOP` (31) are declared but never emitted** by
  `aes/` or `desk/` (grep-confirmed) — TOS4/MultiTOS messages this build does
  not generate. The prior task hypothesis listed them as active; they are not.
- **`WF_*` field numbers** confirmed against source (`aesdefs.h:534-552`):
  `WF_WXYWH` 4, `WF_CXYWH` 5, `WF_PXYWH` 6, `WF_FXYWH` 7, `WF_HSLIDE` 8,
  `WF_VSLIDE` 9, `WF_TOP` 10, `WF_FIRSTXYWH` 11, `WF_NEXTXYWH` 12, `WF_NEWDESK`
  **14** (no field 13), `WF_HSLSIZ` 15, `WF_VSLSIZ` 16, `WF_SCREEN` 17,
  `WF_COLOR` 18, `WF_DCOLOR` 19, `WF_OWNER` 20. The source spells them
  `WF_HSLSIZ`/`WF_VSLSIZ`/`WF_WXYWH`/`WF_CXYWH`/`WF_FXYWH` (Compendium's
  `…SIZE`/`…WORKXYWH`/`…CURRXYWH`/`…FULLXYWH` are the same numbers).
- **`WM_*` message word layouts** (`gemctrl.c:81-315`, `gemwmlib.c:851-904`):
  `WM_SIZED` (27)/`WM_MOVED` (28) carry the **proposed new border rect** in
  `msg[4..7]`; `WM_HSLID` (25)/`WM_VSLID` (26) carry the **new 0..1000 slider
  position** in `msg[4]`; `WM_ARROWED` (24) carries a **`WA_*` sub-action** in
  `msg[4]`; `WM_REDRAW` (20) carries the **dirty rect** in `msg[4..7]`;
  `WM_TOPPED`/`CLOSED`/`FULLED`/`UNTOPPED` carry only `msg[3]=wh`. `WA_*` =
  `UPPAGE 0, DNPAGE 1, UPLINE 2, DNLINE 3, LFPAGE 4, RTPAGE 5, LFLINE 6,
  RTLINE 7` (`aesdefs.h:245-253`).
- **`graf_watchbox` returns the `out` flag, not a literal "1 = released
  inside".** The actioned semantics are in `gemctrl.c:218-220` (CLOSER/FULLER
  fire when it returns TRUE — button up inside the gadget) (`gemgrlib.c:337-355`).
- **`graf_growbox`/`graf_shrinkbox` big rect is `int_in[4..7]` (`GR_I5`)**, one
  word after the small rect at `int_in[0..3]` (`gemsuper.c:275-279`).
- **`graf_mouse` form numbers** confirmed: `ARROW 0` … `OUTLN_CROSS 7`,
  `USER_DEF 255`, `M_OFF 256`, `M_ON 257`, plus `M_SAVE 258`/`M_RESTORE 259`/
  `M_PREVIOUS 260` under `CONF_WITH_GRAF_MOUSE_EXTENSION` (`aesdefs.h:423-438`).
- **`wind_calc` border model** (`gemwmlib.c:1807-1868`): base 1 px/side; title
  band adds to top, INFO adds to top, a vertical bar adds to right, a horizontal
  bar adds to bottom; a **SIZER-only** window still reserves a vertical bar band
  (TOS4 rule). 3D build adds `2·ADJ3DSTD` per band via `adj_wbox`/`adj_hbox`.

**Not a Compendium discrepancy** (except `WF_KIND` and `WM_NEWTOP`/`WM_ONTOP`,
which are genuine EmuTOS-vs-Compendium gaps) — mostly a spec
precision/completeness fix. No PDFs in `references/`, so the Compendium
cross-check of the field numbers, message layouts, `WA_*` codes, and
`graf_mouse` forms is **deferred**.

**Decision:** follow EmuTOS — topped title = `IP_2PATT` dither, untopped =
hollow; `WF_KIND` unsupported here (NewTOS may add it, decide vs Compendium);
`WM_NEWTOP`/`WM_ONTOP` recognised but not emitted (MultiTOS, deferred); the
`WF_*`/`WM_*`/`WA_*`/`graf_mouse` tables above; `graf_watchbox` returns the
`out` flag with `gemctrl` inside-release action; `graf_growbox` big rect at
`GR_I5`; the `wind_calc` border model incl. the SIZER-only vertical band.
Corrected in `aes/window-graphics.md`; window/rect/active-window globals added
to `aes/_state.md`.

---

## D-054: `rsrc_*`/`fsel_*`/`scrp_*`/`shel_*` — `R_*` type table 0–16, `NEW_FORMAT_RSC` bit, `rs_obfix` signed-offset + width-80 rule, `rsrc_gaddr` sole `addr_out`, `rsrc_load`→`sh_find`, `fsel_*` button = `int_out[1]`, `scrp_read`/`write` always-1/no-validate, `shel_write` four modes + `isgem`/`isover`, `shel_envrn` slot swap, shell buffer sizes (spec-vs-source)

**Date:** 2026-07-03
**Phase 00 spec:** `docs/api/aes/resource-fileselector.md`,
`docs/api/aes/scrap-shell.md`, `docs/api/aes/_state.md`

**EmuTOS:** `aes/gemrslib.c`, `aes/gemfslib.c`, `aes/gemsclib.c`,
`aes/gemshlib.c`, `aes/gemsuper.c:308-405`, `include/rsdefs.h:16-39`,
`include/aesdefs.h:80-115,456-465,568-621`, `aes/gemrslib.h:15-27`,
`aes/gemlib.h:97,120-129`, `include/sysconf.h:51-74`, `util/optimize.c:157-172`.

The prior `aes/resource-fileselector.md` + `aes/scrap-shell.md` (compact
outlines) were verified against the source. Substantive corrections:

**Resource manager:**
- **`R_*` type table is 0–16, not 0–15.** `R_FRIMG = 16` (free images) exists
  (`aesdefs.h:594`). Codes 8/11/14 alias 2/3/4 (sub-pointer = struct's first
  field). `R_STRING`(5)/`R_IMAGEDATA`(6) return the **dereferenced data**;
  `R_FRSTR`(15)/`R_FRIMG`(16) return the **address of the pointer slot**
  (`gemrslib.c:112-176`).
- **`rsh_vrsn` is a bit field, not `0 = classic`.** `NEW_FORMAT_RSC = 0x0004`
  bit marks colour-icon files whose true length is a LONG at offset `rsh_rssize`
  (`rsdefs.h:39`, `gemrslib.c:764-772`).
- **`rs_obfix` low byte is signed, and width 80 → full screen.** `char × cell` +
  signed low byte (`>128 → −256`); `ob_w` char `== 80` maps to `gl_width`
  (`gemrslib.c:53-83`). Box `ob_spec` (`G_BOX`/`G_IBOX`/`G_BOXCHAR`) is **not**
  relocated (colour word); `G_CICON` `ob_spec` is an index→pointer
  (`gemrslib.c:622-635`).
- **`rsrc_load` calls `sh_find` first** to locate the file via the shell search
  path (`gemrslib.c:836`) — not just the current dir.
- **`rsrc_gaddr` is the only AES call whose `addr_out[0]` the dispatcher copies
  back** (`gemsuper.c:446-447`); it returns `1`/`0` for valid/invalid **type**
  and does **not** range-check the index (`gemrslib.c:715-721`).

**File selector:**
- **`fsel_*` button return is `int_out[1]` (`FS_BUTTON`), not an `addr_out`**
  (`aesdefs.h:465`). `fs_input`'s **`int_out[0]` is 1 whenever the dialog ran**
  (0 only on NULL path / no memory); the OK/Cancel value is `inf_what(tree,
  FSOK)` = **1 (OK) / 0 (Cancel) / −1** (`gemfslib.c:922`, `optimize.c:157-172`).
- **`fsel_exinput` label is `addr_in[2]` (`FS_TITLE`)**, defaulting to the "Item
  Selector" string when NULL (`gemfslib.c:691-692`).
- Path/mask split via `fs_pspec`/`fs_back`; empty mask → `*.*`; empty path →
  `"<curdrive>:\*.*"`; folders sort first (`\007` prefix); editing the mask
  turns OK into a rescan (`gemfslib.c:199-236,524-532,634-638,762-767`).

**Scrap manager:**
- **`scrp_read`/`scrp_write` always return 1 and never validate/create** — bare
  `strcpy`s to/from `D.g_scrap` (`gemsclib.c:35-62`). The old "returns 0 if
  unset" / "creates it if needed" were both wrong.

**Shell manager:**
- **`shel_write` has four modes, not two:** `SHW_NOEXEC 0`, `SHW_EXEC 1`,
  `SHW_SHUTDOWN 4`, `SHW_RESCHNG 5` (`aesdefs.h:617-621`). The middle/third
  `int_in` words are **`isgem`** (GEM vs char mode) and **`isover`** (shutdown
  level / rez family), not `isgr`/`iscr`/"launch mode" (`gemshlib.c:127-196`).
  Always returns `1`. **`SHW_SHUTDOWN` is `#if CONF_WITH_SHUTDOWN` = default 0**
  → compiled out on the generic build (`config.h:2028-2034`).
- **`shel_envrn`'s output pointer is `addr_in[0]` (a `char **`) and its search
  string is `addr_in[1]`** — the reverse of the old spec (`aesdefs.h:611-612`,
  `gemsuper.c:396`). Returns a pointer *into* `ad_envrn`, or NULL.
- **`shel_find`'s search order** is app-dir → current-dir → drive-root → AES
  `PATH=` (`gemshlib.c:389-472`); it rewrites the buffer in place and returns
  1/0, saving/restoring the caller's DTA.
- **Buffer sizes are contract:** command tail = `CMDTAILSIZE = 128` bytes
  (`memcpy`, not NUL-bounded); shell buffer = `SIZE_SHELBUF = 4192` bytes
  (`gemlib.h:97,129`, `sysconf.h:74`). `shel_get`/`shel_put`/`shel_read`/
  `shel_write` are **unbounded `memcpy`s** — no length clamp.
- **Scrap/shell/rdef/wdef gating:** `scrp_clear`(82)/`shel_rdef`(126)/
  `shel_wdef`(127) are `#if CONF_WITH_PCGEM` (default 1) → `-1` when off.

**Not (mostly) a Compendium discrepancy** — a spec precision/completeness fix.
No PDFs in `references/`, so the Compendium cross-check of the `R_*` codes, the
`rs_obfix` rule, the `fsel_*` button convention, the `shel_write` modes, the
`shel_envrn` slots, and the buffer sizes is **deferred**.

**Decision:** follow EmuTOS — `R_*` 0–16 with the deref/address-of split;
`NEW_FORMAT_RSC` bit + trailing-length; signed-low-byte + width-80 `rs_obfix`;
box `ob_spec` not relocated; `rsrc_load`→`sh_find`; `rsrc_gaddr` sole `addr_out`,
no index check; `fsel_*` button at `int_out[1]` = 1/0/−1, `int_out[0]` = ran;
`fsel_exinput` label at `addr_in[2]`; `scrp_read`/`write` always-1/no-validate;
`shel_write` four modes with `isgem`/`isover`, `SHW_SHUTDOWN` off by default;
`shel_envrn` output `addr_in[0]`/search `addr_in[1]`; `shel_find` app-dir-first
order; 128-byte tail + 4192-byte shell buffer. Corrected in
`aes/resource-fileselector.md` + `aes/scrap-shell.md`; resource/scrap/shell
globals added to `aes/_state.md` §6c. **This completes the AES manager set and
the full Phase-00 API spec re-derivation.**

---

## D-055: `Protobt`'s "executable" flag is a whole-sector checksum trick, not a BRA.S byte write; `disktype` is 0..6, not 0..3 (spec-vs-source)

**Date:** 2026-07-08 (XBIOS re-validation pass, Task #25)
**Phase 00 spec:** `docs/api/xbios/_floppy-stubs.md`

**EmuTOS:** `protobt()` (`bios/floppy.c:690-737`) never writes offset 0x00/0x01
(`struct bs.bra`, `bios/blkdev.h:44`). The "executable" state is controlled
entirely by the boot sector's **512-byte word checksum**
(`compute_cksum()`, `bios/blkdev.c:69-75`, sums all 256 words): `is_exec =
(compute_cksum(b) == 0x1234)` (`:696`); `execflag < 0` keeps the current
`is_exec`; `execflag == 0`/`> 0` force non-exec/exec. Either way `protobt`
recomputes the **last two bytes of the sector** (`cksum[0..1]`, offset
0x1FE/0x1FF) so the whole-sector checksum becomes exactly `0x1234`
(executable) or, "for hyper-compatibility" with real Falcon/TT TOS (source
comment `:723-729`), exactly `0x1235` (non-executable — not "any value ≠
0x1234" as the generic Atari spec allows). Separately, `disktype` selects one
of **7** template entries (`NUM_PROTOBT_ENTRIES = ARRAY_SIZE(protobt_data) =
7`, `bios/floppy.c:672-682`), not 4; `serialno >= 0x01000000` triggers a
**random** serial via `Random()` (`:702-703`), not a literal stamp.

**Prior NewTOS doc (wrong):** `_floppy-stubs.md`'s `Protobt` section said
"`execflag`: 0 = non-bootable (clears bytes 0..1); ≠0 = bootable (sets `0x60
0x1E` BRA.S …)" — a mechanism EmuTOS does not implement — and "`disktype`:
0..3", omitting both the 7-entry range and the `>= 0x01000000` random-serial
rule.

**Compendium:** the generic BRA.S / "any non-0x1234 checksum is
non-executable" description matches the *classic* Atari ST boot-sector
convention; EmuTOS's `protobt()` implements the checksum-only mechanism (no
BRA.S write) and pins the non-executable checksum to `0x1235` specifically,
matching Falcon/TT TOS rather than the generic spec.

**Decision:** follow EmuTOS — document the checksum-based exec mechanism,
the 7-entry `disktype` range, and the random-serial rule. Corrected in
`_floppy-stubs.md`.

---

## D-056: `TT_SHIFTER_BITMASK` is `0x970f`, not `0x0700` (spec-vs-source)

**Date:** 2026-07-08 (XBIOS re-validation pass, Task #25)
**Phase 00 spec:** `docs/api/xbios/_tt-shifter.md`

**EmuTOS:** `#define TT_SHIFTER_BITMASK 0x970f` (`bios/screen.h:33`). The mask
covers bits 0-3 (colour bank), bits 8-10 (resolution — the subset the
mode-code table documents), bit 12, and bit 15 — not just bits 8-10.
`esetshift`/`egetshift` mask the *whole* register with this value
(`bios/screen.c:194-195,215`).

**Prior NewTOS doc (wrong):** `_tt-shifter.md` stated `TT_SHIFTER_BITMASK =
0x0700` ("only bits 8..10 encode the resolution"), understating the real
mask by omitting bits 0-3/12/15.

**Compendium:** not present in `references/`; cross-check deferred.

**Decision:** follow EmuTOS — correct the mask value; keep the resolution
sub-table (still accurate as a subset of the corrected mask). Corrected in
`_tt-shifter.md`.

---

## D-057: `EsetPalette`/`EgetPalette` are `WORD`-returning in EmuTOS (not `void`); most TT-shifter ABI shapes are not in `xbiosbind.h` (spec-vs-source)

**Date:** 2026-07-08 (XBIOS re-validation pass, Task #25)
**Phase 00 spec:** `docs/api/xbios/_tt-shifter.md`

**EmuTOS:** `bios/screen.c:265` / `:290` declare `WORD esetpalette(...)` /
`WORD egetpalette(...)`, with an explicit comment (`:260-263`) that this is a
**deliberate deviation** from the Atari-documented `void` signature: *"if the
TT shifter is not present, it should return the function number in a WORD,
which is the de facto TOS standard for unimplemented xbios functions."* Both
return `0` on success or the function number (`0x54`/`0x55`) when
`!has_tt_shifter` — the same D-008 pattern as the other six TT-shifter calls.
Separately, `include/xbiosbind.h` only defines binding macros for **two** of
the eight TT-shifter opcodes — `EgetShift() xbios_w_v(81)` (`:59`) and
`EsetColor(a,b) xbios_w_ww(83,a,b)` (`:60`); `EsetShift`/`EsetBank`/
`EsetPalette`/`EgetPalette`/`EsetGray`/`EsetSmear` have **no** macro there.

**Prior NewTOS doc (wrong):** `_tt-shifter.md`'s ABI table listed `xbios_v_wwl`
(`void`) for `EsetPalette`/`EgetPalette` and attributed all eight ABI shapes
to `include/xbiosbind.h`, when only two are actually defined there.

**Compendium:** documents `EsetPalette`/`EgetPalette` as `void`, matching the
*declared* Atari API but not EmuTOS's actual C signature/return value.

**Decision:** follow EmuTOS — document the `WORD` return (D-008 pattern) for
`EsetPalette`/`EgetPalette` and correct the ABI-source attribution.
Corrected in `_tt-shifter.md`.

---

## D-058: The NewTOS XBIOS "profile" mixes two mutually-exclusive EmuTOS build branches for the screen group (spec-internal consistency finding)

**Date:** 2026-07-08 (XBIOS re-validation pass, Task #25)
**Phase 00 specs:** `docs/api/xbios/_dispatcher.md`, `_tt-shifter.md`

**EmuTOS:** `include/config.h:2147-2157` — `#if !CONF_WITH_ATARI_VIDEO` /
`#if CONF_WITH_TT_SHIFTER` / `#error CONF_WITH_TT_SHIFTER requires
CONF_WITH_ATARI_VIDEO.` A real EmuTOS build can **never** have
`CONF_WITH_TT_SHIFTER=1` (the TT-shifter opcode table shape `_dispatcher.md`
/ `_triage.md` select as the "NewTOS target profile") together with
`CONF_WITH_ATARI_VIDEO=0` (the "no Atari video" runtime fallback —
`Physbase()==Logbase()`, `Getrez()` returns the `sshiftmod` shadow, `Setcolor`
fakes `return 0`, … — that `02-physbase.md`/`04-getrez.md`/`05-setscreen.md`/
`06-setpalette.md`/`07-setcolor.md`/`25-vsync.md` extract as the NewTOS
profile behaviour).

**Finding:** this is not a spec-vs-source error in any single function's
extraction — each function's documented behaviour is a real, correctly-cited
EmuTOS `#if` branch — but the two groups of specs silently assume **different,
mutually-exclusive** `CONF_WITH_*` combinations without saying so. A reader
combining "table has real `Eset*`/`Eget*` slots" with "`Physbase` falls back
to `Logbase`" would be describing a `#if` combination EmuTOS itself refuses
to compile.

**Decision:** NewTOS deliberately combines the *table-shape* branch (TT
shifter opcodes present) with the *runtime-behaviour* branch (no Atari video
MMIO) because the real target (BCM2711, no Shifter/TT-Shifter hardware at
all) cannot faithfully run either branch's hardware path — both are already
software shadows on NewTOS. Documented explicitly in `_dispatcher.md §4` and
`_tt-shifter.md`'s header so the combination is understood as a NewTOS
choice, not an EmuTOS-attested configuration.

---

## D-059: `struct _protobt` is 19 bytes, not 17; several stale line citations in the `Protobt` section (spec-vs-source)

**Date:** 2026-07-09 (XBIOS re-validation pass, code-audit follow-up)
**Phase 00 spec:** `docs/api/xbios/_floppy-stubs.md`

**EmuTOS:** `struct _protobt` (`bios/floppy.c:662-674`, `PACKED`) totals
**19 bytes**: `bps[2]+spc+res[2]+fat+dir[2]+sec[2]+media+spf[2]+spt[2]+
sides[2]+hid[2]` = 2+1+2+1+2+2+1+2+2+2+2 = 19. This is consistent with the
BPB field range already stated elsewhere in the same doc — `bps..hid`
occupies offset `0x0B-0x1D` in `struct bs` (`bios/blkdev.h:47-57`), and
`0x1D-0x0B+1 = 19`. Separately, several `bios/floppy.c`/`bios/blkdev.c` line
citations in the `Protobt` section had drifted from the current source
revision: `protobt_data[]` (the 7-entry template array) is
`bios/floppy.c:676-687` (`NUM_PROTOBT_ENTRIES = ARRAY_SIZE(protobt_data)`
defined at `:688`, not embedded in the array's own range); the `protobt()`
function itself is `bios/floppy.c:690-736` (not `-737`, a blank line);
`compute_cksum()` is `bios/blkdev.c:69-79` (not `-75`, which cuts off the
closing brace); the `serialno`-handling `if` block is `:698-704` (not
`:701-706`), with the "reversed as per Atari TOS" comment on `:701` (not
`:704`); the `disktype` copy loop is `:706-713` (not `:708-715`); the media
byte literals are on `:679-685` (not `:674-680`); and the "hyper-
compatibility" comment justifying the `0x1235` non-executable checksum is
`:722-726` (not `:723-729`, which runs into unrelated code).

**Prior NewTOS doc (wrong):** `_floppy-stubs.md` stated "the whole
**17-byte** `_protobt` template" — directly contradicting its own correct
`0x0B-0x1D` (= 19-byte) offset range given two sentences earlier in the same
paragraph — plus the stale line citations listed above.

**Compendium:** not present in `references/`; cross-check deferred.

**Decision:** follow EmuTOS — the struct is 19 bytes; the offset range
(`0x0B-0x1D`) was already correct and needed no change, only the "17-byte"
prose. Corrected the prose and every stale citation in `_floppy-stubs.md`'s
`Protobt` section.

---

## D-060: `Setprt` (XBIOS 0x21) is a pure software config word — NewTOS honours it independent of `CONF_WITH_PRINTER_PORT` (spec-vs-source / NewTOS decision)

**Date:** 2026-07-09 (XBIOS re-validation pass, code-audit follow-up)
**Phase 00 specs:** `docs/api/xbios/21-setprt.md` (new), `_dispatcher.md`,
`_triage.md`

**EmuTOS:** `WORD setprt(WORD config)` (`bios/parport.c:49-57`) is pure
software: `old_config = printer_config; if (config != -1) printer_config =
config; return old_config;`. It touches **no hardware register at all** —
the Centronics/PSG output path lives entirely in the separate `prnout()`
helper (`bios/parport.c:62-105`), which `setprt()` never calls.
`printer_config` (`bios/parport.c:36`) defaults to `0` at `parport_init()`
(`bios/parport.c:118`, comment: *"Setprt() default: output via parallel
port"*). However, in EmuTOS both `setprt()` and `prnout()` sit inside the
*same* `#if CONF_WITH_PRINTER_PORT` block (`bios/parport.c:45-106`), and the
`xbios_vecs[]` slot for 0x21 is `xbios_unimpl` when that config is off
(`bios/xbios.c:1332-1336`, returning the function number per D-008).
`CONF_WITH_PRINTER_PORT` defaults to `0` for non-Atari hardware
(`include/config.h:673,707-708`) and to `1` otherwise
(`include/config.h:987-988`) — a literal EmuTOS build for the
`!CONF_ATARI_HARDWARE` profile (the one NewTOS's target most resembles)
would not compile `setprt()` at all.

**Prior NewTOS doc (wrong/missing):** `_dispatcher.md` and `_triage.md`
grouped 0x21/Setprt with the MFP/NVRAM "printer/NVRAM" `xbios_unimpl`
entries (`_dispatcher.md:171` pre-fix, `_triage.md:59` pre-fix), i.e.
treated it as absent hardware in the same category as the floppy/PSG stub
groups. No per-function spec existed.

**Decision — NewTOS deviates deliberately from a literal
`CONF_WITH_PRINTER_PORT=0` port:** `Setprt`'s get/set of `printer_config`
has zero hardware dependency; only `prnout` needs the (absent) parallel
port. Per `docs/COMPATIBILITY.md` ("if a program can read it … through a
documented interface, it is contract"), the save/restore idiom (`old =
Setprt(cfg); …; Setprt(old);`) is part of the observable contract. Falling
back to `xbios_unimpl`'s function-number-return pattern (D-008) would make
`Setprt(-1)` return `33` instead of the stored config word, silently
corrupting that idiom — with no compensating benefit, since honouring the
word costs nothing (a single scalar `WORD`, default `0`, no I/O). This is
distinct from the genuinely hardware-dependent stubs (floppy `EUNDEV`,
PSG `0`/no-op) — those fail because the *operation* needs absent silicon;
`Setprt` needs none. NewTOS therefore implements `Setprt` with the same
store/return contract EmuTOS gives it when `CONF_WITH_PRINTER_PORT=1`, while
still having **no** parallel-port hardware — the same "config variable =
honour, real HW = absence contract" split already applied to `Rsconf`'s
baud-index shadow (`docs/api/xbios/0f-rsconf.md`). Documented in the new
`docs/api/xbios/21-setprt.md`; `_dispatcher.md`/`_triage.md` and
`docs/HARDWARE-PROFILE.md` §3.9 updated to move 0x21 out of the
printer-hardware-absence grouping.

---

## D-061: GEMDOS's unimplemented-function convention is `EINVFN`, the **opposite** of BIOS/XBIOS's function-number return — confirmed, not a spec bug (cross-trap verification)

**Date:** 2026-07-09 (GEMDOS Batch 1 re-validation, Task 1)
**Phase 00 spec:** `docs/api/gemdos/_dispatcher.md` §4

**Context:** D-008/D-039 established that BIOS/XBIOS's `xbios_unimpl` returns
the **function number**, not `EINVFN`, for both an out-of-range ID and an
in-range-but-unimplemented slot — the original XBIOS bug was returning
`EINVFN` where the function number was expected. Before auditing GEMDOS
(trap 1) it had to be confirmed from source whether GEMDOS follows the same
convention or a different one — guessing by analogy from the BIOS/XBIOS
finding would risk "fixing" GEMDOS in the wrong direction.

**EmuTOS (verified against `bdos/bdosmain.c` @ `1a7484ab`):** GEMDOS does
**not** follow the BIOS/XBIOS convention. Both of GEMDOS's unimplemented
paths return `EINVFN` (-32), a literal error code, never the function number:
- **Out-of-range** (`fn > MAX_FNCALL`, i.e. `fn > 0x57`): `osif()` returns
  `EINVFN` directly (`bdos/bdosmain.c:444-445`).
- **In-range, unimplemented slot:** every gap in `funcs[]` is built with the
  `NI` macro (`bdos/bdosmain.c:145` `#define NI F(ni)`), which points
  `f->fncall.v` at `static long ni(void) { return EINVFN; }`
  (`bdos/bdosmain.c:307-310`), reached through the ordinary arity-dispatch
  switch (`bdos/bdosmain.c:667-700`).

Both paths in GEMDOS agree with each other (`EINVFN`, always); both paths in
BIOS/XBIOS agree with each other (function number, always) — but the two
traps use **opposite** conventions from each other. This is a genuine,
verified difference between traps, not an inconsistency in either spec.

**Prior NewTOS doc:** `docs/api/gemdos/_dispatcher.md` §4 already documented
`EINVFN` correctly for both paths before this pass — no factual correction
was needed. What was missing was the explicit cross-trap contrast, now added
to §4, so a future auditor does not "fix" GEMDOS toward the BIOS/XBIOS
function-number convention (or vice versa) by analogy instead of re-reading
each trap's source.

**Decision:** NewTOS's GEMDOS dispatcher returns `EINVFN` for any unknown
function ID (out-of-range or in-range-unimplemented), matching EmuTOS;
NewTOS's BIOS/XBIOS dispatcher returns the function number for the same
cases (D-008/D-039). These are two independently-correct, opposite contracts
— do not unify them.

**Additional finding (same pass):** GEMDOS's dispatcher does have a
per-function argument-count convention relevant to a later code audit: each
`funcs[]` entry (`FND`, `bdos/bdosmain.c:120-133`) carries `wparms` — the
parameter size in 16-bit WORDs (a `LONG`/pointer counts as 2) — and `osif()`
dispatches through a `switch (f->wparms)` arity table (0/1/2/3/4/5/7 slots,
`bdos/bdosmain.c:667-700`) that calls the matching `union fncall` member with
that many `short` arguments pulled from `pw[1]…pw[wparms]`. This table was
cross-checked function-by-function against `bdos/bdosmain.c:142-289` and
matches `_dispatcher.md` §6 exactly; no correction needed.

---

## D-062: `Super` (GEMDOS 0x20) is a new spec, and its NewTOS implementation is grouped with ADR-Supexec (spec gap filled / decision grouping)

**Date:** 2026-07-09 (GEMDOS Batch 1, follow-up)
**Phase 00 spec:** `docs/api/gemdos/process/20-super.md` (new)

**Gap found:** unlike every other function EmuTOS implements in the GEMDOS
`funcs[]` table, `Super` (0x20) had no spec file, because it is intercepted
directly in the trap-1 assembly trampoline (`bdos/rwa.S:159`,
`x20_usr`/`x20_sup`/`x20_inq` at `:280-346`) and therefore never shows up in
a "one spec per `funcs[]` entry" inventory sweep (`_dispatcher.md` §6 already
noted it is "not in `funcs[]`", but no per-function spec existed). Flagged
as a STOP-block in the GEMDOS Batch 1 report; the coordinator confirmed
creating the spec now (documenting EmuTOS's behaviour), while leaving the
NewTOS/AArch64 implementation itself undecided.

**EmuTOS (now documented in full in `20-super.md`):** `Super` toggles the
CPU mode relative to the caller's current mode (never "set to supervisor")
- user-to-super always raises the S-bit, super-to-user always clears it,
with `stack` (0 / 1-inquiry / explicit address) selecting only which stack
pointer becomes active in the destination mode. The supervisor-to-user
direction has a genuine, source-verified D0 asymmetry: the user-to-super
direction explicitly sets D0 to the previous stack pointer
(`bdos/rwa.S:298`, commented "set return value"), but the super-to-user
direction never assigns D0 at all (no `move.l ...,d0` in `x20_sup`/`xs_0`/
`xs_usp`/`xs_exit`/`xs_scram`) - a caller returning to user mode gets no
meaningful D0, matching the classic idiom where only the entering call's
return value is ever used.

**Grouping decision:** `Super`'s contract ("flip my own privilege bit,
using this stack, give me a token to flip it back") requires exactly the
in-process EL0-to-EL1 privilege flip that ADR-Supexec (open, referenced in
`docs/api/xbios/26-supexec.md`) already identifies as unresolved for
`Supexec`. AArch64 has no per-instruction supervisor-bit flip a user
program can trigger on itself (no m68k-SR equivalent writable from EL0), so
whichever of the three candidate mechanisms ADR-Supexec picks (EL1
callback / pseudo-supervise at EL0 / kernel-side pattern translation)
determines what `Super` can mean on NewTOS too. Decision: `Super`'s NewTOS
implementation is deferred and tracked together with ADR-Supexec, not as an
independent ADR - do not implement one without revisiting the other. Until
resolved, `Super` should surface as `EINVFN` only because it has no
dispatcher slot yet (honest "not wired"), never as a fabricated
stack-pointer value, matching how `Supexec` is currently deferred.

**Not a Compendium discrepancy** - a spec-completeness gap plus a
decision-tracking grouping, not a factual disagreement with any source.

---

## D-063: `Fdelete`'s "0xE5→0x05 substitution" claim is fabricated — EmuTOS has no such convention (spec-vs-source)

**Date:** 2026-07-09
**Phase 00 spec:** `docs/api/gemdos/file/41-fdelete.md`

**EmuTOS:** the classic MS-DOS convention of storing a filename that
legitimately starts with byte `0xE5` as `0x05` on disk (to avoid confusing it
with the erased-entry marker) is **not implemented anywhere** in this build.
`ixcreat()`/`builds()` (`bdos/fsopnclo.c:99-208`, `bdos/fsdir.c:737-796`) copy
and uppercase name bytes verbatim; `toupper()` (`util/string.c:171-177`) is a
plain `'a'..'z'` fold with no special-casing of `0xE5`. A full grep of
`bdos/fsdir.c`/`bdos/fsopnclo.c` finds no `0x05` substitution logic at all. A
file whose 8.3-encoded name genuinely starts with `0xE5` is therefore
indistinguishable from an erased entry on the next scan (`match()`,
`bdos/fsdir.c:1818`; `ixgetfcb()`'s end-of-directory test only checks byte
`0x00`, but `scan()`/`ixsnext()` both treat `f_name[0] == ERASE_MARKER` as
"skip/deleted").

**Prior NewTOS doc (wrong):** `41-fdelete.md` edge case 5 claimed "a name
legitimately beginning with the byte value `0xE5` is stored on disk as `0x05`
by the create path to avoid false-erasure — this is a FAT convention
preserved on-media." No such code exists in EmuTOS `1a7484ab`.

**Not a Compendium discrepancy** — an internal spec fabrication, caught the
same way the earlier `scasb` misread was: by re-reading the cited source
lines instead of trusting the existing prose. This is a real, latent EmuTOS
limitation (not a NewTOS bug) — filenames with a raw `0xE5` lead byte are rare
(the byte is outside 7-bit ASCII) but not impossible via 8-bit path input.

**Decision:** corrected `41-fdelete.md` to state plainly that no substitution
happens. NewTOS is **not required** to reproduce the collision (it is a
latent EmuTOS bug, not an observed-by-programs contract) but should decide
explicitly if it ever supports non-ASCII 8.3 names; flagged for awareness,
not for mandatory bug-compat.

---

## D-064: DND/DMD/OFD share EmuTOS's static `osmem` pool with `MD`s; exhaustion **halts the system**, not `ENSMEM` (spec-vs-source)

**Date:** 2026-07-09
**Phase 00 spec:** `docs/api/structures/dnd.md`

**EmuTOS:** `xmgetblk(WORD memtype)` (`bdos/osmem.c:291-354`) is the sole
allocator for `MDBLOCK`, `DMD`, `DND`, and `OFD` requests, all carved as
fixed 64-byte blocks from one static array `osmem[LENOSM]`
(`NUM_OSM_BLOCKS = 118`, `bdos/osmem.c:36-39` — see also `docs/api/structures/md.md`).
When the free list and the raw pool are both exhausted:
- for `MEMTYPE_MDBLOCK`, `xmgetblk` returns `NULL` (`bdos/osmem.c:327-329`);
- for `MEMTYPE_DMD`/`DND`/`OFD`, it first calls `free_available_dnds()`
  (`bdos/fsdir.c:2053-2074`) to scavenge eligible DNDs tree-wide; **if that
  still doesn't yield a block**, it prints `"Out of internal memory... Use
  FOLDR100.PRG to get more.\nSystem halted!"` and calls `halt()`
  (`bdos/osmem.c:338-342`) — an unrecoverable system freeze, not a GEMDOS
  error return. This is why essentially every DND/OFD/DMD allocation site in
  `bdos/fsdir.c`/`bdos/fsopnclo.c`/`bdos/fsdrive.c` carries a comment like
  "`MGET(OFD)` only returns if it succeeds" — it is **literally true**: those
  three memory types cannot signal allocation failure to their caller.

**Impact on the FS mutation batch:** `Fopen`, `Fcreate`, `Dcreate`,
`Frename` (cross-directory), and any path resolution that must allocate a
fresh `DND` (`makdnd`) or `OFD` (`makofd`/`opnfil`) are all reachable from
this path. Under sustained internal-memory pressure (many simultaneously
open files/directories across many drives), real EmuTOS **halts the whole
machine** rather than returning `ENSMEM` from the syscall.

**Prior NewTOS docs:** `dnd.md` did not mention this at all; it instead
invented a fictitious `NDIRS = 16` "DND pool size" constant (see D-066)
that implied ordinary bounded allocation failure, not a halt.

**Not a Compendium discrepancy** — Compendium does not document internal
allocator exhaustion behaviour at all.

**Open question for the lead / STOP:** NewTOS's kernel allocator for the
Rust ports of DND/OFD/DMD-equivalent structures should almost certainly
**not** replicate a full-system halt on exhaustion — but this requires an
explicit decision (return `ENSMEM` gracefully instead, most likely paired
with the already-planned larger static arenas per PHASE-04 D5's memory
sizing notes in `docs/api/structures/md.md`). Flagging here rather than
silently assuming the "obviously better" behaviour, since it changes an
observable failure mode of the whole FS layer under load and any Phase-05
test suite asserting resource-exhaustion behaviour needs to target the
decided-upon NewTOS behaviour, not EmuTOS's halt.

---

## D-065: Disk buffer cache — 2 buffers per list (not 4), FAT list is FAT-only (not "FAT/dir"), and there is no `flush(drive)`/`flush()` API (spec-vs-source)

**Date:** 2026-07-09
**Phase 00 spec:** `docs/api/gemdos/_state.md` §3, `docs/api/structures/dnd.md`

**EmuTOS:**
- `NUMBUFS = 2` (`bdos/fsbuf.c:26`, no `CONF_WITH_*` gating) — each of the
  two lists (`bufl[BI_FAT]`, `bufl[BI_DATA]`) holds exactly **2** `BCB`s, not
  4; 4 physical sector buffers exist system-wide.
- `getbcb()`'s list selector (`bdos/fsbuf.c:121`) is
  `phdr = &bufl[buftype==BT_FAT ? BI_FAT : BI_DATA]` — `bufl[BI_FAT]`
  (index 0) holds **only** `BT_FAT` buffers; `bufl[BI_DATA]` (index 1) holds
  **both** `BT_ROOT` (root-directory sectors) **and** `BT_DATA` (ordinary
  file/subdirectory sectors) buffers, combined in one 2-slot list.
- There is **no** `flush(int drv)` or no-argument `flush(void)` function.
  `void flush(BCB *b)` (`bdos/fsbuf.c:85-105`) always writes back exactly one
  buffer (plus its FAT-mirror copy on a 2-FAT medium). "Flush all dirty
  buffers, all drives" is achieved by an explicit loop at the call site —
  e.g. `ixclose()`'s full close (`bdos/fsopnclo.c:576-579`) — not by a
  dedicated flush-everything primitive. A separate, **non-flushing** cleanup
  path, `mark_bcbs_invalid(drv)` (`bdos/bdosmain.c:409-424`), runs on media
  change and **discards** (does not write back) that drive's dirty buffers,
  because the medium is assumed to be physically gone.

**Prior NewTOS doc (wrong):** `_state.md` §3 said "4 buffers per list by
default", mislabelled the lists as "`[0] = FAT/dir buffers, [1] = data
buffers`", and asserted a `flush(d)`/`flush()` two-arity API with "writes all
dirty buffers" semantics in both cases — none of which exists; the no-arg
form doesn't exist and the media-change path specifically *discards without
flushing*.

**Not a Compendium discrepancy** — internal-cache-shape spec errors.

**Decision:** corrected `_state.md` §3 and cross-referenced from `dnd.md`.
NewTOS's actual FAT driver cache size/shape is a free implementation choice
(PHASE-05); the *observable* contract worth preserving is: a real `Fclose`
flushes all dirty state across all drives (crash-consistency behaviour
programs may rely on), while a media-change event discards unwritten data
for the affected drive without an attempt to persist it.

---

## D-066: `docs/api/structures/dnd.md` had stale `BLKDEVNUM = 16` and a fabricated `NDIRS = 16` / "LRU" / "d_scan"/"m_ofl" description (spec-vs-source)

**Date:** 2026-07-09
**Phase 00 spec:** `docs/api/structures/dnd.md`

**EmuTOS:** `BLKDEVNUM = 26` (`include/sysconf.h:43`) — `dnd.md`'s
"Configuration" section still said 16 even though D-025 (2026-07-03) already
corrected every drive/directory *function* spec; the shared *structures* doc
was missed in that pass. Separately: there is no `NDIRS` constant anywhere in
EmuTOS (grepped, absent) — DND/DMD/OFD allocation is unbounded-by-type,
sharing one static block pool with `MD`s (see D-064). `d_scan`
(`bdos/fs.h:223`) is written in exactly one place (`makdnd`,
`bdos/fsdir.c:1644`, zeroed on recycle) and **read nowhere** — the file's own
1986 changelog says "removed all references to d_scan" (`bdos/fsdir.c:55,60`).
`d_fill` (`bdos/fs.h:209`, `/* attributes? */`) is likewise referenced
nowhere. `DMD.m_ofl` (`bdos/fs.h:258`) is declared but never assigned or read
anywhere in `bdos/*.c` — dead, not "every OFD whose drive == this DMD, used at
Mediach-detection/unmount". Reclamation of DNDs is an eligibility sweep
(`free_available_dnds()`/`process_dnd_tree()`, `bdos/fsdir.c:1957-2074`) or a
same-parent scavenge (`makdnd()`, `bdos/fsdir.c:1610-1651`) — neither tracks
recency, so "LRU order" mischaracterised both mechanisms.

**Prior NewTOS doc (wrong):** all of the above were asserted as fact in
`dnd.md` without a matching source citation that actually supports them; none
survive a direct grep of `bdos/*.c`.

**Not a Compendium discrepancy** — internal spec fabrication /
staleness, same category as D-030/D-032 (pasted-looking prose that didn't
match a re-read of the actual source).

**Decision:** corrected in full in `dnd.md` (BLKDEVNUM, DND/DMD/OFD shared
pool + halt-on-exhaustion cross-reference to D-064, dead `d_scan`/`d_fill`/
`m_ofl` fields, eligibility-sweep-not-LRU reclamation, and the real
media-change cleanup path cross-referenced to D-065).

---

## D-067: `ckdrv`/`dcrack` discard the specific error code — `EDRIVE`/`ENSMEM` never surface from `Fopen`/`Fcreate`/`Fdelete`/`Fattrib`/`Frename`/`Dcreate`/`Ddelete`/`Fsfirst`/`Dsetpath` (cross-cutting, spec-vs-source)

**Date:** 2026-07-09
**Phase 00 specs:** cross-cutting; see `docs/api/gemdos/_state.md` §4/§5 and
every `docs/api/gemdos/{file,directory}/*.md` entry whose path resolution goes
through `findit()`.

**EmuTOS:** `findit()` (`bdos/fsdir.c:1407`) resolves the drive via
`dcrack()` (`bdos/fsdir.c:1695-1739`), which calls `ckdrv(d, TRUE)`
(`bdos/fsdrive.c:77-134`, distinguishes `EDRIVE` -46 for an invalid drive
number, `EPTHNF` -34 for an unmounted removable drive, and `ENSMEM` -39 for
`log_media()` OOM *or* a full `dirtbl[]`) but **`dcrack` discards the specific
code**: `if (ckdrv(d, TRUE) < 0) return NULL;` — any negative `ckdrv` result
collapses to a plain "path not found" signal. `findit()` then returns `NULL`
uniformly, and each caller applies its *own* generic "resolution failed"
error (`EPTHNF` for most; `EFILNF` for `Fopen`/`Fdelete`'s `ixopen`/`ixunlink`,
already noted in D-018). The practical effect: **an invalid drive letter, an
unmounted removable drive, and a `dirtbl[]`/`osmem` exhaustion are all
observationally identical** from any of these callers — none of them can
ever produce `EDRIVE` or `ENSMEM`, regardless of which of the three actually
happened. Only functions that call `ckdrv`/`Drvmap()` **directly**, bypassing
`findit`/`dcrack` — `Dgetpath` (`xgetdir`, returns `EDRIVE` directly,
`bdos/fsdir.c:1237-1240`) and `Dfree` (`xgetfree`, collapses to generic `ERR`,
already logged as D-026) — preserve any distinction at all, and even then not
the full `EDRIVE`/`ENSMEM` split.

**Prior NewTOS docs:** each individual function spec already documents its
*own* generic error return correctly (e.g. `EPTHNF` for `Fcreate`/`Dcreate`,
`EFILNF` for `Fopen`), so no single spec is factually wrong — but the
*reason* (this shared `dcrack`/`ckdrv` collapse) was previously stated only
for `Dsetpath` (`_state.md` §5, the `dirtbl[]`-full case). It was not
connected as a general fact spanning `Fopen`/`Fcreate`/`Fdelete`/`Fattrib`/
`Frename`/`Dcreate`/`Ddelete`/`Fsfirst` as well.

**Decision:** documented here as a cross-cutting fact rather than editing all
nine affected per-function specs individually (time-boxed; flagged in the
researcher report). No spec correction needed *per function* — each already
states the right observable error — but any NewTOS test suite probing
"invalid drive letter" or "directory-cache exhaustion" behaviour for these
nine functions must expect the *generic* path-resolution error, never
`EDRIVE`/`ENSMEM`, and should cite this entry rather than re-deriving it.

---

## D-068: `docs/api/structures/ofd.md`'s `DFD` was documented as 16 bytes with a fabricated 2-byte pad; actual size is 14 bytes (spec-vs-source)

**Date:** 2026-07-09
**Phase 00 spec:** `docs/api/structures/ofd.md`

**EmuTOS:** `DFD` (`bdos/fs.h:116-124`) is `{ UWORD o_flag; WORD o_usecnt;
DOSTIME o_td; CLNO o_strtcl; long o_fileln; }` = 2+2+4+2+4 = **14 bytes**, with
every field already 2-byte-aligned and nothing declared after `o_fileln` —
there is no padding to 16 bytes anywhere in the source or its usage (the
tail three fields, 10 bytes, are `memcpy`'d as a block against the matching
FCB tail in `ixclose`, `bdos/fsopnclo.c:538`, which only works if there is
**no** gap). With the corrected 14-byte `DFD`, `sizeof(OFD)` (embedding
`DFD o_disk` as its last member) is confirmed at **56 bytes (0x38)** — the
number the doc's own byte-offset table already implied field-by-field; only
the `DFD` sub-figure (16, with an invented "2 bytes pad") was wrong.

**Prior NewTOS doc (wrong):** `ofd.md`'s `DFD` section stated 16 bytes with
"2 bytes pad to 16 in FCB-compatible layout" — no such pad exists.

**Not a Compendium discrepancy** — internal struct-arithmetic error, same
category as D-030/D-032/D-066 (asserted numbers not re-derived from the
actual field list).

**Decision:** corrected `DFD` to 14 bytes in `ofd.md`; `OFD`'s 56-byte total
was already right and is unchanged, now with the arithmetic shown explicitly
so it doesn't drift again.

---

## D-069: `docs/api/structures/dta.md` lifecycle note said `EFILNF` where the source returns `ENMFIL` for `Fsnext` exhaustion (self-consistency fix)

**Date:** 2026-07-09
**Phase 00 spec:** `docs/api/structures/dta.md`

**EmuTOS:** `xsnext()` (`bdos/fsdir.c:654-676`) returns `ENMFIL` (-49) for
every exhausted/uninitialised case (already established as D-019). `dta.md`'s
own "Lifecycle / invariants" list item 4 said "After `EFILNF` (no more
matches)…", contradicting item 5 two lines below (which correctly says
`ENMFIL`) and contradicting D-019/the `4f-fsnext.md` spec it cross-references.

**Not a Compendium discrepancy** — an internal self-consistency slip (`EFILNF`
is `Fsfirst`'s "no match at all" code, `ENMFIL` is `Fsnext`'s "no more
matches" code; the two were conflated in one sentence).

**Decision:** corrected item 4 in `dta.md` to say `ENMFIL`.

---

## D-070: `Frename`'s cycle-prevention check (`is_subdir`) only catches a one-level self-nesting move, not deeper ones (source-level EmuTOS gap, not a spec-reading error)

**Date:** 2026-07-09
**Phase 00 spec:** `docs/api/gemdos/file/56-frename.md`

**EmuTOS:** `xrename()`'s cross-directory cycle guard is
`if (is_subdir(s1,dn1,dn2)) return EACCDN;` (`bdos/fsdir.c:1002-1003`).
`is_subdir()`'s own doc comment states its scope precisely: "check if
directory 2 is an **immediate** subdirectory of directory 1"
(`bdos/fsdir.c:844-863`) — it tests only `dn2->d_parent == dn1 && dn2's name
== s1`, i.e. "is the target's parent directory *exactly* the folder being
moved". This catches the one-level case
(`Frename(0,"C:\A","C:\A\NEW")` — moving `A` to nest directly inside
itself) but **does not walk further up the ancestor chain**. A move like
`Frename(0,"C:\A","C:\A\SUB\NEW")`, where `SUB` is a subdirectory of `A`,
passes the check (`dn2` = the DND for `C:\A\SUB` has `d_parent` = `A`'s DND,
not the root `dn1`, so `is_subdir` returns `FALSE`) and **proceeds**: the
directory entry for `A` is relocated to live inside `A\SUB`, one of its own
descendants — a genuine cycle in the on-disk/in-memory directory tree. Any
future recursive tree walker (`Dgetpath`'s `dopath()` already recurses via
`d_parent`; a CHKDSK-style scan would recurse via `d_left`/`d_right`) would
not terminate against such a structure.

**This is a source-level EmuTOS limitation, not a misreading of the spec** —
confirmed by `is_subdir`'s own comment stating "immediate" as its documented
scope, and by tracing the exact DND-parent comparison for a concrete
multi-level example. It is the closest analogue in this codebase to the
"the code IS the bug" class of finding (cf. the historical `scasb` misread,
which was a *documentation* error; this one is a genuine *gap in EmuTOS's
own defensive check*).

**Prior NewTOS doc:** `56-frename.md` described the one-level case
correctly ("prevents cycles… e.g. `a\b -> a\b\c`") but did not note that the
check's scope stops there.

**Decision:** documented as edge case 6a and a new error-table footnote in
`56-frename.md`. **Open question for the lead:** does NewTOS's `Frename`
implementation widen this check to walk the full ancestor chain (a
deliberate safety improvement over EmuTOS, analogous to the D-023
open-file-rename decision), or intentionally reproduce the gap for
m68k-binary bug-compat? Given this batch's explicit focus on the historical
`Frename` corruption class, the researcher's recommendation is to **widen
the check** — but this is a decision for the Developer/lead, not something
to silently implement here.

---

## D-071: `docs/api/structures/basepage.md` also missed the D-025 `BLKDEVNUM` fix (16 → 26)

**Date:** 2026-07-09
**Phase 00 spec:** `docs/api/structures/basepage.md`

**EmuTOS:** `BLKDEVNUM = 26` (`include/sysconf.h:43`), a plain unconditional
`#define` — not gated by `CONF_WITH_TT_SHIFTER` or any other build flag.
`NUMCURDIR = BLKDEVNUM` (`include/bdosdefs.h:62`), so `PD.p_curdir[26]` +
`p_2fill[6]` fill the fixed 32-byte region at basepage offset 0x40-0x60
(`include/bdosdefs.h:89-90`).

**Prior NewTOS doc (wrong):** `basepage.md` still said `BLKDEVNUM = 16`
("16 in default cfg", "uses 16 of the 32 reserved bytes", and "16 with
default `CONF_WITH_TT_SHIFTER`" — the last also fabricating a conditional
that does not exist in the source) in three places. D-025 (2026-07-03)
corrected every drive/directory *function* spec and `_state.md` §4/§5 for
this same fact, but this particular structures file was missed in that pass
— the same gap D-066 found in `dnd.md`.

**Not a Compendium discrepancy** — internal spec staleness, same root cause
as D-025/D-066 (a shared fact that needed propagating to every file that
restates it, and one file was missed each time).

**Decision:** corrected all three occurrences in `basepage.md`. **Process
note for future passes:** `BLKDEVNUM`/`NUMCURDIR` values are restated in at
least five separate files (`_state.md`, the drive/directory function specs,
`dnd.md`, `basepage.md`); a grep for `BLKDEVNUM = 16\|A:\.\.P:\|NDIRS = 16`
across all of `docs/api/` is a cheap way to catch a future regression or a
missed file in one pass.

---

## D-072: `Ssystem(-1, …)` "does this call exist" probe confirmed `E_OK`; NewTOS `ssystem.rs` has no arm for it (spec correct, implementation gap)

**Date:** 2026-07-09
**Phase 00 spec:** `docs/api/gemdos/154-ssystem.md`
**Source:** FreeMiNT @ `f906416` (EmuTOS has no `Ssystem`).

**FreeMiNT:** `sys_s_system` pre-initializes `register long r = E_OK;`
(`sys/ssystem.c:66`) before the `switch (mode)` (`sys/ssystem.c:70`).
`case -1:` (`sys/ssystem.c:72-76`) does only a `TRACE()` and `break`s — it
never assigns `r`. Execution falls through to `return r;`
(`sys/ssystem.c:651-652`) with `r` still `0`. So `Ssystem(-1, arg1, arg2)`
returns `E_OK` (0) **unconditionally**, independent of `arg1`/`arg2`; every
*other* unrecognized mode instead falls into `default:` and gets `ENOSYS`
(`sys/ssystem.c:642-648`).

**Spec status:** `154-ssystem.md:82-83` (pre-audit line numbers) already
stated this correctly ("`mode == -1` … returns `E_OK`"), and the
verification criterion "Probe: `Ssystem(-1, 0, 0)` returns `E_OK` (0)"
already tested it. **No spec correction needed** — the lead's audit
suspected the opposite (that `-1` would fall into an unhandled/`EINVFN`
path), so this entry exists to record the line-by-line source confirmation
and to log the actual implementation gap the audit uncovered.

**Implementation gap (not a spec bug):**
`tos-api/src/gemdos/ssystem.rs:25-32` (`ssystem()`) matches only
`S_GETCOOKIE`(8)/`S_SETCOOKIE`(9)/`S_DELCOOKIE`(26) on `args.arg_u16(0)`;
every other value — including `-1` (`0xFFFF` as `u16`, i.e.
`args.arg_i16(0) == -1`) — falls to the wildcard arm and returns `EINVFN`.
A MiNT-aware caller probing for `Ssystem` support before touching the cookie
jar sees "not implemented" and falls back to a `$5A0`/`Supexec` path that
ADR-012 removed — cookie access then fails outright.

**Fix (for the Developer, exact match arm):**
```rust
pub fn ssystem(args: &TrapArgs) -> Long {
    if args.arg_i16(0) == -1 {
        // "does this call exist" probe (sys/ssystem.c:66,70-76): `r` is
        // pre-set to E_OK and case -1 never touches it.
        return E_OK;
    }
    match args.arg_u16(0) {
        S_GETCOOKIE => s_getcookie(args),
        S_SETCOOKIE => s_setcookie(args),
        S_DELCOOKIE => s_delcookie(args),
        _ => EINVFN,
    }
}
```

**Decision:** confirmed against source; the fix is scoped to `ssystem.rs`
only (no `cookie.rs` change, no further spec change beyond the two
clarifying notes added to `154-ssystem.md`).

---

## D-073: `Ssystem` cookie error codes must be FreeMiNT-verbatim (`EINVAL`=-25, not `ERROR`/`EINVFN`); `S_GETCOOKIE(0)` returns jar **capacity**, not occupied count (open-question resolution + implementation gap)

**Date:** 2026-07-09
**Phase 00 spec:** `docs/api/gemdos/154-ssystem.md`
**Source:** FreeMiNT @ `f906416`, `sys/cookie.c`, `sys/mint/errno.h`.

**FreeMiNT error values** (`sys/mint/errno.h:52,85,241,265,318,322`):
`E_OK`=0, `EERROR`=-1, `EINVAL`=-25, `ENOSYS`=-32, `EPERM`=-38, `ENOMEM`=-39.

**Confirmed per-path codes** (re-derivation of D-038, now cross-checked
against the live `errno.h` values):
- `get_cookie`, tag search miss → `EERROR` (-1) (`sys/cookie.c:508-509`).
- `get_cookie`, slot-index too big → `EINVAL` (-25) (`sys/cookie.c:458-462`).
- `set_cookie`, invalid tag (a zero byte) → `EINVAL` (-25)
  (`sys/cookie.c:538-541`).
- `set_cookie`, jar full → `ENOMEM` (-39) (`sys/cookie.c:574-575`).
- `del_cookie`, tag not found → `EINVAL` (-25) (`sys/cookie.c:606-607`).

**`S_GETCOOKIE` with `arg1 == 0` (capacity query):** `get_cookie` scans to
the terminator (`cjar->tag == 0`) and returns `cjar->value`
(`sys/cookie.c:422-442`). That value is the jar's **fixed total slot
capacity**, set once in `init_cookies()`
(`ncsize / sizeof(struct cookie)`, `sys/cookie.c:322-326`) and **preserved
verbatim** on every successful `set_cookie` append: `set_cookie` copies the
*same* `n = cjar->value` (the capacity) into the new terminator
(`sys/cookie.c:557-566`) — it never decrements a "remaining free slots"
counter, and it never becomes the count of *occupied* slots.

**Spec status:** `154-ssystem.md` was already correct on the capacity
question (`S_GETCOOKIE (mode 8)` §, "the NULL-slot value = the jar's total
slot capacity"; verification criterion "Get capacity"). **Open Question 1
(error-code mapping) is now RESOLVED and its "open question" marking
removed** in `154-ssystem.md`: NewTOS's `Ssystem` cookie subset returns the
FreeMiNT values verbatim (`E_OK`/`EERROR`/`EINVAL`/`EPERM`/`ENOMEM` as
above); no numeric collision exists within this subset against classical
`gemerror.h` codes (`-25` and `-38` are unused slots in the TOS table). This
follows `docs/COMPATIBILITY.md`'s contract/internal split: a MiNT-aware
caller's `Ssystem` return value is observable behaviour and therefore
contract, not a free NewTOS internal.

**Implementation gap (not a spec bug):**
- `tos_types::error` has no `EINVAL`/`EPERM` equivalents (only `ERROR`=-1
  and `ENSMEM`=-39 exist, which numerically match `EERROR`/`ENOMEM`;
  `EINVAL`=-25 and `EPERM`=-38 are missing entirely).
- `s_getcookie`'s slot-index-too-big and `s_delcookie`'s not-found both
  currently return `ERROR` (`-1`) instead of `EINVAL` (`-25`);
  `s_setcookie`'s invalid-tag path returns `EINVFN` (`-32`) instead of
  `EINVAL` (`-25`) (`tos-api/src/gemdos/ssystem.rs:39-86`). Jar-full already
  returns `ENSMEM` (`-39`), which is correct (numerically == FreeMiNT
  `ENOMEM`) — no change needed there. `s_getcookie`'s tag-search-miss path
  already returns `ERROR` (`-1`), which is correct (== FreeMiNT `EERROR`) —
  no change needed there either.
- `cookie::count()` (`tos-api/src/cookie.rs:101-106`, occupied-slot count)
  is used by `s_getcookie`'s `tag == 0` branch
  (`tos-api/src/gemdos/ssystem.rs:44`), where FreeMiNT wants the jar's fixed
  **capacity**, not the occupied count. With the current 4-cookie boot seed
  and `JAR_CAP = 32`, `Ssystem(S_GETCOOKIE, 0, 0)` today returns `4` instead
  of `32` — a MiNT program pre-flighting jar headroom before `S_SETCOOKIE`
  undercounts available slots by a wide margin.

**Fix (for the Developer):**
1. Add to `tos_types::error` (values confirmed above, FreeMiNT
   `sys/mint/errno.h:241,318`):
   ```rust
   pub const EINVAL: Long = -25; // FreeMiNT / MiNT: invalid argument
   pub const EPERM:  Long = -38; // FreeMiNT / MiNT: operation not permitted
   ```
2. In `tos-api/src/cookie.rs`, add a capacity accessor distinct from the
   existing occupied-count `count()`:
   ```rust
   /// Total slot capacity of the jar — what FreeMiNT stores in the
   /// terminator slot (`sys/cookie.c:322-326`) and returns for
   /// `S_GETCOOKIE(tag == 0)`. NOT the occupied count; see `count()`.
   pub fn capacity() -> u32 {
       JAR_CAP as u32
   }
   ```
3. In `tos-api/src/gemdos/ssystem.rs`:
   - `s_getcookie`'s `tag == 0` branch: use `cookie::capacity()` instead of
     `cookie::count()`.
   - `s_getcookie`'s slot-index branch (`tag & 0xFFFF_0000 == 0`): `None`
     (index out of range) → `EINVAL`, not `ERROR`.
   - `s_getcookie`'s tag-search branch (the `else` arm): `None` (tag not
     found) → stays `ERROR` (already correct, matches FreeMiNT `EERROR`).
   - `s_setcookie`: invalid-tag → `EINVAL`, not `EINVFN`. Jar-full stays
     `ENSMEM` (already correct).
   - `s_delcookie`: not-found → `EINVAL`, not `ERROR`.

**Decision:** confirmed against source; Open Question 1 in
`154-ssystem.md` is resolved and its marking removed (see spec edit
referenced above). No change to the root-gate logic (`EPERM` on
`S_SETCOOKIE`/`S_DELCOOKIE` for non-root callers) is part of this entry —
`ssystem.rs`'s current code has no privilege check at all for those two
modes, which is a **separate**, pre-existing gap (FreeMiNT
`sys/ssystem.c:285-289,295-299`) outside the two questions this audit
scoped; flagged for the lead, not fixed here.
