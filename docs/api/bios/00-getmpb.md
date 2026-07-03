# `Getmpb` (BIOS 0x00)

> Phase 00 spec (full-contract template). Source: EmuTOS @ `1a7484ab`.
> Verified against `bios/biosmem.c:134` (`getmpb`).

## NewTOS status: **decision deferred** (see NewTOS implementation notes)

Historically called exactly once, by GEMDOS init, to discover the ST-RAM
extent. NewTOS initialises its allocator directly from the AArch64 memory
map, so this call is vestigial. Whether to expose a thin MPB-over-free-list
is a `docs/COMPATIBILITY.md` port-vs-reimplement decision, deferred until a
program needs it. The dispatcher's out-of-range fall-through is *not* the
answer — `getmpb` is a real, in-range handler (`bios_vecs[0]`).

---

## EmuTOS source

**Primary location:** `bios/biosmem.c:134` — `void getmpb(MPB *mpb)`
(the exported `getmpb` symbol; `bios/bios.c:895` `bios_0()` is only a
`DBGBIOS` debug wrapper and is compiled out in the default build).

**Helpers called:** none. The function is a straight-line field fill.

**Headers / type definitions used:**
- `include/bdosdefs.h:108` — `MD` (memory descriptor) definition
- `include/bdosdefs.h:120` — `MPB` (memory partition block) definition
- `include/tosvars.h:83` — `extern struct _md themd;`

**System variables / globals read:**
- `membot` (`bios/biosmem.c:145`, TOS var `0x432`) — bottom of TPA (free ST-RAM)
- `memtop` (`bios/biosmem.c:146`, TOS var `0x436`) — top of TPA
- (`end_os`, `0x4fa`, is set to `membot` in `balloc_stram`; `getmpb` reads
  `membot` after all `balloc_stram` calls have settled)

**System variables / globals written:**
- `themd` (`bios/biosmem.c:144-147`) — the single static BIOS `MD`
- `*mpb` (`bios/biosmem.c:149-150`) — caller-supplied struct
- `bmem_allowed = FALSE` (`bios/biosmem.c:137`) **only under `DBG_BALLOC`** —
  the "no more BIOS allocation past this point" guard

**Build configuration affecting this function:**
- `DBG_BALLOC` (default `0`, `bios/biosmem.c:15`) — when set, `getmpb`
  additionally clears `bmem_allowed` so any later `balloc_stram` panics.
  In the default build the body is only the six field stores.

---

## Source extraction summary

`Getmpb` reports the initial free ST-RAM region to GEMDOS as a Memory
Partition Block. It fills a single static memory descriptor `themd`
describing the whole free region `[membot, memtop)`: `m_link = NULL` (only
one descriptor), `m_start = membot`, `m_length = memtop - membot`,
`m_own = NULL` (unowned). It then points the caller's MPB free list at
`themd` (`mp_mfl = &themd`) and empties the allocated list (`mp_mal = NULL`).

Note what it does **not** touch: `mp_rover` (the third MPB field) is left
exactly as the caller supplied it. `getmpb` writes only `mp_mfl` and
`mp_mal`. (`mp_rover` is documented as "roving pointer — no longer used" in
`include/bdosdefs.h:125`.)

`membot`/`memtop` are set by `bmem_init()` (`bios/biosmem.c:51`) at boot:
`membot = end_os` (end of OS static data), `memtop = phystop` (top of
ST-RAM), then adjusted downward/upward by every `balloc_stram()` call during
BIOS init. By the time GEMDOS calls `Getmpb`, they bound the final free
region.

---

## Trap interface

- **Trap number:** 13 (BIOS)
- **Function ID:** 0x00
- **Stack frame on entry (m68k):**
  ```
  +2: mpb (LONG, MPB *)
   0: function_id (word) = 0x0000
  ```
- **Total parameter bytes:** 6 (word ID + LONG pointer)
- **Native (AArch64) ABI:** function ID `0x00` in `x8`; the single pointer
  argument in `x0`. `getmpb` is `void` — the dispatcher leaves whatever is in
  `x0` (classically `d0`); programs must read the populated `*mpb`, never the
  return value.

  | m68k stack | AArch64 reg | Name  | Type / size |
  |------------|-------------|-------|-------------|
  | +2         | `x0`        | `mpb` | `MPB *` (pointer) |

---

## Inputs

| Position | Name  | Type    | Size | Source location      | Notes |
|----------|-------|---------|------|----------------------|-------|
| arg0     | `mpb` | `MPB *` | 4/8  | `bios/biosmem.c:134` | caller-owned, writable 12-byte (m68k) MPB. **Not validated**: `NULL` is dereferenced. |

`getmpb` performs no input validation whatsoever.

---

## Outputs

### Return value

- **Register:** D0 (m68k) / `x0` (native)
- **Type:** none — `getmpb` is declared `void`
- **Encoding:** undefined; the result is delivered entirely through `*mpb`.
- **Source location:** `bios/biosmem.c:134` (no `return` statement)

### Side effects on global state

- `themd` rewritten (`bios/biosmem.c:144-147`): `m_link=NULL`,
  `m_start=membot`, `m_length=memtop-membot`, `m_own=NULL`.
- `*mpb` written (`bios/biosmem.c:149-150`): `mp_mfl=&themd`, `mp_mal=NULL`.
  `mp_rover` is **not** written.
- Under `DBG_BALLOC` only: `bmem_allowed=FALSE`.

The MPB after the call:

| Field      | Value                          | Written? |
|------------|--------------------------------|----------|
| `mp_mfl`   | `&themd`                       | yes      |
| `mp_mal`   | `NULL`                         | yes      |
| `mp_rover` | (caller's prior value)         | **no**   |

The MD `themd` after the call:

| Field      | Value               |
|------------|---------------------|
| `m_link`   | `NULL`              |
| `m_start`  | `membot`            |
| `m_length` | `memtop - membot`   |
| `m_own`    | `NULL`              |

### Side effects on hardware

None.

---

## Error conditions

| Condition | Source location | Returned value |
|-----------|-----------------|----------------|
| (none — the function cannot fail) | `bios/biosmem.c:134` | n/a |

`getmpb` has no error path. A `NULL` or non-writable `mpb` faults; EmuTOS
does not guard it.

**Error paths not in Compendium:** none (Compendium also documents no error).

---

## Verification criteria

- **Nominal:** given a writable `MPB` at `mpb`, after the call
  `mpb->mp_mfl` points at a single `MD` with `m_link == NULL`,
  `m_start == membot`, `m_length == memtop - membot`, `m_own == NULL`, and
  `mpb->mp_mal == NULL`.
- **`mp_rover` untouched:** pre-load `mpb->mp_rover` with a sentinel; after
  the call it is unchanged (`getmpb` writes only `mp_mfl`/`mp_mal`).
- **Error:** none defined. (A conforming NewTOS `Getmpb` need not fault on
  `NULL`; matching EmuTOS's unguarded dereference is not required — it is a
  scarcity-era omission, not observable contract.)

---

## Edge cases discovered in source

1. **Single static MD, shared across calls.** `themd` is one static `MD`
   (`bios/biosmem.c:144`). A second `Getmpb` call after GEMDOS init rewrites
   `themd` from the *current* `membot`/`memtop` and replaces the free list
   with one descriptor — discarding all of GEMDOS's allocation state. EmuTOS
   does not protect against this; the call is meant to run exactly once.

2. **`mp_rover` deliberately preserved.** Unlike `mp_mfl`/`mp_mal`, the
   third MPB field is never written, so the caller's value survives. (Old
   docs correctly flagged this; the source confirms it.)

3. **ST-RAM only.** `Getmpb` reports `[membot, memtop)` = ST-RAM. Alternate
   (TT-)RAM is added to the BDOS pool separately via `Maddalt`
   (`altram_init`, gated by `CONF_WITH_ALT_RAM`), never through `Getmpb`.

4. **The "two-bank MPB" documentation fiction.** Some Atari docs describe
   `Getmpb` returning ST and TT MPBs. EmuTOS fills exactly the one MPB the
   caller points at; there is no second bank.

---

## Dependencies

### Requires (must be initialized / functional)

- `bmem_init()` (`bios/biosmem.c:51`) — sets `membot`/`memtop`/`end_os`.
- All BIOS `balloc_stram()` allocations complete (final `membot`/`memtop`).
- `themd` (static, in BIOS BSS).

### Used by

- GEMDOS `osinit()` — the one caller, at boot, to seed the free-memory pool.

---

## Related data structures

- `docs/api/structures/md.md` — `MD` memory descriptor (16 bytes m68k)
- `docs/api/structures/basepage.md` — TPA context
- `MPB` (`include/bdosdefs.h:120`): three pointer fields `mp_mfl`, `mp_mal`,
  `mp_rover` (12 bytes on m68k). Note the struct name in source is
  **Memory _Partition_ Block**, and `mp_rover` is "no longer used".

---

## Cross-references

### Atari Compendium

- Not present in `references/` (no Compendium PDF). Cross-check deferred.
  Historically documents populate-MPB-and-return-void.

### Atari Profibuch

- Not present in `references/`. Cross-check deferred.

### Atari ST Internals

- Not present in `references/`.

---

## Configuration / version notes

- `DBG_BALLOC` (debug builds only) adds the `bmem_allowed=FALSE` guard.
- Configuration-independent in release builds across all TOS versions.

---

## NewTOS implementation notes

(Empty during Phase 00.)

PHASE-03: NewTOS initialises the GEMDOS allocator directly from the AArch64
device-tree / memory map, so `Getmpb` is vestigial. If exposed at all it
should be a read-only report of the ST-RAM-equivalent pool; calling it
post-init must **not** reproduce EmuTOS's "destroy your free list" behaviour.

---

## Pi / BCM2711 binding

> Re-points the free-RAM discovery from the **`membot`/`memtop` low-memory
> globals** (set by `bmem_init` + `balloc_stram` on ST-RAM) to the **AArch64
> memory map** discovered from the device tree at boot. The MPB/MD layout is
> the contract; `Getmpb` itself is vestigial on the Pi. See
> `docs/HARDWARE-PROFILE.md` §3.6, §3.8.

### The allocator is seeded from the memory map, not from `Getmpb`

NewTOS builds its pools directly: `kernel/src/bios/machine/layout.rs::discover`
computes the ST-RAM (`pmd`) and TT-RAM (`pmdalt`) extents from the DTB memory
node (or the board fallback), and `kernel/src/lib.rs:452-481` hands those ranges
to `bdos::mem::init::init`. GEMDOS never calls `Getmpb` to learn the free region
— the classic "one caller, at boot" role is filled by the memory-map path.

| Atari reference | Pi-4 model today |
|-----------------|------------------|
| `membot`/`memtop` from `bmem_init` + `balloc_stram` (ST-RAM only) | ST-RAM pool `[kernel_end, DRAM_START + 14 MiB)` from the memory map (`layout.rs:310-313`) |
| `themd` single static MD over `[membot, memtop)` | the `pmd` MPB's seed MD, plus a separate `pmdalt` for TT-RAM (`Getmpb` never reported TT-RAM) |
| GEMDOS `osinit` calls `Getmpb` once | GEMDOS pools seeded from the AArch64 map before any trap runs |

### Current-code gap (Step-2 candidate)

- **`Getmpb` (BIOS 0x00) is not wired in the BIOS dispatcher.**
  `tos-api/src/bios/mod.rs` has no `0x00` entry, so a call hits the BIOS
  catch-all (`_ => EINVFN`, `mod.rs:58`). This is harmless — `Getmpb` is
  vestigial — but a program that calls it gets `EINVFN`, not a filled MPB.
  **Decision deferred** (see status banner): if exposed, it must be a
  **read-only** report of the ST-RAM pool (`pmd`) that does **not** rewrite the
  live free list (EmuTOS's "second call destroys your allocator state" quirk is
  explicitly rejected for NewTOS). Recommend leaving it unwired until a program
  needs it.
