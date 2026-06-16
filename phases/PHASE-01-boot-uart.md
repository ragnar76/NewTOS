# PHASE-01: Boot + UART

## Goal

Boot a `#![no_std]` Rust kernel on QEMU `aarch64-virt` and on real Raspberry Pi 4,
print "NewTOS booting…" to the serial console, and halt cleanly.

## Inputs

- `CLAUDE.md` (project root) — read every time
- BCM2711 ARM Peripherals datasheet, sections "Auxiliaries: UART1 & SPI1, SPI2"
  and "PL011 UART"
- Raspberry Pi 4 boot process documentation
  (https://www.raspberrypi.com/documentation/computers/configuration.html#boot)
- Andre Richter, `rust-raspberrypi-OS-tutorials`, chapters 01–05
- QEMU manual entry for machine `virt` (note: virt uses PL011 UART at a
  different base address than RPi4 — both must be supported via HAL trait)

## Deliverables

### D1: Workspace skeleton

Create the Cargo workspace with the structure described in `CLAUDE.md` §
"Source Tree Conventions". Only `kernel`, `tos-types`, and `tools` need
crate manifests in this phase.

- `Cargo.toml` (workspace root)
- `rust-toolchain.toml` pinning a specific nightly (document the choice in
  `docs/STATUS.md`)
- `.cargo/config.toml` with target `aarch64-unknown-none-softfloat` and
  appropriate `rustflags`

### D2: Linker script + boot stub

- `kernel/src/arch/aarch64/link.ld` — places `.text.boot` at `0x80000` for
  RPi4, satisfies QEMU virt at `0x40000000` via two configs
- `kernel/src/arch/aarch64/boot.s` (or `.rs` with `naked` if preferred) —
  selects core 0, parks cores 1–3, sets up SP, zeroes BSS, jumps to Rust

Acceptance: `objdump -h` shows `.text.boot` at the correct load address for
both targets.

### D3: HAL trait for UART output

`kernel/src/hal/uart.rs`:

```rust
pub trait Uart {
    fn init(&self);
    fn write_byte(&self, b: u8);
    fn write_str(&self, s: &str) {
        for b in s.bytes() {
            self.write_byte(b);
        }
    }
}
```

No default impl beyond `write_str`. No `flush` in this phase (added when needed).

### D4: Two UART driver implementations

- `kernel/src/drivers/bcm2711/pl011.rs` — for RPi4 (UART0, GPIO 14/15)
- `kernel/src/drivers/qemu_virt/pl011.rs` — for QEMU virt (different base)

Both implement the `Uart` trait. Selection at compile time via Cargo features
`board-rpi4` and `board-qemu-virt` (mutually exclusive, exactly one required).

Init sequence must follow the datasheet: disable UART, set integer + fractional
baud rate divisors, configure line control, enable TX + UART. No "magic" values
without a comment citing the datasheet section.

### D5: Panic handler + halt

`kernel/src/panic.rs` writes panic info via the active UART, then loops with
`wfe`. No unwinding. `Cargo.toml` sets `panic = "abort"` for both profiles.

### D6: Main entry

`kernel/src/main.rs` `kernel_main()` initializes the UART, prints
`"NewTOS booting on <board>\nPhase 01 reached.\n"`, then halts.

### D7: `cargo xtask` for build + run

`tools/xtask/`:

- `cargo xtask build --board rpi4` — produces `kernel8.img`
- `cargo xtask build --board qemu-virt` — produces `kernel.elf`
- `cargo xtask run` — boots in QEMU, captures serial to stdout
- `cargo xtask test` — runs `cargo test` for host-runnable unit tests

## Exit criteria

- **EC1**: `cargo xtask run` outputs `"NewTOS booting on qemu-virt\nPhase 01 reached."`
  within 2 seconds and the process exits cleanly when QEMU is killed.
- **EC2**: `kernel8.img` placed on a FAT32-formatted SD card with the RPi
  firmware boot files results in the same string appearing on the RPi's UART
  pins (GPIO 14 TX, baud 115200, 8N1) within 5 seconds of power-on.
- **EC3**: A panic in `kernel_main` (e.g., `panic!("test")` line temporarily
  added) prints the panic message via UART before halting.
- **EC4**: All deliverable files exist, the workspace builds with zero
  warnings under `RUSTFLAGS="-D warnings"`, and `cargo clippy --all-targets`
  is clean.

## Out of scope

- MMU setup → Phase 03
- Exception vectors beyond the minimum for panic → Phase 03
- HDMI / framebuffer output → Phase 08
- Interrupt handling → Phase 03 (timer) / Phase 07 (peripherals)
- USB, networking, GPU mailbox interface
- Multicore beyond parking → much later, possibly never (TOS is single-core)

## Risks

- **R1: GPIO pin function selection on RPi4.** UART0 pins must be set to
  ALT0 via the GPFSEL1 register. Forgetting this means TX produces nothing.
  Cite the BCM2711 GPIO Alt-Function Table.
- **R2: `kernel8.img` filename.** RPi4 firmware looks specifically for this
  name in 64-bit mode. Wrong name = silent boot failure.
- **R3: QEMU virt's PL011 base differs from RPi4.** `0x09000000` vs
  `0xFE201000`. Compile-time selection, not runtime detection.
- **R4: BSS not zeroed.** Rust expects zero-initialized statics. The boot
  stub must zero `__bss_start..__bss_end` before calling `kernel_main`.
- **R5: Stack pointer location.** Must be set before any function call in
  Rust. Place the initial stack below `_start` (growing down) with a guard
  page (the guard page comes in Phase 03 with the MMU).

## Claude Code instructions

- This phase has **no TOS API surface**. Do not implement `Cconout` or any
  GEMDOS function here. The UART path is pre-syscall plumbing, used directly
  via the HAL trait.
- The temptation to add a `println!` macro using `core::fmt::Write` is fine
  if it stays in `kernel::hal::uart` and does not pretend to be `std::println`.
- Do **not** use `core::fmt::Arguments` formatting in the panic handler if
  the formatter would allocate. Buffer-bound `write!` into a fixed `[u8]`
  is acceptable.
- Cargo features `board-rpi4` and `board-qemu-virt` are mutually exclusive.
  Enforce at compile time with `compile_error!` if neither or both are set.
- Commit granularity: one commit per deliverable (D1…D7), each independently
  buildable. Do not squash before merge.
