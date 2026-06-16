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
