# 08 — Porting checklist

Tick each line; every line points at the section that explains it.

## Before touching anything

- [ ] The app is C (plus at most inline m68k asm) and needs only mintlib /
      gemlib / cflib — not sockets, threads, `setjmp`, or a libm
      → [07 §3–4](07-mintlib-limits.md#3-mintlib-assembly-that-is-never-assembled)
- [ ] Toolchain present: `clang`, the pinned Rust nightly with
      `llvm-tools-preview` (`rust-lld`, `llvm-ar`, …), `make`, `python3`
      → [02](02-prerequisites.md)
- [ ] `sdk/libc/build-libc.sh`, `sdk/gemlib/build-gemlib.sh`,
      `sdk/cflib/build-cflib.sh` run clean: `failures match expected-failures.txt`
      → [04 §library chain](04-porting-anatomy.md#the-library-chain-and-its-staleness-test)

## Setting the port up

- [ ] Source vendored under `sdk/apps/<app>/` (untracked), provenance recorded
      like `sdk/apps/coremark-sources.md5` → [04 §skeleton](04-porting-anatomy.md#adding-a-new-port-the-skeleton)
- [ ] `sdk/overlays/<app>/` created (tracked, empty at first)
- [ ] `sdk/apps/build-<app>.sh` copied from `build-qed.sh` (cflib app) or
      `build-teradesk.sh` (header overlays / own dialog lib); keeps
      `lib_ensure`, the shared flag set, `-D__NEWTOS__`, `--start-group`
      link, the "any failure stops" rule → [04](04-porting-anatomy.md)
- [ ] Output name decided: `.NAP` (GEM), `.NCP` (console), `.NTP` (takes a
      tail — no dialog yet) → [01](01-overview.md#output-native-elf-binaries-with-a-new-extension-family)

## Getting it to compile and link — the rule

- [ ] Every compile error is classified: **m68k asm** → overlay retarget;
      **anything else** → toolchain gap, fixed in a library overlay / shim /
      flag, never in the app → [04 §rule](04-porting-anatomy.md#the-rule-the-application-source-stays-pristine)
- [ ] Every undefined symbol traced with `build/why.txt`; classified against
      the dangling-reference list → [07 §6](07-mintlib-limits.md#6-references-libca-leaves-unresolved-measured)
- [ ] `undefined symbols in the final image (goal 0): 0`

## Walking the ABI traps against the app's own code

- [ ] Every TOS structure the app reads or writes itself (`BASEPAGE`, `_DTA`,
      `_DISKINFO`, `OSHEADER`, cookie slots, `_MD`, …) is either one the
      overlay pins to 32-bit fields, or gets a fixed-width declaration with
      `_Static_assert`s → [05 B1](05-abi-pitfalls.md#b1-long-and-pointers-are-8-bytes-tos-structures-are-32-bit-on-the-wire)
- [ ] Cookie jar accessed via `Getcookie()`/`Ssystem`, or as 4-byte reads
      with 8-byte slots → [05 B2](05-abi-pitfalls.md#b2-the-cookie-jar-at-5a0)
- [ ] No dereference of `Kbdvbase()`; returned pointers range-checked;
      screen writes assume XRGB8888 → [05 B3](05-abi-pitfalls.md#b3-kernel-addresses-vs-user-addresses)
- [ ] `Super`/`Supexec` bracket idioms reviewed: they run, but grant no
      privilege; shutdown/reset paths become live → [05 B4](05-abi-pitfalls.md#b4-super-supexec-supertouser)
- [ ] Directory probes via `Fattrib` expect the TOS4/FreeMiNT answer
      → [05 B5](05-abi-pitfalls.md#b5-fattrib-on-directories--tos-4--freemint-semantics)
- [ ] No reliance on `argv`/`environ`; command tail read from `p_cmdlin` or
      `shel_read` → [05 B6](05-abi-pitfalls.md#b6-argcargv-the-command-tail-and-the-environment)
- [ ] No `uintptr_t` from mintlib's `stdint.h` for pointer round-trips
      → [05 B7](05-abi-pitfalls.md#b7-mintlibs-stdinth-gives-a-32-bit-uintptr_t-on-our-lp64-build)
- [ ] `.RSC` converted with `tools/mkrsc.py convert` (kernel loader) or
      `--rshdr` (own loader); shipped beside the `.NAP` → [05 B8](05-abi-pitfalls.md#b8-resource-files-format-alignment-and-who-loads-them)
- [ ] `AESGLOBAL` read only through gemlib's macros → [05 B9](05-abi-pitfalls.md#b9-aesglobal-is-15-words-with-4-byte-pointer-fields)
- [ ] Pointer arguments to `wind_set` go through `wind_set_str`/`_ptr`
      → [05 B10](05-abi-pitfalls.md#b10-pointer-arguments-to-wind_set-and-friends)
- [ ] Bit-fields on `unsigned int`, big-endian word tricks, `ror`/`swap`
      asm reviewed → [05 B11](05-abi-pitfalls.md#b11-bit-fields-int-alignment-and-big-endian-word-tricks)
- [ ] Every AES/VDI call that passes a `long` or a pointer through `int_in`
      goes through a binding that splits it arithmetically, not by casting
      `&arg` to `short *` → [05 B16](05-abi-pitfalls.md#b16-gemlib-bindings-that-split-a-long-with-a-pointer-cast)
- [ ] No `long double` arithmetic; no libm calls → [05 B12](05-abi-pitfalls.md#b12-long-double-is-128-bit-and-there-is-no-compiler-rt), [07 §4](07-mintlib-limits.md#4-mintlib-directories-not-compiled-at-all)
- [ ] Large buffers via GEMDOS `Malloc`, not libc `malloc` (4-MiB arena)
      → [05 B15](05-abi-pitfalls.md#b15-a-libc-malloc-that-suddenly-fails)
- [ ] Console output strategy chosen (`.NCP` vs. log file) → [05 B14](05-abi-pitfalls.md#b14-console-output-is-not-the-desktop)

## On the image and in the machine

- [ ] Install block added to `tools/make-sd-image.sh` (`C:\Apps\<APP>\`,
      `.rsc` beside the `.NAP`; `.NAP` **never** in `\AUTO\`) → [06](06-image-and-testing.md#the-sd-image-toolsmake-sd-imagesh)
- [ ] After any manual copy onto the FAT volume: `dot_clean -m` (AppleDouble
      `._X.NCP` files would be launched) → [06](06-image-and-testing.md#the-sd-image-toolsmake-sd-imagesh)
- [ ] Headless QEMU run via `#Z 01 C:\…\APP.NAP@` in `NEWDESK.INF`; serial
      log shows `[shell] sh_ldapp: starting …` and no `[EXC]`/`[PANIC]`;
      screendump via QMP checked → [06](06-image-and-testing.md#newdeskinf-z--a-gem-application-instead-of-the-desktop)
- [ ] Kernel built fresh for the board being booted (`--board rpi4`), release
      profile for hardware → [06 footgun](06-image-and-testing.md#the-board-footgun)
- [ ] Interactive behaviour (clicks, keys, menus, closing) verified on a real
      Pi 4 — QEMU `raspi4b` has no input → [06](06-image-and-testing.md#real-raspberry-pi-4)

## Before handing over

- [ ] Every overlay file carries a header saying **why** it exists and what it
      changes against upstream (the pattern of `sdk/overlays/*`)
- [ ] `sdk/libc/expected-failures.txt` unchanged, or a new entry with a
      reason and a ticket → [04](04-porting-anatomy.md#the-library-chain-and-its-staleness-test)
- [ ] Measured evidence kept (serial log, screendump, hashes), not claims
