# NewTOS native C SDK — Stage 1

A minimal, clean-room toolchain that compiles a C program with **clang** into a
**native NewTOS ELF** (`.NCP`, ADR-018) that the kernel's program loader runs at
EL0. No emulation, no host libc: a freestanding AArch64 program that talks to the
OS through the TOS trap ABI.

End-to-end proof (`sdk/examples/hello.c`): a clang-built C program prints a
banner via `Cconws` and returns; the process terminates cleanly through `Pterm`.

## Layout

| File | Purpose |
|------|---------|
| `include/newtos.h` | Clean-room inline-asm trap helpers + Stage-1 GEMDOS wrappers (`Cconws`, `Cconin`, `Pterm`, `Pterm0`). Derived from `docs/api/`, not copied from mintlib. |
| `crt0.S` | `_start`: point SP into the mapped TPA page, call `main`, `Pterm(main's return)`. |
| `newtos.ld` | Linker script: `ENTRY(_start)`, one PT_LOAD at VA `0x100`, image kept inside the single 4 KiB TPA page. |
| `build.sh` | `clang -c` the C + crt0, then link with `rust-lld` (LLVM lld) → the `.NCP` ELF. |
| `examples/hello.c` | The Stage-1 example / proof program. |

## Trap ABI

From `kernel/src/arch/aarch64/exceptions.rs` / `tos-api/src/trap.rs`:

- **GEMDOS** = `svc #1`, **GEM (AES/VDI)** = `svc #2`, **BIOS** = `svc #13`,
  **XBIOS** = `svc #14`.
- Function id in **`x8`**; integer/pointer arguments in **`x0..x7`** (left to
  right); result is a **sign-extended i32 in `x0`**.
- The kernel SVC handler preserves `x1..x29` across the trap and only overwrites
  `x0` with the result, so the inline-asm helpers clobber only `x0` + memory.

## Build

```sh
sdk/build.sh [source.c] [output.NCP]
#   source.c    defaults to sdk/examples/hello.c
#   output.NCP  defaults to /tmp/HELLO2.NCP
```

Exact commands it runs (target `aarch64-unknown-none-elf`):

```sh
clang -target aarch64-unknown-none-elf -ffreestanding -nostdlib -O2 \
      -fno-stack-protector -fno-pic -fno-builtin -Wall -Wextra -Iinclude \
      -c crt0.S            -o crt0.o
clang -target aarch64-unknown-none-elf -ffreestanding -nostdlib -O2 \
      -fno-stack-protector -fno-pic -fno-builtin -Wall -Wextra -Iinclude \
      -c examples/hello.c  -o app.o
rust-lld -flavor gnu -T newtos.ld -no-pie -z max-page-size=0x1000 \
      --build-id=none crt0.o app.o -o HELLO2.NCP
```

`rust-lld` ships with the Rust toolchain (`$(rustc --print sysroot)/lib/rustlib/*/bin/`
or `~/.cargo/bin/rust-lld`); `build.sh` locates it automatically and falls back
to `ld.lld` if that is on `PATH`. The `.NCP` **is** the ELF — the loader parses
ELF directly, so there is no flat-binary `objcopy` step (unlike the kernel image).

## ELF / TPA constraints (matched to the loader)

The loader (`tos-api/src/bdos/process/{elf,loader,mmu,arena}.rs`) maps exactly
**one 4 KiB page** for the process, at low-half VA `0x0`:

```
[0x000, 0x100)  BasePage (populated by the loader)
[0x100, 0x1000) PT_LOAD image (.text/.rodata/.data/.bss) + the C stack
```

So a Stage-1 program must satisfy:

- ELFCLASS64, ELFDATA2LSB, **ET_EXEC**, **EM_AARCH64**, `e_version = 1`,
  `e_phentsize = 56`.
- One PT_LOAD segment with `0x100 <= p_vaddr` and `p_vaddr + p_memsz <= 0x1000`.
- `e_entry >= 0x100` (the linker puts `_start` at `0x100`).
- The whole ELF file (headers + segment) fits in the loader's 8 KiB read buffer.

**Stack:** the loader pre-sets `SP_EL0` to `USER_STACK_TOP_EXCLUSIVE`
(`0x0000_FFFF_FFFF_F000`), which is **not** mapped under the single-page model.
`crt0.S` therefore repoints `SP` to `0x1000` (top of the TPA page); the stack
grows downward within `[0x100, 0x1000)`. Keep code+data+stack together under
one page for Stage 1. (The hand-crafted selftest ELFs never touch the stack,
which is why they run without this fix; a compiled `main` needs it.)

## Verified

`HELLO2.NCP` was placed on the FAT16 `C:` image and launched on QEMU `raspi4b`
via `run_program(b"C:\\HELLO2.NCP\0")`. Serial output:

```
[sdk-verify] === running C:\HELLO2.NCP (clang-built .NCP) ===
[launch] EL0: slot 0 entry 0x100
Hello from a C program built with clang!
[launch] returned (program Pterm'd)
```

No exception/fault — a clean load → EL0 run → `Cconws` → `Pterm` round-trip.

## Roadmap

- **Stage 1 (this):** trap helpers + `Cconws`/`Cconin`/`Pterm`/`Pterm0`, crt0,
  linker script, one-page TPA. Proves a clang-built C program runs natively.
- **Stage 2:** AES/VDI bindings (`svc #2`, selectors 200/115) for GUI apps
  (`.NAP`), each against its `docs/api/` spec entry.
- **Stage 3:** a real libc + a multi-page TPA (once the loader grows beyond the
  single-page D3 limit), so ordinary C applications build unmodified.
