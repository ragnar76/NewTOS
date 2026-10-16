# NewTOS native C SDK

Builds native AArch64 programs for NewTOS with **clang + rust-lld**
(`aarch64-unknown-none-elf`) on top of three ported Atari C libraries —
mintlib-0.60.1, gemlib-0.44.0, cflib-21 — following ADR-019 ("port real
Atari C libraries, don't reimplement", `docs/MEMORY.md`). The output is an
ELF64 executable named `.NCP` (console), `.NAP` (GEM application) or `.NTP`
(takes parameters) per ADR-018; the kernel loader parses the ELF directly.

**The developer documentation lives in [`docs/sdk/`](../docs/sdk/README.md).**
It covers prerequisites, a built-and-run Hello World / mintlib / GEM example,
the overlay mechanism that keeps ported application sources pristine, the
build scripts for QED, TeraDesk and CoreMark, the LP64/ABI pitfalls that have
bitten ports, and how a binary gets onto the SD image and into QEMU or a real
Pi 4. This file is only a map of the directory.

## Layout

| Path | Purpose |
|------|---------|
| `build.sh` | freestanding `.NCP` builder (no libc): `clang -c crt0.S + source`, `rust-lld -T newtos.ld` |
| `crt0.S` | startup for freestanding programs: basepage from `x0`, `SP = p_hitpa`, `bl main`, `Pterm` |
| `newtos.ld` | linker script for **every** native program, a position-independent executable (`ET_DYN`, `-fPIE` + `-pie --no-dynamic-linker -z text`; the loader applies its `R_AARCH64_RELATIVE` relocations, ARCH03 0d): two `PT_LOAD`s from `0x1000` — `text` (R-X: `.text`+dynamic metadata+`.rodata` → `p_tbase/p_tlen`) and `data` (RW-: `.dynamic`+`.got`+`.data` + `.bss` tail → `p_dbase/p_dlen`, `p_bbase/p_blen`) plus `PT_DYNAMIC`, `_start` first, `__bss_start`/`__bss_end`/`_end` exported |
| `include/newtos.h` | clean-room inline-asm trap helpers + `Cconws`/`Cconin`/`Pterm`/`Pterm0` |
| `examples/*.c` | freestanding proof programs (`hello`, `big`, `physbase`, `scrflip`, `dtachild`, `ptermch`, `ctrlcpar`) installed by `tools/make-sd-image.sh` |
| `lib-stale.sh` | the library dependency chain (`libc → gemlib → cflib`), staleness test and failure bookkeeping shared by every script |
| `libc/` | mintlib port: pristine tarball, `overlay/` (trap macros, fixed-width TOS structs, LP64/endian fixes), `newtos-arch/` (AArch64 shims), libc-aware `crt0.S`, `build-libc.sh`, `expected-failures.txt`, `tests/` |
| `gemlib/` | gemlib port: tarball, `overlay/` (`svc #2` primitives, LP64 header corrections), `build-gemlib.sh`, `tests/`; plus `build-gemlib-m68k.sh` for the real-Atari build of the same library |
| `cflib/` | cflib port: tarball, `overlay/`, `build-cflib.sh`, `tests/` |
| `overlays/{qed,teradesk,coremark}/` | per-application overlays — the only place application-level changes live |
| `apps/` | `build-qed.sh`, `build-teradesk.sh`, `build-coremark.sh`, the tracked diagnostics `apitest/`, `gemtest/`, `chaintest/`, and the **untracked** vendored application trees |

## Quick start

```sh
sdk/build.sh sdk/examples/hello.c /tmp/HELLO2.NCP     # freestanding console program
sdk/libc/build-libc.sh                                 # libc.a + build/libtest.NCP
sdk/gemlib/build-gemlib.sh                             # gemlib.a + build/gemtest.NCP
sdk/cflib/build-cflib.sh                               # cflib.a + build/cftest.NCP
sdk/apps/build-qed.sh                                  # sdk/apps/qed/build/QED.NAP
sdk/apps/build-teradesk.sh                             # sdk/apps/teradesk/build/dist/
tools/make-sd-image.sh /tmp/sd.img 64                  # FAT16 image with everything found
cargo xtask run --board rpi4 --sd /tmp/sd.img --window # boot it in QEMU raspi4b
```

Full walk-through, exact flags, the ABI contract and the pitfall catalogue:
[`docs/sdk/README.md`](../docs/sdk/README.md).
