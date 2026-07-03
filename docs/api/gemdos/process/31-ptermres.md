# `Ptermres` (GEMDOS 0x31)

> Phase 00 spec (full-contract template). Source: EmuTOS @ `1a7484ab`.
> Verified against `bdos/proc.c:624` (`xtermres`) and its helper
> `bdos/proc.c:77` (`reserve_blocks`).

> **Contract vs. reference** — see [`docs/COMPATIBILITY.md`](../../../COMPATIBILITY.md).
> **Contract:** trap ID/ABI, "does not return", the exit code passed to the
> parent (same as `Pterm`), and the **keep-resident** semantics — the process's
> memory (its basepage/TPA, shrunk to `blkln`, plus any other blocks it owns)
> **survives** termination instead of being freed. **Reference (NewTOS
> reimplements freely):** the `xsetblk` MD-shrink, the `reserve_blocks`
> `mp_mal`-splice / `xmfremd` orphan trick, and the chain through `xterm`.

## EmuTOS source

**Primary location:** `bdos/proc.c:624` — `WORD xtermres(long blkln, WORD rc)`.

**Installed handler / vector:** (none) — direct `funcs[]` entry `0x31` =
`xtermres`, `wparms = 3` (`bdos/bdosmain.c:237`; `docs/api/gemdos/_dispatcher.md
§6`).

**Helpers called:**
- `bdos/umem.c:170` — `xsetblk()` — shrink the owning MD of the basepage to
  `blkln` (this is the same helper `Mshrink`/0x4A uses); **its return value is
  discarded** here.
- `bdos/proc.c:77` — `reserve_blocks()` — walk a pool's `mp_mal`, unlink every MD
  owned by the process and free the MD *struct* via `xmfremd`, **without**
  returning the memory to the free list (the "orphan/reserve" trick).
- `bdos/proc.c:601` — `xterm()` — the normal termination chain (term handler,
  file close, current-dir release, parent restore, exit-code delivery).

**Headers / type definitions used:**
- `include/bdosdefs.h:65-97` — `PD`; `bdos/mem.h:68` — `xsetblk` prototype;
  `include/bdosdefs.h:108-` — `MD`/`MPB` (owner walk).

**System variables / globals read:**
- `run` (`bdos/proc.c:626,628`) — the terminating process; the block to shrink
  is `run` itself (the basepage = `p_lowtpa`).
- `pmd`/`pmdalt`, `has_alt_ram` — pools scanned by `reserve_blocks`.

**System variables / globals written:**
- The pools' `mp_mal` lists — the process's MDs are unlinked (and their MD
  structs freed) but their memory is **left out of `mp_mfl`** (reserved).
- Then everything `xterm` writes (parent restore, `p_dreg[0] = rc`, etc.).

**Build configuration affecting this function:**
- `CONF_WITH_ALT_RAM` — adds the second `reserve_blocks(run, &pmdalt)` call
  (`bdos/proc.c:629-632`).

---

## Source extraction summary

`Ptermres` terminates the calling process but **keeps its memory resident** — the
TSR (terminate-and-stay-resident) primitive. It first shrinks the basepage's
memory block to `blkln` bytes by calling `xsetblk(0, run, blkln)` (the block
address is `run`, i.e. the basepage itself). It then calls `reserve_blocks` on
each memory pool: this walks the allocated list and, for every MD owned by the
process, splices it out of `mp_mal` and frees only the MD *bookkeeping struct*
(`xmfremd`) — the described memory region is thereby left in **neither** the
allocated nor the free list, so it stays permanently allocated with no owner.

Finally it chains to `xterm(rc)`, which performs the ordinary termination:
invoke the `0x102` user term handler, close files, drop current-dir references,
restore the parent, deliver `rc`. Because the process's MDs are already gone from
`mp_mal`, `xterm`'s `free_all_owned` finds nothing to free — so the reserved
region survives. `xtermres` is declared `WORD` but never returns (the return
type is historical; callers ignore it).

The classic caller computes `blkln` as "basepage through the end of the resident
code/data" and passes `rc = 0`.

---

## Trap interface

- **Trap number:** 1 (GEMDOS)
- **Function ID:** 0x31
- **Stack frame on entry (m68k):**
  ```
  +6: rc    (WORD)
  +2: blkln (LONG)
   0: function_id (word) = 0x0031
  ```
- **Total parameter bytes:** 6 (LONG + WORD) → `wparms = 3`.
- **Native (AArch64) ABI:** ID `0x31` in `x8`; `blkln` in `x0`, `rc` in `x1`.
  **No return** — the parent's `Pexec` receives `rc` (WORD zero-extended to LONG)
  in `x0`.

  | m68k stack | AArch64 reg | Name    | Type / size |
  |------------|-------------|---------|-------------|
  | +2         | `x0`        | `blkln` | `LONG` (32) — new size of the basepage block, in bytes |
  | +6         | `x1`        | `rc`    | `WORD` (16) — exit code to the parent |

---

## Inputs

| Position | Name    | Type   | Size | Source location   | Notes |
|----------|---------|--------|------|-------------------|-------|
| arg0     | `blkln` | `LONG` | 4    | `bdos/proc.c:626` | new size in bytes for the basepage's MD; passed to `xsetblk` |
| arg1     | `rc`    | `WORD` | 2    | `bdos/proc.c:624` | exit code, forwarded to `xterm` |

`blkln` should cover the basepage (min `sizeof(PD)` = 256) plus the resident
code/data. It is passed straight to `xsetblk`; **the shrink result is ignored**
(see Errors / Edge cases).

---

## Outputs

### Return value

- **Register:** D0 / `x0` — delivered to the **parent**, not returned to the
  caller.
- **Type:** the parent's `Pexec` sees a LONG.
- **Encoding:** `rc` zero-extended into `p_dreg[0]` (same as `Pterm`, D-005).
- **Source location:** via `xterm(rc)` (`bdos/proc.c:633` → `bdos/proc.c:616`).

### Side effects on global state

- The basepage MD shrunk to `blkln` (if valid — else the shrink is a silent
  no-op, see below).
- Every MD the process owns is removed from `pmd.mp_mal` (and `pmdalt.mp_mal`)
  and its MD struct freed via `xmfremd`; the memory itself is **reserved**
  (orphaned, out of both lists) — `reserve_blocks`, `bdos/proc.c:77-89`.
- Then all of `xterm`'s side effects (parent restore, file/current-dir cleanup,
  `p_dreg[0] = rc`).

### Side effects on hardware

- Dirty-buffer writeback on file close (via `xterm`→`ixterm`); the `0x102`
  handler may touch hardware.

---

## Error conditions

`Ptermres` has **no error return** — it does not return at all, and it discards
`xsetblk`'s result:

| `xsetblk(0, run, blkln)` internal result | Source location | Effect on `Ptermres` |
|------------------------------------------|-----------------|----------------------|
| `EIMBA` (-40) — `run` not found in any pool's `mp_mal` | `bdos/umem.c:177,196` | **ignored**; nothing shrinks |
| `EGSBF` (-67) — `blkln` larger than the current block | `bdos/umem.c:211` | **ignored**; nothing shrinks |
| `E_OK` — shrink succeeded (or `blkln == current`) | `bdos/umem.c:212-` | proceed |

**Error paths not in Compendium (D-006):** Compendium implies a bad `blkln`
yields a GEMDOS error to the parent; EmuTOS **silently ignores** it and
terminates anyway. If the shrink is rejected, `reserve_blocks` reserves the
*original* (full) block, wasting the TPA the program meant to release — but no
error is signalled.

---

## Verification criteria

- **Nominal keep-resident:** a process that `Ptermres(size, 0)`s leaves a
  resident region of (about) `size` bytes at its basepage that is **not** on the
  free list afterward — a later `Malloc(-1)` does not reclaim it, and the region
  contents are preserved.
- **Exit code:** the parent's `Pexec` returns `rc` (e.g. `Ptermres(size, 0)` →
  parent sees `0`).
- **Files/dirs still closed:** despite staying resident, all the process's open
  files are closed and current-dir references dropped (the `xterm` chain runs) —
  a TSR cannot hold files open across `Ptermres`.
- **Bad `blkln` is silent (D-006):** `Ptermres(100, 0)` (below `sizeof(PD)`) or a
  `blkln` larger than the block still terminates with no error; the shrink is a
  no-op and the full block is reserved.
- **Term handler runs:** the `0x102` user term handler is invoked (via `xterm`).

---

## Edge cases discovered in source

1. **The "reserve" (orphan-MD) trick.** `reserve_blocks` unlinks each owned MD
   from `mp_mal` and frees only the MD struct (`xmfremd`) — the memory region is
   left in **neither** list, so `xterm`'s later `free_all_owned` finds nothing
   and the region persists (`bdos/proc.c:77-89`). This is the technical heart of
   a TSR. There is no OS mechanism to later reclaim it (the MDs are gone); a warm
   boot is the only general recovery.

2. **Order matters: shrink before reserve.** `xsetblk` runs first (adjusts the
   basepage MD's length), then `reserve_blocks` unlinks it (`bdos/proc.c:626-628`).
   Reversed, the shrink would target an already-unlinked MD.

3. **`xsetblk` result ignored (D-006).** A `blkln` below `sizeof(PD)` or above
   the current block size makes `xsetblk` return `EGSBF`/`EIMBA`, which
   `xtermres` drops — the full block is reserved and no error reaches the parent.

4. **All owned blocks are reserved, including the environment.** `reserve_blocks`
   reserves **every** MD with `m_own == run`, which includes the `p_env` block. A
   TSR that does not want its environment left resident must `Mfree(p_env)`
   *before* `Ptermres` — EmuTOS does not auto-free it.

5. **Files and current dirs are still released.** The chained `xterm`→`ixterm`
   closes all open handles and decrements current-dir usage. A resident program
   that needs file access must open/close inside its own hooks/ISR later.

6. **Fragmentation, no uninstall.** Each `Ptermres` permanently reserves its
   block; the TPA fragments from the bottom up and the orphaned MDs cannot be
   found by ownership. (Contract-relevant only as "the memory stays"; the
   fragmentation is an allocator-internal consequence.)

7. **`WORD` return type is vestigial.** The function is typed `WORD` but never
   returns; the dispatcher classifies it by `wparms` and callers ignore any
   "return".

---

## Dependencies

### Requires (must be initialized / functional)

- `xsetblk` (needs `find_mpb` + the pools initialised).
- `reserve_blocks` + `xmfremd` (MD meta-allocator).
- `xterm` (transitively: term handler, `xclose`, `decr_curdir_usage`, parent
  restore, exit-code delivery).

### Used by

- TSR programs; keyboard/desk accessories installed before the AES; low-level
  resident caches.

---

## Related data structures

- `docs/api/structures/md.md` — the `mp_mal` list, `m_own`, `xmfremd`, the
  orphan/reserve manipulation.
- `docs/api/structures/basepage.md` — the basepage block that gets shrunk.
- `docs/api/gemdos/memory/4a-mshrink.md` — the same `xsetblk` helper (as
  `Mshrink`).
- `docs/api/gemdos/_state.md §8` — the pool/`m_own` ownership model.
- [`4c-pterm.md`](4c-pterm.md) — the inherited termination semantics.

---

## Cross-references

### Atari Compendium

- Not present in `references/`. Cross-check deferred. Documents `Ptermres` as
  "shrink TPA to `blkln`, keep resident" and implies a bad `blkln` returns an
  error — EmuTOS silently ignores it (see D-006).

### Atari Profibuch

- Not present in `references/`. Cross-check deferred.

### Discrepancies

- **D-006** — `Ptermres` ignores the `xsetblk` return; a bad `blkln` is silently
  a no-op (no error to the parent). See `docs/api/_discrepancies.md#D-006`.

---

## Configuration / version notes

- `CONF_WITH_ALT_RAM` — second-pool `reserve_blocks`.

---

## NewTOS implementation notes

(Empty during Phase 00.)

PHASE-06 D7 status (2026-06-16): **implemented and verified.**
`reserve_all_owned` (`tos-api/src/bdos/mem/free_all_owned.rs`) ports
`reserve_blocks`: it walks each pool's `mp_mal` with the `**q` splice idiom,
unlinks every MD owned by the dying process and frees the MD struct via
`xmfremd_global` — **without** returning the memory to `mp_mfl`, exactly the
EmuTOS reserve effect. `pterm::terminate_resident` shares its body with
`terminate` via a `reserve` flag, so the exit-code / parent-resume path is
identical. The kernel handler routes GEMDOS `0x31` with `rc` from `x1` and
`blkln` from `x0`.
- **`blkln` is currently a no-op** under the static-arena TPA model (the basepage
  TPA is not a BDOS MD, so `xsetblk` has nothing to shrink); what `Ptermres`
  reserves is the process's `Malloc`ed pool blocks. Precise keep-resident sizing
  returns once TPAs are pool-allocated (ADR-010 / Phase 09).
- **Env block:** edge case 4 (auto-reserve of `p_env`) does not apply yet —
  `p_env` is unset in the current basepage.
- The orphan-MD trick requires the allocator to support MDs that live outside any
  list while their memory stays off the free list; the current implementation
  models this with slot lifetime.
