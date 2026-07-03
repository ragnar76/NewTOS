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
**Phase 00 spec:** `docs/api/gemdos/process/01-pterm.md`

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
**Phase 00 spec:** `docs/api/gemdos/process/03-ptermres.md`

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
