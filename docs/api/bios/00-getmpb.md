# `Getmpb` — BIOS 0x00

> Phase 00 spec. Source: EmuTOS @ `1a7484ab` —
> `bios/biosmem.c:134` (`getmpb`).

## EmuTOS source

```c
void getmpb(MPB *mpb)
{
    themd.m_link   = NULL;
    themd.m_start  = membot;
    themd.m_length = memtop - themd.m_start;
    themd.m_own    = NULL;

    mpb->mp_mfl = &themd;       /* free list = single MD */
    mpb->mp_mal = NULL;
    /* mpb->mp_rover left untouched (caller's MPB) */
}
```

`themd` is a single static `MD` in BIOS BSS. `membot`/`memtop` are
the global ST-RAM bounds set by `biosmem_init()` after the BIOS
balloc-stage allocations have settled.

`Getmpb` is **called once at boot by GEMDOS init** to discover the
ST-RAM extent. After this point, `bmem_allowed = FALSE` (with
`DBG_BALLOC`) and the BIOS may not allocate further. GEMDOS takes
ownership of the `MPB` and from that point manages all memory.

## Trap interface

| | |
|--|--|
| Trap | 13 (BIOS) |
| Function ID | `0x00` |
| WORD args | 2 (LONG `mpb`) |
| Stack frame | `+2: mpb (LONG)`, `+0: 0x0000` |
| Vector entry | `bios_vecs[0]` (`bios.c:1252`) |

## Inputs

| Pos | Name  | Type   | Range / meaning                              |
|-----|-------|--------|----------------------------------------------|
| 0   | `mpb` | MPB *  | caller-owned MPB struct to populate          |

## Outputs

The function is `void` — no return value. The dispatcher returns 0
(or whatever was in D0). Programs must read the populated `*mpb`
not the return value.

The MPB after the call:

| Field        | Value                                       |
|--------------|---------------------------------------------|
| `mp_mfl`     | `&themd` (free list with one MD)            |
| `mp_mal`     | `NULL` (allocated list empty)               |
| `mp_rover`   | **untouched** (caller-supplied)             |

The MD `themd`:

| Field        | Value                                       |
|--------------|---------------------------------------------|
| `m_link`     | `NULL`                                      |
| `m_start`    | `membot` (first byte of free ST-RAM)        |
| `m_length`   | `memtop - membot`                           |
| `m_own`      | `NULL`                                      |

## Errors

None. Function cannot fail. Caller is responsible for `mpb` being a
writable 12-byte region.

## Edge cases

### 1. Single static MD

`themd` is a single static MD shared across all `Getmpb` calls.
Calling `Getmpb` a second time (after GEMDOS has already
initialised) **rewrites `themd`** based on the **current** values of
`membot`/`memtop` and replaces the free list with a single MD —
forgetting all of GEMDOS's allocations and free-list state. This
will corrupt the system; programs must not call `Getmpb` after
boot. EmuTOS does not protect against this.

### 2. ST-RAM only

`Getmpb` reports only ST-RAM. Alternate RAM is registered later via
`Maddalt` from `bios/altram.c` after device detection.

### 3. NULL `mpb` not handled

Passing `mpb = NULL` dereferences null. EmuTOS does not check.

### 4. The "two-bank" Atari fiction

Some Atari documentation describes Getmpb as returning two MPBs (ST
and TT). EmuTOS, like classic TOS, fills exactly one — the one
pointed at by the caller's argument. A second `Getmpb` call with a
different argument would just rewrite `themd` again.

## Dependencies

- `membot`, `memtop`, `end_os` globals (set by `biosmem_init`).
- `themd` (static MD).

Used by: GEMDOS `osinit()` once at boot.

## Cross-references

- **Compendium:** "Getmpb" — agrees on populate-and-return-void.
- **Profibuch:** "Getmpb (0x00)" — agrees, notes "obsolete after
  GEMDOS init".
- `docs/api/structures/md.md`
- `docs/api/bios/_state.md`

## Configuration

- `DBG_BALLOC` (debug build) — enables `bmem_allowed` assertion.

## NewTOS implementation notes

(Empty during Phase 00.)

PHASE-03 plan: NewTOS will implement `Getmpb` purely for ABI
compatibility but the GEMDOS allocator is initialised directly from
the AArch64 device-tree memory region — `Getmpb` is essentially
vestigial. Calling it post-init may be made a no-op rather than the
EmuTOS "destroy your free list" behaviour.
