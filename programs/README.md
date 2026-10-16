# programs/ — NewTOS native programs in Rust

DESK43 U6a (`docs/analysis/desk43-u5-u10-review-2026-10-14.md` §3.2, §5):
the toolchain **T1** for Rust user programs. A program is a `no_std` +
`alloc` static library for the kernel's target, compiled
position-independent and linked with **`sdk/crt0.S` + `sdk/newtos.ld`**
exactly like a C program (`sdk/build.sh`). There is no second linker
script and no second start-up: the loader contract (ET_DYN/PIE, only
`R_AARCH64_RELATIVE`, the `DT_*` positive list, the PRGFLAGS note, the
`p_parent` accessory test, the Mshrink to image + 64 KiB stack) lives in
those two files only.

```
programs/
├── Cargo.toml            own workspace (root Cargo.toml: exclude = ["programs"])
├── .cargo/config.toml    target aarch64-unknown-none-softfloat,
│                         -C relocation-model=pic, target-dir ../target/programs,
│                         rustc-wrapper ./rustc-stable-metadata.sh
├── build.sh              crt0.o + cargo staticlib + rust-lld -> <name>.NAP
├── rustc-stable-metadata.sh  path-independent crate hashes (DESK43 U9-2)
├── newtos-gem/           runtime crate: gemdos / aes / vdi / xbios trap
│                         bindings (since U6b the only copy), and with
│                         feature `rt`
│                         (default) the heap (GlobalAlloc over GEMDOS Malloc
│                         hunks) and the panic handler (Cconws, Pterm(-1));
│                         feature `panic-wait-key`: a key first (D6-a)
├── hello-gem/            HELLOGEM.NAP, the U6a spike (window, heap test,
│                         stack measurement, `--features panic-test`)
├── desktop/              the desktop (EmuTOS desk/ in Rust), a library crate
│                         (DESK43 U8b; until then kernel/src/desk/): linked by
│                         NEWDESK (until U10 also by the kernel)
└── newdesk/              NEWDESK.NAP (DESK43 U9-2): the desktop as an EL0
                          program -- `main` calls `desktop::run()`; embedded in
                          the kernel image under the registry name NEWDESK
```

## Build

```
cargo xtask build-programs --board rpi4             # every program, board feature on,
cargo xtask build-programs --board qemu-virt        #   -> target/programs/<board>/*.NAP
cargo xtask build --board rpi4 [--release]          # build-programs first, then the kernel
programs/build.sh                                   # -> target/programs/HELLOGEM.NAP
programs/build.sh hello-gem /path/HELLOGEM.NAP
programs/build.sh hello-gem /path/HGPANIC.NAP -- --features panic-test
PRGFLAGS=0x2 programs/build.sh ...                  # crt0's PRGFLAGS note, as sdk/build.sh
```

Steps (printed by the script): `clang -c sdk/crt0.S` with `sdk/build.sh`'s
flags; `cargo build -p <crate> --release` in `programs/` (release profile:
`panic = "abort"`, `opt-level = "s"`, fat LTO, no DWARF of our own);
`rust-lld -flavor gnu -T sdk/newtos.ld -pie --no-dynamic-linker -z text
-z max-page-size=0x1000 --build-id=none crt0.o lib<crate>.a`, the link line
of `sdk/build.sh`.

`cargo xtask build-programs --board <b>` (DESK43 U6b) runs `build.sh` for
every program in its `PROGRAMS` table (`tools/xtask/src/main.rs`) with
`-- --features board-<b>` and writes `target/programs/<b>/<NAME>.NAP`. The
board feature is the X03 pass-through: the desktop's About-box machine line
is a per-board build constant, so the desktop program needs the board the
kernel it is embedded in is built for (`cargo xtask build` runs
`build-programs` for its own board first). **Every program declares `board-rpi4` and
`board-qemu-virt`** (cargo refuses an unknown feature; the host test
`programs_tests` checks the table and the features).

The programs workspace is separate (root `Cargo.toml`
`exclude = ["programs"]`). From DESK43 U6b to U10 the kernel depended on
`newtos-gem` and `desktop` by path for its built-in desktop; since U10 it
links no crate of this workspace and only embeds the built program files
(`kernel/embed.rs`).

**The desktop crate (DESK43 U8b).** `desktop/` is a library, not a program
(no `staticlib`, not in `PROGRAMS`): the `NEWDESK` program (`newdesk/`, U9-2)
is a thin crate around it. (From U8b to U10 the kernel linked it as a second
target and called `desktop::run`/`desktop::reset` in EL1 — one source, two
link targets, review §3.1; U10 removed the kernel's.) Its
dependencies are **`newtos-gem` (without `rt`) and `tos-types` only**: a
`tos_api::` or kernel path in the desktop does not compile (guard G1; checked
by `tools/desk-drift-probe.sh` group D and the host test
`programs_tests::desktop_depends_only_on_newtos_gem_and_tos_types`). It
declares `board-rpi4`/`board-qemu-virt` (exactly one required: the About-box
machine line) and `selftest-desk`; NEWDESK's features of the same names pass
through (`board-rpi4 = ["desktop/board-rpi4"]`). Building or linting it on
its own therefore needs a board:
`cargo clippy -p desktop --features board-rpi4` (in `programs/`); a bare
`cargo build`/`cargo clippy` over the whole programs workspace stops at the
desktop's missing-board `compile_error!`.

**The desktop program (DESK43 U9-2).** `newdesk/` is a `staticlib` whose
`main` calls `desktop::run()` and returns 0 (EmuTOS `deskstart.S`: `jsr
_deskmain` … `Pterm(0)`; crt0 does the `Mshrink` and the stack). It depends on
`desktop` and on `newtos-gem` **with** `rt` and the feature `panic-wait-key`
(crash policy D6-a: a panic shows its message and waits for a key before
`Pterm(-1)`, as EmuTOS' exception report does), and passes `board-*` and
`selftest-desk` through to `desktop` (host test
`programs_tests::newdesk_is_the_desktop_with_the_program_runtime`).

**Embedding (decision D3).** `cargo xtask build` builds the board's programs
first; `kernel/build.rs` embeds the ones listed in `kernel/embed.rs` from
`target/programs/<board>/` (one `include_bytes!` each, registered under their
registry name at boot, `kernel/src/embedded.rs`) and **stops with a hard
error** when a listed program has not been built. Since U9-2 the list is
`NEWDESK` (`NEWDESK.NAP`, the shell's `DEF_DESKTOP`); the shell loop's
DESKTOP arm starts it (U9-4), and since U10 it is the only desktop. If it
cannot be loaded, the arm runs the console EmuCON instead (D6-d).

**Reproducible program files (U9-2).** The kernel image embeds a program file,
so the program files must not depend on the checkout path. Cargo hashes the
absolute path of `tos-types` (outside this workspace) into every crate's
`-C metadata`, from which rustc derives the symbol hashes — and the code
order. `programs/.cargo/config.toml` therefore runs rustc through
`rustc-stable-metadata.sh`, which replaces that value by a hash of the
crate's name, version, type, target and features (rationale in the script).
Cargo does not track the wrapper in its fingerprints: a `target/programs/`
from before it needs one `cargo clean` (in `programs/`).

## Writing a program

The crate is a `staticlib` depending on `newtos-gem` and exporting

```rust
#[no_mangle]
pub unsafe extern "C" fn main(basepage: usize) -> i32
```

which `sdk/crt0.S` calls with the basepage address; the return value is the
`Pterm` exit code. `alloc` works from the first line (the heap needs no
initialisation); a panic prints its location on the console and ends the
program with `newtos_gem::PANIC_EXIT` (−1).

The traps follow `docs/sdk/05-abi-pitfalls.md` §A1. Remember B1 there:
TOS structures are 32-bit on the wire — declare them with fixed-width
fields, never with `usize`/pointers.
