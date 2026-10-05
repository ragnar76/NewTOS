# ARCHITECTURE — NewTOS

## Layer Diagram

```
┌──────────────────────────────────────────────────────────┐
│  User Programs                                           │
│                                                          │
│  Native ARM PRGs       │  m68k PRGs (via emulator)       │
│  (NewTOS magic)        │  (0x601A magic, RuSTari core)   │
└────────────┬───────────┴──────────────┬──────────────────┘
             │ svc #1/13/14             │ trap #1/13/14
             │                          │ ▼ marshaling shim
             │                          │ (m68k → AArch64 ABI)
             ▼                          ▼
┌──────────────────────────────────────────────────────────┐
│  TOS API Layer  (crate: tos-api)                         │
│  ─────────────────────────────────────                   │
│  GEMDOS   │  BIOS   │  XBIOS                             │
│  table    │  table  │  table                             │
│           │         │                                    │
│  trap dispatcher → function table → handler              │
└────────────────────────┬─────────────────────────────────┘
                         │
┌────────────────────────▼─────────────────────────────────┐
│  Kernel Services  (crate: kernel)                        │
│  ─────────────────────────────────────                   │
│  Process │ Memory │ VFS  │ Console │ Scheduler           │
│  Mgmt    │ Mgmt   │      │         │                     │
└────────────────────────┬─────────────────────────────────┘
                         │
┌────────────────────────▼─────────────────────────────────┐
│  HAL Traits (crate: kernel::hal)                         │
│  Uart, BlockDevice, Framebuffer, Timer, IrqController    │
└────────────────────────┬─────────────────────────────────┘
                         │
┌────────────────────────▼─────────────────────────────────┐
│  HAL Implementations                                     │
│  bcm2711 (RPi4)        │  qemu_virt (QEMU virt)          │
└────────────────────────┬─────────────────────────────────┘
                         │
┌────────────────────────▼─────────────────────────────────┐
│  AArch64 Architecture Layer                              │
│  Boot, MMU, Exceptions, GIC, Context Switch              │
└──────────────────────────────────────────────────────────┘
```

## ABI Rules

### Native AArch64 callers

- AAPCS64 standard: x0–x7 for parameters, x0 for return
  (sign-extended `i64`; `tos-api/src/trap.rs`)
- `svc #N`, N = the m68k trap number: 1 (GEMDOS), 2 (GEM: VDI + AES),
  13 (BIOS), 14 (XBIOS) — `TrapClass::from_imm`, `tos-api/src/trap.rs`
- GEMDOS / BIOS / XBIOS: function ID in **x8** (low 16 bits), arguments
  in x0–x7 in their TOS order
- GEM (`svc #2`): **x0 = selector** (115 = VDI, 200 = AES), **x1 = the
  parameter block** (VDIPB / AESPB of the caller's pointer width, ADR-020);
  the VDI returns `flip_y` in x0, the AES returns 0 — its result is
  `int_out[0]` (`kernel/src/arch/aarch64/exceptions.rs`, GEM arm)

### m68k callers (via emulator)

- TOS standard: parameters pushed right-to-left on stack as their natural
  m68k sizes (WORD = 2 bytes, LONG = 4 bytes, pointer = 4 bytes)
- Function ID pushed last (top of stack)
- `trap #1/13/14`
- Return value in `D0.l` (sign-extended to 32 bits)
- Caller cleans up stack

### Marshaling boundary

The m68k marshaling shim (Phase 10) is the **only** code that knows about
m68k. It:

1. Catches the trap from the emulator's step result.
2. Reads parameters from the m68k stack (big-endian).
3. Translates m68k pointers to host pointers via the emulator's RAM
   translation.
4. Calls the native TOS API function with AArch64 ABI.
5. Places the return value in the emulator's `D0` register.
6. Resumes emulation at the instruction after `trap`.

The TOS API layer **does not branch on caller type**. It is identical for
both worlds.

## Address Space Layout

### Kernel space (TTBR1_EL1)

- High virtual addresses (`0xFFFF_xxxx_xxxx_xxxx`)
- Identity-mapped to physical RAM
- MMIO regions mapped Device-nGnRnE
- Kernel code, data, heap, page tables

### User space (TTBR0_EL1, one root per process, ARCH03 stage 1)

- Low virtual addresses (`0x0000_0000_xxxx_xxxx`), **one address space
  shared by every process**: each user address is its own physical address
  (VA = PA) — the two BDOS pools (mapped once at boot, 2-MiB blocks where
  possible), the published kernel structures (ADR-023) and the screen memory
  (`tos-api/src/bdos/process/mmu.rs`; plan
  `docs/analysis/arch03-implementation-plan.md` §3.1). FreeMiNT's model with
  memory protection off; per-process rights come with stage 3.
- **Page 0** `[0, 0x1000)` is the only per-process translation: the classic
  low-memory system area — sysvars, OSHEADER, `os_run` cell, cookie jar at
  `[0x400, 0x670)` — plus the upcall/userdef trampolines, the `Keytbl` area
  and the KEYS02 vector table, materialised per process (`populate_sysvars`,
  B1 / Option 1, plan E3). Each process root shares all tables below it
  except the ones leading to its page 0.
- Process TPA (Transient Program Area): a page-aligned BDOS-pool block, 4 MiB
  (`tos-api/src/bdos/process/{loader,arena}.rs`, plan E9). Its address is
  the process's **PD pointer**: the basepage (256 bytes per Compendium) sits
  at its start.
- The image starts at TPA + 0x1000 (`RESERVED_LOW_END`), relocated for the
  TPA address (position-independent ELF, ARCH03 0d): text (R-X `PT_LOAD`,
  `p_tbase/p_tlen`), then data (RW- `PT_LOAD`, `p_dbase/p_dlen`) with the
  BSS as its tail (`p_bbase/p_blen`); `sdk/newtos.ld`
- Free TPA above the BSS (`_end`): heap grows up, user stack at the top of
  the TPA (`p_hitpa`) grows down; the top 64 KiB are reserved for the stack
- GEMDOS `Malloc`/`Mxalloc` return pool addresses, valid in every process

## Process Model

- Single CPU core for user code (RPi4 has 4 cores; cores 1–3 currently
  parked)
- Cooperative multitasking; one process active at a time until `Pexec`
  blocks or `Pterm` returns
- `Pexec` modes per Compendium implemented
- **EL0 entry is uninterruptible.** Whoever hands control to user code
  owns `ELR_EL1` / `SPSR_EL1` until the `eret`; an exception taken in
  that window overwrites both with the interrupted EL1 state (ARM ARM
  D1.4.2). The trap path gets this for free — exception entry masks
  `DAIF` — so the rule binds the one path that erets without an
  exception frame:
  `arch::aarch64::user_entry::enter_el0_with_resume_capture`, which
  masks D/A/I/F itself (QUIRK-003 in `docs/MEMORY.md`).
- m68k processes are NewTOS processes whose code happens to run in the
  emulator instead of native; from the kernel's perspective they are
  identical

## File System Model

- TOS drive letters: A: B: (floppy — not present on RPi4 hardware,
  optional virtual mount), C: (SD card root), D: onwards (additional
  partitions or USB storage in later phases)
- 8.3 names primary; LFN considered after Phase 05
- Per-process current directory per drive
- Per-process file handle table with TOS-standard handles 0–5 mapped to
  CON, AUX, PRN, etc.

## Key ADRs (planned)

- ADR-001: Native ARM PRG executable format
- ADR-002: License choice (MIT / Apache-2.0)
- ADR-003: Mxalloc mode flag handling on uniform RAM
- ADR-004: VT52 emulation scope (full vs subset)
- ADR-005: m68k JIT — when, if ever
- ADR-006: USB HID input vs UART input ordering
- ADR-007: Long File Name support — yes / no / partial

ADRs land in `docs/STATUS.md` as they are written.
