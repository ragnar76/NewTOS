# PHASE-03: Memory Management

## Goal

Bring up the MMU at EL1 with a sane initial page table layout, implement a
physical frame allocator and a kernel heap, then implement the GEMDOS memory
syscalls (`Malloc`, `Mfree`, `Mshrink`, `Mxalloc`).

## Inputs

- `CLAUDE.md`
- `phases/PHASE-01-boot-uart.md`, `phases/PHASE-02-gemdos-console.md` complete
- `docs/api/gemdos.md` — entries for `Malloc` (0x48), `Mfree` (0x49),
  `Mshrink` (0x4A), `Mxalloc` (0x44) must exist before code starts
- ARM Architecture Reference Manual (ARMv8-A), chapter "VMSAv8-64"
- BCM2711 datasheet, section on physical memory map
- Andre Richter tutorials, chapters on MMU and memory

## Deliverables

### D1: Memory map declaration

`kernel/src/mm/layout.rs` — a single source of truth for the physical and
virtual memory layout. Includes:

- DRAM range per board (RPi4: 0x00000000–0xFC000000 typical for 4GB model;
  QEMU virt: 0x40000000–end-of-RAM)
- MMIO range per board
- Kernel image load address and size (linker symbols)
- Reserved regions (firmware-used memory on RPi4)

No magic numbers. Every constant cites a datasheet or manual reference.

### D2: Page table builder

`kernel/src/arch/aarch64/mmu/`:

- 4KB granule, 48-bit VA, identity mapping for kernel space + MMIO
- Two TTBRs: TTBR1_EL1 for kernel (high half), TTBR0_EL1 disabled for now
  (process address spaces come in Phase 06)
- Memory attributes: Normal Cacheable for RAM, Device-nGnRnE for MMIO,
  configured via MAIR_EL1
- Page tables built at compile time where possible, runtime-extensible

### D3: MMU enable

A single `enable_mmu()` function that:

1. Configures MAIR_EL1, TCR_EL1
2. Loads TTBR1_EL1 (and TTBR0_EL1 with a null table)
3. Issues the required barriers (DSB, ISB)
4. Sets SCTLR_EL1.M = 1
5. Issues another ISB

After return, the kernel runs with caches and MMU on.

### D4: Physical frame allocator

`kernel/src/mm/frames.rs` — a buddy or bitmap allocator over the free
physical RAM. Records reserved regions from D1.

API:
```rust
pub fn alloc_frame() -> Option<PhysFrame>;
pub fn alloc_frames_contig(count: usize) -> Option<PhysFrame>;
pub fn free_frame(frame: PhysFrame);
```

### D5: Kernel heap

`kernel/src/mm/heap.rs` — a small linked-list or slab allocator backed by
frames from D4. Implements `core::alloc::GlobalAlloc`. Set as
`#[global_allocator]`.

Once this is in, `extern crate alloc;` becomes legal and `Vec`, `Box`,
`String` are usable in the kernel.

### D6: GEMDOS memory syscalls

Implement per spec:

| ID    | Function   | Notes                                                |
|-------|------------|------------------------------------------------------|
| 0x48  | `Malloc`   | Special: arg `-1` returns largest free block size    |
| 0x49  | `Mfree`    |                                                      |
| 0x4A  | `Mshrink`  | Resize previously allocated block                    |
| 0x44  | `Mxalloc`  | Mode flag: ST-RAM / TT-RAM / preferred — all map to  |
|       |            | the single heap on RPi4; mode is recorded but ignored|
|       |            | functionally. Document this NewTOS-specific decision.|

These calls operate on a **per-process** heap once Phase 06 lands. For now,
they operate on the kernel heap directly. A clear ADR in `docs/STATUS.md`
must record this and the migration plan.

### D7: Stack overflow detection

Place an unmapped guard page below the boot stack. An access to it triggers
a synchronous exception, caught by the existing exception handler (extended
to print "STACK OVERFLOW" before halting).

### D8: Heap diagnostics

Provide `kernel::mm::heap_stats()` returning total/used/largest-free, used
by `Malloc(-1)` and by a debug command (later phase).

## Exit criteria

- **EC1**: After `enable_mmu()`, `Cconws` from Phase 02 still works
  (proves caches + MMIO mapping correct).
- **EC2**: 1000 iterations of `Malloc(rand_size)` + `Mfree(ptr)` complete
  without leaks (verified via `heap_stats`).
- **EC3**: `Malloc(-1)` returns a value within 1 KB of `total - used`.
- **EC4**: Stack overflow test (deliberate deep recursion) prints
  "STACK OVERFLOW" and halts cleanly, does not loop or triple-fault.
- **EC5**: `docs/api/gemdos.md` entries for all four memory syscalls are
  complete, including m68k ABI mapping.
- **EC6**: `cargo clippy --all-targets` clean, zero warnings.

## Out of scope

- Process address spaces (Phase 06)
- Demand paging, swap, COW
- TT-RAM as a separate pool — RPi4 has uniform RAM; the spec quirk is
  documented but not functionally distinct
- ST-RAM 16-bit alignment quirks — not relevant on AArch64

## Risks

- **R1: Cache coherency before MMU.** Code executed before MMU enable runs
  with caches disabled. After enable, cleanly invalidate I-cache and TLBs.
- **R2: Identity mapping vs. high-half kernel.** Decide early. NewTOS uses
  high-half (TTBR1_EL1) so user processes (Phase 06) get the low half via
  TTBR0_EL1. Document in ARCHITECTURE.md.
- **R3: Mxalloc mode flag.** Real TOS used this to distinguish ST-RAM
  (chip-accessible) from TT-RAM (CPU-only). On NewTOS/RPi4, all RAM is
  uniform. The flag is accepted, recorded, and ignored — but documented.
- **R4: Heap fragmentation over long runs.** A simple linked-list allocator
  fragments. For Phase 03 this is acceptable. Replace with slab/buddy when
  it becomes a problem (record as ADR, not done preemptively).

## Claude Code instructions

- The MMU is the single most error-prone part of bring-up. Do not "make
  small adjustments" to TCR_EL1 fields without citing the ARM ARM section
  and explaining why each field has its chosen value.
- A failure during MMU enable produces silent hangs (no UART because cache
  state is wrong). Test in QEMU with `-d int,mmu` first; only then on RPi4.
- The `GlobalAlloc` impl must handle ZST allocations (size 0) per the Rust
  docs. Forgetting this is a common source of UB.
- Do not introduce `lazy_static` or `once_cell` to manage the allocator
  state; use `spin::Once` or hand-rolled `OnceCell<T>`.
- Heap setup is the gateway from "no_alloc" to "alloc available". Make
  sure no code path accidentally allocates before `init_heap()` returns.
