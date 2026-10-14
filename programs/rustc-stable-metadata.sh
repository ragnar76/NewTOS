#!/usr/bin/env bash
#
# rustc-stable-metadata.sh -- the `build.rustc-wrapper` of the programs/
# workspace (programs/.cargo/config.toml), DESK43 U9-2.
#
# Why: the program files must not depend on where the repository is checked
# out, because the kernel image embeds NEWDESK.NAP (decision D3, kernel/embed.rs)
# and the kernel image is path-reproducible. Cargo's `-C metadata` value of a
# path package outside the workspace root is a hash over its ABSOLUTE path
# (here `tos-types`, ../tos-types from programs/), and every dependent's value
# hashes its dependencies' values. Rustc derives the crate's StableCrateId --
# the hash in every mangled symbol name -- from `-C metadata`; codegen orders
# functions by symbol name, so the same source in two checkouts gave two
# different program files (measured: 815 differing bytes in the loaded image of
# NEWDESK.NAP, measurement-2026-10-14-desk43-u9-1-2). Stripping the symbol
# table does not help: the order of the code itself differs. Rustc hashes every
# `-C metadata` it is given, so a second, fixed value cannot cancel cargo's.
#
# What: replace cargo's `-C metadata=<hash>` with a hash over what identifies
# the crate in this build without its location -- package name and version
# (cargo's CARGO_PKG_* environment), `--crate-name`, `--crate-type`,
# `--target` and every `--cfg` (the enabled features) -- and run the real
# compiler (the first argument; with clippy, `clippy-driver rustc`). Nothing
# else is touched: `-C extra-filename` (cargo's file names, its rebuild
# tracking) and all other arguments pass through. Calls without `-C metadata`
# (cargo's `rustc -vV` probe, the precompiled sysroot is not rebuilt) pass
# through unchanged.
#
# Distinctness: two crates of the same name in one link would need different
# values; that needs a different version, crate type, target or feature set,
# each of which is in the hash.
#
# Note for an existing target/programs/: cargo does not track the wrapper in
# its fingerprints, so artefacts built before it existed are reused as they
# are; `cargo clean` once (in programs/) makes every crate go through it.
set -euo pipefail

args=("$@")
n=${#args[@]}

key="${CARGO_PKG_NAME:-} ${CARGO_PKG_VERSION:-}"
for ((i = 1; i < n - 1; i++)); do
  case "${args[$i]}" in
    --crate-name | --crate-type | --target | --cfg) key+=" ${args[$i]} ${args[$((i + 1))]}" ;;
  esac
done

if command -v shasum >/dev/null 2>&1; then
  meta=$(printf '%s' "$key" | shasum -a 256 | cut -c1-16)
else
  meta=$(printf '%s' "$key" | sha256sum | cut -c1-16)
fi

for ((i = 1; i < n; i++)); do
  case "${args[$i]}" in
    metadata=*) if [ "${args[$((i - 1))]}" = "-C" ]; then args[$i]="metadata=$meta"; fi ;;
    -Cmetadata=*) args[$i]="-Cmetadata=$meta" ;;
  esac
done

exec "${args[@]}"
