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

### Addendum 2026-07-20 (task #108): free-string / free-image tables

The `NrscHeader` gained a **free-string** (`frstr`) and a **free-image**
(`frimg`) pointer table so `rsrc_gaddr`'s `R_FRSTR`/`R_STRING`/`R_FRIMG`/
`R_IMAGEDATA` selectors resolve (QED fetches its alert table via
`rsrc_gaddr(R_FRSTR, …)`; without it `alertmsg` was NULL and cflib panicked).
Layout: after the ICONBLKs come `[ptr; n_frstr]` then `[ptr; n_frimg]`, each
slot a blob offset (string data for frstr, a BITBLK for frimg — a classic free
image *is* a BITBLK) relocated to a user VA at the app's ABI width (8 B LP64 /
4 B ILP32, matching `ob_spec`). The header grew from 28 to 44 bytes and the
`version` field was bumped: **1 = LP64/28 B, 2 = ILP32/28 B (legacy, no
tables); 3 = LP64/44 B, 4 = ILP32/44 B (with tables)**. The loader is
**backward-compatible**: for `version < 3` it treats the free-string/-image
counts as 0, so pre-existing v1/v2 resources (GEM.RSC, DESK.RSC) load unchanged
with those selectors reporting the honest "not found". Emitted by
`tools/mkrsc.py convert [--ilp32]`; relocated in `relocate_blob_core` /
`relocate_blob_core_ilp32`; registered/served in `relocate_and_register_core` /
`rsrc_gaddr` (`tos-api/src/aes/resource.rs`).

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

## ADR-014: All MMIO through the high-half; TTBR0 holds no low-half identity

**Date:** 2026-06-25
**Status:** Accepted
**Phase:** PHASE-06 / PHASE-14 (cross-cutting, MMU + drivers)

### Context

The first EL0 program launch (AUTO folder / desktop) worked in QEMU but
faulted on real Pi4: Instruction Abort, lower EL, **permission fault,
level 1**, at the entry VA `0x100` (`ESR=0x8200000d`). The Stage-1 boot
identity map covers 0..4 GiB with 1-GiB **UXN** L1 blocks and stayed live in
`TTBR0` because the kernel reached MMIO (UART, eMMC, mailbox, GIC) through
the low-half identity. The Cortex-A72 speculatively (re-)cached a UXN block
for the low-half; `install_ttbr0(slot)` swaps `TTBR0` to the slot tables but
**cannot flush the TLB** — a `tlbi` on the eret-to-EL0 path wedges QEMU
(QUIRK-001) — so the stale UXN block shadowed the slot's L3 page and the EL0
fetch faulted. QEMU flushes its TLB on the `TTBR0` write, masking it.

Two failed fixes confirmed the constraints: a `tlbi` in the launch path
wedged QEMU; emptying `TTBR0` while MMIO was still low-half hung the kernel
at the first UART write.

### Decision

Reach **every** peripheral register through the high-half (TTBR1) Device
mapping (`drivers::mmio_va(pa) = pa + VA_OFFSET`). With MMIO independent of
`TTBR0`, `install_stage2_ttbr1` points `TTBR0` at an all-invalid root
(`EMPTY_L0`) after the high-half switch: no UXN identity block remains to be
cached, and per-process `install_ttbr0` swaps in slot tables cleanly with no
TLBI on the eret path. The pre-MMU boot heartbeat UART stays physical.

### Rationale

- Eliminates the fault at the root rather than fighting QUIRK-001.
- A kernel should not depend on a low-half identity at runtime; high-half
  MMIO is the clean design and also lets the kernel touch MMIO while a
  process owns `TTBR0`.
- QEMU now exercises the **same** path as real HW (high-half MMIO + empty
  TTBR0), so it validates the launch path instead of hiding the bug.

### Consequences

- Also required: freshly-loaded code needs D-cache-clean-to-PoU +
  I-cache-invalidate (ARM ARM B2.4.4) and table writes need a `dsb` before
  the walk (D5.10.1) — real-HW-only, no-ops under QEMU's cacheless model.
- `remap_normal_nc` must map **invalid** blocks too: the VideoCore puts the
  framebuffer in reserved memory above `dram.end`, which Stage-2 leaves
  unmapped; the old "convert only valid Normal blocks" guard skipped it and
  the desktop draw took a translation fault. (8-GiB SKUs with a framebuffer
  above 4 GiB still need a `NUM_L2` coverage bump.)
- **Verified end-to-end on real Pi4 (2026-06-25):** init screen → AUTO
  program loads from SD, runs at EL0, Pterm → desktop with 4 drives, no
  exception.

### References

- QUIRK-001 (this file); commits `7231e9d` (high-half MMIO + empty TTBR0),
  `c91c9e5` (framebuffer mapping), `3f93fde` (cache/barrier maintenance).

## ADR-015: USB-HID input — xHCI host on Pi4, upper stack clean-room from FreeMiNT

**Date:** 2026-06-25
**Status:** Accepted
**Phase:** PHASE-16 (USB-HID input)

### Context

The desktop renders on real Pi4 but is not interactive — there is no
keyboard/mouse. The native Atari IKBD plan (a vintage 5 V keyboard on UART2)
proved too fiddly (voltage divider, TX level-shifter, uncertain HD6301
hardware reset). A USB host stack is the "right" answer but is large.

Two reference bodies make it tractable:
- **FreeMiNT's USB stack** (`references/freemint/sys/usb`, a sanctioned
  source) is a **TOS-native, modular** USB stack: a host-controller layer
  (`struct ucdif` + `ucd_register`), a hardware-independent core + hub +
  enumeration (`usb.c`/`hub.c`), class drivers (`udd/`), and — crucially —
  HID keyboard/mouse drivers that **translate USB HID → Atari scancodes and
  inject them into the IKBD vector** (`udd/hid/keyboard/usb_keyboard.c`:
  `Setexc(0x114/4, …)`, `SEND_SCAN`). Only the `ucdif` host-controller layer
  is hardware-specific.
- **Circle** (C++, `rsta2/circle`) + the no_std **`rust-osdev/xhci`** crate
  are the references for the Pi4 host controller itself.

### Decision

Target **xHCI** (the VL805 controller behind the four USB-A ports, MMIO
`0xFE9C0000`), not DWC2. Build the stack as: (1) clean-room reimplement
FreeMiNT's upper stack in Rust (core, hub, enumeration, HID keyboard/mouse,
HID→IKBD injection) per the source-analysis protocol — exactly the EmuTOS
methodology; (2) write one Pi4 xHCI host-controller driver (the `ucdif`
equivalent) on top of `rust-osdev/xhci`, with Circle as the behavioural
reference and the xHCI spec as Track-B; (3) feed HID output into NewTOS's
**existing** IKBD sinks (`ikbd.rs` → console RX + `vdi::feed_relative`
cursor), so the desktop is source-agnostic.

### Rationale

- **xHCI = the real ports.** A keyboard + mouse plug into the normal USB-A
  ports; power stays on USB-C. DWC2 would only drive the USB-C/OTG port
  (also the power input) and need a hub — awkward as a product.
- **QEMU validates the whole path.** `qemu-virt` emulates `qemu-xhci` with
  `-device usb-kbd`/`-device usb-mouse`, so the entire stack (controller +
  core + hub + HID) is developed and tested **interactively in the emulator**
  — the same "QEMU exercises the real path" win as ADR-014. Real Pi4 then
  adds only the VL805 firmware-load (via the existing mailbox driver), which
  QEMU doesn't need. DWC2 has no equivalent qemu-virt dev loop.
- **~70 % already specified.** FreeMiNT supplies the architecture and the
  TOS-native HID→IKBD integration; NewTOS already owns the IKBD-sink side.

### Consequences

- Reuses the existing IKBD decoder's output side untouched; the USB-HID
  driver maps HID→Atari-scancode, then the existing path takes over.
- Real-HW bring-up needs VL805 firmware bootstrap via mailbox (have the
  driver) + xHCI at `0xFE9C0000` (a fixed MMIO the firmware maps; no full
  PCIe enumeration required).
- The IKBD/UART2 plan is shelved (kept only as a fallback note).

### References

- `references/freemint/sys/usb` (sanctioned source); `rsta2/circle`;
  `rust-osdev/xhci`; xHCI spec; RPi firmware VL805 mailbox interface.
  See ADR-014 (high-half MMIO — the mailbox/MMIO groundwork this builds on)
  and [[keyboard-via-atari-ikbd]] (the shelved IKBD path).

## ADR-016: Ground-up desktop rebuild — EmuTOS-faithful, in `kernel/src/desk/`

**Date:** 2026-07-04
**Status:** **Accepted** (user decision 2026-07-04).
**Phase:** PHASE-14 (Desktop).

### Context

The hand-rolled desktop (`kernel/src/desktop.rs` ~2329 lines +
`kernel/src/desktop_icons.rs` ~442 lines) keeps producing rendering defects:
icon-view 8.3 label overlap on real Pi-4 (`CONFIG.TXFIXUP4.DARERNEL8.IME…`),
view-switch (text↔icon) residue, and a redraw path that repaints by handle
order with a full-screen clip instead of honoring the window's damage-rectangle
list. The root causes sit in the **ad-hoc directory-window subsystem** (its own
`DIRTREE`/`build_dir_*`/`draw_dir_listing`/`redraw_*`/`VIEW_MODE`/`SORT_MODE`),
**not** in the AES/VDI foundation, which is EmuTOS-aligned. An earlier QMP
screendump "passed" only because the test image used short names — the
verification was not representative. A targeted patch was drafted
(`~/.claude/plans/mossy-knitting-scone.md`), but after repeated regressions the
user judged the desktop untrustworthy and chose a full ground-up rebuild.

### Decision

Rebuild the entire desktop **application** from scratch as an EmuTOS-faithful
reimplementation in a **new module `kernel/src/desk/`**, mirroring EmuTOS
`desk/` (`deskmain`/`deskapp`/`deskwin`/`desksupp`/`deskdir`/`deskfun`/
`deskact`/`deskins`/`deskinf`/`deskrez`). `desk::run()` replaces
`desktop::run()` at boot. Keep only the OS layer below it — AES/VDI/GEMDOS in
`tos-api` — which is the compatibility contract, **not** "the desktop". The old
`desktop.rs`/`desktop_icons.rs` are declared **deprecated** and stay in git
until the new module reaches feature parity, then are removed. Extraction of
`references/emutos/desk/` into `docs/api/desktop/` **precedes** implementation
(Rule 5, spec before code).

### Rationale

The bug class is structural — an ad-hoc data model + view builder + a redraw
that ignores the real AES rectangle list. Faithfully re-deriving the EmuTOS
window/file data model (`FNODE`/`WNODE`), grid-metric globals (`win_view`), view
builder (`win_bldview`) and damage-rect redraw (`do_wredraw`) on top of the
already-faithful AES/VDI kills the class at the root instead of patching the
three symptoms currently visible. A **module** (chosen over a separate crate,
user decision) gives a clean home and a clear old/new boundary with minimal
workspace-wiring churn.

### Consequences

- Positive: a clean, EmuTOS-conformant desktop; the label-overlap / redraw bug
  class eliminated at the root; the desktop is spec-backed (`docs/api/desktop/`)
  for the first time.
- Positive: reuse of the verified AES/VDI/GEMDOS layer — no contract risk.
- Negative: substantial reimplementation (~full desktop app); two
  implementations coexist until parity (old deprecated + guarded off).
- Negative: as a module (not a crate) the desktop *can* still reach kernel
  internals; the EmuTOS discipline (go through AES/VDI/BIOS calls, as `desk/`
  uses `aesbind`) must be enforced by **review**, not by the compiler — this is
  the Auditor's standing charge.
- Neutral: supersedes the partial `mossy-knitting-scone` plan.

### Alternatives considered

- **Separate `desk/` crate:** the strongest EmuTOS-faithful boundary
  (compiler-enforced app/OS split) but more workspace wiring and API-surface
  exposure. Rejected by the user in favor of the lighter module; revisit if the
  boundary needs enforcing.
- **Targeted patch (`mossy-knitting-scone`):** fix `draw_text` clip + icon-cell
  metric + redraw only, keep the rest. Rejected: the user judged the desktop
  untrustworthy after repeated regressions; a rebuild removes the whole ad-hoc
  subsystem rather than the three symptoms currently visible.
- **Keep everything, patch in place:** rejected — same reason; leaves the ad-hoc
  model that keeps regressing.

### Orchestration (process)

Executed by a **three-role subagent team** under team-lead (Claude) supervision,
defined in `.claude/agents/`:
- **Researcher** (Sonnet) — extracts `references/emutos/desk/` → `docs/api/desktop/`
  per SOURCE-ANALYSIS-PROTOCOL + defect catalog of the current desktop; **no
  product code**.
- **Developer** (Opus) — writes **all** Rust/AArch64 code in `kernel/src/desk/`;
  **no stubs**.
- **Auditor** (Opus) — reviews Developer code against spec + COMPATIBILITY,
  adversarially verifies (representative data), reports findings; **no code**.

Model rule: simple tasks on Sonnet, **code exclusively on Opus**.

### References

- `references/emutos/desk/` (`deskmain.c`/`deskapp.c`/`deskwin.c`/`desksupp.c`/
  `deskdir.c`/`deskinf.c`/`deskrez.c`/`deskfpd.c`/…)
- `~/.claude/plans/mossy-knitting-scone.md` (superseded)
- `docs/api/desktop.md` (to be expanded into `docs/api/desktop/`)
- `docs/COMPATIBILITY.md` (desktop = app; AES/VDI/GEMDOS = contract)
- STATUS Recent-Decisions 2026-07-03 (icon-overlap partial fix — now subsumed)
- `.claude/agents/{researcher,developer,auditor}.md`

## ADR-017: NewTOS resolution policy — native, geometry-independent, default 1920×1080, switchable to 1280×800

**Date:** 2026-07-09 (revises a same-day, never-committed draft of this ADR —
see "Superseded draft" below).
**Status:** **Accepted** (user course-correction 2026-07-09, confirmed via
team lead).
**Phase:** cross-cutting (framebuffer driver, VDI workstation, AES globals,
desktop `deskapp`/`deskwin`/`deskrez`, XBIOS screen group).

### Revision 2 (2026-07-16) — config.txt-driven, no in-menu switch

The 2026-07-09 policy kept an *in-menu runtime resolution switch* (deskrez ->
`shel_write(SHW_RESCHNG)` -> reinit) as on classic TOS. On reflection with the
user (2026-07-16) that is **dropped**: on Atari, rez switching mattered for
gaming (ST Low/Med/High; programs demanded a specific mode). On NewTOS/Pi4 the
**firmware `config.txt` owns the HDMI mode** (`boot/rpi4/config.txt`
`hdmi_group`/`hdmi_mode`), and a framebuffer-only in-menu switch to a *different*
geometry would merely be **scaled** by the VideoCore to the fixed HDMI signal —
not native. So:

- **Resolution is driven by `config.txt`.** NewTOS **auto-detects** the display
  geometry from the VideoCore at boot (mailbox *get physical (display)
  width/height*, tag `0x00040003`) and renders natively at whatever `config.txt`
  set. `drivers::BOOT_WIDTH/BOOT_HEIGHT` (1920x1080) becomes the **fallback** —
  used when the query returns 0/none and as the **QEMU/ramfb default** (ramfb has
  no firmware display to query; the guest defines the geometry). Change the panel
  resolution by editing `config.txt`; NewTOS follows with no code change.
- **The in-menu switch (deskrez runtime switch) is not implemented.** `Getrez`
  reports the auto-detected mode honestly; there is **no in-system `Setscreen`
  rez change**. (This narrows Decision point 6 / the still-open Getrez question:
  Getrez only *reports*, it does not *switch*.)
- **Config surface = exactly two files:** `config.txt` (firmware: HDMI/resolution,
  `gpu_mem`, `kernel=`, UART, DTB) and `NEWDESK.INF` on C: (desktop state:
  background, view/sort, drive icons `#M`, installed apps `#G/#P/#F/#Y`, autorun
  `#Z`). No third NewTOS-specific config.
- **Wallpaper** (a full-screen background image, a NewTOS extension) is a separate
  future feature: a `NEWDESK.INF` path + an image loader + the desktop background
  painter (`paint_desktop_bg`). It is sensibly done **after** the truecolor VDI
  step — a photo wallpaper in the current 16-colour palette would dither badly.

Everything below about *geometry-independence*, the *arena/cache blockers*, and
the native-vs-16-colour split still holds; only the "runtime switchable via menu"
mechanism is replaced by "config.txt-driven + boot-time auto-detect".

### Superseded draft

Earlier the same day this ADR was drafted — and never committed — as
"**single fixed 640×480 mode, truthfully `TT_MEDIUM`**": exactly one
hardcoded VDI resolution forever, `Getrez` corrected from `2` to `4`,
`Setscreen`'s range widened to EmuTOS's flat `[0,7]` bound with every
non-`4` code accepted-but-ignored, and native/1080p geometry filed as an
explicitly deferred future option. The user overrode this direction before
it was committed: NewTOS presents a **native, geometry-independent**
resolution — default **1920×1080**, switchable at runtime to **1280×800** —
not a single fixed 640×480 canvas. The EmuTOS source analysis performed for
the superseded draft (its own "silently ignore an unsupported `rez`"
precedent, `bios/screen.c:990-1013`, `bios/videl.c:619-659,976-993`) is
still factually correct and is kept below where it still bears on the
still-open `Getrez`/`Setscreen` question (see Decision point 6); the
**policy conclusion built on top of it — "there is only one mode, ever" —
is replaced.**

### Context

NewTOS's physical framebuffer is currently a **compile-time-fixed** 640×480,
32bpp truecolor VideoCore mailbox buffer (`kernel/src/drivers/bcm2711/
framebuffer.rs:23-24` `const WIDTH: u32 = 640; const HEIGHT: u32 = 480;`,
mirrored in the QEMU dev-vehicle `kernel/src/drivers/qemu_virt/ramfb.rs:75-76`).
This is an **implementation** limit, not a **hardware** one: the RPi4's own
boot firmware config already forces a 1920×1080@60 Hz HDMI signal
(`boot/rpi4/config.txt:12-16`, `hdmi_group=2`/`hdmi_mode=82` = "DMT
1080p60", per the file's own comment), and the mailbox property tags the
driver already speaks (`set physical/virtual size`, `set depth`, `allocate`,
`get pitch` — `kernel/src/drivers/bcm2711/mailbox.rs`,
`docs/hardware/mailbox-framebuffer.md` §3) are fully parametric; only the
values fed into them are hardcoded today. The superseded draft's central
premise — "the framebuffer is fixed at boot, there is no second mode to
switch into" — conflated *what the code currently asks for* with *what the
hardware and firmware config can do*; this rewrite corrects that premise.

A Researcher code survey (2026-07-09, same session) of `tos-api::vdi`,
`tos-api::aes` and `kernel::desk` found that **most of the VDI/AES stack is
already geometry-independent**: `RasterBackend::width()/height()`
(`kernel/src/vdi_backend.rs:130-136`) and `Host::screen()`
(`kernel/src/host_env.rs:262-273`) both re-derive from the live framebuffer
atomics on every call — nothing caches 640/480 as a value, only
`kernel/src/drivers/bcm2711/framebuffer.rs`'s `WIDTH`/`HEIGHT` **constants**
feed those atomics once at boot. `tos-api/src/vdi/workstation.rs:100-117,
143-160` (`DEV_TAB`/`INQ_TAB` patching), `tos-api/src/aes/{graf,form,fsel,
menu,appl,window}.rs` (screen-rect/centring/planes) all read
`backend.width()/height()` fresh per call; `tos-api/src/aes/window.rs:1007-
1013`'s `640, 480` is an unreachable-in-practice fallback (no backend
installed), not the live path. The full file:line inventory — including the
few genuine gaps found (see Consequences) — was handed to the team lead
alongside this ADR, not duplicated here.

EmuTOS's own resolution-change precedent (Track A, `1a7484ab`) directly
informs the switch-mechanism decision below: a rez change is **never** a
live hot-patch of a running desktop. The desktop's `RESITEM` menu action
(`desk/deskmain.c:900-917`) runs a resolution dialog
(`desk/deskrez.c:change_resolution`/`change_falcon_rez`/`change_tt_rez`),
then calls `shel_write(SHW_RESCHNG, …)` (`aes/gemshlib.c:157-196`), which
only sets the `gl_changerez`/`gl_nextrez` globals and asks the desktop
process to **exit**. The actual `Setscreen()` call happens at the very top
of the AES's cold-start entry point, `gem_main()`
(`aes/geminit.c:753,775-806`), **before** any window/menu/VDI-workstation
state exists — i.e. "switch resolution" == "apply the new mode, then run
the exact same from-scratch init sequence boot uses", not an incremental
re-layout of live state. The choice is persisted across boots by
overloading the `#E` NEWDESK.INF line's mode-word field
(`desk/deskapp.c:1161-1174` writes it via `desk_get_videomode()`;
`aes/geminit.c:343-392` `process_inf1()` reads it back and sets
`gl_changerez`/`gl_nextrez` from it at the next boot).

Separately, EmuTOS's own Falcon `Getrez` has no classic code for a
non-ST-compatible (true-colour/high-res VGA) Videl mode: `atari_getrez()`
(`bios/screen.c:948-971`) reads `vsetmode(-1)`, and for any Videl mode
**without** the `VIDEL_COMPAT` bit it hardcodes `rez = 2` (`ST_HIGH`) —
*not* `FALCON_REZ` (3) — regardless of the mode's real geometry/depth
(`screen.c:968-970`). Meanwhile `vsetmode()` itself
(`bios/videl.c:619-659`) sets the **`sshiftmod`** shadow to `FALCON_REZ`
for exactly the same non-compat modes (`videl.c:648-656`) — so even in
EmuTOS, two internal state variables disagree about "the" rez code the
instant the live mode isn't one of the eight classic Atari codes, and the
XBIOS-visible `Getrez()` picks the degenerate ST-compatible guess, not the
Falcon indicator. Real Falcon programs query the *actual* mode via
`VsetMode(-1)` (a raw Falcon mode word: bpp bits, VGA/RGB, sync, resolution
flags), never by inferring it from `Getrez`. This precedent is why point 6
below leaves NewTOS's `Getrez`/`Setscreen` classic-rez contract an **open
question** rather than guessing a value: 1920×1080 and 1280×800 are exactly
the situation EmuTOS itself has no honest classic code for.

### Decision

1. **Two native, live VDI resolutions**, both delivered through the
   existing VideoCore mailbox path: **1920×1080×32bpp (default)** and
   **1280×800×32bpp (switchable at runtime)**. Both ride the *same* fixed
   1920×1080@60 Hz HDMI wire signal already forced by
   `boot/rpi4/config.txt`; a 1280×800 mailbox "set physical size" request is
   expected to be scaled/letterboxed by the VideoCore firmware onto that
   fixed signal without an HDMI/EDID renegotiation — **this is an
   expectation to verify empirically (Stufe 1), not an assumed fact**, since
   `docs/hardware/mailbox-framebuffer.md` does not yet document VideoCore's
   physical-size-vs-forced-`hdmi_mode` scaling behaviour.
2. **Colour model unchanged for this round.** The VDI keeps presenting the
   existing 16-colour/4-plane palette raster
   (`RasterBackend::num_colors() == 16`, `kernel/src/vdi_backend.rs:21`)
   over the (now larger) 32bpp truecolor physical buffer, exactly as today
   at 640×480. A genuine truecolor VDI mode (a real `DEV_TAB`/`INQ_TAB`
   colour-depth increase, a new pixel-format contract for programs) is
   **explicitly out of scope for this ADR** — a later, separate decision.
3. **Geometry independence is the actual engineering target** and a hard
   rule going forward: no VDI/AES/desktop code may assume 640/480/639/479
   (or any other fixed dimension) — every consumer reads the live geometry
   (`RasterBackend::width()/height()`, `Host::screen()`, or the desktop's
   own `DeskState::g_desk`/`screen_dims()` plumbing). Most of the stack
   already follows this (see Context); the remaining gaps are enumerated in
   Consequences and must close.
4. **Staging rule (non-negotiable):** the physical framebuffer stays at its
   present geometry until geometry independence is verified end-to-end
   (VDI + AES + desktop rendering correctly at a live resolution ≠
   640×480, confirmed by screendump) — only then does the framebuffer
   driver's target geometry change to 1920×1080. Raising the physical
   resolution before that point would render a broken desktop, per the
   user's explicit instruction.
5. **Switch-mechanism skeleton (proposed, mirrors the EmuTOS precedent in
   Context, not yet a final API — Developer's to implement):** a
   resolution change re-runs the same cold-start sequence boot already
   uses, rather than hot-patching live state: re-issue the mailbox request
   with the new geometry → invalidate the desktop's one-time-computed state
   (`kernel/src/desk/global.rs`'s idempotent `DESK` static,
   `kernel/src/desk/menu.rs`'s idempotent `MENU.built` cache — both
   currently "compute once at boot, never again") → re-run the equivalent
   of `DeskState::new`/`app_start`/`win_start`/`build_menu` → full redraw.
   NewTOS has no separate AES *process* to restart (the desktop is a kernel
   module, `desk::run()`), so "restart" here means "re-run the one-time
   setup", not a reboot.
6. **`Getrez`/`Setscreen`'s classic-rez mapping is explicitly NOT settled by
   this ADR — flagged as an open Lead decision.** Neither 1920×1080 nor
   1280×800 (16 colours) matches any entry of EmuTOS's `vmode_table[]`
   (`bios/screen.c:724-735`); per the Falcon precedent in Context, EmuTOS
   itself has no honest classic code for an "off-table" mode either. Three
   sketched options (none chosen): (a) a fixed degenerate code regardless of
   which of the two live resolutions is active — mirroring Falcon's
   `Getrez`-returns-`2`-regardless precedent; (b) a resolution-derived code
   that differs between the two modes; (c) a NewTOS-native query function
   (cookie-jar or a new trap) alongside a degenerate classic `Getrez`,
   mirroring Falcon's real `VsetMode(-1)`-is-ground-truth /
   `Getrez`-is-a-hint split. See the Researcher's 2026-07-09 report to the
   lead for the full option writeup.
7. **Persistence + UI entry points (proposed, mirrors EmuTOS):** a resolution
   choice persists across boots via a NEWDESK.INF field analogous to the
   `#E` mode word (`desk/deskapp.c:1161-1174`, `aes/geminit.c:343-392`); a
   desktop menu item analogous to `RESITEM`/`deskrez` offers the two
   resolutions. Whether NewTOS reuses the literal `#E` byte layout (risk:
   collides with real `#E` semantics for the environment/time-format bits
   sharing that line) or adds a NewTOS-private INF line is an **open Lead
   decision**, not decided here — `docs/api/desktop/deskrez.md`
   (previously "out of scope" under the superseded draft) is back in scope
   and needs its own extraction pass before implementation (Rule 5).

### Rationale

- **The Pi-4 hardware trivially supports more than 640×480**: the boot
  firmware already negotiates a 1920×1080@60 Hz HDMI signal
  (`boot/rpi4/config.txt:12-16`) and the mailbox driver already speaks the
  parametric tags needed (`mailbox.rs`, `framebuffer.rs:build_request`);
  nothing here is invented hardware behaviour (CLAUDE.md Rule 6) — it is
  the same documented mailbox interface (`docs/hardware/mailbox-framebuffer.md`
  §3) fed different, still-data, values.
- **"New" means modern, not artificially ST/TT/Falcon-constrained.** NewTOS
  is a clean-room reimplementation, not a binary-compatible emulator
  (`CLAUDE.md` non-goals); a source-ported program queries the VDI for its
  geometry (`v_opnvwk`/`vq_extnd`) rather than assuming a fixed Atari
  screen size, so raising the native canvas is transparent to a
  well-behaved program exactly as it was across real ST/TT/Falcon
  resolutions.
- **Colour truecolor is deliberately not bundled with this change**: it is
  an orthogonal VDI/backend redesign (an actual `DEV_TAB`/`INQ_TAB`
  colour-depth increase, a program-visible pixel-format change) that would
  multiply the risk of this change; isolating the work to geometry keeps
  the blast radius bounded and independently verifiable.
- **Source-backed switch model.** EmuTOS itself never hot-patches a live
  AES for a resolution change — it always applies `Setscreen()` at the very
  top of a fresh `gem_main()` and lets the rest of AES/desktop init run from
  scratch (Context). Mirroring "tear down cached state, re-run the one-time
  setup" rather than inventing a live incremental re-layout is the
  source-faithful choice, not a shortcut.
- **The `Getrez`/`Setscreen` question is left open, not guessed**, per
  CLAUDE.md Rule 3 (STOP on uncertainty): EmuTOS's own Falcon behaviour
  shows that "what classic code does an off-table mode get" has no single
  precedent-mandated answer (its own `Getrez`/`sshiftmod` even disagree with
  each other for the same mode, Context) — inventing a value here would be
  exactly the kind of guess CLAUDE.md forbids.

### Consequences

- Positive: matches modern expectations (an HD desktop) using only the
  mailbox interface the codebase already implements — no invented hardware.
- Positive: the code survey shows most of VDI/AES already reads geometry
  live; the remaining work is bounded and enumerated, not open-ended.
- Negative (correctness blocker, must close before 1080p goes live): the
  desktop's fixed-size object arena
  (`MAX_WIN_ITEMS = 48`, `INITIAL_ITEM_SLOTS = (NUM_WNODES+1)*MAX_WIN_ITEMS`,
  `kernel/src/desk/global.rs:43-62`) is explicitly sized "at 640×480" in its
  own doc comment; a maximized window/desktop at 1920×1080 fits roughly
  17×20 = 340 icon cells with the current fixed cell pitch (`g_wicon=100`,
  `g_hicon=50`, `min_wint=8` — `kernel/src/desk/app.rs:44-61`), not 48. The
  arena cannot grow after `obj_init` (raw `ob_spec`/`ib_ptext` pointers baked
  into the tree would dangle on a `Vec` regrow, per the existing doc
  comment), so this constant must be recomputed for the worst-case
  resolution *before* 1080p is enabled, not patched reactively.
- Negative: two desktop caching spots need an explicit invalidate/rebuild
  path that does not exist today — `kernel/src/desk/global.rs:471-487`'s
  `DESK` static (`init()` is a no-op on a second call) and
  `kernel/src/desk/menu.rs:372-378,483`'s `MENU.built` flag (the menu-bar
  object tree, sized to screen width/height, is built exactly once).
- Negative/Open: `docs/api/desktop/deskrez.md` re-enters scope (was
  "out of scope" under the superseded draft) and needs its own EmuTOS
  extraction pass (`desk/deskrez.c`) before the Developer implements the
  switch-UI, per Rule 5.
- Negative/Open: the classic-rez (`Getrez`/`Setscreen`) contract question
  is reopened (point 6) — no classic Atari code matches either target
  resolution; this needs a Lead decision, not a Researcher default.
  `docs/api/_discrepancies.md` D-076/D-077 (written for the superseded
  single-mode draft) are **not** retracted (append-only log) but a
  follow-up entry marks them reopened — see D-078.
- Neutral: `Physbase`/`Logbase` behaviour is unaffected by this ADR.
- Deferred: real truecolor VDI remains a documented future step, out of
  scope here — inverse framing of the superseded draft (geometry ships now,
  colour depth later, rather than the other way around).

### Alternatives considered

- **Keep the single fixed 640×480/`TT_MEDIUM` mode** (the just-superseded
  draft): rejected same-day by the user — the Pi4 hardware and firmware
  config trivially support more; "New" should mean modern, not an
  artificial 1980s-Atari-geometry ceiling.
- **Ship geometry independence and real truecolor together:** rejected —
  bundling maximizes scope/risk for one change; colour depth is a separate,
  program-visible contract question better isolated as its own later ADR.
- **Query HDMI/EDID for the monitor's native mode instead of a fixed
  1920×1080 default:** not chosen for this round — `hdmi_mode` is already
  forced in `boot/rpi4/config.txt`, so there is no live EDID negotiation
  today; real EDID-driven mode detection remains a further-future option,
  not a blocker for the two-fixed-resolution switch requested here.
- **A single resolution that just moves from 640×480 to one higher fixed
  value (no runtime switch):** rejected — the user explicitly asked for two
  selectable live resolutions (1920×1080 default, 1280×800 alternate).

### References

- `kernel/src/drivers/bcm2711/framebuffer.rs:23-24,86-143` (fixed
  `WIDTH`/`HEIGHT` consts + the parametric mailbox request they feed)
- `kernel/src/drivers/qemu_virt/ramfb.rs:75-76` (QEMU dev-vehicle mirror)
- `boot/rpi4/config.txt:12-18` (forced DMT 1080p60 HDMI timing)
- `docs/hardware/mailbox-framebuffer.md` §3 (mailbox tag reference —
  needs a Track-B addendum on physical-size-vs-forced-`hdmi_mode` scaling
  and the buffer-release tag before a live re-request is implemented)
- `kernel/src/vdi_backend.rs:130-136`, `kernel/src/host_env.rs:262-273`,
  `tos-api/src/vdi/workstation.rs:100-117,143-160`,
  `tos-api/src/aes/window.rs:1007-1013` (already-dynamic geometry reads)
- `kernel/src/desk/global.rs:43-62,471-487`, `kernel/src/desk/menu.rs:372-
  378,483`, `kernel/src/desk/app.rs:44-61` (the desktop-side gaps: arena
  sizing, the two one-time caches)
- EmuTOS `1a7484ab`: `aes/gemshlib.c:157-196` (`SHW_RESCHNG`),
  `aes/geminit.c:334-392,753,775-806` (`process_inf1`, `gem_main` cold
  `Setscreen`), `desk/deskmain.c:900-917` (`RESITEM`), `desk/deskrez.c`
  (`change_resolution`/`change_falcon_rez`/`change_tt_rez`),
  `desk/deskapp.c:1083-1174` (`#E` mode-word read/write), `bios/screen.c:
  724-735,948-971` (`vmode_table`, `atari_getrez` Falcon quirk),
  `bios/videl.c:619-659` (`vsetmode`, `sshiftmod = FALCON_REZ`)
- `docs/api/xbios/04-getrez.md`, `docs/api/xbios/05-setscreen.md`,
  `docs/HARDWARE-PROFILE.md` §3.1 (updated alongside this ADR, open points
  marked)
- `docs/api/_discrepancies.md` D-076, D-077 (superseded draft's corrections,
  now reopened — see D-078)
- `docs/api/desktop/deskrez.md`, `docs/api/desktop/_overview.md` (back in
  scope — needs its own extraction pass)
- [[qemu-window-view-command]] (how to visually verify framebuffer output
  in QEMU at a non-default resolution)
- ADR-016 (desktop rebuild — the geometry work is cross-cutting and must be
  sequenced against the in-flight `kernel/src/desk/` task list)

## ADR-018: Native executable extension family — `.NAP` / `.NCP` / `.NTP`

**Date:** 2026-07-16
**Status:** **Accepted** (user decision 2026-07-16, confirmed via team lead)
**Phase:** cross-cutting (desktop launcher `kernel/src/desk/`, `kernel/src/launch.rs`,
program-build tooling, NEWDESK.INF associations)

### Context

NewTOS programs are **native AArch64 ELF** binaries: `pexec::load` reads the
file, parses the ELF, loads it into a process slot, and `kernel::launch::
run_program` drops to EL0, runs it, and resumes when it `Pterm`s. They are
**not** m68k Atari `.PRG`/`.APP`/`.TOS`/`.TTP` binaries. The desktop must be
able to launch a program by double-click; today `open_item`
(`kernel/src/desk/main.rs`) returns `OpenTarget::None` for a plain file (an
explicit no-op).

Reusing the classic Atari suffixes for native ELF apps would conflate two
architectures under one name: once the m68k userland emulator (a later phase)
exists, a `.PRG` could be *either* a native ELF *or* an original m68k binary,
indistinguishable by name. We cannot rename existing Atari binaries — they
*are* named `.PRG`/`.APP`/`.TOS`/`.TTP` — so the m68k side is fixed and the
**native** side takes the new, distinct extensions.

### Decision

Native NewTOS ELF programs use a new **`N_P` family** (middle letter = role);
the classic Atari suffixes are **reserved for m68k** binaries (run via the
future emulator). The file extension is a desktop/shell convention, **not** part
of the TOS syscall ABI, so this does not touch TOS API compatibility.

| Ext | Role | m68k analog | Launcher behavior |
|-----|------|-------------|-------------------|
| `.NAP` | Native **A**pp (GEM) | `.PRG` / `.APP` | `run_program`; the app uses AES and draws its own windows over the desktop |
| `.NCP` | Native **C**onsole program | `.TOS` | `fbcon::activate` (VT52 text screen), `run_program`, restore the desktop on `Pterm` |
| `.NTP` | Native, **T**akes params | `.TTP` / `.GTP` | prompt for a command tail (form dialog), pass it as the child's cmdline, then run |

**Why exactly three:** NewTOS's AES environment is always up, so the launcher
does *not* need the classic GEM-vs-console environment selection at load time —
the program decides its own UI. The three roles are precisely the cases that
need distinct *launcher* behavior: GUI app (nothing special), console app (text
screen), takes-params (tail dialog). "GEM with params" collapses into `.NTP`
(the tail dialog is identical). More roles can be added later without breaking
the scheme.

### Routing

- **Double-click / File→Open** on a file (`open_item`):
  - native ext → build the full `C:` path (window `p_spec` drive+dir + filename)
    → `run_program` (console mode for `.NCP`, tail dialog for `.NTP`) →
    `full_redraw` on return.
  - classic m68k ext (`.PRG`/`.APP`/`.TOS`/`.TTP`/`.GTP`) → an honest
    `form_alert` "m68k programs need the emulator (not yet available)" until the
    m68k userland phase.
  - anything else → document / no-op (associated-app opening is future work).
- **AUTO-folder autorun** (`run_auto_folder`, `launch.rs`): runs native
  **console** programs (`.NCP`) only — it executes at boot BEFORE the desktop and
  AES/`appl_init`, so a GEM app (`.NAP`) must NOT run from `\AUTO\` (it needs AES).
  GEM apps auto-launch instead via the desktop `#Z` NEWDESK.INF autorun (after AES
  init). A `.PRG` in `\AUTO\` (m68k) is unsupported and skipped. *(Revised
  2026-07-20: originally `\AUTO\` also ran `.NAP`; that let a GEM app launch before
  AES — see [[singletasking-app-owns-full-screen]] and the AUTO-console-only rule.)*

### Change surface

- `kernel/src/desk/main.rs` — `open_item`: native-ext detection +
  `OpenTarget::App{path, flavor}` + path construction + route to `run_program`
  + return `full_redraw` + m68k/error alerts.
- `kernel/src/desk/` — an extension-classification helper
  (`NAP`/`NCP`/`NTP`/m68k/document). The dead `kernel/src/desktop.rs`
  `name_is_program` is a **reference only** (do not resurrect it).
- `kernel/src/launch.rs` — `run_program` extended for a command tail (`.NTP`)
  and console mode (`.NCP`: `fbcon::activate` + restore); `run_auto_folder`
  filter to include native extensions.
- `kernel/src/desk/inf.rs` — NEWDESK.INF file-type/icon associations gain the
  native family alongside the m68k ones (`inf.rs:566-570`).
- **Program-build tooling — PREREQUISITE.** The old `programs/lineecho` crate +
  generated ELF are **gone**; native programs currently exist only as
  hand-crafted ELF byte arrays in the selftests (`selftest_pexec.rs`) + the
  in-kernel `registry`. To ship a *double-clickable file on C:*, either (a)
  hand-craft a minimal native ELF (a console `.NCP`: `Cconws` + `Pterm`) and
  write its bytes onto the SD image, or (b) re-establish a program-build crate
  (Rust `no_std` → stripped ELF → `make-sd-image.sh`) for a real `.NAP` GEM
  demo. (a) proves the chain now; (b) is needed for a real GEM app.
- `tools/make-sd-image.sh` — place `HELLO.NCP` (and later `HELLO.NAP`) on C:
  (+ optionally in `\AUTO\`).
- Doc/comment reconciliation — `launch.rs` / `bdos/fs` comments that say
  "`.PRG`" for the *native* launch path.

### Staged plan (Task #72)

1. Family + routing + classification helper + `.NAP` wiring (`run_program` on
   double-click) + m68k/error alerts + return redraw.
2. `.NCP` console mode (`fbcon` activate/restore).
3. `.NTP` command-tail dialog.
4. A native demo program on C: — start with a hand-crafted `.NCP` for an
   end-to-end proof; a real `.NAP` GEM demo needs the build-crate prerequisite.

### Related

ADR-013 (native `.RSC` format — the same "native, not m68k-packed" principle),
ADR-016 (the desk rebuild `open_item` lives in), and the AES shell manager
(`shel_write`/`pending_launch`, `tos-api/src/aes/shell.rs` — the AES-faithful
launch path that the kernel-driven `run_program` deliberately bypasses).

## ADR-019: Native C toolchain — port real Atari C libraries, don't reimplement

**Date:** 2026-07-18
**Status:** **Accepted** (user decision 2026-07-18)
**Phase:** cross-cutting (SDK — `sdk/`, separate from the clean-room OS)

### Context

To run existing Atari C software (target app: **QED**, later **zView**) natively
on NewTOS, we need a C toolchain: clang-aarch64 + a libc + GEM libraries. QED
links **mintlib** (libc) + **gemlib** (GEM/AES/VDI) + **cflib** (dialogs). The
tempting shortcut — hand-write a "minimal libc" — was rejected: we have the full
LGPL sources, and reimplementing them from scratch is both wasteful and a poor
test of whether the toolchain can build real software.

### Decision

**Port, don't reimplement.** Reuse the mintlib/gemlib/cflib source unchanged and
retarget only the **thin arch/OS boundary**; compile the rest with clang. The
SDK/libc is a **separate LGPL artifact** under `sdk/` (like glibc to a kernel),
**not** part of the clean-room OS — the OS stays clean-room; the SDK reuses
ported third-party code.

The port surface, proven small for mintlib (Task #90):
- **The trap boundary** — mintlib's ~56 `trap_1_*`/`trap_13_*`/`trap_14_*` macros
  (`include/mint/{osbind,mintbind,falcon,metados}.h`), retargeted from m68k
  stack-`trap` to NewTOS register-`svc` (args left-to-right x0..x7, fn in x8,
  result x0). One change → all ~200 GEMDOS/BIOS/XBIOS bindings work unchanged.
  gemlib's `crys_if` (AES/VDI) retargets the same way to `svc #2`.
- **crt0** (`sdk/libc/newtos-arch/crt0.S`, sets SP from the BasePage `p_hitpa`,
  inits the malloc arena over the free TPA), **setjmp** (aarch64, when pulled).
- **A few honest LP64/endian fixes** to vendored files: aarch64 `byteswap.h`
  (`__builtin_bswap*`), an `is_longlong` assign-to-constant fix in `vfprintf.c`,
  and lean NewTOS `newtos-arch/{do_fstat,do_stat,isatty,ioctl}.c` that fill only
  what stdio needs (cutting the Unix time/uid/passwd tail).
- **Build harness** `sdk/libc/build-libc.sh` replicates mintlib's `buildrules`
  (`-D_LIBC -D_REENTRANT -DHAVE_CONFIG_H -D_GNU_SOURCE -nostdinc` + include
  order) → `libc.a`. `>8`-arg XBIOS macros (Falcon, unused) are guarded with a
  loud `_Static_assert`, not stubbed.

The 99% (printf/vfprintf, malloc, stdio buffering, string, qsort, strtol)
compiles **as-is** on top of the retargeted trap layer.

**The ported app is sacrosanct.** QED (and zView) source is **not modified** —
every build failure is fixed in the toolchain (libc/gemlib/cflib/flags/headers),
never in the app. The one exception: genuine **m68k assembly** (inline `asm`/
`.S`) in the app must be retargeted to aarch64 (the arch boundary). Only after
*every* toolchain avenue is exhausted do we even discuss an app-source change.
This keeps QED an honest test that the toolchain builds real Atari software
unmodified.

### Verified

`sdk/libc/tests/libtest.c` — real mintlib `printf("%d %ld")` / `malloc`+`free` /
a `fopen`/`fwrite`/`fread`/`fclose` file round-trip on `C:` — runs on QEMU
raspi4b (`\AUTO\LIBTEST.NCP`): `-42 305419896` / `heap: heap string` /
`roundtrip OK (37 bytes)`, clean `Pterm`, then the desktop. No libc code was
hand-written.

### Related

ADR-018 (the `.NCP`/`.NAP` native-extension family these programs use), the
multi-page TPA loader (Task #89, the prerequisite for real-size programs), and
the deferred native 64-bit ABI (Task #91 — the SDK is LP64 C over 32-bit-TOS
structs today).

## ADR-020: App ABI is ILP32 (32-bit pointers) for Atari-compat programs; LP64 native deferred

**Date:** 2026-07-19
**Status:** **Accepted** (user decision 2026-07-19) — implementation staged
**Phase:** cross-cutting (SDK toolchain + kernel loader/AES/VDI ABI boundary)

### Context

The goal is to run **unmodified** Atari C software (QED, then zView). We first
built the native toolchain LP64 (ADR-019: 64-bit pointers, the aarch64 default),
and it works — QED compiled + ran through the whole ported mintlib/gemlib/cflib
stack. But **old Atari C assumes ILP32** (`sizeof(pointer)==sizeof(long)==4`,
universal on m68k) in its *own* code. Under LP64 that breaks: QED's own bin
allocator (`sdk/apps/qed/src/memory.c`) desyncs on 8-byte pointers, and the
patch burden **grows with app complexity** (zView + FreeType/libpng/zlib/xpdf
would need many). Patching each app abandons "unmodified" and adds divergence.

### Decision

**Target the ILP32 app ABI** — 32-bit pointers — so Atari C's assumptions hold
and it compiles **unmodified**. This is the standard "32-bit apps on a 64-bit
kernel" pattern (WoW64 / Linux compat): the kernel stays LP64 internally; only
the app-facing boundary is 32-bit. Toolchain: **`aarch64-elf-gcc -mabi=ilp32`**
(4-byte pointers, ELF32) **+ `aarch64-elf-ld -m aarch64elf32`** — spike-confirmed
working. GCC is also the *native* compiler for this gcc-written code (clang
frictions like the vfprintf computed-goto vanish).

Payoffs: QED's allocator becomes correct (crash gone, no patch); the TOS structs
(BASEPAGE/DTA) are naturally 4-byte-correct (the [[lp64-tos-struct-trap]] audit,
task #98, vanishes); several LP64/clang overlays revert to pristine mintlib.

**LP64 native stays the deferred future** (task #91): a 64-bit `.NAP` path for
*new* NewTOS software that wants >4 GB — non-urgent (Pi4 RAM + a 4 MiB TPA make
32-bit's 4 GB no constraint). The loader is **dual**: ELF32 = ILP32 compat apps,
ELF64 = LP64-native apps + the embedded selftests. The two coexist.

### Change surface (staged)

1. **Foundation (done, commit `14a1f32`):** `tos-api/.../process/elf.rs` gains an
   ELF32 AArch64 parse path (class-dispatched `ClassLayout`; ELF64 byte-identical);
   `sdk/build-ilp32.sh`. A minimal ELF32 program loads + runs in QEMU alongside
   ELF64.
2. **mintlib ILP32** — rebuild with the GNU ILP32 toolchain; drop the now-moot
   LP64 overlays (vfprintf is_longlong, labs, basepage/DTA); libtest as ELF32.
3. **gemlib + cflib ILP32** + the kernel **AESPB/VDIPB** param-block readers
   (`tos-api/src/aes|vdi`) read **32-bit** pointers for EL0 (svc #2) callers.
4. **QED ILP32 — UNMODIFIED** → runs (the `memory.c` crash is gone by construction).

### Related

ADR-019 (the LP64 toolchain this pivots — not wasted; the trap retarget, port
structure, and the crt0/loader/AES-rsrc fixes carry over), ADR-018 (the `.NAP`
family gains an ILP32/LP64 distinction by ELF class), and the app-ABI memory
[[native-64bit-abi-future]].

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

## ADR-021: EL0 apps get GEMDOS memory via a per-process mapped window; the pool allocator stays EmuTOS-faithful

**Date:** 2026-07-20
**Status:** Accepted
**Phase:** cross-cutting (BDOS mem + process MMU)

### Context

Classic TOS is flat/unprotected: a program dereferences any physical address
`Malloc`/`Mxalloc` returns. NewTOS runs apps at **EL0** behind `TTBR0_EL1`; the
ported EmuTOS pool returns physical addresses mapped only in the kernel high
half, so an EL0 app faults touching them (QED `Mxalloc` → `0x0160a000` write
fault; note [[el0-app-memory-must-be-mapped]]). TT-RAM-aware apps *all* request
TT-RAM, so this hits every non-trivial app, not just QED. The TPA image loads at
a fixed low VA (non-relocating loader), and `Mshrink(basepage@VA 0)` already
fails `EIMBA` for the same address-mismatch reason.

Three sources were analysed (full extraction in
`docs/analysis/emutos-memory-model.md`): **EmuTOS** (primary — the pool model:
`MD`/`MPB`, two pools `pmd`/`pmdalt`, first-fit `ffit`+coalesce, TPA = the app
owns ~all free RAM and sub-allocates, `m_own` is a reap-tag not access control);
**FreeMiNT** (`references/freemint/`, the sanctioned MiNT-extension source — the
one TOS-family OS that solved memory protection for *unmodified* TOS/GEM apps:
one global identity-mapped ST/TT pool `sys/memory.c:210-211`, per-process page
tables holding only *protection bits* over that same layout
`sys/arch/mprot030.c`, default owner-exclusive `PROT_P` `sys/mint/mem.h:130`, an
explicit `PROT_G` for shared memory); and **NewTOS-current** (a faithful pool
port that returns raw kernel-reachable PAs + a fixed 4 MiB TPA). FreeMiNT's
identity-map relies on load-time-**relocatable** binaries (§1B.4) and so does not
transfer to NewTOS's fixed-load-VA ELF loader — but its *pattern* ("global pool;
allocate then grant the owner exclusive access; reverse on free/`Pterm`")
independently confirms the design below.

### Decision

Keep the ported EmuTOS pool allocator (`bdos/mem/{pool,traps}.rs`) **unchanged
and authoritative**. Add a thin **per-process EL0 memory manager** that, for
SVC-originated (EL0) callers, allocates from the real ST/TT pool, **maps the
block into a per-process EL0 heap window**, returns the **user VA**, and tracks
`user_va ↔ m_start` for `Mfree`/`Mshrink`/`Pterm`. This generalises the task-#99
RSC window (the RSC becomes just another mapped block) and registers the TPA as
its first entry (fixing `Mshrink(basepage)`). EL1/kernel callers keep raw PAs.
`Malloc(-1)` for EL0 reports `min(pool-free, window-free)`; ST/TT preference is
preserved because the block still comes from the requested pool.

**Offset-map now** (compact low-VA window → 32-bit-safe pointers, tiny page
tables); **identity-map with dynamic tables is the v2 target** once the
LP64/PIC-relocatable path (#91) lands — the FreeMiNT precedent is independent
evidence that identity-map is the right end-state once binaries no longer need a
fixed load VA. **`Mxalloc`'s `F_PROTMODE` bits (FreeMiNT's `PROT_P/G/S/PR/I`)
stay deferred**: continue masking them exactly as EmuTOS does
(`mode &= MX_MODEMASK`) until real multitasking needs FreeMiNT-compatible
isolation/sharing — EmuTOS, the primary source, ignores these bits too, so
deferring is spec-faithful, not a gap.

**Multitasking is the acknowledged Kernel end-goal (user, 2026-07-20).** When it
lands, much of FreeMiNT's machinery is what we adopt: the `PROT_*` protection
modes on `Mxalloc`, per-process protection over the shared pool, and `PROT_G`
inter-process shared memory. This ADR is the deliberate v1 stepping-stone toward
that — the block table already records `m_start ↔ user_va`, so extending one
entry to a *list* of `(slot, user_va)` pairs is the natural growth path to shared
memory; per-process `TTBR0` windows generalise to per-process protection tables.
The FreeMiNT analysis is therefore **not** discarded — it is the roadmap for the
multitasking phase, captured in [[freemint-extension-direction]].

### Rationale

Faithful (allocator untouched; contract preserved), uniform (image + heap + RSC
all offset-mapped through one table), and minimal (a boundary shim). NewTOS's
per-process `TTBR0` (same VA → different PA per process) is the ISA-appropriate
generalisation of FreeMiNT's per-process-table-over-one-global-layout mechanism,
not a weaker substitute. Pure identity-map cannot be uniform while the loader
places images at fixed VAs — FreeMiNT shows *why* that is a loader property
(relocatable binaries), not a limitation of this design.

### Consequences

- **Positive:** EL0 apps get dereferenceable `Malloc`/`Mxalloc` memory of the
  right RAM type; `Mshrink(basepage)` works; the RSC window is subsumed; default
  owner-exclusive access matches both EmuTOS's block-ownership model and
  FreeMiNT's `PROT_P` default.
- **Negative:** a per-process block table + window cap; an EL0 `Malloc(-1)`
  bounded by mappable space (documented adaptation); one new EL0-caller signal to
  plumb from the SVC path; **no inter-process shared memory** yet (FreeMiNT's
  `PROT_G` has no NewTOS equivalent — documented forward gap for multitasking).
- **Neutral:** returned pointers no longer equal PAs (fine — EL0 apps do no DMA).

### Alternatives considered

- **(b) Per-process physical arenas** (carve each app a private pool at `Pexec`):
  breaks ST/TT faithfulness, caps memory at load time, wastes resident RAM,
  fragments the address space. Independently corroborated as wrong by FreeMiNT
  *also* using global, not per-process, pools.
- **(c) Pure identity-map now** (VA = PA, FreeMiNT-style): impossible while the
  loader places images at a fixed low VA — needs a relocating/PIC loader first.
  Deferred to v2 (#91), where it becomes the maximally-faithful end-state.
- **Adopt FreeMiNT's full protection model now** (`PROT_*` + per-process
  protection tables over one identity space): premature — it presupposes both
  multitasking and relocatable binaries. Its *pattern* is adopted; its mechanism
  waits for the multitasking phase.

### References

- `docs/analysis/emutos-memory-model.md` (EmuTOS @1a7484ab: `umem.c`,
  `osmem.c`, `iumem.c`, `biosmem.c`, `bdosdefs.h`; FreeMiNT: `sys/memory.c`,
  `sys/arch/mprot030.c`, `sys/mint/mem.h`; NewTOS `bdos/mem/{pool,traps}.rs`).
- ADR-020 (ILP32 pointers bound the window), ADR-010 / QUIRK-001 (no TLBI before
  `eret` → fresh-VA cursor), ADR-013 (`.RSC` — the #99 window this subsumes).
- Notes [[el0-app-memory-must-be-mapped]], [[freemint-extension-direction]],
  [[native-64bit-abi-future]] (#91 LP64/identity-map v2).
- Tasks #99 (RSC window — subsumed), #102 (this research — closed).

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
