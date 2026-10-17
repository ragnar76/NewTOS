# `Mfree` (GEMDOS 0x49)

> Phase 00 spec (full-contract template). Source: EmuTOS @ `1a7484ab`.
> Verified against `bdos/umem.c:135` (`xmfree`) and `bdos/iumem.c:144`
> (`freeit`).

> **Contract vs. reference** — see [`docs/COMPATIBILITY.md`](../../../COMPATIBILITY.md).
> Contract: trap ID/ABI, the "address must be a block start" rule, the
> `E_OK`/`EIMBA` result codes, the *absence* of an ownership check (a program
> can free any block). Reference: `find_mpb`, `freeit`, the MD/MPB lists,
> coalescing.

## EmuTOS source

**Primary location:** `bdos/umem.c:135` — `long xmfree(void *addr)`.

**Installed handler / vector:** (none) — direct `funcs[]` entry `0x49` =
`xmfree`, `wparms = 2` (`bdos/bdosmain.c:269`).

**Helpers called:**
- `bdos/umem.c:99` — `find_mpb()` — decide which pool (`pmd`/`pmdalt`) owns the
  address, by ST-RAM range test then alt-RAM fallback.
- `bdos/iumem.c:144` — `freeit()` — unlink the MD from `mp_mal`, insert it into
  the address-sorted `mp_mfl`, coalesce with adjacent free MDs.

**Headers / type definitions used:**
- `include/bdosdefs.h:108-115` — `MD`.
- `include/gemerror.h:19,48` — `E_OK = 0`, `EIMBA = -40`.

**System variables / globals read:**
- `start_stram`, `end_stram` (`bdos/umem.c:44-45`) — ST-RAM bounds used by
  `find_mpb`.
- `pmd`, `pmdalt` (`has_alt_ram`) — the two pools.

**System variables / globals written:**
- The chosen pool's `mp_mal` (MD unlinked) and `mp_mfl` (MD inserted,
  neighbours coalesced) (`bdos/iumem.c:164-222`). An absorbed neighbour MD is
  returned to the OS-memory MD pool via `xmfremd`.

**Build configuration affecting this function:**
- `CONF_WITH_ALT_RAM` — only then does `find_mpb` consult `pmdalt`; without it,
  a non-ST-RAM address yields `NULL` → `EIMBA`.

---

## Source extraction summary

`Mfree` releases a block previously returned by `Malloc`/`Mxalloc`. It first
asks `find_mpb` which pool the address belongs to: if the address is within
`[start_stram, end_stram)` it is `pmd`; otherwise, if any alt-RAM has been
registered, it is *assumed* to be `pmdalt` (a trust-the-caller shortcut — the
real membership check is the following scan); otherwise `find_mpb` returns NULL
and `Mfree` returns `EIMBA`.

It then scans that pool's allocated list (`mp_mal`) for an MD whose `m_start`
**exactly equals** `addr`. There is no "block containing this address"
semantics — a pointer into the middle of an allocation is not found. If no MD
matches, it returns `EIMBA`; this is also what a double-free hits (the first
free already moved the MD off `mp_mal`).

On a match it calls `freeit`, which unlinks the MD from `mp_mal`, walks
`mp_mfl` to the correct ascending-`m_start` position, links the MD in there,
and coalesces: if the newly-inserted MD is contiguous with its higher neighbour
it absorbs it (and frees that neighbour's MD via `xmfremd`), and likewise for
the lower neighbour. `Mfree` returns `E_OK`.

**Note — no ownership check, and `m_own` is left stale.** `xmfree` never
compares `m_own` with `run`, so any process can free any process's block by
passing its address (a preserved Atari quirk). And `freeit` does **not** set
`m_own = NULL` on the freed MD — it only relinks it. The freed MD keeps
whatever `m_own` it had; nothing in the free path reads `m_own` (membership is
by list position + `m_start`), so this is harmless internally, but it means
"`m_own == NULL` ⇔ free" is **not** an invariant of this build.

---

## Trap interface

- **Trap number:** 1 (GEMDOS)
- **Function ID:** 0x49
- **Stack frame on entry (m68k):**
  ```
  +2: addr (LONG, void *)
   0: function_id (word) = 0x0049
  ```
- **Total parameter bytes:** 4 (LONG) → `wparms = 2`.
- **Native (AArch64) ABI:** ID `0x49` in `x8`; `addr` (pointer, full 64-bit) in
  `x0`. LONG result (error code) sign-extended into `x0`.

  | m68k stack | AArch64 reg | Name   | Type / size |
  |------------|-------------|--------|-------------|
  | +2         | `x0`        | `addr` | `void *` (ptr) |

---

## Inputs

| Position | Name   | Type     | Size | Source location   | Notes |
|----------|--------|----------|------|-------------------|-------|
| arg0     | `addr` | `void *` | 4    | `bdos/umem.c:135` | must equal the `m_start` of an allocated MD |

`addr` is not otherwise validated; a wild pointer simply fails the scan and
returns `EIMBA` (no fault, no crash from `Mfree` itself).

---

## Outputs

### Return value

- **Register:** D0 / `x0`
- **Type:** LONG (GEMDOS error code)
- **Encoding:**

  | Case | Return |
  |------|--------|
  | Block found on `mp_mal` and freed | `E_OK` (0) |
  | Address in no pool (`find_mpb` NULL) | `EIMBA` (-40) |
  | Address in a pool but not a block start | `EIMBA` (-40) |

- **Source location:** `bdos/umem.c:144,153,158`.

### Side effects on global state

- On success: MD moved from `mp_mal` to `mp_mfl` (address-sorted), coalesced
  with contiguous free neighbours; absorbed neighbour MDs returned to the MD
  pool (`xmfremd`). No change to `m_own`.
- On error: no state change.

### Side effects on hardware

None.

---

## Error conditions

| Condition | Source location | Returned value |
|-----------|-----------------|----------------|
| `find_mpb(addr)` returns NULL (address below ST-RAM and no alt-RAM, etc.) | `bdos/umem.c:142-144` | `EIMBA` (-40) |
| No MD on the pool's `mp_mal` has `m_start == addr` (wrong pointer, mid-block, or double-free) | `bdos/umem.c:148-153` | `EIMBA` (-40) |

**Error paths not in Compendium:** Compendium documents `EIMBA` for a bad
address; it does not spell out that *double-free* and *mid-block* pointers both
return `EIMBA` rather than faulting — that is EmuTOS behaviour.

---

## Verification criteria

- **Nominal free:** after `p = Malloc(N)`, `Mfree(p)` returns `E_OK` and the
  block's bytes return to `mp_mfl` (a subsequent `Malloc(N)` can reuse them).
- **Coalescing:** freeing two blocks that are address-adjacent leaves a single
  merged free MD (the higher/lower neighbour is absorbed).
- **Mid-block pointer:** `Mfree(p + 4)` for a block `p` returns `EIMBA` (-40)
  and changes nothing.
- **Double free:** a second `Mfree(p)` on the same pointer returns `EIMBA`.
- **Out-of-pool pointer:** `Mfree(addr)` for an address in no pool returns
  `EIMBA`.

---

## Edge cases discovered in source

1. **Must be the block *start*.** Matching is `addr == p->m_start`
   (`bdos/umem.c:149`); interior pointers → `EIMBA`. No "containing block"
   lookup.

2. **No ownership check.** `xmfree` never tests `m_own == run`; any process can
   free any block. `Pterm` cleanup *does* filter by owner
   (`free_all_owned`/`reserve_blocks`, `bdos/proc.c:77-101`), but interactive
   `Mfree` does not.

3. **`freeit` does not clear `m_own`.** The freed MD retains its old owner
   pointer (`bdos/iumem.c:144-222` — no `m_own` write). "Free ⇔ `m_own ==
   NULL`" is therefore **not** an invariant here; the free flag is list
   membership, not `m_own`. (Corrects the earlier docs.)

4. **`find_mpb` trusts non-ST-RAM addresses.** Any address outside ST-RAM with
   alt-RAM registered is routed to `pmdalt` *without* a range check
   (`bdos/umem.c:104-107`); the actual validation is the `mp_mal` scan, so a
   wild non-ST-RAM pointer still fails cleanly with `EIMBA`.

5. **Double-free is not a crash in EmuTOS.** The MD is already off `mp_mal`, so
   the second free returns `EIMBA`. The caller's pointer is still dangling,
   though — later access corrupts whoever now owns the region.

6. **Freeing basepage / environment is legal.** These are ordinary owned MDs; a
   process freeing its own basepage is self-destructive but not prevented.

---

## AES resource block (A76, 2026-09-20)

**A block `rsrc_load` obtained is an ordinary process-owned `Mxalloc` block,
not a protected region.** `rs_readit` allocates it with
`dos_alloc_anyram(rslsize)` (`aes/gemrslib.c:775`) — a thin wrapper:
`dos_alloc_anyram(nbytes)` is `Mxalloc(nbytes, MX_PREFTTRAM)`
(`util/gemdos.c:117-120`; `MX_PREFTTRAM = 3`, `include/bdosdefs.h:21`) — and
the base is stored as `rs_global->ap_rscmem = rs_hdr` right after
(`aes/gemrslib.c:787`, in `rs_readit`). Nothing marks this block differently
from any other `Malloc`/`Mxalloc` allocation of the same process, and
`dos_free(maddr)` is a plain `Mfree(maddr)` (`util/gemdos.c:130-133`). The
same holds for every colour-icon `col_data`/`sel_data` buffer
`transform_all_cicons` allocates (`aes/gemrslib.c:428,438` —
`dos_alloc_anyram` again, a **separate** block per `CICON`, not part of the
`rs_hdr` blob). So an application calling `Mfree(ap_rscmem)` directly hits
the ordinary path documented above — this block's `m_start` is on `mp_mal` —
and gets `E_OK`, exactly like freeing any other block it owns (no ownership
check, edge case 2).

**Effect on a later `rsrc_free`.** `rs_free` (`aes/gemrslib.c:685-708`)
re-reads `rs_hdr = pglobal->ap_rscmem` via `rs_sglobe` (`:677-678`); nothing
at the AES layer clears `ap_rscmem` when the app frees the block directly, so
`rs_hdr` is still that (now dangling) pointer and `rs_free` proceeds:

- `free_cicon_buffers(rs_hdr)` (`:515-536`, under `CONF_WITH_COLOUR_ICONS`)
  walks the **freed** block's header — `get_ciconblkptr` (`:484-508`) reads
  `hdr->rsh_rssize` and the extension array at that offset — to find the
  `CICONBLK` pointer table and `dos_free` each `col_data` buffer it lists.
  **This reads already-freed pool memory.** That is EmuTOS's own hazard (the
  block may since have been coalesced into `mp_mfl` or its bytes reused by a
  later allocation) — not a contract NewTOS must reproduce byte-for-byte; it
  is simply what happens on real EmuTOS if a program frees the blob out from
  under `rsrc_free`.
- `dos_free(rs_global->ap_rscmem)` (`:699`) — `Mfree` on the same address a
  second time — finds no MD with that `m_start` on `mp_mal` (already
  unlinked by the app's own `Mfree`) and returns `EIMBA` (edge case 5,
  double free is a clean error, not a fault). `rc` is left `0`, so `rs_free`
  returns `0` — the value `RSRC_FREE`'s `int_out[0]` reports
  (`aes/gemsuper.c:366-367`; see
  [`resource-fileselector.md`](../../aes/resource-fileselector.md) §
  `rsrc_free`).

So: **`rsrc_free` after an application `Mfree(ap_rscmem)` returns 0** in
EmuTOS — not because `Mfree`/`dos_free` crash (a double free is a clean
`EIMBA`, edge case 5), but because the second `dos_free` inside `rs_free`
fails. Any crash risk lives entirely in the `free_cicon_buffers` header walk
over freed memory, which is EmuTOS's own latent hazard, outside the `Mfree`
contract itself.

**The colour-icon `col_data`/`sel_data` blocks are not freed by
`Mfree(ap_rscmem)` alone.** They are separate `Mxalloc` blocks
(`transform_all_cicons`, `:428,438`), reachable only through
`free_cicon_buffers`'s walk of the blob's own header — a bare
`Mfree(ap_rscmem)` never runs that walk. They stay allocated (on the
process's `mp_mal`) until either a `rsrc_free` that still finds a sound
header frees them, or the process terminates and `free_all_owned` reclaims
every MD whose `m_own` is this process — including these buffers —
independently of the resource header's (possibly stale) contents
(`bdos/proc.c:92-101`, called via `ixterm`/`xterm`, `:136,139`).

**NewTOS target contract:** `Mfree` on the mapped `app_va` of a `Kind::Rsc`
block must return `E_OK` and release its pool block exactly as for a
`Malloc`ed block — not `EIMBA`. The AES resource entry for that blob must
then learn the block is gone, so a later `rsrc_free` answers `0` without
touching the freed pool memory (matching EmuTOS's *observable* `rsrc_free
→ 0`, without reproducing its hazardous header re-walk), and process exit
must not free the same pool key a second time. Full NewTOS-side derivation:
[`resource-fileselector.md`](../../aes/resource-fileselector.md) §
`rsrc_free`, A39 callout item 8.

---

## Dependencies

### Requires (must be initialized / functional)

- `umem_init` — `start_stram`/`end_stram` and `pmd`.
- `freeit`, `xmfremd` — the coalescing free path and MD meta-free.

### Used by

- User programs releasing `Malloc`/`Mxalloc` blocks.
- `Pterm`/`Pexec` cleanup via `free_all_owned` (which calls `freeit` directly,
  not `xmfree`, `bdos/proc.c:99`) and `xmfree` for env/PD blocks
  (`bdos/proc.c:223,297,325-326,340-341`).

---

## Related data structures

- `docs/api/structures/md.md` — `MD`/`MPB`, `freeit` coalescing.
- `docs/api/gemdos/_state.md` §8 — the pool model.

---

## Cross-references

### Atari Compendium

- Not present in `references/`. Cross-check deferred. Documents `EIMBA` for a
  bad address — consistent with EmuTOS.

### Atari Profibuch

- Not present in `references/`. Cross-check deferred.

---

## Configuration / version notes

- `CONF_WITH_ALT_RAM` gates the `pmdalt` branch of `find_mpb`; otherwise
  identical across TOS versions.

---

## NewTOS implementation notes

(Empty during Phase 00.)

PHASE-04 (D9): route via `find_mpb` (ST-RAM range test, then alt-RAM); on a
match unlink from `mp_mal`, insert address-ordered into `mp_mfl`, coalesce both
neighbours, return the redundant MD to the MD arena. **Default: no ownership
check** (Atari-compatible; parents legitimately free child memory after
`Pwait`). An **optional** `strict-ownership` build feature may return `EACCDN`
when `m_own != current_pd`, off by default, for catching bugs in ported
sources. NewTOS should also clear `m_own` on free (cleaner than EmuTOS) since it
does **not** rely on the stale-`m_own` quirk.

### NewTOS: one shared address space (ARCH03 stage 1)

Every caller frees by the address `Malloc`/`Mxalloc` returned — the pool
block's `m_start`, valid in every process (VA = PA). There is no
translation step any more (the per-process block table of ADR-021 is gone);
an address that starts no allocated block returns `EIMBA`, EmuTOS's "address
not on `mp_mal`", and — as in EmuTOS (`bdos/umem.c:135-159`) — there was no
ownership check until stage 2 (plan E15, below). The freed memory stays mapped, like
on EmuTOS' flat machine; unmapping freed blocks belongs to stage 3 (plan I8).
A free that returns the block to the pool is reported to the AES
(`aes::resource::on_mfree`), which may hold the block as a resource blob.

**Correction (A76, 2026-09-20).** This paragraph used to continue here: "a
loaded AES resource is reclaimed via `rsrc_free`, not `Mfree`, so a stray
`Mfree(rsc_va)` returns `EIMBA` rather than racing `rsrc_free` into a double
free." That described the *current* implementation
(`usermem::free` refuses `Kind::Rsc`, `tos-api/src/bdos/process/usermem.rs:
605-624`), not EmuTOS's contract, and is wrong as a spec statement. EmuTOS's
resource blob is an ordinary `Mxalloc` block (§ "AES resource block" above);
a direct `Mfree(ap_rscmem)` there succeeds (`E_OK`) exactly like freeing any
other block the process owns — there is no protected-region concept to
preserve, and the double-free EmuTOS would hit on a *subsequent* `rsrc_free`
is a clean `EIMBA`, not a fault (edge case 5). The refusal is a **defect**
tracked as `TASK/A76.md` (measured `Mfree(0x427000) -> -40` on TeraDesk's
`free_icons` → `xrsrc_free`), not a deliberate choice. See § "AES resource
block" above for the full EmuTOS derivation and the target NewTOS contract.

> **OPEN CONFLICT (2026-10-18, TASK/PROC03 spec).** The user decided on
> 2026-10-18 that `run`, `*os_run` and `Malloc` ownership of accessories are
> "as EmuTOS" (`TASK/PROC03.md`). In EmuTOS neither `Mfree` nor `Mshrink` has
> an owner check (`bdos/umem.c:135-159,170-229`; the whole EmuTOS side is in
> [`../process/_run-ownership-curdir.md`](../process/_run-ownership-curdir.md)
> §3.4), and accessories rely on it by construction: memory they allocate
> during init is tagged for the accdesk PD, memory they allocate later for
> whatever `run` is, and they free it under a different `run`. The E15 rule
> below (non-owner → `EIMBA`) turns those frees into errors. **E15 and the
> 2026-10-18 decision cannot both hold for accessory-originated calls; the
> Developer needs an explicit answer (D-8).** Nothing below is changed by this
> note.

### NewTOS: several owners, and only an owner frees (ARCH03 stage 2d, plan E15)

Decision E15 (user, 2026-10-04: "wie empfohlen"): **FreeMiNT's rule** — a block
can have **several owners**, and only an owner may give it back
(`sys/dosmem.c:330-398` `sys_m_free` → `detach_region_by_addr`,
`sys/memory.c:512-660` `attach_region`/`detach_region`: a region has `links`;
detaching drops the caller's link and frees the region when `links` reaches 0).
EmuTOS frees for whoever names the address (`bdos/umem.c:135-159`, no check).

- **Owners.** Every block has its EmuTOS owner tag `m_own` (the allocating
  process's PD, the child for `Pexec` 0, the TSR holder after `Ptermres`, the
  kernel sentinel for kernel allocations). Further owners are recorded beside
  it (`tos-api/src/bdos/mem/region.rs`); today only `Pexec` 4/6 adds one — the
  started child becomes an owner of its basepage block and its environment
  (FreeMiNT `attach_region(p, env); attach_region(p, base)`,
  `sys/k_exec.c:518-520`). A child started with `Pexec` 4 shares its caller's
  memory space (plan E6, FreeMiNT `FORK_SHAREVM`): it counts as an owner of
  what its caller owns.
- **`Mfree(addr)`** by an owner drops **that** owner; the memory goes back to
  the pool only when no owner is left (then it is reported to the AES as
  before). By someone who is not an owner → **`EIMBA` (-40)** — FreeMiNT
  answers `EFAULT`, whose value is the same −40. An address that starts no
  block stays `EIMBA`.
- **`Mshrink(blk, len)`** only by an owner (FreeMiNT `sys_m_shrink`,
  `sys/dosmem.c:410-455`: only regions of the caller's memory space), else
  `EIMBA`; `len == 0` is `Mfree` with the same rule.
- **Process end** (`Pterm`) drops the process from every block it owns; a
  block with another owner left survives, and if the process held the EmuTOS
  tag, the next owner takes it over. With a single owner — the only case
  EmuTOS knows — this is exactly `free_all_owned`.
- **`Ptermres`** hands every link of the process to the TSR holder (see
  `../process/31-ptermres.md`): nobody frees resident memory with `Mfree`.
- **Kernel-internal frees** (the AES freeing a resource blob it loaded, the
  loader cleaning up) are not `Mfree` calls by a program and take no owner
  check; whatever frees a block also forgets its extra owners.

**Observable difference to EmuTOS:** a program that frees (or shrinks) a block
it does not own — another process's memory, a TSR's memory — gets `EIMBA`
instead of success; a block shared with a running child is freed only after
the child let go too. Correct programs free their own memory and are not
affected (FreeMiNT runs them the same way).


### NewTOS: freed memory is nobody's (ARCH03 stage 3b, invariant I8)

With memory protection on, the pages of a block that goes back to the pool
are made invalid for **every** process — its former owners included — before
`Mfree` returns (FreeMiNT `free_region` → `mark_region(PROT_I)`,
`sys/memory.c:932`), with the TLB invalidated. A program that writes a block
after freeing it ends with the crash report (`Memory violation: free memory,
write (translation fault …)`, `docs/api/bios/_exception-crash.md`). Closes
MEM04. With `memprot=off` the pool stays mapped for everybody (stage 3a).

### NewTOS: `F_KEEP` blocks the system inherited (ARCH03 stage 3c)

A block allocated with `Mxalloc(…, F_KEEP | mode)` (`44-mxalloc.md`) whose
last owner terminated belongs to the system (FreeMiNT: the root process,
`sys/proc_help.c:242-258`). `Mfree` of such a block succeeds for **any**
caller — FreeMiNT `sys_m_free` falls back to the root process's `M_KEEP`
regions when the address is not one of the caller's (`sys/dosmem.c:365-398`)
— and returns it to the pool (`E_OK`). Every other non-owner `Mfree` stays
`EIMBA` (E15), including one on a kept block whose owner still lives, and on
resident (`Ptermres`) memory. Host tests: `tos-api/tests/arch03_3c_keep.rs`.

---

## Pi / BCM2711 binding

> Re-points the pool backing to **uniform LPDDR4 split into ST-RAM + TT-RAM
> logical pools** — pinned in `44-mxalloc.md`'s binding and
> `docs/HARDWARE-PROFILE.md` §3.6. The `Mfree` semantics are unchanged.

`mfree` (`tos-api/src/bdos/mem/traps.rs:413-440`) locates the owning pool with
`find_mpb` (the cached ST-RAM range test → `pmd`, else `pmdalt`;
`traps.rs:386-393`), walks `mp_mal` for `m_start == addr`, returns `EIMBA` if
not found, else `freeit` (address-ordered `mp_mfl` insert + coalesce). Which
physical pool an address belongs to is decided purely by the logical ST-RAM
range on the Pi — there is no hardware zone.

### Gaps vs. the contract (Step-2 candidates)

- **None pool-specific.** The `find_mpb` routing is correct for the logical
  split; the ownership/`m_own`-clearing choices above are generic (not Pi-bound).
