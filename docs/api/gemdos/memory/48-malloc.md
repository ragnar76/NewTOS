# `Malloc` (GEMDOS 0x48)

> Phase 00 spec (full-contract template). Source: EmuTOS @ `1a7484ab`.
> Verified against `bdos/umem.c:116` (`xmalloc`).

> **Contract vs. reference** — see [`docs/COMPATIBILITY.md`](../../../COMPATIBILITY.md).
> Contract: trap ID/ABI, the `-1L` largest-free-query protocol, the pointer /
> `0L` / LONG result shapes, the `PF_TTRAMMEM`-driven pool selection *as
> observed* (which RAM a block comes from). Reference: `ffit`, the MD/MPB
> free/alloc lists, the ST-RAM/alt-RAM pool split (`pmd`/`pmdalt`).

## EmuTOS source

**Primary location:** `bdos/umem.c:116` — `void *xmalloc(long amount)`.
It is a thin policy wrapper: it reads the calling process's `PF_TTRAMMEM`
flag and forwards to `xmxalloc` (`Mxalloc`, 0x44) with a derived mode.

**Installed handler / vector:** (none) — direct `funcs[]` entry `0x48` =
`xmalloc`, `wparms = 2` (`bdos/bdosmain.c:268`).

**Helpers called:**
- `bdos/umem.c:234` — `xmxalloc()` — the actual allocator dispatch (see
  [`44-mxalloc.md`](44-mxalloc.md)); `Malloc` supplies the mode.
- `bdos/iumem.c:35` — `ffit()` — reached through `xmxalloc`; first-fit search
  + MD split + owner tagging.

**Headers / type definitions used:**
- `include/bdosdefs.h:102` — `PF_TTRAMMEM = 0x0004` (PRGFLAGS TT-RAM bit).
- `include/bdosdefs.h:18-22` — `MX_STRAM`/`MX_PREFTTRAM` mode constants.
- `include/bdosdefs.h:108-115` — `MD` (memory descriptor).

**System variables / globals read:**
- `run` (`bdos/proc.c:54`) — current PD; `run->p_flags` supplies `PF_TTRAMMEM`.

**System variables / globals written:**
- via `ffit`: `pmd`/`pmdalt` free & allocated MD lists; the new MD's
  `m_own = run` (`bdos/iumem.c:134`).

**Build configuration affecting this function:**
- `CONF_WITH_ALT_RAM` — when 0, `pmdalt` does not exist; the `MX_PREFTTRAM`
  path that `Malloc` selects for `PF_TTRAMMEM` programs collapses to ST-RAM
  (see `xmxalloc`, `44-mxalloc.md`). `xmalloc` itself is unconditional.

---

## Source extraction summary

`Malloc` allocates a contiguous block from the GEMDOS heap and returns a
pointer to it. It does **not** implement the allocator; it chooses *which pool
to prefer* and delegates everything to `Mxalloc`. The choice is automatic,
driven by the `PF_TTRAMMEM` bit (bit 2) of the calling program's `PRGFLAGS`
(recorded in `run->p_flags` at load time): if set, `Malloc` calls
`xmxalloc(amount, MX_PREFTTRAM)` (TT-RAM preferred, ST-RAM fallback); if clear,
`xmxalloc(amount, MX_STRAM)` (ST-RAM only). This preserves the classic Atari
DMA-safety rule — old programs that never opted into alt-RAM only ever see
ST-RAM addresses, which are safe for the ST floppy/DMA hardware.

Because `Malloc` forwards unchanged, the two documented special inputs are
handled by `Mxalloc`: `amount == -1L` is the "how big is the largest free
block" query, and `amount <= 0` (other than -1) returns `NULL`. For a normal
`amount > 0`, `ffit` does a first-fit scan of the selected pool's free list,
splits off the requested (alignment-rounded) bytes, tags the new MD with
`m_own = run`, and returns `m->m_start`.

There is no in-band length header on the returned block (unlike libc `malloc`);
the MD list is the only metadata. The block is released by `Mfree`/`Mshrink`,
or automatically when the owning process terminates (`Pterm`'s
`free_all_owned` walks `mp_mal` and frees every MD with `m_own == the PD`,
`bdos/proc.c:92-101,136-139`).

---

## Trap interface

- **Trap number:** 1 (GEMDOS)
- **Function ID:** 0x48
- **Stack frame on entry (m68k):**
  ```
  +2: amount (LONG)
   0: function_id (word) = 0x0048
  ```
- **Total parameter bytes:** 4 (LONG) → `wparms = 2`.
- **Native (AArch64) ABI:** ID `0x48` in `x8`; `amount` in `x0`. Result in
  `x0`, LONG. A **pointer** result occupies the full 64-bit `x0` (not
  sign-extended from 32 bits); a **`0L`/NULL** failure is all-zero `x0`; the
  `-1L` **size query** returns a LONG (sign-extended).

  | m68k stack | AArch64 reg | Name     | Type / size |
  |------------|-------------|----------|-------------|
  | +2         | `x0`        | `amount` | `LONG` (32/`long`) |

---

## Inputs

| Position | Name     | Type   | Size | Source location   | Notes |
|----------|----------|--------|------|-------------------|-------|
| arg0     | `amount` | `long` | 4    | `bdos/umem.c:116` | bytes to allocate; `-1L` = largest-free query; `<= 0` (else) → NULL |

`amount` is not otherwise validated; `xmxalloc` handles the sign cases.

---

## Outputs

### Return value

- **Register:** D0 / `x0`
- **Type:** LONG (pointer, or size, or `0L`)
- **Encoding:**

  | `amount` | Return |
  |----------|--------|
  | `> 0`, success | pointer to the allocated region (full 64-bit on native) |
  | `> 0`, pool has no fitting free block | `0L` (NULL) |
  | `-1L` | size in bytes of the largest single free block in the selected pool(s) |
  | `0` or `< -1L` | `0L` (NULL) |

- **Source location:** `bdos/umem.c:122,125` (mode selection); value produced by
  `xmxalloc` (`bdos/umem.c:327,334`).

### Side effects on global state

- On a successful `amount > 0` allocation, `ffit` removes/splits a free MD on
  the selected pool and prepends the allocated MD to that pool's `mp_mal` with
  `m_own = run` (`bdos/iumem.c:103-134`). A split consumes one fresh MD from the
  OS-memory MD pool (`xmgetmd`, `bdos/osmem.c:169`).
- `amount == -1L` and failure paths change **no** state (query/NULL are pure).

### Side effects on hardware

None.

---

## Error conditions

| Condition | Source location | Returned value |
|-----------|-----------------|----------------|
| Requested `amount > 0` but no free block fits (out of memory) | `bdos/iumem.c:97-101` → `bdos/umem.c:324-325` | `0L` (NULL) |
| `amount == 0` or `amount < -1L` | `bdos/umem.c:284-286` | `0L` (NULL) |
| MD-pool exhausted during a split | `bdos/iumem.c:113-117` | `0L` (NULL) |

**Error paths not in Compendium:** `Malloc` never returns a negative GEMDOS
error code. Every failure is `0L`. Callers cannot distinguish "out of memory"
from "MD pool exhausted"; both surface as NULL.

---

## Verification criteria

- **Nominal alloc:** `Malloc(4096)` with a pool that has ≥ 4096 contiguous free
  bytes returns a non-NULL, pool-resident pointer; the block's MD carries
  `m_own == run`.
- **Largest-free query:** `Malloc(-1)` returns the size (LONG) of the largest
  single free MD in the selected pool(s) — **not** the sum of free space — and
  changes no allocator state.
- **Out of memory:** `Malloc(size)` larger than every free block returns `0L`.
- **Zero / bad amount:** `Malloc(0)` and `Malloc(-2)` return `0L`.
- **Pool selection:** a program with `PF_TTRAMMEM` set gets a TT-RAM block when
  TT-RAM has room (else ST-RAM fallback); a program without the flag never
  receives a TT-RAM address (ST-RAM only).

---

## Edge cases discovered in source

1. **`Malloc` is pure policy — all logic is in `Mxalloc`.** The only thing
   `xmalloc` decides is the mode from `PF_TTRAMMEM` (`bdos/umem.c:120-126`).

2. **`Malloc(-1)` scope depends on `PF_TTRAMMEM`.** Without the flag →
   `MX_STRAM`, so the query reports the largest ST-RAM block **only**. With the
   flag → `MX_PREFTTRAM`, so `Mxalloc` reports the larger of the two pools'
   biggest blocks (`bdos/umem.c:257-273`). The old spec's blanket "largest
   across both pools" is only true for `PF_TTRAMMEM` programs.

3. **`Malloc(-1)` returns a single-block size, not total free.** `ffit(-1L,…)`
   returns `max(m_length)` over the free list (`bdos/iumem.c:66-74`). A program
   wanting total free space must walk the list itself; there is no such call.

4. **No length header / no ownership check on later free.** The returned pointer
   is the raw region start; `Mfree` (0x49) matches only on `m_start` and does
   **not** verify `m_own`. See [`49-mfree.md`](49-mfree.md).

5. **Alignment rounds the request up.** ST-RAM requests round to
   `malloc_align_stram + 1` bytes (2 on non-Falcon, 4 on Falcon), alt-RAM to 4
   bytes (`MALLOC_ALIGN_ALTRAM = 3`, `bdos/iumem.c:83-86`). The block is at
   least the requested size; the extra is not separately reported.

---

## Dependencies

### Requires (must be initialized / functional)

- `umem_init` (`bdos/umem.c:549`) — seeds `pmd` (and derives `start_stram`/
  `end_stram`) and sets `malloc_align_stram`.
- `Mxalloc`/`ffit` — the allocator body.
- `run` — a valid current PD with `p_flags` set by the loader.

### Used by

- Every C-runtime `malloc()`, AES allocations, and any TOS program building
  dynamic structures.

---

## Related data structures

- `docs/api/structures/md.md` — `MD`/`MPB`, the free/alloc lists, the MD
  meta-allocator.
- `docs/api/gemdos/_state.md` §8 — the `pmd`/`pmdalt` pool model.

---

## Cross-references

### Atari Compendium

- Not present in `references/` (no Compendium PDF). Cross-check deferred.
  Historically documents `Malloc(-1)` = largest-free and the pointer/`0L`
  result — consistent with EmuTOS.

### Atari Profibuch

- Not present in `references/`. Cross-check deferred.

---

## Configuration / version notes

- `CONF_WITH_ALT_RAM` (default 1 on the full build; 0 on cut-down machines):
  when 0, the `MX_PREFTTRAM` path a `PF_TTRAMMEM` program takes has no TT-RAM
  pool and every allocation comes from ST-RAM.
- `malloc_align_stram` is 1 (2-byte align) on ST/STe/TT, 3 (4-byte align) on
  Falcon (`bdos/umem.c:574-581`). This matches Atari TOS 1-3 (2-byte) vs. TOS 4
  (4-byte) so programs that rely on the alignment quirk see the machine-correct
  value.

---

## NewTOS implementation notes

(Empty during Phase 00.)

PHASE-04 (D8): `Malloc(amount>=0)` is a thin wrapper over
`Mxalloc(amount, default_mode)`, `default_mode` from the caller PD's
`PF_TTRAMMEM`. `Malloc(-1)` mirrors EmuTOS (per-pool max, combined only for the
preferred modes). **Return width:** `Malloc` returns `LONG` (i32). On QEMU and
RPi4 `mem=1G` the map fits in 32 bits; above 1 GiB of TT-RAM the address is not
representable in i32, so a low-half user window (Phase 05) is required before
≥ 2 GiB RPi4 configs are enabled.

### NewTOS: one shared address space (ARCH03 stage 1)

`Malloc` for a running process reads that process's `p_flags` and picks
`MX_PREFTTRAM` (when `PF_TTRAMMEM` is set) or `MX_STRAM` (EmuTOS `xmalloc`);
a kernel caller (no process running) keeps the historical `MX_PREFTTRAM`.
The answer is the pool block's own address, valid in every process (see
[`44-mxalloc.md`](44-mxalloc.md) "NewTOS: one shared address space"), and
`Malloc(-1)` is the largest free block of the selected pool(s) — EmuTOS'
answer, no longer bounded by a per-process window (ADR-021, superseded by
ARCH03 stage 1).

---

## Pi / BCM2711 binding

> Re-points the pool backing to **uniform LPDDR4 split into ST-RAM (14 MiB) +
> TT-RAM (256 MiB) logical pools** — pinned in `44-mxalloc.md`'s binding and
> `docs/HARDWARE-PROFILE.md` §3.6. `Malloc` is unchanged as a wrapper.

`malloc` (`tos-api/src/bdos/mem/traps.rs:363-366`) forwards to
`mxalloc(amount, MX_PREFTTRAM)` — TT-RAM preferred, ST-RAM fallback — because
until Phase 06 the sentinel kernel PD is treated as `PF_TTRAMMEM = 1`. So
`Malloc(-1)` returns the larger of the two pools' biggest free blocks, and a
normal `Malloc(n)` lands in TT-RAM when it fits. The pool sizes/addresses are
the `44-mxalloc.md` table.

### Gaps vs. the contract (Step-2 candidates)

- **Fixed `MX_PREFTTRAM` mode** — no per-process `PF_TTRAMMEM` yet (Phase 06);
  every `Malloc` prefers TT-RAM regardless of caller.
- **≥ 2 GiB real-Pi-4 not representable in the `i32` return** — needs the
  Phase-05 low-half window (shared with `44-mxalloc.md`).
