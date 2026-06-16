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
- Function ID in **x8**
- `svc #N` where N = 1 (GEMDOS), 13 (BIOS), 14 (XBIOS)

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

### User space (TTBR0_EL1, per process)

- Low virtual addresses (`0x0000_0000_xxxx_xxxx`)
- Process TPA (Transient Program Area) per TOS convention
- Basepage at TPA start (256 bytes per Compendium)
- Text, data, BSS following basepage
- User stack at top of TPA, growing down

## Process Model

- Single CPU core for user code (RPi4 has 4 cores; cores 1–3 currently
  parked)
- Cooperative multitasking; one process active at a time until `Pexec`
  blocks or `Pterm` returns
- `Pexec` modes per Compendium implemented
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
