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

## ADR-012: Cookie-jar access mechanism on NewTOS

**Date:** 2026-06-20
**Status:** **Accepted** (2026-06-20). Unblocks PHASE-08 D5 (spec next).
**Phase:** PHASE-08 (cookie jar).

### Context

The TOS cookie jar is a feature registry: an array of `{tag, value}`
entries (terminated by `{0, capacity}`) that programs scan to detect
system features (`_MCH`, `_IDT`, `_FLK`, …). Full data-model extraction:
[`docs/api/bios/_cookie-jar.md`](api/bios/_cookie-jar.md).

There is **no trap** for it. Classic access is: read the system-variable
pointer at **`$5A0`** (`_p_cookies`) and linear-scan; because `$5A0` is
supervisor-only low memory, applications elevate via **`Supexec`**
(XBIOS 0x26). NewTOS blocks this path **twice**:

1. `$5A0` is not mapped — Atari low-memory sysvars are not given AArch64
   addresses (same call as the `$420`/`$432` vars: direct poke is out of
   scope, m68k-emulator-only).
2. `Supexec` is itself deferred (Open-ADR *ADR-Supexec*).

So NewTOS needs its **own** cookie set (the still-meaningful tags + a
NewTOS identity cookie; the Shifter/PSG/FDC/FPU tags are meaningless) and
a **native access mechanism** replacing `Supexec` + `$5A0`.

### Decision

**A kernel-owned cookie jar, read back through a function (value-query),
never by direct memory access.**

- **Storage.** An init routine allocates the jar from the **kernel heap**
  (available since Phase 03) and stores the pointer in a **kernel-internal
  global** (the NewTOS analogue of EmuTOS' `p_cookies`, but *not* the
  `$5A0` sysvar). The jar lives in high-half (TTBR1) kernel memory; its
  address is arbitrary and irrelevant.
  *Implementation note (2026-06-20):* `tos-api` is `#![no_std]` **without
  `alloc`**, so the jar is a **fixed-capacity static array** (32 slots,
  `tos-api/src/cookie.rs`) rather than a heap allocation — this is also
  closer to FreeMiNT's own fixed `dflt_jar[]`. Growing past the cap would
  need the kernel allocator; deferred until a real need appears.
- **Access.** A function returns the **content**, not a pointer:
  `Getcookie(tag) -> (found, value)`. The kernel scans its own jar and
  returns only the value. Because the jar is kernel-heap memory (EL1-only,
  not mapped at EL0), a raw pointer would be unusable to a user program
  anyway — so value-return is the *forced* and cleanest choice. A
  companion `Setcookie(tag, value)` adds/updates an entry (the kernel
  owns and serialises the jar). Optional enumeration (`Getcookies(buf)`
  copy-out) only if a real program needs to list all cookies; deferred
  until then.
- **No direct access.** No `$5A0`, no other fixed sysvar address, no
  `Supexec` — none of the three NewTOS blockers is touched.
- **Porting.** Ported C programs are adapted at port time anyway; a tiny
  shim maps the classic `getcookie()`/`$5A0` idiom onto `Getcookie`.

**Syscall placement (decided 2026-06-20):** **FreeMiNT `Ssystem`** —
GEMDOS `0x154`, `Ssystem(WORD mode, LONG arg1, LONG arg2)`, with modes
`S_GETCOOKIE`(8) / `S_SETCOOKIE`(9) / `S_DELCOOKIE`(26). NewTOS adopts the
MiNT ABI verbatim so the MiNT/GEM software base runs (see
[[freemint-extension-direction]]); FreeMiNT is now a sanctioned source
(`references/freemint` @ `f906416`). ABI + cookie semantics extracted in
[`docs/api/gemdos/154-ssystem.md`](api/gemdos/154-ssystem.md).

### Rationale

The user's framing settled it: the kernel must allocate the jar, so store
the pointer kernel-side and hand back *content* through a function instead
of exposing the address. This matches the project's altitude (TOS **API**
compatibility, not memory-map emulation; CLAUDE.md non-goals), needs no
user-space mapping and no `Supexec`, and is implementable now. Apps being
adapted during porting removes the only downside (loss of literal `$5A0`
bit-compatibility), which a one-line shim covers.

### Consequences

- The data model (`_cookie-jar.md`) is access-agnostic and unchanged;
  this ADR fixes the **access path** (value-query function) + **storage**
  (kernel-heap, kernel-held pointer).
- NewTOS defines its own cookie content: keep `_MCH` (NewTOS machine id),
  `_IDT`, `_FLK`, `_AKP`; **drop** `_VDO`/`_SND`/`_FDC`/`_FPU` (Shifter/
  PSG/floppy/m68k-FPU — meaningless); add an `NTOS`/`_NEW` identity +
  version cookie so ported programs can detect NewTOS.
- `Getcookie`/`Setcookie` are NewTOS extensions; the chosen function id(s)
  must be documented as reserved.

### Alternatives considered

- **Map a `$5A0` (or any fixed) sysvar page so apps read+scan it**
  directly: rejected. The address itself is a non-issue once access is
  mediated, but letting apps *scan* the jar requires it to be readable
  from EL0 — a shared read-only user mapping per process (fiddly under the
  per-process-TTBR0 interim, adjacent to ADR-010) — and still wants
  `Supexec` for the classic idiom. Value-query needs none of that.
- **Mirror EmuTOS verbatim** (`$5A0` + `Supexec`): impossible — both
  prerequisites are absent/deferred on NewTOS.
- **Defer entirely**: rejected; the mechanism is now decided.

### References

- `docs/api/bios/_cookie-jar.md` (data model + access extraction)
- EmuTOS `util/cookie.c`, `bios/machine.c:665`, `cli/cmdutil.c:321`
- Open-ADR *ADR-Supexec*; `$420`-vars decision (STATUS Recent-Decisions
  2026-05-03); ADR-010 (low-memory aliasing on QEMU)

---

## ADR-013: Native `.RSC` resource format (not classic Atari)

**Date:** 2026-06-21
**Status:** **Accepted** (2026-06-21, user decision).
**Phase:** PHASE-13 D7 (Resource Manager).

### Context

`rsrc_load` must load a `.RSC` resource (object trees, strings, TEDINFOs,
icons) for GEM programs. A classic Atari `.RSC` is **big-endian m68k**,
with a **24-byte 16-bit OBJECT**, 16-bit internal offsets, and char-grid
coordinates fixed up by `rsrc_obfix`. NewTOS uses a **little-endian
64-bit OBJECT (32 bytes, 64-bit `ob_spec`)** and pixel coordinates.

### Decision

NewTOS defines and loads a **native `.RSC` format**: our own 64-bit
OBJECT layout, little-endian, internal pointers stored as **byte offsets
from the blob base**, coordinates already in pixels. Loading is a pure
**offset→pointer relocation** — no endianness swap, no 16→64-bit
expansion. `rsrc_obfix` is an identity (coords are already pixels).

NewTOS-native (AArch64) programs ship native `.RSC` files built by a host
tool; this matches the clean-room, recompile-for-AArch64 model. Loading
classic Atari `.RSC` files (full byte-swap + 24→32-byte expansion) is
**not** done now — it would belong with the optional m68k userland
emulator phase, if ever.

### Consequences

- Loader is small and fast (relocation = add base to offset fields,
  type-aware for `ob_spec`).
- Needs a **host tool** to author native `.RSC` (no ORCS/Interface);
  Batch 2 deliverable.
- Existing Atari `.RSC` files are **not** directly loadable.
- The format header is `NrscHeader` (magic `"NRSC"`); see
  [`tos-api/src/aes/resource.rs`].

### Alternatives rejected

- **Classic Atari `.RSC`**: full big-endian + 16→64-bit conversion on
  load. More code; valuable only for unmodified Atari binaries, which are
  out of scope until the m68k emulator.

### References

- `docs/api/aes/resource-fileselector.md`; EmuTOS `aes/gemrslib.c`,
  `include/rsdefs.h`

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

## QUIRK-002: GIC-400 SPIs reset to Group 0 (secure) — peripheral IRQs not delivered to the NS kernel on real RPi4

### Symptom

On real RPi4 silicon, the PL011 UART0 RX interrupt (INTID 153) is never
delivered to the core, although the identical path works in QEMU. TX and
polled RX work; only IRQ delivery fails. Diagnosed 2026-06-20 with a
register dump (`gic_diag.rs`, since removed): `GICD_CTLR=1`, `GICC_CTLR=1`,
`ISENABLER[153]=1`, `ITARGETSR=CPU0`, `ICFGR=level`, `PL011 IMSC` RX
unmasked — all correct — but **`GICD_IGROUPR[153] = 0` (Group 0 /
secure)** and `GICC_PMR` reads back `0xF0` (the non-secure view).

### Cause

The GIC-400 implements the GICv2 Security Extensions. **At reset every
interrupt is in Group 0 (secure)** → signalled as **FIQ to secure**, not
as IRQ to the Non-secure kernel. The RPi4 firmware drops the kernel to
**Non-secure EL1**, and **Non-secure software cannot move interrupts to
Group 1** (`GICD_IGROUPR` is RAZ/WI from NS). Only secure boot code (an
armstub) can set Group 1. QEMU's GICv2 model has no effective security
split, so it "just works" there. This is general — it blocks **every**
peripheral IRQ on this firmware (PL011, a future UART2 IKBD IRQ, timer,
…), not just the console.

### Resolution

**Not fixed — and deliberately not pursued.** The serial console is a
**development-only** tool; the production system takes input from the
UART2 IKBD keyboard/mouse (already polled) and outputs to HDMI. Console
RX uses the **UART0 poll workaround** (`console::pl011_rx_poll` in the
blocking read + `pump_input`), which is fully functional. The IRQ buys
almost nothing in the current cooperative single-app model; it only
matters later (preemption, smooth mouse via background FIFO drain).

A self-built `armstub8-gic.bin` (raspberrypi/tools `armstub8.S`
`-DGIC=1 -DBCM2711=1`, which writes `IGROUPR=0xFFFFFFFF`) was tried but
**left IGROUPR=0** — it did not take effect on this firmware (reason
unconfirmed: the firmware may re-init the GIC after the stub, or the
self-built stub's layout mismatches what the firmware expects). Reverted.
**Correction:** a framebuffer + memory-split regression first blamed on
the armstub was actually an unrelated mistake — copying the project's
`config.txt` template over the card dropped `gpu_mem=256`, so the
VideoCore framebuffer went away and the ARM saw ~977 MiB instead of
~768 MiB. Restored `gpu_mem=256` (+ the `hdmi_*` lines) into
`boot/rpi4/config.txt`; the armstub itself broke nothing.

**When revisited** (multitasking / smooth mouse): get the GIC into
Group 1 via secure boot code — a *matching* prebuilt GIC armstub for the
installed firmware, or a firmware update, tested against the framebuffer.
To re-diagnose, dump `IGROUPR`/`ISPENDR`/`IPRIORITYR` at the GICD
(`0xFF841000`) and PL011 `RIS`/`MIS` (`0xFE201000`) while a key sits
undrained in the FIFO.

### Source

BCM2711 ARM Peripherals §6 (GIC-400); ARM IHI 0048B §1.4 (GICv2 security,
Group 0/1 → FIQ/IRQ); raspberrypi/tools `armstubs/armstub8.S` `setup_gic`.

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
