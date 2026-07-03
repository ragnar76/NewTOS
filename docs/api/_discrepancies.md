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
