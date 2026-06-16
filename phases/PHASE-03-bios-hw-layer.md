# Phase 03 — BIOS Hardware Layer

> **Status:** complete (D1–D9 done, 2026-05-03).

## Goal

Provide the AArch64-specific hardware foundation that EmuTOS' `bios/`
layer assumes: working CPU caches, MMU, panic/halt path, kernel-side
heap, and a structured kernel-print (`kprintf`) facility.

This phase has no direct EmuTOS counterpart — EmuTOS' target CPU has
no MMU and one flat address space. The corresponding C files
(`bios/machine.c`, `bios/processor.S`, `bios/kprint.c`) are tiny
because the m68k requires almost no setup. AArch64 needs significant
plumbing before Phase 04 (BDOS Memory) can run, hence a dedicated
phase.

## EmuTOS sources to read

- `bios/machine.c` — what the BIOS *thinks* the machine looks like
  after init: `cookie_jar`, `mch`, `has_*` flags. We mirror the
  shape, but the values come from device-tree / VideoCore mailbox.
- `bios/kprint.c` — `kcprintf`-style debug output that bypasses the
  BDOS console. Maps to a Rust `kprintf!` macro using our PL011.
- `bios/processor.S` — cache flush / instruction barrier helpers.
  AArch64 equivalents live in `kernel/src/arch/aarch64/`.
- `references/emutos/bios/startup.S` — already implemented in
  Phase 01 (`kernel/src/arch/aarch64/boot.s`).

## Phase 00 prerequisites

None — this phase predates the BDOS specs. It produces *infrastructure*
the BDOS specs will build on.

## Deliverables

| Id | Title                                | Status   | File(s)                                  |
|----|--------------------------------------|----------|------------------------------------------|
| D1 | Memory map (ST-RAM + TT-RAM slots)   | done     | `kernel/src/bios/machine/layout.rs`      |
| D2 | Page-table builder (4-KiB granule)   | done     | `kernel/src/arch/aarch64/mmu.rs`         |
| D3 | MMU enable (identity, TTBR0 path)    | done     | `kernel/src/arch/aarch64/mmu.rs`         |
| D4 | Bitmap frame allocator (per-pool)    | done     | `kernel/src/bios/machine/frames.rs`      |
| D5 | Kernel heap (`#[global_allocator]`)  | done     | `kernel/src/bios/machine/heap.rs`        |
| D6 | Stack guard page + double-fault path | done     | `kernel/src/arch/aarch64/exceptions.rs`  |
| D7 | `kprint!` / `kprintln!` macros       | done     | `kernel/src/lib.rs`                      |
| D8 | Runtime DRAM detection (DTB)         | done     | `kernel/src/arch/aarch64/dtb.rs`         |
| D9 | Refactor: `mm/` → `bios/machine/`    | done     | `kernel/src/bios/machine/`               |

D6 makes stack overflows survivable for diagnostics. D7 replaces ad-hoc
`CONSOLE.write_str` / hex dumpers with a real formatter that is safe
to call from any context (including panics, before MMU, from IRQ).
D8 removes the hard-coded `DRAM_SIZE`. D9 aligns the source tree with
the EmuTOS module layout.

## Exit criteria

- **EC1**: After MMU enable, all of Phase 02's selftests still pass
  (Cconws, EINVFN, RX-IRQ). ✅ already verified.
- **EC2**: `Box::new`, `Vec::with_capacity` work in kernel code, with
  `heap_stats()` returning to baseline after a 1000-cycle alloc/free
  loop. ✅ already verified.
- **EC3**: A deliberate stack overflow prints "STACK OVERFLOW" via
  `kprintf!` and halts cleanly (no triple fault, no recursive
  exception loop).
- **EC4**: `kprintf!("{:#x}", value)` works from `panic!` and from
  inside an IRQ handler.
- **EC5**: `mm::layout::discover()` reads physical RAM size from the
  device tree (QEMU) or VC4 mailbox (RPi4); QEMU `-m 1024M` → kernel
  reports 1024 MiB, RPi4-4GB reports ~3.7 GiB ARM-visible RAM.
- **EC6**: `cargo clippy --workspace --all-targets` is clean.

## Out of scope

- BDOS-level memory pools (MD/MPB) — Phase 04.
- Per-process virtual address spaces — Phase 05/06.
- `Pterm`-driven memory cleanup — Phase 06.

## Risks

- **R1 — silent MMU hangs**: a wrong `TCR_EL1` field disables UART
  output without any panic. Mitigation: MMU code is fully cited
  against ARM ARM D5; review before changes.
- **R2 — kprintf re-entrancy**: must be safe from inside the panic
  path and from IRQ. Mitigation: lock-free byte queue, fall back to
  raw PL011 poll when the lock is contended.
- **R3 — DTB parsing complexity**: a full FDT parser is overkill;
  only `/memory@…/reg` is needed in D8. Mitigation: hand-rolled
  minimal parser in <300 lines.

## Implementation notes

- Refactor (D9) is **last**: only after D6–D8 land, so it is one
  monolithic move plus a `pub use` shim, not a moving target.
- The crate-level `mm::*` re-exports stay so the BDOS code in
  `tos-api/` does not need to chase the move.
