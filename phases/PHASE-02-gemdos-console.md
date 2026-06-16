# PHASE-02: GEMDOS Console I/O

## Goal

Implement the GEMDOS console I/O syscalls (`Cconin`, `Cconout`, `Cconws`,
`Cnecin`, `Crawcin`, `Crawio`, `Cauxin`, `Cauxout`, `Cprnout`) and the trap
dispatcher that routes `trap #1` calls to them.

This is the first phase that produces real **TOS API surface**.

## Inputs

- `CLAUDE.md`
- `docs/SOURCE-ANALYSIS-PROTOCOL.md`
- `docs/MEMORY.md` — for relevant ADRs from Phase 00 / 01
- `phases/PHASE-00-emutos-extraction.md` — **must be complete for the
  console function group** (specs in `docs/api/gemdos/console/` exist)
- `phases/PHASE-01-boot-uart.md` (must be complete and merged)
- `docs/api/gemdos/console/` — **the spec extracted from EmuTOS in Phase 00.**
  This is what we implement against. If incomplete → return to Phase 00.
- `docs/api/gemdos/_dispatcher.md` — dispatcher spec from Phase 00
- AArch64 exception model documentation: ARM ARM, "AArch64 Exceptions"
- BCM2711 datasheet: GIC-400 section (for D5 minimal IRQ wiring)

## Source Analysis (D0 — first deliverable)

This phase is **implementation work**, not extraction. The TOS API has
already been extracted in Phase 00. D0 here is much shorter:

**Track A (TOS API):**
- [ ] Verify `docs/api/gemdos/console/` covers all 11 functions for
      this phase. If gaps exist → return to Phase 00 to fill them.
- [ ] Verify `docs/api/gemdos/_dispatcher.md` is complete.
- [ ] No EmuTOS re-reading needed. Implement against the extracted spec.

**Track B (hardware):**
- [ ] ARM ARM: AArch64 exception model, VBAR_EL1, ESR_EL1, SVC instruction
- [ ] Document SVC immediate encoding and exception entry register state
      in `kernel/src/arch/aarch64/exceptions.rs` doc-comment
- [ ] BCM2711 GIC-400 section, PL011 interrupt registers
- [ ] QEMU virt GIC differences

If any required input is missing, **stop and report**. Do not proceed
with partial inputs.

## Deliverables

### D1: Trap dispatch infrastructure

- `kernel/src/arch/aarch64/exceptions.s` — exception vector table at
  `VBAR_EL1`, 16 entries, each branching to a Rust handler stub
- `kernel/src/arch/aarch64/exceptions.rs` — Rust handlers, one per exception
  class, currently routing only **synchronous EL0** to the trap dispatcher
- The trap dispatcher reads `ESR_EL1` to identify SVC, extracts the SVC
  immediate (which we use as the trap number: 1 = GEMDOS, 13 = BIOS, 14 = XBIOS),
  and dispatches to the appropriate API layer

**Important ABI decision (document in `docs/ARCHITECTURE.md`):**

Native (ARM) callers use the AArch64 PCS — parameters in `x0`–`x7`,
return value in `x0`. The TOS function ID is passed in `x8`. Stack frame
follows AAPCS64.

The m68k emulator (Phase 10) will translate m68k stack-based parameters
into this native ABI before calling the dispatcher. **The dispatcher itself
sees only the native ABI.** This keeps the TOS-API layer free of m68k
specifics.

### D2: GEMDOS function table

`tos-api/src/gemdos/mod.rs`:

```rust
pub type GemdosFn = fn(args: &TrapArgs) -> i32;

pub static GEMDOS_TABLE: [Option<GemdosFn>; 0x100] = {
    let mut t = [None; 0x100];
    t[0x01] = Some(cconin);
    t[0x02] = Some(cconout);
    // …
    t
};
```

`TrapArgs` is a struct exposing `arg0()` through `argN()` with type-safe
accessors (`arg0_u16()`, `arg1_ptr::<T>()`, etc.). Defined in `tos-api::trap`.

### D3: Console syscall implementations

Implement, with full spec compliance per `docs/api/gemdos.md`:

| ID    | Function   | Behavior                                              |
|-------|------------|-------------------------------------------------------|
| 0x01  | `Cconin`   | Read one char from console, with echo                 |
| 0x02  | `Cconout`  | Write one char to console                             |
| 0x03  | `Cauxin`   | Read one char from AUX (mapped to 2nd UART or stub-error) |
| 0x04  | `Cauxout`  | Write one char to AUX                                 |
| 0x05  | `Cprnout`  | Write one char to printer (returns error in Phase 02) |
| 0x06  | `Crawio`   | Raw console I/O, no echo, no Ctrl-C                   |
| 0x07  | `Crawcin`  | Read one char from console, no echo                   |
| 0x08  | `Cnecin`   | Read one char, no echo, with Ctrl-C handling          |
| 0x09  | `Cconws`   | Write zero-terminated string                          |
| 0x0A  | `Cconrs`   | Read line into buffer with length prefix              |
| 0x0B  | `Cconis`   | Returns -1 if console char available, 0 otherwise     |

**Forbidden in this phase:**
- Implementing `Cprnout` as anything other than returning the documented
  error code. There is no printer driver yet.
- Implementing `Cauxin`/`Cauxout` against the same UART as the console.
  AUX is a separate device per the spec.

### D4: Console abstraction in the kernel

`kernel/src/console.rs`:

```rust
pub trait Console {
    fn read_byte(&self) -> Option<u8>;       // non-blocking
    fn read_byte_blocking(&self) -> u8;
    fn write_byte(&self, b: u8);
    fn has_input(&self) -> bool;
}
```

Backed by the UART driver from Phase 01. The UART is now wrapped: a small
ring buffer is filled from a UART receive interrupt (which means **interrupts
must be enabled in this phase**, even though the wider IRQ subsystem comes
in Phase 03).

### D5: Minimal IRQ wiring for UART RX

This is the smallest possible interrupt path: enable PL011 RX interrupt,
route it through the GIC-400 (RPi4) or GICv2 (QEMU virt) to a Rust handler
that drains the FIFO into the console ring buffer.

**Scope discipline:** only RX, only PL011, no timer, no other peripherals,
no SGI/PPI, no SMP. The handler is a single function with no preemption.

The full IRQ subsystem (priorities, handler registration, deferred work) is
Phase 03+. This is a stopgap.

### D6: Test program

`tools/test-runner/`: a script that boots NewTOS in QEMU, sends a known
byte sequence to the console, and verifies the kernel echoed/processed it
per the spec.

Tests:
- T1: `Cconout('A')` produces 'A' on the UART.
- T2: `Cconws("Hello\r\n")` produces 7 bytes in order.
- T3: `Cconin` blocks until input arrives, then returns the byte.
- T4: `Crawcin` does not echo.
- T5: `Cnecin` handles Ctrl-C per spec (sends EINTR or whatever the spec
  says — read it before implementing).
- T6: Function ID `0xFF` (undefined) returns the documented error
  (`EINVFN = -32`).

### D7: A real "userland" test

Even though we don't have processes yet, write a test that:

1. From `kernel_main`, executes an `svc #1` instruction with `x8 = 0x09`
   and `x0` pointing to a string `"NewTOS GEMDOS works\r\n"`.
2. Returns from the trap.
3. Continues execution and writes a confirmation via the HAL UART
   (bypassing GEMDOS) to prove control returned cleanly.

This proves the entire trap path is functional end-to-end.

## Exit criteria

- **EC1**: All seven test cases T1–T7 pass in QEMU.
- **EC2**: D7 self-test runs successfully on real RPi4 and produces both
  the GEMDOS-routed message and the post-trap confirmation on UART.
- **EC3**: `docs/api/gemdos.md` covers all eleven functions with complete
  specs (semantics, parameters, returns, errors, m68k ABI mapping, AArch64
  ABI mapping, EmuTOS reference).
- **EC4**: Calling an undefined GEMDOS function returns `EINVFN`, not a
  panic, not a crash, not silence.
- **EC5**: Building with `RUSTFLAGS="-D warnings"` produces no warnings.

## Out of scope

- File I/O (Phase 05)
- Process control (Phase 06)
- BIOS/XBIOS dispatchers (Phase 07/08) — even though the trap infrastructure
  could be extended trivially, do not. Each dispatcher gets its own phase
  with its own spec.
- Memory allocation (Phase 03) — strings are static or stack-allocated
  in the test code.
- Line editing in `Cconrs` beyond what the Compendium specifies (no fancy
  history, no completion).

## Risks

- **R1: PL011 FIFO depth.** PL011 has a 32-byte FIFO. The RX interrupt
  trigger level must be set such that we don't lose bytes during burst
  input. Datasheet section "UARTIFLS Register".
- **R2: GIC initialization.** The GIC must be enabled at EL1. Cite the GIC
  programming guide section used.
- **R3: Spinlock on the ring buffer.** The ring buffer is touched from
  both IRQ context and thread context. A `spin::Mutex` is acceptable
  **only if** IRQs are disabled while held in thread context, otherwise
  deadlock. Document this invariant in the source.
- **R4: TOS error code values.** GEMDOS error codes are negative `i32`.
  Do not invent values. Use Compendium's table.
- **R5: m68k ABI documentation in spec.** Even though we are not running
  m68k code yet, every spec entry must include the m68k ABI mapping
  (parameter stack layout, return register, …). Phase 10 will rely on this.

## Claude Code instructions

- This is the first phase that produces TOS API surface. Each function
  must have a corresponding entry in `docs/api/gemdos.md` written **before**
  any code is touched.
- Do not unify the AUX device with the console UART even if "they are the
  same physical port on RPi4". The TOS API treats them as distinct devices,
  and a future RPi setup may have a separate USB-serial adapter as AUX.
  Use a HAL trait `AuxDevice` even if the implementation in Phase 02 is a
  stub-error-return. (This is **not** a code stub; it's a documented
  not-yet-available device.)
- The trap dispatcher must be a single, well-tested entry point. Resist
  the urge to add "convenience" paths or fast-paths at this stage.
- When you encounter a Compendium ambiguity (e.g., "behavior on EOF is
  implementation-defined"), document the chosen behavior in
  `docs/api/gemdos.md` under "NewTOS-specific decisions" and continue.
  Do **not** silently pick something.