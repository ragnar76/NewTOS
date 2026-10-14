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
│                         -C relocation-model=pic, target-dir ../target/programs
├── build.sh              crt0.o + cargo staticlib + rust-lld -> <name>.NAP
├── newtos-gem/           runtime crate: gemdos / aes / vdi / xbios trap
│                         bindings (since U6b the only copy -- the kernel
│                         desktop links them too), and with feature `rt`
│                         (default) the heap (GlobalAlloc over GEMDOS Malloc
│                         hunks) and the panic handler (Cconws, Pterm(-1))
└── hello-gem/            HELLOGEM.NAP, the U6a spike (window, heap test,
                          stack measurement, `--features panic-test`)
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
is a per-board build constant, so a separately built desktop needs the board
as the kernel has it. **Every program declares `board-rpi4` and
`board-qemu-virt`** (cargo refuses an unknown feature; the host test
`programs_tests` checks the table and the features).

The programs workspace is still separate (root `Cargo.toml`
`exclude = ["programs"]`), but the kernel depends on `newtos-gem` by path
(`default-features = false`: the bindings without `rt`): the kernel desktop
calls the system through the same bindings a program does. Cargo builds it
as a non-member path package with the kernel's profile and target.

**Embedding (decision D3).** `cargo xtask build` builds the board's programs
first; `kernel/build.rs` embeds the ones listed in `kernel/embed.rs` from
`target/programs/<board>/` (one `include_bytes!` each, registered under their
registry name at boot, `kernel/src/embedded.rs`) and **stops with a hard
error** when a listed program has not been built. At U6b the list is empty —
the desktop program (`NEWDESK`) arrives with U9; nothing is embedded and
nothing changes at boot.

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
