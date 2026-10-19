# 02 — Prerequisites: the tools the scripts use, and where they look

Everything below is what the build scripts actually invoke. Versions are the
ones present on the host while this document was written (command output
quoted); the scripts do not pin versions.

## Compiler: clang

Every script uses `${CLANG:-clang}` from `PATH` (`sdk/build.sh:35`,
`sdk/libc/build-libc.sh:88`, `sdk/gemlib/build-gemlib.sh:98`,
`sdk/cflib/build-cflib.sh:87`, `sdk/apps/build-qed.sh:69`,
`sdk/apps/build-teradesk.sh:70`, `sdk/apps/build-coremark.sh:115`). The
target is always `-target aarch64-unknown-none-elf`. Measured on the host:

```
$ which clang && clang --version | head -2
/usr/bin/clang
Apple clang version 21.0.0 (clang-2100.1.1.101)
Target: arm64-apple-darwin27.0.0
```

The freestanding headers (`stddef.h`, `stdarg.h`, `stdint.h` … ) come from
clang's resource directory: `RES="$($CLANG -print-resource-dir)/include"`,
passed as `-isystem "$RES"` after the mintlib include path
(`sdk/libc/build-libc.sh:90,103`). On this host that is
`/Library/Developer/CommandLineTools/usr/lib/clang/21/include`.

**Header precedence matters:** `-nostdinc -I<mintlib>/include … -isystem $RES`
means mintlib's own `stdint.h` wins over clang's. Pristine, its `uintptr_t`
was `unsigned int` on a non-`__MSHORT__` build — 32 bits on our LP64 target;
since TASK/PORT10 the overlay `sdk/libc/overlay/include/stdint.h` makes
`intptr_t`/`uintptr_t` `long` on LP64 (with `INTPTR_*`/`UINTPTR_MAX` and the
`PRI*PTR`/`SCN*PTR` macros in `inttypes.h`). Since TASK/SDK01 A
(2026-10-20) `PTRDIFF_MIN/MAX` and `SIZE_MAX` in the same header and
`SSIZE_MAX` (overlay `include/bits/posix1_lim.h`) are the 64-bit values too
(`sdk/libc/tests/lp64-limits.sh`). See
[05-abi-pitfalls.md](05-abi-pitfalls.md#b7-mintlibs-stdinth-gives-a-32-bit-uintptr_t-on-our-lp64-build).

## Linker and binutils: the Rust toolchain's LLVM

The library and app scripts take `rust-lld`, `llvm-ar`, `llvm-readobj`,
`llvm-nm` and `llvm-objdump` from the **pinned Rust nightly's** sysroot:

```sh
SYSROOT="$(rustc --print sysroot 2>/dev/null)"
BIN="$SYSROOT/lib/rustlib/$(rustc -vV | sed -n 's/host: //p')/bin"
AR="${AR:-$BIN/llvm-ar}"
LLD="${LLD:-$BIN/rust-lld}"
```
(`sdk/libc/build-libc.sh:91-94`; identical in the gemlib/cflib/qed/teradesk/
coremark scripts.) The pin is `rust-toolchain.toml`:

```toml
channel = "nightly-2026-04-15"
components = ["rust-src", "rustfmt", "clippy", "llvm-tools-preview"]
targets = ["aarch64-unknown-none-softfloat"]
```

`llvm-tools-preview` is what puts `rust-lld` & co. into that `bin/`
directory. Measured:

```
$ rustc -vV
rustc 1.97.0-nightly (a5c825cd8 2026-04-14)
host: aarch64-apple-darwin
LLVM version: 22.1.2
$ $BIN/rust-lld -flavor gnu --version
LLD 22.1.2 (…) (compatible with GNU linkers)
```

`sdk/build.sh` (the freestanding builder) is the exception: it searches
`ld.lld` on `PATH`, then `rust-lld` on `PATH`, then the sysroot, then
`~/.cargo/bin/rust-lld` (`sdk/build.sh:39-50`). On this host `ld.lld` is
not on `PATH` but `rust-lld` is (`~/.cargo/bin/rust-lld`, the rustup proxy),
so the second branch fires — the script printed `lld : rust-lld -flavor gnu`.
Note that this proxy resolves to the *default* toolchain's `rust-lld`, not
necessarily the pinned nightly's; the library/app scripts avoid the question
by taking the sysroot copy directly.

## Other host tools the scripts call

| Tool | Used by | For |
|------|---------|-----|
| `make` (GNU Make 3.81 on this host) | `build-gemlib.sh:129-132`, `build-cflib.sh:119-122`, `build-qed.sh:99-102` | expanding the libraries' own `SRCFILES` `COBJS` manifests |
| `tar` (xz-capable) | all three `build-*lib.sh` | extracting the pristine tarballs |
| `python3` (3.14.6 on this host) | `build-teradesk.sh:197-201`, `tools/make-sd-image.sh` | `tools/mkrsc.py` resource conversion; `mk-hello-ncp.py`; MBR sanity check |
| `md5` or `md5sum` | `build-coremark.sh:75-86` | run-rule integrity check |
| `hdiutil`, `diskutil`, `newfs_msdos`, `dot_clean`, `mdutil` | `tools/make-sd-image.sh:13,36-50,291` | building and populating the FAT16 image — **macOS only** |
| `qemu-system-aarch64` (11.0.2 on this host) | `cargo xtask run` (`tools/xtask/src/main.rs:477,536`) | booting the image |
| `cc` | `tools/qmpshot.c:13` | the QMP screendump client |

## Host assumptions baked into the scripts

* **macOS paths and tools.** `tools/make-sd-image.sh` is documented as "macOS
  only (uses hdiutil / diskutil / newfs_msdos)" (`:13`) and mounts the image
  under `/Volumes/NEWTOS` (`:44`). `sdk/build.sh` defaults its output to
  `/tmp/HELLO2.NCP` (`:27`); `make-sd-image.sh` defaults to `/tmp/sd.img`,
  which is also what `cargo xtask run --sd` looks for when no path is given
  (`tools/xtask/src/main.rs:414-421`).
* **No network.** "A clone builds with no external network"
  (`sdk/.gitignore:6-7`): the three library tarballs are in the tree. The
  application trees under `sdk/apps/*/` are **not** tracked
  (`sdk/.gitignore:29-33`); QED and TeraDesk are embedded git checkouts,
  CoreMark is re-obtained per `sdk/apps/coremark-sources.md5:5-8`
  (`git clone https://github.com/czietz/coremark.git … checkout b546802`).
  Where the QED and TeraDesk checkouts come from is **unverified** here —
  no provenance file exists for them (the embedded repos' `HEAD` was
  `f12055f` for QED and `b5532b8` for TeraDesk on this host).
* **`set -uo pipefail` without `-e`** in every library/app script: a failing
  step is handled explicitly (`|| exit 1`), not by the shell.
* **Paths with spaces are not handled** in `build-coremark.sh`'s
  `CFLAGS=(${PORT_CFLAGS} …)` word-split (`:136`); keep the checkout on a
  space-free path.
