# 01 — Overview: what the toolchain is and what comes out of it

## What it is

The NewTOS SDK is a **clang + LLVM lld** cross toolchain targeting
`aarch64-unknown-none-elf`, plus three ported Atari C libraries:

| Component | Source | Port surface |
|-----------|--------|--------------|
| libc | mintlib-0.60.1 (`sdk/libc/mintlib-0.60.1.tar.xz`) | trap macros in `include/mint/{osbind,mintbind,falcon,metados}.h`, a libc-aware `crt0.S`, eleven AArch64 shims in `sdk/libc/newtos-arch/`, a handful of LP64/endian overlays (`sdk/libc/build-libc.sh:3-43`) |
| GEM bindings | gemlib-0.44.0 (`sdk/gemlib/gemlib-0.44.0.tar.xz`) | only the AES/VDI trap primitive (`_gc_aes.c` / `_gc_vdi.c`: m68k `trap #2` → `svc #2`), plus LP64 corrections to `mt_gem.h.in` / `gem.h` (`sdk/gemlib/build-gemlib.sh:5-30`) |
| dialog/window helpers | cflib, pinned at commit `ce57817` (`sdk/cflib/cflib-21-ce57817.tar.xz`) | pure C on top of gemlib; two files gated under `-D__NEWTOS__` (`sdk/cflib/build-cflib.sh:5-27`). cflib's version number has stood at 0.21.0 since 2004, so the pin is a COMMIT (PORT03) |

Every library is vendored as a **pristine tarball**; the build scripts extract
it and copy the overlay directory over it (`sdk/libc/build-libc.sh:63-85`,
`sdk/gemlib/build-gemlib.sh:67-88`, `sdk/cflib/build-cflib.sh:89-120`). The
extracted trees and the archives are build artifacts and untracked
(`sdk/.gitignore:9-27`). cflib's script also re-extracts when the vendored
tarball is newer than the extracted tree, and `sdk/lib-stale.sh` watches that
tarball, so moving the pin actually reaches the build (PORT03); mintlib and
gemlib still need `MINTLIB_REFRESH=1` / `GEMLIB_REFRESH=1` for that.

The decision behind this — "port real Atari C libraries, don't reimplement"
— is ADR-019 (`docs/MEMORY.md:1109-1173`). The SDK is a separate LGPL artifact
under `sdk/`; the OS itself stays clean-room (`docs/MEMORY.md:1124-1129`).

## Output: native ELF binaries with a new extension family

A NewTOS program is a **native AArch64 ELF64 executable**. The kernel loader
parses the ELF directly; there is no flat-binary conversion
(`sdk/build.sh:10-12`). The loader accepts exactly:

* `ELFCLASS64`, `ELFDATA2LSB`, `ET_DYN` (a position-independent executable;
  `ET_EXEC` is refused with `EPLFMT` since ARCH03 stage 0d), `EM_AARCH64`,
  `e_flags = 0`, `e_version = 1`, `e_phentsize = 56`, no `PT_INTERP`, at most
  8 `PT_LOAD` segments (`tos-api/src/bdos/process/elf.rs`);
* dynamic relocations: only `R_AARCH64_RELATIVE` in one `DT_RELA` table,
  applied by the loader (`tos-api/src/bdos/process/reloc.rs`);
* every `PT_LOAD` with `0x1000 <= p_vaddr`; the image must fit the largest
  free block, which becomes the TPA (EmuTOS `alloc_tpa`; else `ENSMEM`) —
  `tos-api/src/bdos/process/loader.rs` `image_end_of`, `arena.rs`.

The file extension decides how the desktop launches the program (ADR-018,
`docs/MEMORY.md:1004-1107`; classifier `kernel/src/desk/ftype.rs:50-70`):

| Extension | Role | m68k analogue | Launcher behaviour (`kernel/src/desk/main.rs:467-500`) |
|-----------|------|---------------|---------------------------------------------------------|
| `.NAP` | native GEM **A**pplication | `.PRG` / `.APP` | desktop relinquishes the screen (`desktop_relinquish`), runs the program, rebuilds itself on `Pterm` (`run_gem_app`, `:524-529`) |
| `.NCP` | native **C**onsole **P**rogram | `.TOS` | `fbcon::clear_and_home()`, run, wait for a key, redraw the desktop |
| `.NTP` | native, **T**akes **P**arameters | `.TTP` (**not** `.GTP` — decided 2026-09-26, DESK44) | a **console** program with a command tail: `isgraf = FALSE`, so `sh_chgrf` takes the screen to alpha before it runs (`main.rs:1068`, `gemshlib.c:513-523`). A double-click puts EmuTOS's `ADOPENAP` box up first and passes what was typed there as the tail (`opn_appl`, `deskinf.c:1250-1271`, NewTOS `kernel/src/desk/inf.rs`); *Cancel* starts nothing |

The extension is a desktop/shell convention, **not** part of the TOS syscall
ABI (`ftype.rs:16-17`). The classic suffixes `.PRG/.APP/.TOS/.TTP/.GTP` are
reserved for real m68k binaries; double-clicking one today shows the alert
*"This is an m68k Atari program. The m68k emulator is not available yet."*
(`kernel/src/desk/main.rs:342-347`).

Only `.NCP` files are auto-run from `C:\AUTO\` at boot
(`kernel/src/launch.rs:216-226`: `FileType::Native(Flavor::Console)` only);
`.NAP` starts either by double-click or through the `NEWDESK.INF` `#Z`
autorun line — see [06-image-and-testing.md](06-image-and-testing.md).

## The application binary interface, in one paragraph

Programs are **LP64** (8-byte `long` and pointers). That was a deliberate
pivot away from an ILP32 build path on 2026-07-21; the ILP32 toolchain was
retired on 2026-09-04 and "there is now no way to produce an ELFCLASS32
`.NCP`/`.NAP` from this repository" (`docs/MEMORY.md:1233-1259`). The kernel
still keeps a runtime ABI switch (`Abi::Ilp32`) for a future m68k thunk, but
no toolchain reaches it (`docs/MEMORY.md:1261-1278`). The consequences of LP64
for TOS data structures that are 32-bit on the wire are the subject of
[05-abi-pitfalls.md](05-abi-pitfalls.md).

## What is NOT possible

* **No m68k binaries.** The loader takes AArch64 ELF only
  (`elf.rs:339-346`); m68k programs wait for the emulator phase
  (`docs/MEMORY.md:1011-1026`).
* **No POSIX beyond what the ported mintlib provides.** NewTOS explicitly
  targets no Linux/POSIX compatibility (`CLAUDE.md`, "Non-goals"). What mintlib
  provides is what compiled: 613 of 624 core files
  (`sdk/libc/build-libc.sh` run of 2026-09-16, see
  [07-mintlib-limits.md](07-mintlib-limits.md)); the eleven that did not are
  listed with reasons in `sdk/libc/expected-failures.txt`.
* **Command-line arguments via `argv`: yes, since 2026-09-25 (TASK/PORT01).**
  The libc `crt0` calls `__libc_parseargs()`
  (`sdk/libc/newtos-arch/cmdargs.c`), the argument half of mintlib's
  `crtinit.c` `parseargs()` reimplemented for AArch64, and hands `main` the
  `argc`/`argv` it builds from the basepage command tail (`p_cmdlin`,
  `tos-api/src/bdos/process/loader.rs:680-691`). With an empty tail that is
  `argc = 1`, `argv = {"", NULL}` — the TOS "program name not available"
  convention. mintlib's own `crtinit.c` still does not build
  (`sdk/libc/expected-failures.txt:25`), so the **environment** half of
  `parseargs()` — `environ` and the `ARGV=` extension — is still absent; see
  the next point. `shel_read` remains available as a second route
  (`sdk/apps/chaintest/chaintest.c:41`).
* **No environment.** The desktop and `AUTO` launchers pass
  `EnvSpec::None`, so `p_env = 0` (`kernel/src/launch.rs:70-79`,
  `loader.rs:675`).
* **No supervisor mode.** `Super` and `Supexec` exist as *bindings* that do not
  change privilege: `Super(1L)` truthfully answers "not privileged", the rest
  are no-ops with a round-trippable token; `Supexec(f)` is a plain call of
  `f` (`sdk/libc/overlay/include/mint/osbind.h:445-574`, `:776-814`). The
  kernel itself answers GEMDOS 0x20 with `EINVFN` and XBIOS 0x26 with `38`
  (`TASK/X01.md`, header).
* **No Atari bit-plane screen.** `Physbase()`/`Logbase()` hand an EL0 program
  a linear XRGB8888 framebuffer (`sdk/examples/physbase.c:24-29`,
  `docs/MEMORY.md:1694-1757`), never Shifter planes.
* **No 32-bit (`--ilp32`) build.** See above.
