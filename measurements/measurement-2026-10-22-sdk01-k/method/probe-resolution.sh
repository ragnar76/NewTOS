#!/bin/sh
# probe-resolution.sh -- exercise sdk/toolchain.sh's lookup order and error
# paths (SDK01 K). Arguments: <repo> <scratch k dir>. Prints one block per case.
R=$1; K=$2; NCC=$R/sdk/bin/newtos-cc
B="HOME=$HOME LANG=de_DE.UTF-8 TMPDIR=$K/tmpdir"
MIN=/usr/bin:/bin:/usr/sbin:/sbin
case_() { echo "=== $1"; shift; "$@" 2>&1; echo "--- rc=$?"; }
case_ "1 default env (rustup on PATH, no LLVM packages): Rust fallback" "$NCC" --toolchain
case_ "2 no Rust, no LLVM anywhere: error message (newtos-cc)" env -i $B PATH=$K/rustctrap:$MIN "$NCC" --toolchain
case_ "3 no Rust, no LLVM: error message (build-libc.sh)" env -i $B PATH=$K/rustctrap:$MIN "$R/sdk/libc/build-libc.sh"
case_ "4 LLVM dir on PATH, no Rust" env -i $B PATH=$K/rustctrap:$MIN:$K/fakellvm "$NCC" --toolchain
case_ "5 NEWTOS_TOOLCHAIN=rust with LLVM on PATH: Rust only" env NEWTOS_TOOLCHAIN=rust PATH=$K/fakellvm:$PATH "$NCC" --toolchain
case_ "6 NEWTOS_TOOLCHAIN=llvm with rustup on PATH, no LLVM: error, no Rust" env NEWTOS_TOOLCHAIN=llvm "$NCC" --toolchain
case_ "7 NEWTOS_LLVM_BIN=<dir>, no Rust" env -i $B NEWTOS_LLVM_BIN=$K/llvmkeg/bin PATH=$K/rustctrap:$MIN "$NCC" --toolchain
case_ "8 clang's own directory (CLANG via a symlink into a keg-like bin/), no Rust" env -i $B CLANG=$K/linkdir/clang PATH=$K/rustctrap:$MIN "$NCC" --toolchain
case_ "9 NEWTOS_LLD explicit file wins over PATH" env -i $B NEWTOS_LLD=$K/llvmkeg/bin/ld.lld PATH=$K/rustctrap:$MIN:$K/fakellvm "$NCC" --toolchain
case_ "10 NEWTOS_TOOLCHAIN=bogus" env NEWTOS_TOOLCHAIN=bogus "$NCC" --toolchain
case_ "11 NEWTOS_AR pointing nowhere" env NEWTOS_AR=/nonexistent/llvm-ar "$NCC" --toolchain
case_ "12 newtos-cc under dash, LLVM on PATH, no Rust: link a libc program" env -i $B PATH=$K/rustctrap:$MIN:$K/fakellvm dash "$NCC" -v -lm -o "$K/tmpdir/MATHD.NCP" "$R/sdk/examples/mathtst.c"
echo "=== 12b sha256 dash/ld.lld vs baseline MATHTST.NCP"
shasum -a 256 "$K/tmpdir/MATHD.NCP" | cut -d' ' -f1
awk '$2=="RUN/MATHTST.NCP"{print $1}' "$K/runs/baseline2/sha256.txt"
case_ "13 sdk/build.sh, LLVM on PATH, no Rust (prints lld path, no -flavor)" env -i $B PATH=$K/rustctrap:$MIN:$K/fakellvm "$R/sdk/build.sh" "$R/sdk/examples/hello.c" "$K/tmpdir/HELLOB.NCP"
case_ "14 old-style LLD=<rust-lld> override in a script still gets -flavor gnu" env LLD="$(readlink $K/fakellvm/ld.lld)" "$R/sdk/examples/fsbench/build-fsbench.sh"
echo "=== rustc trap invocations (expected: none file)"
cat "$K/rustctrap/invocations.log" 2>&1
