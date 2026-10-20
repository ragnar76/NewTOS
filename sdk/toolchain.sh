# toolchain.sh -- locate the Rust toolchain's sysroot (rust-lld, llvm-ar,
# llvm-nm, llvm-readobj, llvm-objdump, llvm-size) for the SDK build scripts,
# independent of the directory they are started from (TASK/SDK01 package I).
#
# WHY THIS FILE EXISTS
#
# rustup chooses the toolchain by the CURRENT directory: rust-toolchain.toml
# at the repository root pins the nightly that carries llvm-tools-preview.
# Every build script used to ask `rustc --print sysroot` wherever the caller
# happened to stand. Started from outside the checkout, rustup picked no
# toolchain (empty sysroot) or the user's default one (no LLVM tools), and the
# build stopped with "missing LLVM tool: /lib/rustlib//bin/llvm-ar".
# sdk/bin/newtos-cc already asked rustc from inside the SDK directory; the
# scripts now do the same, through this one file instead of ~20 copies.
#
# Sourced by bash scripts (`. "$SDK/toolchain.sh"`); it locates the SDK from
# its OWN path, so it needs nothing from the caller. It sets NEWTOS_TC_SDK
# (the sdk/ directory) and defines two functions; nothing else is touched:
#
#   newtos_rust_sysroot   print the pinned toolchain's sysroot
#   newtos_rust_bindir    print <sysroot>/lib/rustlib/<host>/bin, where
#                         rust-lld and the llvm-* tools live
#
# Both run rustc in a subshell that cd's into sdk/, so the caller's working
# directory is untouched. Their exit status is rustc's (non-zero when rustup
# finds no toolchain), as it was when the scripts called rustc inline.
# clang is NOT resolved here: it comes from PATH or $CLANG, which does not
# depend on the working directory.

NEWTOS_TC_SDK="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"      # .../sdk

newtos_rust_sysroot() {
  (cd "$NEWTOS_TC_SDK" && rustc --print sysroot 2>/dev/null)
}

newtos_rust_bindir() {
  local sysroot host rc=0
  sysroot="$(newtos_rust_sysroot)" || rc=$?
  host="$(cd "$NEWTOS_TC_SDK" && rustc -vV | sed -n 's/host: //p')" || rc=$?
  # Printed even when rustc failed, so the callers' "missing LLVM tool: …"
  # check reports the same path as before (/lib/rustlib//bin/llvm-ar).
  printf '%s\n' "$sysroot/lib/rustlib/$host/bin"
  return $rc
}
