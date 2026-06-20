# MEMORY — NewTOS

> **Project memory: knowledge that must persist across sessions.**
>
> Unlike `STATUS.md` (which tracks "what's happening now") and the
> root-level [`memory.md`](../memory.md) (which is the coding-agent's
> compact-survival working notes), this file records **durable
> project knowledge**: ADRs with their full rationale, hard-won bug
> solutions, rejected alternatives with reasoning, hardware quirks
> discovered during implementation, and any context that would otherwise
> be lost between sessions or human contributors months apart.
>
> **Read this file at the start of every working session.**
> Append-only by convention. Do not rewrite history; document corrections
> by writing a superseding entry that references the old one.

---

## How to use this file

- **ADRs**: Architectural Decision Records. Numbered, dated, with full
  rationale. Once an ADR is written it is not modified — superseded
  decisions get a new ADR that references the old one.
- **Quirks**: Bugs, hardware oddities, surprising spec interpretations
  that bit us during development. One per entry, with reproduction info
  and resolution.
- **Lessons**: Process / workflow / tooling lessons. "We learned the
  hard way that …".
- **Rejected**: Alternatives we considered and rejected, with the reason.
  Prevents re-litigating the same decisions.

---

## ADRs (Architectural Decision Records)

ADRs are numbered globally, prefix `ADR-NNN`. They follow the
"context / decision / consequences / alternatives" structure.

## ADR-010: Single shared user address space (identity-mapped DRAM pools)

**Date:** 2026-06-16
**Status:** Accepted as the **real-hardware target — NOT YET IMPLEMENTED.**
Stage 1 is blocked on the QEMU cortex-a72 TCG (see Implementation note /
Conclusion below). The **live implementation is option (b)**: per-process
`TTBR0_EL1` + opaque PD handle + the QUIRK-001 Device-write barrier — the
model this ADR's Decision rejects "for fidelity", kept as the only
QEMU-workable interim. Consequently the per-process-TTBR0 decision in
STATUS Recent-Decisions 2026-05-03/04 is **still in force, not
superseded**; `USER_VA_BASE = 0` and the per-slot `l0_pa` /
`install_ttbr0` / `restore_boot_ttbr0` machinery are all still live
(verified 2026-06-20). Flip this to plain "Accepted" only once the shared
user TTBR0 is actually installed (on real RPi4 hardware).
**Phase:** PHASE-06 (process model).

### Context

`Pexec(PE_LOAD)` must return a PD pointer (as `LONG` = i32) that the
parent holds and passes back to `Pexec(PE_GO)`, and that source-ported
programs dereference (the classic shell reads `p_tlen`/`p_dlen`/`p_blen`
to size the TPA before `Mshrink`). The Phase-05/06 model gave each
process its **own** `TTBR0_EL1` with its TPA at `USER_VA_BASE = 0`, so
every basepage VA is `0` — not a unique, parent-dereferenceable, or
LONG-representable pointer. Per-process TTBR0 also forces a TTBR0 swap
on every `Pexec`/`Pterm`, which is what triggers QUIRK-001 (the QEMU
cortex-a72 TCG wedge), and per-process ASIDs (the "proper" fix) wedge
QEMU even harder (QUIRK-001).

Classic TOS has exactly **one** address space: programs are loaded into
a `Malloc`'d TPA and run *at that block's address* (no MMU; phys = VA).
PD pointers are therefore real, shared, dereferenceable pointers.

### Decision

NewTOS uses a **single shared user address space** for all processes,
realised by **identity-mapping the DRAM user/pool region (VA = PA) into
one shared `TTBR0_EL1`** that is installed once and never swapped. Each
process's TPA is a BDOS-pool block; the program runs at `VA = PA` of
that block; `p_lowtpa` / basepage VA = the block address (a DRAM phys
~`0x4000_0000`+, which fits a positive `i32`). The kernel stays in the
high half (`TTBR1_EL1`), untouched.

### Rationale

- **EmuTOS fidelity / source-porting.** This is precisely the TOS
  model: TPA = allocated block, phys = VA, PD pointers are real shared
  pointers. The full `Pexec(PE_LOAD)`→read PD→`Mshrink`→`Pexec(PE_GO)`
  shell idiom works, and ported C programs that dereference their
  basepage / `Malloc` results see consistent, dereferenceable pointers.
- **Unifies `Malloc`.** `Malloc`/`Mxalloc` already return pool DRAM
  addresses (~`0x4000_0000`+); under identity mapping those *are* valid
  user VAs, removing today's inconsistency (pool returns phys, code
  runs at VA 0).
- **Dissolves QUIRK-001.** One shared user TTBR0 that is never swapped
  ⇒ no TTBR0 switch between VA-aliased processes ⇒ no TCG wedge ⇒ the
  interim Device-write barrier and the whole boot-restore/install dance
  are deleted. ASIDs become unnecessary (single ASID 0 is correct).
- **Matches the cooperative model.** Only one process runs at a time
  (cooperative, `Pexec` blocks until child `Pterm`); inter-process MMU
  isolation buys nothing here and TOS programs never expected it.

### Consequences

- Positive: faithful PD/pointer semantics; `Malloc` consistency;
  QUIRK-001 + the barrier + per-process page-table machinery
  (`mmu::init_slot_tables`/`map_page`/`install_ttbr0`, `__USER_TABLES`,
  per-slot `l0_pa`, the `restore_boot_ttbr0` dance) all go away.
- Negative: no MMU isolation between user processes (a buggy process
  can corrupt another's TPA). Acceptable — classic TOS has none either;
  revisit only if/when preemptive multitasking is ever in scope.
- Negative: the loader must allocate the TPA from a BDOS pool (not the
  static `__PROCESS_USER_PAGES` arena) and load at `VA = PA`; `Mshrink`
  becomes meaningful (real TPA sizing) instead of a no-op.
- Neutral: user region is mapped `User-RWX` for now (no W^X);
  text/data/bss permission split is a later refinement.

### Alternatives considered

- **Opaque slot-handle under per-process TTBR0** (PE_LOAD returns
  `basepage_pa`, parent treats it opaquely): keeps isolation and is
  less work, but the parent cannot *dereference* the PD, so the real
  shell idiom and ported programs that read the PD break. Rejected for
  fidelity.
- **Distinct per-slot TPA VA + map child basepage into parent's TTBR0:**
  half-measure that still carries per-process TTBR0 swaps (and thus
  QUIRK-001) and the 64-bit-VA-vs-LONG problem. Rejected.
- **Per-process ASIDs:** wedges QEMU worse (QUIRK-001); does not by
  itself solve the LONG-pointer problem. Rejected.

### Implementation note — Stage-0 prerequisite (discovered 2026-06-16)

A first attempt to install the shared user TTBR0 at boot (Stage 1)
**broke the boot**, surfacing a prerequisite that must land first:

- The kernel currently relies on the **low-half identity mapping**
  that the Stage-1 boot tables leave in `TTBR0_EL1` after `reinstall`.
  Concretely, the kernel heap dereferences raw **physical** addresses
  as pointers — `heap.rs`: `let start = frame.pa() as usize; … start
  as *mut Header` — i.e. it touches DRAM through low-half VAs
  (`VA = PA`), served by those boot tables (AP=00, EL1-only, no PXN).
- Installing the user table in `TTBR0` (DRAM-only, 2-MiB blocks,
  `PXN=1`, and ultimately `AP=01`/EL0) removes that EL1-friendly
  low-half mapping, so the next `frame.pa()` dereference in
  `heap::init` dies. (`AP=00` did not help — the remaining deltas are
  PXN / DRAM-only coverage; either way the kernel must not depend on
  TTBR0 for its own data.) Also observed: `tlbi vmalle1` on the
  install path wedges QEMU (QUIRK-001-adjacent) — the install must be
  `msr`+`isb` only.

**Stage 0 (prerequisite) — DONE (commit `55b4390`).** Audit found the
kernel heap was the *only* low-half-deref site (frames manage PAs as
values, BDOS pools keep MDs in a static high-half arena and treat
`m_start` as values, the ELF loader writes via the static high-half
arena). One-line fix: `heap::init` addresses its arena through
`pa + VA_OFFSET`. Boot stays green.

**Stage 1 — BLOCKED on QEMU (attempted 2026-06-16, reverted).**
With Stage 0 in place, installing the shared user table in `TTBR0` at
boot still breaks the boot — but now the failure is a **hard hang on
the first kernel Normal-memory store after the install** (verified: the
heap header write to high-half `0xffff000041000000` hangs, no
exception). Diagnosis: under ADR-010 a DRAM physical page is **aliased
with mismatched permission attributes** — TTBR1 high-half (EL1 Normal)
*and* TTBR0 low-half (EL0 Normal + PXN). Real ARMv8 permits this
(memory *type* + shareability match; AP/PXN differ), but QEMU 11.0.0's
cortex-a72 TCG wedges on a store to such a doubly-mapped PA — the same
class of TCG fragility as QUIRK-001 (TTBR0 swap) and the ASID attempt.
`tlbi vmalle1` on the install path wedges even earlier.

**Conclusion:** ADR-010 is correct for **real hardware** but cannot be
developed or validated on this QEMU model — three independent TCG walls
(TTBR0 swap = QUIRK-001, ASID-tagged TTBR0, and now kernel/user DRAM
aliasing) all block it. Options: (a) develop/validate ADR-010 on real
RPi4 hardware; (b) keep the working per-process-TTBR0 + barrier model
for QEMU and implement `Pexec` `PE_LOAD`/`PE_GO` with an opaque
slot-handle (the parent does not dereference the PD — acceptable until
a real FS-backed loader + dereferenceable PD land in Phase 09). The
Stage-0 heap improvement is kept regardless.

### References

- EmuTOS `bdos/proc.c` (`alloc_tpa`, `proc_go`), `docs/api/gemdos/process/4b-pexec.md`
- QUIRK-001 (this file); STATUS Recent-Decisions 2026-05-03/04 (superseded)
- ARM ARM D8 (TTBR0/TTBR1 translation regimes), D5 (PAN)

## ADR-011: Per-device VDI physical workstation (multi-screen-ready)

**Date:** 2026-06-16
**Status:** Accepted
**Phase:** PHASE-00b (VDI spec) / PHASE-12 (VDI impl) — design intent
locked during spec extraction, before any VDI code exists.

### Context

EmuTOS implements the VDI screen device with **global** state: a single
`phys_work` physical workstation plus Line-A globals that describe *the*
one screen — `v_bas_ad` (screen base), `DEV_TAB`/`xres`/`yres`
(geometry), the palette. Every virtual workstation (`v_opnvwk`) shares
that one screen. The VDI *API*, however, was designed for **multiple
devices**: `v_opnwk(device_id)` opens one physical workstation per
device, `contrl[6]` (the handle) selects it, and `v_opnvwk` opens a
virtual workstation *on* a physical device. The single-screen limit is
purely an implementation choice in EmuTOS, not in the API.

Raspberry Pi 4 has **two HDMI outputs**; QEMU can expose multiple
framebuffer devices (ramfb / virtio-gpu). NewTOS already models the
framebuffer as a per-instance HAL type (`hal::framebuffer::Framebuffer`
is a trait; `RamFb` is a struct — multiple instances are natural).

### Decision

NewTOS implements the VDI **physical workstation as a per-device
struct**, not global Line-A state. Each physical (screen) workstation
owns its own `Framebuffer` instance, geometry/`DEV_TAB`, and palette,
held in a small **screen-device registry** instead of a single
`phys_work` global. Each virtual workstation (`Vwk`) carries a **link
to its physical device** (which screen it draws on); the Line-A
primitives draw into *that device's* framebuffer rather than a global
`v_bas_ad`. `v_opnwk(device_id)` selects the screen device.

### Rationale

- **API stays bit-exact.** The VDI handle model already supports
  multiple physical devices, so no compatibility is sacrificed — only
  EmuTOS's *global screen state* is replaced with *per-device state*.
- **Cheapest now.** Baking the per-device shape into the Phase-12
  design from the start avoids a disruptive globals→per-device refactor
  later. Recording it during spec extraction costs nothing.
- **Real hardware wants it.** The Pi 4's two HDMI ports make multi-
  screen physically reachable, and NewTOS's own software could use it.

### Consequences

- Positive: multi-screen at the VDI level is reachable without an API
  break; the framebuffer HAL already fits (per-instance).
- Negative / scope limit: **AES/desktop** multi-monitor (windows
  spanning screens) is a *much* larger, separate undertaking — classic
  GEM is single-screen. ADR-011 covers the **VDI device layer only**.
- Neutral: classic single-screen TOS programs are unaffected (they use
  one workstation on the default screen device); they neither benefit
  from nor break under the per-device model.

### Alternatives considered

- **Mirror EmuTOS's globals** (single `phys_work` + global Line-A
  vars): simplest, but hardcodes single-screen and would force a
  globals→per-device refactor to ever add a second display. Rejected —
  the per-device shape is nearly free now and expensive later.

### References

- `docs/api/vdi/_dispatcher.md`, `docs/api/vdi/workstation/100-v_opnvwk.md`
- EmuTOS `vdi/vdi_control.c` (`phys_work`, `vwk_ptr[]`, `CUR_WORK`),
  `vdi/lineavars.S` (`v_bas_ad`, `DEV_TAB`)
- `kernel/src/hal/framebuffer.rs` (per-instance framebuffer)

---

> Anticipated further ADRs:
>
> - ADR-001: License (MIT vs Apache-2.0)
> - ADR-002: EmuTOS configuration variant to track (CONF_WITH_TT vs
>   default ST)
> - ADR-003: EmuTOS pinned commit (currently `1a7484ab`, recorded in
>   `docs/STATUS.md`; promote to formal ADR when Phase 00 starts)
> - ADR-004: Native ARM PRG executable format
> - ADR-005: Mxalloc mode-flag handling on uniform RAM
> - ADR-006: VT52 emulation scope

### Template for new ADRs

```markdown
## ADR-NNN: Title

**Date:** YYYY-MM-DD
**Status:** Proposed | Accepted | Superseded by ADR-MMM | Deprecated
**Phase:** PHASE-XX (or "cross-cutting")

### Context

What is the situation that requires a decision? What constraints apply?
What sources were consulted (per SOURCE-ANALYSIS-PROTOCOL.md)?

### Decision

The chosen course of action, stated in one or two sentences.

### Rationale

Why this decision, not the alternatives? What did the source analysis
reveal? What does this optimize for, and what does it trade off?

### Consequences

- Positive: …
- Negative: …
- Neutral: …

### Alternatives considered

For each alternative, why it was rejected. **Do not omit alternatives
that seem obviously wrong** — future readers may not see why obvious.

### References

- Spec sections, datasheet pages, EmuTOS source paths (with SHA)
- Related ADRs
- Discussion thread / commit hash if any
```

---

## Quirks Discovered

Format: `QUIRK-NNN`, dated, with severity (low / medium / high / critical).

## QUIRK-001: QEMU cortex-a72 TCG wedges on TTBR0 switch into a VA-aliased, ASID-0 address space

**Date discovered:** 2026-06-16
**Severity:** High (blocks `Pexec`/`Pterm` round-trip under QEMU)
**Phase / context:** PHASE-06 D3/D6 — `Pexec(PE_LOADGO)` parent/child round-trip
**Status:** Worked around (interim); permanent fix = per-process ASIDs

### Symptom

After a child process calls `Pterm`, the kernel restores the parent's
`TTBR0_EL1` and `eret`s back to the parent at EL0. On QEMU
`aarch64-virt,gic-version=2,cortex-a72` (QEMU 11.0.0) the guest then
makes **no forward progress** — a hard hang, no exception, no further
trap. The parent's next instruction (its own `Pterm` SVC) is never
observed. Deterministic.

### Cause

Parent and child are both loaded by the minimal Phase-06 D3 loader with
their TPA (and `e_entry`) at the **same** user VA (`0x100`), and
`install_ttbr0` writes the L0 phys to `TTBR0_EL1` with **ASID = 0** for
every process. So the parent's and the child's distinct physical code
pages live at the *same* VA under the *same* ASID. QEMU's TCG model
keeps the child's already-translated block for VA `0x118` and reuses it
when the parent re-enters that VA across the `TTBR0` switch — the
`tlbi vmalle1` in `restore_boot_ttbr0` flushes the soft-TLB but does not
get TCG to re-walk before the immediately-following `eret`-to-EL0, so
the model wedges. On real ARMv8 hardware this cannot happen: the L1
I-cache is PIPT, so different physical pages never alias, and the
ordering is immaterial.

### Resolution

**Interim (committed):** emit one line of UART traffic (Device-memory
*writes*) **between** the `restore_boot_ttbr0()` TLBI and the
`install_ttbr0(parent)` in the `Pterm` handler
(`kernel/src/arch/aarch64/exceptions.rs::handle_pterm`). A Device
access forces TCG to end the translation block and re-walk under the
new `TTBR0`. Verified deterministic across 4/4 QEMU runs.
`handle_pexec` already relied on exactly this (its `[pexec] …` prints
sit in the same spot). Empirically: a single Device *read* does **not**
suffice, and pure arithmetic/store "distance" between the TLBI and the
`eret` does **not** suffice — only the write traffic of a console line.

**Per-process ASIDs — ATTEMPTED 2026-06-16, does NOT work on this QEMU.**
The architecturally correct fix is a distinct ASID per process in
`TTBR0_EL1[55:48]` (`TCR_EL1.A1 = 0` already makes TTBR0 supply the
ASID; `AS = 0` gives 8-bit ASIDs, enough for `MAX_PROCESSES = 16`).
On real hardware that would let TCG/TLB distinguish the same-VA address
spaces. **But QEMU 11.0.0's cortex-a72 TCG wedges on the ASID-tagged
`TTBR0_EL1` write itself** — tagging the EC4 install with a non-zero
ASID hangs at the `msr ttbr0_el1` / `isb`, *before* any `eret`, so it
makes things strictly worse than ASID 0. Reverted in full. **Key
insight:** the QUIRK-001 wedge is purely a QEMU artifact (real ARMv8
L1 I-caches are PIPT and never alias across distinct physical pages),
so ASIDs are **not needed to fix the wedge** at all — the interim
Device-write barrier is the correct long-term solution for the QEMU
test target (a harmless MMIO read on real hardware). ASIDs remain
worth adding later purely for real-hardware TLB efficiency, and should
be validated on real RPi4 / a newer QEMU, not against this model.

So: **do not re-attempt ASIDs to "fix" QUIRK-001** — that is solved by
the barrier. Keep the barrier; it is correct on both targets.

### Why this is hard to rediscover

The failure is a silent hang with no exception, and it *disappears the
moment you add a debug print* near the TTBR0 switch — so naive "add a
print to see where it dies" debugging makes the bug vanish and hides the
real cause. The load-bearing nature of the print is the trap.

### Source

QEMU 11.0.0 TCG behaviour, established by bisecting the hang with
incrementally-placed UART writes (a Device *read* alone did not fix it;
a `kprintln` between TLBI and install did). ASID mechanics per ARM ARM
D8 (TTBR/TCR ASID fields). Cross-checked against `handle_pexec`, which
exhibits and silently works around the same quirk.

### Other quirks worth recording when hit

> - "BCM2711 PL011 RX FIFO trigger level must be set before enabling
>   UART, not after, or the first byte is lost."
> - "EmuTOS's `Fopen` returns `EFILNF` (-33) for empty filenames, but
>   Compendium says 'undefined' — we follow EmuTOS."
> - "QEMU `aarch64-virt` PL011 base address differs from RPi4
>   (0x0900_0000 vs 0xFE20_1000) — must be compile-time selected."
> - "BCM2711 GPIO pull-up/down uses `GPIO_PUP_PDN_CNTRL_REGn` at
>   `0xFE20_00E4`, **not** the older BCM2837 `GPPUD` sequence."

### Template for new quirks

```markdown
## QUIRK-NNN: Short descriptive title

**Date discovered:** YYYY-MM-DD
**Severity:** Low | Medium | High | Critical
**Phase / context:** Where it bit us
**Status:** Open | Worked around | Fixed | Documented

### Symptom

What we observed. Specific, reproducible.

### Cause

What was actually happening, after analysis.

### Resolution

How we fixed or worked around it. Code reference if applicable
(commit hash, file:line).

### Why this is hard to rediscover

What made this non-obvious? What would a future contributor (or a fresh
agent session) miss?

### Source

How was the truth ultimately established? Datasheet section?
EmuTOS source comparison? Test program?
```

---

## Lessons Learned

Process and workflow lessons.

### LESSON-001: Source-first prevents drift

**Context:** This file itself, and `docs/SOURCE-ANALYSIS-PROTOCOL.md`.

**Lesson:** AI assistants and humans both default to "I know this" when
working on familiar APIs. The TOS API *seems* familiar to anyone with
Atari background, but plausible-sounding behavior is wrong often enough
that every spec entry must be source-backed. The protocol is annoying
but non-negotiable.

---

### LESSON-002: `rust-lld` does not accept `-nostartfiles`

**Context:** Phase 01, kernel `build.rs`.

**Lesson:** Passing `-nostartfiles` (a GCC/ld driver flag) to `rust-lld`
produces "unknown argument" and breaks the link. The `ENTRY()` directive
in the linker script is sufficient on its own; no equivalent flag is
needed.

---

### LESSON-003: Naked functions are stable since Rust 1.88

**Context:** Phase 01 `boot.rs`.

**Lesson:** `#[unsafe(naked)]` + `core::arch::naked_asm!` works on
stable-style nightly without any `#![feature(naked_functions)]` gate.
Adding the feature gate now produces a deprecation/already-stable
warning.

---

## Rejected Alternatives

Things we considered and chose not to do, with reasoning.

### REJECTED-001: Direct port of EmuTOS C source to AArch64

**Considered:** 2026-05-02 (initial planning)
**Status:** Rejected in favor of clean reimplementation

**Why rejected:** EmuTOS contains 25+ years of accreted m68k assumptions
(endianness, stack frames, register-direct optimisations). A direct port
would carry all of these implicit assumptions forward as latent bugs on
ARM. A clean reimplementation against the extracted TOS spec, with EmuTOS
source as the *primary reference*, surfaces these assumptions instead of
hiding them.

**What would change our mind:** Nothing realistic. This decision is
foundational.

---

### REJECTED-002: Building a project-specific MCP server

**Considered:** 2026-05-02 (early planning)
**Status:** Rejected; revisit only if a concrete need emerges

**Why rejected:** A custom MCP server exposing TOS API specs as
structured tools would be elegant but solves no problem the agent
cannot solve by reading Markdown files directly. Maintenance cost
would exceed convenience benefit.

---

### REJECTED-003: m68k JIT in Phase 10

**Considered:** During Phase 10 design
**Status:** Deferred to a possible later phase

**Why rejected:** Interpreter throughput on modern ARM should easily
exceed 16 MHz 68000 equivalent for TOS-typical workloads (which spend
most time in OS calls, not in interpreted instructions). JIT adds
substantial complexity (code-cache, invalidation, debuggability) that
is not justified until measurements show the interpreter as the
bottleneck.

---

### REJECTED-004: `bindgen` over EmuTOS C headers

**Considered:** 2026-05-02
**Status:** Rejected (also in CLAUDE.md "Tech Stack")

**Why rejected:** `bindgen` would tempt us into linking C-defined types
across the m68k/ARM ABI boundary. NewTOS reimplements TOS cleanly in
Rust; types are defined in `tos-types` against the spec in `docs/api/`,
not lifted from EmuTOS headers.
