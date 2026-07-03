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
**compatible in value and semantics, not necessarily at the physical
address**, because AArch64 + MMU makes m68k low memory meaningless for
native code:

- **Native AArch64 source ports** read them through the header accessors /
  BIOS calls (e.g. `Getbpb`, `Drvmap`, cookie-jar lookup, `Ssystem`). Same
  names, same values, same semantics — just not via a raw `*(LONG*)0x4BA`.
- **m68k binaries** (Phase 10 emulator) get the variables at their real
  addresses: the emulator maps the low-memory region, so direct pokes work.
- Therefore "variable-compatible" = *the variables exist with their
  documented meaning and value, reachable by the idiom appropriate to each
  world* — not "the same bytes sit at the same physical address for native
  code". (ADR: Phase-03 R3 — BIOS variables live in a kernel struct behind
  BIOS calls; direct `$420` poke is out-of-scope for native source ports.)

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

## Deferred optimisations (Phase 2)

Optimisations and modern extensions are **not built until the contract is
clean and verified**, and are kept here so they never blur the contract.
This section is a holding area for ideas — nothing in it changes observable
behaviour for a conforming TOS program.

- _(to be filled in — the user's optimisation ideas go here, each tagged
  with which free-implementation area it touches and confirmed not to alter
  the contract.)_
